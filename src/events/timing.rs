//! Converting between seconds and tracker ticks.
//!
//! Stand-in until Reflux reads the real tempo data from a .wax file: one fixed
//! tempo, with rows per beat and ticks per row as in a classic tracker.

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub bpm: f64,
    pub rows_per_beat: u32,
    pub ticks_per_row: u32,
}

impl Timing {
    pub fn ticks_per_second(&self) -> f64 {
        self.bpm / 60.0 * self.rows_per_beat as f64 * self.ticks_per_row as f64
    }

    pub fn seconds_to_ticks(&self, seconds: f64) -> f64 {
        seconds * self.ticks_per_second()
    }

    pub fn row_to_tick(&self, row: u32) -> u32 {
        row * self.ticks_per_row
    }

    pub fn beats_at(&self, seconds: f64) -> f64 {
        seconds * self.bpm / 60.0
    }

    pub fn rows_to_seconds(&self, rows: u32) -> f64 {
        rows as f64 * self.ticks_per_row as f64 / self.ticks_per_second()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_style_tick_rate() {
        let timing = Timing {
            bpm: 125.0,
            rows_per_beat: 4,
            ticks_per_row: 6,
        };
        assert!((timing.ticks_per_second() - 50.0).abs() < 1e-9);
        assert!((timing.rows_to_seconds(16) - 1.92).abs() < 1e-9);
    }
}
