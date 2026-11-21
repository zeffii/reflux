//! Decoding animation-lane rows (as written in Wax) into events.
//!
//! A row is read left to right like a tiny instruction stream:
//!
//! ```text
//! 01 0020   glide = 0x20 ticks      (modifier, applies to the next command only)
//! 29 4000   orbit -> 0x4000, gliding over 32 ticks
//! 60 00FF   flash trigger, velocity 0xFF
//! ```

use super::command::{self, CommandTable, Curve, Kind};

/// One resolved command at one moment in time.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub tick: u32,
    pub command: u8,
    pub value: u16,
    pub glide_ticks: u32,
    pub curve: Curve,
    pub relative: bool,
}

#[derive(Debug, PartialEq)]
pub enum RowError {
    OddToken(String),
    NotHex(String),
    DanglingModifier,
}

/// The modifiers collected so far on a row, waiting for the command they apply to.
#[derive(Default)]
struct PendingModifiers {
    glide_ticks: u32,
    curve: Option<Curve>,
    relative: bool,
    any: bool,
}

pub fn decode_row(tick: u32, row: &str, table: &CommandTable) -> Result<Vec<Event>, RowError> {
    let tokens: Vec<&str> = row.split_whitespace().collect();
    if tokens.len() % 2 != 0 {
        return Err(RowError::OddToken(row.to_string()));
    }

    let mut events = Vec::new();
    let mut pending = PendingModifiers::default();

    for pair in tokens.chunks(2) {
        let command = parse_hex_u8(pair[0])?;
        let value = parse_hex_u16(pair[1])?;

        if table.kind_of(command) == Kind::Modifier {
            apply_modifier(&mut pending, command, value);
            continue;
        }

        // With a glide but no curve, gliding linearly is the least surprising default.
        let default_curve = if pending.glide_ticks > 0 {
            Curve::Linear
        } else {
            Curve::Step
        };
        events.push(Event {
            tick,
            command,
            value,
            glide_ticks: pending.glide_ticks,
            curve: pending.curve.unwrap_or(default_curve),
            relative: pending.relative,
        });
        pending = PendingModifiers::default();
    }

    if pending.any {
        return Err(RowError::DanglingModifier);
    }
    Ok(events)
}

fn apply_modifier(pending: &mut PendingModifiers, command: u8, value: u16) {
    pending.any = true;
    match command {
        command::GLIDE => pending.glide_ticks = value as u32,
        command::CURVE => pending.curve = Some(Curve::from_value(value)),
        command::RELATIVE => pending.relative = value != 0,
        _ => {} // reserved modifiers are accepted and ignored for now
    }
}

fn parse_hex_u8(token: &str) -> Result<u8, RowError> {
    if token.len() != 2 {
        return Err(RowError::NotHex(token.to_string()));
    }
    u8::from_str_radix(token, 16).map_err(|_| RowError::NotHex(token.to_string()))
}

fn parse_hex_u16(token: &str) -> Result<u16, RowError> {
    if token.len() != 4 {
        return Err(RowError::NotHex(token.to_string()));
    }
    u16::from_str_radix(token, 16).map_err(|_| RowError::NotHex(token.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_apply_to_the_next_command_only() {
        let table = CommandTable::standard();
        let events = decode_row(12, "01 0000 41 0000 01 0020 44 3000", &table).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].command, 0x41);
        assert_eq!(events[0].glide_ticks, 0);
        assert_eq!(events[0].curve, Curve::Step);
        assert_eq!(events[1].command, 0x44);
        assert_eq!(events[1].glide_ticks, 0x20);
        assert_eq!(events[1].curve, Curve::Linear);
        assert_eq!(events[1].value, 0x3000);
        assert!(events.iter().all(|e| e.tick == 12));
    }

    #[test]
    fn a_command_after_a_glided_one_does_not_inherit_the_glide() {
        let table = CommandTable::standard();
        let events = decode_row(0, "01 0080 02 0004 29 2000 2A 7000", &table).unwrap();
        assert_eq!(events[0].curve, Curve::Smooth);
        assert_eq!(events[1].glide_ticks, 0);
    }

    #[test]
    fn bad_rows_are_reported() {
        let table = CommandTable::standard();
        assert!(matches!(decode_row(0, "29", &table), Err(RowError::OddToken(_))));
        assert!(matches!(decode_row(0, "zz 0000", &table), Err(RowError::NotHex(_))));
        assert_eq!(decode_row(0, "01 0010", &table), Err(RowError::DanglingModifier));
    }
}
