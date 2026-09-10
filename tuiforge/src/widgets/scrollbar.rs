//! Textual-style scrollbars with eighth-block thumb ends, plus `ScrollbarState` that turns
//! wheel/drag mouse events into offsets. Shared by lists, tables, scroll views, logs…

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::MouseEvent;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::StatefulWidget;

use crate::core::{Hit, HitBox, Outcome};
use crate::draw::{LEFT_BLOCKS, LOWER_BLOCKS, fill};
use crate::theme::{self, Theme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxis {
    #[default]
    Vertical,
    Horizontal,
}

/// Mouse state for one scrollbar: hover highlight and thumb dragging.
#[derive(Clone, Debug, Default)]
pub struct ScrollbarState {
    pub hit: HitBox,
    /// Offset (in content units) at drag start, and the pointer cell where it started.
    drag: Option<(usize, u16)>,
    // cached geometry from the last render, needed to map pixels back to offsets
    content: usize,
    viewport: usize,
    pub offset: usize,
    axis: ScrollAxis,
}

impl ScrollbarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_offset(&self) -> usize {
        self.content.saturating_sub(self.viewport)
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Feed a mouse event. Updates `offset` on wheel/drag; returns `Changed` when it moved.
    pub fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let track_len = match self.axis {
            ScrollAxis::Vertical => self.hit.area.height,
            ScrollAxis::Horizontal => self.hit.area.width,
        } as usize;
        let pointer = match self.axis {
            ScrollAxis::Vertical => m.row,
            ScrollAxis::Horizontal => m.column,
        };
        match self.hit.mouse(&m) {
            Hit::Press => {
                // click on the track: page towards the pointer; on the thumb: start dragging
                let (start, end) = self.thumb_cells(track_len);
                let rel = pointer.saturating_sub(self.track_origin()) as usize;
                if rel >= start && rel < end {
                    self.drag = Some((self.offset, pointer));
                    Outcome::Consumed
                } else {
                    let before = self.offset;
                    if rel < start {
                        self.offset = self.offset.saturating_sub(self.viewport.max(1));
                    } else {
                        self.offset = (self.offset + self.viewport.max(1)).min(self.max_offset());
                    }
                    self.drag = Some((self.offset, pointer));
                    Outcome::changed_if(before != self.offset)
                }
            }
            Hit::Drag => {
                let Some((start_offset, start_pointer)) = self.drag else { return Outcome::Ignored };
                if track_len == 0 || self.content <= self.viewport {
                    return Outcome::Consumed;
                }
                let per_cell = self.content as f32 / track_len as f32;
                let delta = (pointer as i32 - start_pointer as i32) as f32 * per_cell;
                let before = self.offset;
                self.offset = ((start_offset as f32 + delta).round().max(0.0) as usize).min(self.max_offset());
                Outcome::changed_if(before != self.offset)
            }
            Hit::Click | Hit::Cancel => {
                self.drag = None;
                Outcome::Consumed
            }
            Hit::Wheel(d) => {
                let before = self.offset;
                self.scroll_by(d as i64 * 3);
                Outcome::changed_if(before != self.offset)
            }
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
        }
    }

    pub fn scroll_by(&mut self, delta: i64) {
        let max = self.max_offset() as i64;
        self.offset = (self.offset as i64 + delta).clamp(0, max) as usize;
    }

    fn track_origin(&self) -> u16 {
        match self.axis {
            ScrollAxis::Vertical => self.hit.area.y,
            ScrollAxis::Horizontal => self.hit.area.x,
        }
    }

    /// Thumb start/end in whole cells (for hit testing).
    fn thumb_cells(&self, track_len: usize) -> (usize, usize) {
        let (s, e) = thumb_span(self.content, self.viewport, self.offset, track_len as f32);
        (s.floor() as usize, e.ceil() as usize)
    }
}

fn thumb_span(content: usize, viewport: usize, offset: usize, len: f32) -> (f32, f32) {
    if content <= viewport || len <= 0.0 {
        return (0.0, len);
    }
    let thumb = (viewport as f32 / content as f32 * len).max(1.0);
    let start = (offset as f32 / (content - viewport) as f32).clamp(0.0, 1.0) * (len - thumb);
    (start, start + thumb)
}

/// The scrollbar widget. Renders into a 1-cell-thick area; wider areas are filled.
///
/// ```
/// use tuiforge::prelude::*;
/// # let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
/// let mut state = ScrollbarState::new();
/// let (total_rows, visible_rows) = (200, 10);
/// Scrollbar::vertical(total_rows, visible_rows).offset(40).render(Rect::new(39, 0, 1, 10), &mut buf, &mut state);
/// // later: state.handle_mouse(mouse_event) moves `state.offset` on wheel/drag
/// ```
#[derive(Clone, Debug)]
pub struct Scrollbar {
    axis: ScrollAxis,
    content: usize,
    viewport: usize,
    offset: Option<usize>,
    active: bool,
    theme: Option<Theme>,
    /// Draw nothing when the content fits (default: still paints the track).
    hide_when_fits: bool,
}

