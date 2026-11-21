//! Read-only view of the animation lane, tracker style, scrolling with playback.

use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2};

use super::style;
use crate::events::song::Song;

const VISIBLE_ROWS: i32 = 33;
const LINE_HEIGHT: f32 = 15.0;

pub fn draw(painter: &Painter, screen: Rect, song: &Song, row_position: f64) {
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
    let bottom = top + height;
    // Like a tracker: the playhead moves down until it reaches the middle, then stays
    // put while the rows scroll past underneath it.
    let half = (VISIBLE_ROWS / 2) as f64;
    let first_visible = (row_position - half).max(0.0);
    let row_y = |row: f64| top + ((row - first_visible) as f32 + 0.5) * LINE_HEIGHT;

    let first_row = first_visible.floor() as i32;
    for row in first_row..=first_row + VISIBLE_ROWS {
        if row < 0 || row >= song.length_rows as i32 {
            continue;
        }
        let y = row_y(row as f64);
        if y < top + LINE_HEIGHT * 0.4 || y > bottom - LINE_HEIGHT * 0.4 {
            continue;
        }
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

    // The playhead: just a thin red line at the top edge of the playing row.
    let playhead_y = row_y(row_position) - LINE_HEIGHT * 0.5;
    painter.line_segment(
        [Pos2::new(area.left() + 6.0, playhead_y), Pos2::new(area.right() - 6.0, playhead_y)],
        Stroke::new(1.0, style::PLAYHEAD),
    );
}
