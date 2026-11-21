//! Read-only view of the animation lane, tracker style, scrolling with playback.

use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2};

use super::style;
use crate::events::song::Song;

const VISIBLE_ROWS: i32 = 33;
const LINE_HEIGHT: f32 = 15.0;

pub fn draw(painter: &Painter, screen: Rect, song: &Song, row_position: f64) {
    let current_row = row_position.floor() as i32;
    let height = VISIBLE_ROWS as f32 * LINE_HEIGHT;
    let width = 560.0_f32.min(screen.width() - 40.0);
    let area = Rect::from_center_size(screen.center(), Vec2::new(width, height + 24.0));
    painter.rect_filled(area, 3.0, style::GROUND);

    painter.text(
        area.left_top() + Vec2::new(12.0, 10.0),
        Align2::LEFT_CENTER,
        format!("animation lane · {}", song.name),
        style::title_font(),
        style::TEXT,
    );

    let top = area.top() + 24.0;
    let half = VISIBLE_ROWS / 2;
    for offset in -half..=half {
        let row = current_row + offset;
        if row < 0 || row >= song.length_rows as i32 {
            continue;
        }
        let y = top + (offset + half) as f32 * LINE_HEIGHT + LINE_HEIGHT * 0.5;
        let is_beat = row % song.timing.rows_per_beat as i32 == 0;
        let number_colour = if is_beat { style::TEXT } else { style::TEXT_DIM };
        painter.text(Pos2::new(area.left() + 12.0, y), Align2::LEFT_CENTER, format!("{row:03}"), style::mono(11.0), number_colour);

        let cells = song
            .rows
            .iter()
            .find(|(r, _)| *r as i32 == row)
            .map(|(_, text)| text.clone())
            .unwrap_or_else(|| "··".to_string());
        painter.text(Pos2::new(area.left() + 48.0, y), Align2::LEFT_CENTER, cells, style::mono(11.0), style::TEXT);
    }

    // The playhead: just a thin red line, placed with sub-row precision.
    let fraction = (row_position - current_row as f64) as f32;
    let playhead_y = top + half as f32 * LINE_HEIGHT + fraction * LINE_HEIGHT;
    painter.line_segment(
        [Pos2::new(area.left() + 6.0, playhead_y), Pos2::new(area.right() - 6.0, playhead_y)],
        Stroke::new(1.0, style::PLAYHEAD),
    );
}