impl Scrollbar {
    pub fn new(axis: ScrollAxis, content: usize, viewport: usize) -> Self {
        Scrollbar { axis, content, viewport, offset: None, active: false, theme: None, hide_when_fits: false }
    }
    pub fn vertical(content: usize, viewport: usize) -> Self {
        Self::new(ScrollAxis::Vertical, content, viewport)
    }
    pub fn horizontal(content: usize, viewport: usize) -> Self {
        Self::new(ScrollAxis::Horizontal, content, viewport)
    }
    /// Offset to display; defaults to the state's own offset.
    pub fn offset(mut self, o: usize) -> Self {
        self.offset = Some(o);
        self
    }
    /// Highlight thumb (focused container / hovered).
    pub fn active(mut self, v: bool) -> Self {
        self.active = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
    pub fn hide_when_fits(mut self, v: bool) -> Self {
        self.hide_when_fits = v;
        self
    }
}

impl StatefulWidget for Scrollbar {
    type State = ScrollbarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        state.hit.set_area(area);
        state.axis = self.axis;
        state.content = self.content;
        state.viewport = self.viewport;
        if let Some(o) = self.offset {
            state.offset = o;
        }
        state.offset = state.offset.min(state.max_offset());
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.content <= self.viewport && self.hide_when_fits {
            return;
        }
        fill(buf, area, th.scrollbar_bg);
        if self.content <= self.viewport {
            return;
        }
        let color = if state.is_dragging() {
            th.primary
        } else if self.active || state.hit.hover {
            th.scrollbar_hover
        } else {
            th.scrollbar
        };
        let area = area.intersection(buf.area);
        match self.axis {
            ScrollAxis::Vertical => {
                let (start, end) = thumb_span(self.content, self.viewport, state.offset, area.height as f32);
                for i in 0..area.height {
                    let (cl, cr) = (i as f32, i as f32 + 1.0);
                    for x in area.left()..area.right() {
                        let cell = &mut buf[(x, area.y + i)];
                        if end <= cl || start >= cr {
                            continue;
                        } else if start <= cl && end >= cr {
                            cell.set_symbol(" ").set_bg(color.color());
                        } else if start > cl {
                            let idx = ((cr - start) * 8.0).round() as usize;
                            cell.set_symbol(LOWER_BLOCKS[idx.min(8)]).set_fg(color.color()).set_bg(th.scrollbar_bg.color());
                        } else {
                            let idx = ((cr - end) * 8.0).round() as usize;
                            cell.set_symbol(LOWER_BLOCKS[idx.min(8)]).set_fg(th.scrollbar_bg.color()).set_bg(color.color());
                        }
                    }
                }
            }
            ScrollAxis::Horizontal => {
                let (start, end) = thumb_span(self.content, self.viewport, state.offset, area.width as f32);
                for i in 0..area.width {
                    let (cl, cr) = (i as f32, i as f32 + 1.0);
                    for y in area.top()..area.bottom() {
                        let cell = &mut buf[(area.x + i, y)];
                        if end <= cl || start >= cr {
                            continue;
                        } else if start <= cl && end >= cr {
                            cell.set_symbol(" ").set_bg(color.color());
                        } else if start > cl {
                            // thumb starts inside this cell: right part is thumb
                            let idx = ((cr - start) * 8.0).round() as usize;
                            cell.set_symbol(LEFT_BLOCKS[(8 - idx.min(8)).min(8)]).set_fg(th.scrollbar_bg.color()).set_bg(color.color());
                        } else {
                            let idx = ((end - cl) * 8.0).round() as usize;
                            cell.set_symbol(LEFT_BLOCKS[idx.min(8)]).set_fg(color.color()).set_bg(th.scrollbar_bg.color());
                        }
                    }
                }
            }
        }
    }
}

/// Adjust `offset` so `index` is inside `[offset, offset + viewport)`.
pub fn keep_visible(offset: usize, index: usize, viewport: usize) -> usize {
    if viewport == 0 {
        return offset;
    }
    if index < offset {
        index
    } else if index >= offset + viewport {
        index + 1 - viewport
    } else {
        offset
    }
}

/// Position helper for hit testing rows: which row index is under `pos` inside `area`,
/// accounting for `offset`.
pub fn row_at(area: Rect, offset: usize, pos: Position) -> Option<usize> {
    if !area.contains(pos) {
        return None;
    }
    Some(offset + (pos.y - area.y) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

    fn me(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }
    }

    #[test]
    fn keep_visible_moves_minimally() {
        assert_eq!(keep_visible(0, 5, 3), 3);
        assert_eq!(keep_visible(3, 5, 3), 3);
        assert_eq!(keep_visible(4, 2, 3), 2);
    }

    #[test]
    fn drag_and_wheel_move_offset() {
        let mut st = ScrollbarState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 10));
        Scrollbar::vertical(100, 10).render(Rect::new(0, 0, 1, 10), &mut buf, &mut st);
        assert_eq!(st.handle_mouse(me(MouseEventKind::ScrollDown, 0, 5)), Outcome::Changed);
        assert_eq!(st.offset, 3);
        st.offset = 0;
        // press on the thumb (top cell) then drag down 5 cells → 50 units
        st.handle_mouse(me(MouseEventKind::Down(MouseButton::Left), 0, 0));
        assert!(st.is_dragging());
        st.handle_mouse(me(MouseEventKind::Drag(MouseButton::Left), 0, 5));
        assert_eq!(st.offset, 50);
        st.handle_mouse(me(MouseEventKind::Up(MouseButton::Left), 0, 5));
        assert!(!st.is_dragging());
        // drawn thumb spans one cell at the top when offset is 0
        Scrollbar::vertical(100, 10).offset(0).render(Rect::new(0, 0, 1, 10), &mut buf, &mut st);
        assert_ne!(buf[(0, 0)].bg, buf[(0, 9)].bg);
    }
}
