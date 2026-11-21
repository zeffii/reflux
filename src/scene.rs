//! What the demo scene looks like at one moment, derived from the song's commands.
//!
//! This is the hard-wired stand-in for the graph: later each mapping here becomes a
//! wire from a deputy node's output into a node parameter.

use crate::events::song::Song;

#[derive(Clone, Copy, Debug, Default)]
pub struct SceneState {
    pub seconds: f32,
    pub beat: f32,
    pub camera_position: [f32; 3],
    pub camera_target: [f32; 3],
    pub fov_degrees: f32,
    pub hue: f32,
    pub glow: f32,
    pub flash: f32,
    pub twist: f32,
    pub ring_pulse: f32,
    /// How far the latest pulse ring has travelled from the core, in scene units.
    pub ring_travel: f32,
}

impl SceneState {
    pub fn sample(song: &Song, seconds: f64) -> SceneState {
        let tick = song.tick_at(seconds);
        let timeline = &song.timeline;
        let looped_seconds = seconds.rem_euclid(song.length_seconds());

        let orbit_angle = timeline.value(0x29, tick) * std::f32::consts::TAU;
        let orbit_radius = 1.8 + timeline.value(0x2A, tick) * 3.0;
        let bob = (looped_seconds as f32 * 0.4).sin() * 0.5;

        // 80 pulse_ring is a combo command: XX = travel speed, YY = intensity.
        let (ring_pulse, ring_travel) = match timeline.last_hit(0x80, tick) {
            Some((age_ticks, value)) => {
                let fields = song.table.fields_of(0x80, value);
                let speed = fields[0] as f32;
                let intensity = fields.get(1).copied().unwrap_or(0) as f32 / 16.0;
                let envelope = (-age_ticks / 60.0).exp() as f32;
                let age_seconds = (age_ticks / song.timing.ticks_per_second()) as f32;
                (intensity * envelope, age_seconds * speed * 2.0)
            }
            None => (0.0, 0.0),
        };

        let flash_velocity = timeline
            .last_hit(0x60, tick)
            .map(|(_, value)| (value & 0xFF) as f32 / 255.0)
            .unwrap_or(0.0);

        SceneState {
            seconds: looped_seconds as f32,
            beat: song.timing.beats_at(looped_seconds) as f32,
            camera_position: [orbit_angle.cos() * orbit_radius, bob, orbit_angle.sin() * orbit_radius],
            camera_target: [0.0, 0.0, 0.0],
            fov_degrees: 30.0 + timeline.value(0x28, tick) * 70.0,
            hue: timeline.value(0x41, tick),
            glow: timeline.value(0x44, tick) * 2.0,
            flash: timeline.pulse(0x60, tick, 5.0, flash_velocity),
            twist: (timeline.value(0x81, tick) - 0.5) * 1.2,
            ring_pulse,
            ring_travel,
        }
    }
}
