//! Turns a song's events into signals that can be sampled at any (fractional) tick.
//!
//! Target commands become chains of glide segments: each new event starts from
//! wherever the previous glide had got to at that moment, so an interrupted glide
//! never jumps. Trigger commands become a list of hits, sampled as decaying pulses.

use std::collections::BTreeMap;

use super::command::{CommandTable, Curve, Kind};
use super::rows::Event;

#[derive(Clone, Debug)]
struct Segment {
    start_tick: f64,
    start_value: f32,
    end_value: f32,
    glide_ticks: f64,
    curve: Curve,
}

impl Segment {
    fn value_at(&self, tick: f64) -> f32 {
        if self.glide_ticks <= 0.0 {
            return self.end_value;
        }
        let progress = ((tick - self.start_tick) / self.glide_ticks) as f32;
        let blend = self.curve.shape(progress);
        self.start_value + (self.end_value - self.start_value) * blend
    }
}

#[derive(Clone, Debug)]
struct Hit {
    tick: f64,
    value: u16,
}

#[derive(Default)]
pub struct Timeline {
    targets: BTreeMap<u8, Vec<Segment>>,
    triggers: BTreeMap<u8, Vec<Hit>>,
}

/// 16-bit command values are normalised to 0..1 for targets.
fn normalise(value: u16) -> f32 {
    value as f32 / 65535.0
}

/// Relative values are signed: 0x8000..0xFFFF are negative deltas.
fn signed_delta(value: u16) -> f32 {
    (value as i16) as f32 / 32767.0
}

impl Timeline {
    pub fn build(events: &[Event], table: &CommandTable) -> Timeline {
        let mut sorted = events.to_vec();
        sorted.sort_by_key(|e| e.tick); // stable: keeps row order within a tick

        let mut timeline = Timeline::default();
        for event in &sorted {
            match table.kind_of(event.command) {
                Kind::Modifier => {}
                Kind::Trigger => timeline
                    .triggers
                    .entry(event.command)
                    .or_default()
                    .push(Hit {
                        tick: event.tick as f64,
                        value: event.value,
                    }),
                Kind::Target => timeline.push_target(event),
            }
        }
        timeline
    }

    fn push_target(&mut self, event: &Event) {
        let tick = event.tick as f64;
        let segments = self.targets.entry(event.command).or_default();
        let start_value = segments
            .last()
            .map(|previous| previous.value_at(tick))
            .unwrap_or(0.0);
        let end_value = if event.relative {
            start_value + signed_delta(event.value)
        } else {
            normalise(event.value)
        };
        segments.push(Segment {
            start_tick: tick,
            start_value,
            end_value,
            glide_ticks: event.glide_ticks as f64,
            curve: event.curve,
        });
    }

    /// The value (normally 0..1) of a target command at `tick`. 0 before its first event.
    pub fn value(&self, command: u8, tick: f64) -> f32 {
        let Some(segments) = self.targets.get(&command) else {
            return 0.0;
        };
        let index = segments.partition_point(|s| s.start_tick <= tick);
        if index == 0 {
            return 0.0;
        }
        segments[index - 1].value_at(tick)
    }

    /// The most recent hit of a trigger command at or before `tick`, as
    /// (ticks since the hit, raw 16-bit value).
    pub fn last_hit(&self, command: u8, tick: f64) -> Option<(f64, u16)> {
        let hits = self.triggers.get(&command)?;
        let index = hits.partition_point(|h| h.tick <= tick);
        if index == 0 {
            return None;
        }
        let hit = &hits[index - 1];
        Some((tick - hit.tick, hit.value))
    }

    /// A trigger as a pulse: jumps to `strength` on each hit, then decays exponentially.
    pub fn pulse(&self, command: u8, tick: f64, decay_ticks: f64, strength: f32) -> f32 {
        match self.last_hit(command, tick) {
            Some((age, _)) => strength * (-age / decay_ticks).exp() as f32,
            None => 0.0,
        }
    }

    /// Commands that have at least one event, in id order.
    pub fn commands(&self) -> Vec<u8> {
        let mut ids: Vec<u8> = self.targets.keys().chain(self.triggers.keys()).copied().collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::rows::decode_row;

    fn timeline(rows: &[(u32, &str)]) -> Timeline {
        let table = CommandTable::standard();
        let events: Vec<Event> = rows
            .iter()
            .flat_map(|(tick, row)| decode_row(*tick, row, &table).unwrap())
            .collect();
        Timeline::build(&events, &table)
    }

    #[test]
    fn instant_set_and_linear_glide() {
        let t = timeline(&[(0, "41 0000"), (10, "01 0014 41 FFFF")]);
        assert_eq!(t.value(0x41, 5.0), 0.0);
        assert!((t.value(0x41, 20.0) - 0.5).abs() < 1e-4); // halfway through 20 ticks
        assert!((t.value(0x41, 30.0) - 1.0).abs() < 1e-6);
        assert!((t.value(0x41, 99.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn an_interrupted_glide_continues_from_where_it_was() {
        let t = timeline(&[(0, "01 0064 44 FFFF"), (50, "44 0000")]);
        // At tick 50 the first glide was halfway, the second one snaps to 0 instantly...
        assert_eq!(t.value(0x44, 50.0), 0.0);
        // ...but a glided interruption starts from the halfway value.
        let t = timeline(&[(0, "01 0064 44 FFFF"), (50, "01 000A 44 0000")]);
        assert!((t.value(0x44, 50.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn triggers_pulse_and_decay() {
        let t = timeline(&[(8, "60 00FF")]);
        assert_eq!(t.pulse(0x60, 7.0, 4.0, 1.0), 0.0);
        assert!((t.pulse(0x60, 8.0, 4.0, 1.0) - 1.0).abs() < 1e-6);
        assert!(t.pulse(0x60, 16.0, 4.0, 1.0) < 0.2);
        assert_eq!(t.last_hit(0x60, 9.0), Some((1.0, 0x00FF)));
    }

    #[test]
    fn relative_adds_a_signed_delta() {
        let t = timeline(&[(0, "29 4000"), (4, "03 0001 29 F000")]);
        let before = t.value(0x29, 3.0);
        let after = t.value(0x29, 4.0);
        assert!(after < before);
    }
}
