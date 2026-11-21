//! A song as Reflux sees it: timing, the command table and the decoded timeline.
//!
//! Until Reflux parses .wax files, the demo song below is written in the same
//! row notation the Wax animation instrument will use.

use super::command::{CommandTable, Kind, Layout};
use super::rows::{decode_row, Event, RowError};
use super::timeline::Timeline;
use super::timing::Timing;

pub struct Song {
    pub name: String,
    pub timing: Timing,
    pub table: CommandTable,
    pub length_rows: u32,
    pub timeline: Timeline,
    /// The animation-lane rows as written, for the lane overlay.
    pub rows: Vec<(u32, String)>,
}

impl Song {
    pub fn from_rows(
        name: &str,
        timing: Timing,
        table: CommandTable,
        length_rows: u32,
        rows: &[(u32, &str)],
    ) -> Result<Song, (u32, RowError)> {
        let mut events: Vec<Event> = Vec::new();
        for (row, text) in rows {
            let decoded = decode_row(timing.row_to_tick(*row), text, &table).map_err(|e| (*row, e))?;
            events.extend(decoded);
        }
        let timeline = Timeline::build(&events, &table);
        Ok(Song {
            name: name.to_string(),
            timing,
            table,
            length_rows,
            timeline,
            rows: rows.iter().map(|(row, text)| (*row, text.to_string())).collect(),
        })
    }

    pub fn length_seconds(&self) -> f64 {
        self.timing.rows_to_seconds(self.length_rows)
    }

    /// Song position in ticks for a playback time, looping at the song's end.
    /// The row playing at `seconds` (fractional), looping at the song's end.
    pub fn row_at(&self, seconds: f64) -> f64 {
        self.tick_at(seconds) / self.timing.ticks_per_row as f64
    }

    pub fn tick_at(&self, seconds: f64) -> f64 {
        let looped = seconds.rem_euclid(self.length_seconds());
        self.timing.seconds_to_ticks(looped)
    }

    /// The built-in demo: 128 rows at 125 bpm, events deliberately a little off the grid.
    pub fn demo() -> Song {
        let timing = Timing {
            bpm: 125.0,
            rows_per_beat: 4,
            ticks_per_row: 6,
        };

        let mut table = CommandTable::standard();
        table.insert(0x80, "pulse_ring", Kind::Trigger, Layout::Split8x8);
        table.insert(0x81, "tunnel_twist", Kind::Target, Layout::Whole16);

        // Glide lengths are in ticks (6 per row): 0x180 = 64 rows, 0x60 = 16 rows.
        let rows: &[(u32, &str)] = &[
            (0, "29 0000 2A 6000 28 5000 41 2000 44 3000 81 8000"),
            (4, "01 0180 02 0004 29 4000"),
            (15, "60 00FF"),
            (16, "01 0030 02 0005 2A 3000 80 0410"),
            (30, "80 0308"),
            (33, "60 0080 01 0040 41 6000"),
            (48, "01 0100 02 0002 81 C000 01 0020 28 9000"),
            (64, "60 00FF 41 B000 01 00C0 02 0004 29 C000"),
            (70, "80 0610"),
            (78, "01 0060 02 0003 44 C000"),
            (96, "01 0080 02 0004 81 6000 2A 7000"),
            (101, "60 00C0"),
            (112, "01 0060 02 0004 29 FFFF 01 0060 44 3000 01 0040 28 5000 01 0060 41 2000"),
        ];

        Song::from_rows("demo", timing, table, 128, rows).expect("demo song rows are valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_song_decodes_and_loops() {
        let song = Song::demo();
        assert!((song.length_seconds() - 15.36).abs() < 1e-9);
        assert!((song.tick_at(song.length_seconds() + 1.0) - 50.0).abs() < 1e-6);
        assert!(song.timeline.commands().contains(&0x80));
    }
}
