//! Resizable split pane with divider — horizontal or vertical. Drag the divider to resize,
//! double-click to reset, or call `state.resize_by(delta)` from key handlers.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! let mut state = SplitState::new(SplitSize::Ratio(0.3), 10, 10);
//! let (l, r) = SplitPane::new().direction(Direction::Horizontal).render_split(area, &mut buf, &mut state);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::{Direction, Rect};

use crate::core::{Hit, HitBox, Outcome, is_left_drag, is_left_up, mouse_pos};
use crate::draw::{fill, put_cell, st};
use crate::theme::{self, Theme};

/// Initial size of the first pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitSize {
    /// Fixed cells.
    Cells(u16),
    /// Ratio of total size (clamped to 0..=1).
    Ratio(f32),
}

impl Default for SplitSize {
    fn default() -> Self {
        SplitSize::Ratio(0.5)
    }
}

/// Divider style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SplitDivider {
    /// Single line (`│`/`─`).
    #[default]
    Line,
    /// Thick (`┃`/`━`).
    Thick,
    /// Dotted (`┊`/`┄`).
    Dotted,
    /// No divider (zero-width).
    Hidden,
}

/// State for a split pane: current position, drag state, cached layout.
#[derive(Clone, Debug, Default)]
pub struct SplitState {
    /// The *initial* size spec (preserved for reset).
    pub initial: SplitSize,
    /// Current size of the first pane in cells.
    pub pos: u16,
    /// Minimum size of the first pane.
    pub min_first: u16,
    /// Minimum size of the second pane.
    pub min_second: u16,
    /// Divider HitBox.
    pub hit: HitBox,
    /// True while dragging.
    pub dragging: bool,
    /// Cached layout (first, divider, second).
    pub cached: (Rect, Rect, Rect),
    /// Last click time for double-click reset detection.
    last_click: Option<Instant>,
    /// Collapsed state: 0 = expanded, 1 = first collapsed, 2 = second collapsed.
    collapsed: u8,
}

impl SplitState {
    pub fn new(initial: SplitSize, min_first: u16, min_second: u16) -> Self {
        Self {
            initial,
            pos: 0,
            min_first,
            min_second,
            ..Default::default()
        }
    }

    /// Adjust position by `delta` cells, clamped to the minimum sizes.
    pub fn resize_by(&mut self, delta: i32) {
        let (first, div, second) = self.cached;
        // panes side by side share their y/height → horizontal split
        let horizontal = first.y == second.y && first.height == second.height;
        let (total, divider_w) = if horizontal {
            (first.width + div.width + second.width, div.width)
        } else {
            (first.height + div.height + second.height, div.height)
        };
        let max_pos = total.saturating_sub(self.min_second + divider_w);
        self.pos = (self.pos as i32 + delta)
            .clamp(self.min_first as i32, max_pos.max(self.min_first) as i32)
            as u16;
    }

    /// Collapse the first pane (hide it, second pane takes full area).
    pub fn collapse_first(&mut self) {
        self.collapsed = 1;
    }

    /// Collapse the second pane.
    pub fn collapse_second(&mut self) {
        self.collapsed = 2;
    }

    /// Restore to expanded state.
    pub fn restore(&mut self) {
        self.collapsed = 0;
    }

    /// Returns the cached (first, second) pane rects. Call after `render_split`.
    pub fn rects(&self) -> (Rect, Rect) {
        (self.cached.0, self.cached.2)
    }

    /// Reset to initial size.
    fn reset(&mut self, total: u16, divider_w: u16) {
        self.pos = match self.initial {
            SplitSize::Cells(c) => c,
            SplitSize::Ratio(r) => {
                (total.saturating_sub(divider_w) as f32 * r.clamp(0.0, 1.0)) as u16
            }
        };
    }
}

impl crate::core::Interactive for SplitState {
    fn handle_key(&mut self, _k: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let h = self.hit.mouse(&m);
        let mut out = Outcome::Ignored;

        if h == Hit::Press {
            self.dragging = true;
            let now = Instant::now();
            if let Some(last) = self.last_click
                && now.duration_since(last).as_millis() < 400
            {
                // Double-click: reset
                let total = self.cached.0.width.max(self.cached.0.height);
                let divider_w = if self.cached.1.width > 0 || self.cached.1.height > 0 {
                    1
                } else {
                    0
                };
                self.reset(total, divider_w);
                out = Outcome::Changed;
            }
            self.last_click = Some(now);
        } else if is_left_up(&m) && self.dragging {
            self.dragging = false;
            out = Outcome::Consumed;
        } else if is_left_drag(&m) && self.dragging {
            // Drag: the divider follows the pointer along the split axis
            let pos = mouse_pos(&m);
            let (first, divider, second) = self.cached;
            let horizontal = first.y == second.y && first.height == second.height;
            let (total, divider_w, raw) = if horizontal {
                (
                    first.width + divider.width + second.width,
                    divider.width,
                    pos.x.saturating_sub(first.x),
                )
            } else {
                (
                    first.height + divider.height + second.height,
                    divider.height,
                    pos.y.saturating_sub(first.y),
                )
            };
            let max_pos = total.saturating_sub(self.min_second + divider_w);
            self.pos = raw.clamp(self.min_first.min(max_pos), max_pos);
            out = Outcome::Consumed;
        } else if h == Hit::HoverChanged {
            out = Outcome::Consumed;
        }

        out
    }
}

/// Resizable split pane builder.
#[derive(Clone, Debug)]
pub struct SplitPane {
    direction: Direction,
    /// `None` = keep whatever the state was constructed with.
    initial: Option<SplitSize>,
    min_first: Option<u16>,
    min_second: Option<u16>,
    divider: SplitDivider,
    focused: bool,
    theme: Option<Theme>,
}

impl SplitPane {
    pub fn new() -> Self {
        Self {
            direction: Direction::Horizontal,
            initial: None,
            min_first: None,
            min_second: None,
            divider: SplitDivider::default(),
            focused: false,
            theme: None,
        }
    }

    pub fn direction(mut self, d: Direction) -> Self {
        self.direction = d;
        self
    }

    /// Initial size of the first pane (overrides the one given to `SplitState::new`).
    pub fn initial(mut self, s: SplitSize) -> Self {
        self.initial = Some(s);
        self
    }

    pub fn min_first(mut self, m: u16) -> Self {
        self.min_first = Some(m);
        self
    }

    pub fn min_second(mut self, m: u16) -> Self {
        self.min_second = Some(m);
        self
    }

    pub fn divider(mut self, d: SplitDivider) -> Self {
        self.divider = d;
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    /// Render the split and return (first_rect, second_rect).
    pub fn render_split(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut SplitState,
    ) -> (Rect, Rect) {
        let th = self.theme.unwrap_or_else(theme::current);

        // Builder overrides win; the state keeps its own config otherwise. A changed initial
        // (or the very first render) resets the position.
        if let Some(m) = self.min_first {
            state.min_first = m;
        }
        if let Some(m) = self.min_second {
            state.min_second = m;
        }
        let initial_changed = self.initial.is_some_and(|i| i != state.initial);
        if state.pos == 0 || initial_changed {
            if let Some(i) = self.initial {
                state.initial = i;
            }
            let divider_w = if self.divider == SplitDivider::Hidden {
                0
            } else {
                1
            };
            let total = if self.direction == Direction::Horizontal {
                area.width
            } else {
                area.height
            };
            state.reset(total, divider_w);
        }

        let divider_w = if self.divider == SplitDivider::Hidden {
            0
        } else {
            1
        };
        let is_horz = self.direction == Direction::Horizontal;
        let total = if is_horz { area.width } else { area.height };
        let max_pos = total.saturating_sub(state.min_second + divider_w);
        // when the area is smaller than both minimums, the first pane just takes what it can
        let pos = state
            .pos
            .clamp(state.min_first.min(max_pos), max_pos)
            .min(total);
        state.pos = pos;

        // Handle collapsed state
        if state.collapsed == 1 {
            // First collapsed
            state.cached = (Rect::default(), Rect::default(), area);
            state.hit.set_area(Rect::default());
            return (Rect::default(), area);
        } else if state.collapsed == 2 {
            // Second collapsed
            state.cached = (area, Rect::default(), Rect::default());
            state.hit.set_area(Rect::default());
            return (area, Rect::default());
        }

        let (first, divider, second) = if is_horz {
            let f = Rect { width: pos, ..area };
            let d = Rect {
                x: area.x + pos,
                width: divider_w,
                ..area
            };
            let s = Rect {
                x: area.x + pos + divider_w,
                width: total.saturating_sub(pos + divider_w),
                ..area
            };
            (f, d, s)
        } else {
            let f = Rect {
                height: pos,
                ..area
            };
            let d = Rect {
                y: area.y + pos,
                height: divider_w,
                ..area
            };
            let s = Rect {
                y: area.y + pos + divider_w,
                height: total.saturating_sub(pos + divider_w),
                ..area
            };
            (f, d, s)
        };

        state.cached = (first, divider, second);
        state.hit.set_area(divider);

        // Draw divider
        if divider_w > 0 {
            let (sym, grip) = match self.divider {
                SplitDivider::Line => {
                    if is_horz {
                        ("│", "┃")
                    } else {
                        ("─", "━")
                    }
                }
                SplitDivider::Thick => {
                    if is_horz {
                        ("┃", "┃")
                    } else {
                        ("━", "━")
                    }
                }
                SplitDivider::Dotted => {
                    if is_horz {
                        ("┊", "┃")
                    } else {
                        ("┄", "━")
                    }
                }
                SplitDivider::Hidden => ("", ""),
            };

            let color = if state.hit.hover || state.dragging {
                th.primary
            } else {
                th.text_disabled
            };
            fill(buf, divider, th.background);

            if is_horz {
                for y in divider.y..divider.bottom() {
                    let mid = divider.y + divider.height / 2;
                    let use_grip = (y as i16 - mid as i16).abs() <= 1;
                    put_cell(
                        buf,
                        divider.x,
                        y,
                        if use_grip { grip } else { sym },
                        st(color, th.background),
                    );
                }
            } else {
                for x in divider.x..divider.right() {
                    let mid = divider.x + divider.width / 2;
                    let use_grip = (x as i16 - mid as i16).abs() <= 1;
                    put_cell(
                        buf,
                        x,
                        divider.y,
                        if use_grip { grip } else { sym },
                        st(color, th.background),
                    );
                }
            }
        }

        (first, second)
    }
}

impl Default for SplitPane {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Direction;

    #[test]
    fn split_horizontal_ratio() {
        let mut state = SplitState::new(SplitSize::Ratio(0.3), 5, 5);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 20));
        let area = buf.area;
        let (l, r) = SplitPane::new()
            .direction(Direction::Horizontal)
            .render_split(area, &mut buf, &mut state);
        // 100 cells total, divider 1, so 99 available. 0.3 * 99 = 29.7 ~= 29
        assert_eq!(l.width, 29);
        assert_eq!(r.width, 100 - 29 - 1);
    }

    #[test]
    fn split_vertical_cells() {
        let mut state = SplitState::new(SplitSize::Cells(10), 2, 2);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 30));
        let area = buf.area;
        let (t, b) = SplitPane::new()
            .direction(Direction::Vertical)
            .render_split(area, &mut buf, &mut state);
        assert_eq!(t.height, 10);
        assert_eq!(b.height, 30 - 10 - 1);
    }

    #[test]
    fn split_clamped_to_min() {
        let mut state = SplitState::new(SplitSize::Cells(3), 10, 10);
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 20));
        let area = buf.area;
        let (l, _r) = SplitPane::new()
            .direction(Direction::Horizontal)
            .min_first(10)
            .min_second(10)
            .render_split(area, &mut buf, &mut state);
        // pos=3 is below min_first=10, so clamped to 10
        assert_eq!(l.width, 10);
    }

    #[test]
    fn split_resize_by() {
        let mut state = SplitState::new(SplitSize::Cells(20), 5, 5);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        let area = buf.area;
        SplitPane::new().render_split(area, &mut buf, &mut state);
        state.resize_by(10);
        assert_eq!(state.pos, 30);
        state.resize_by(-100); // Should clamp to min_first
        assert_eq!(state.pos, 5);
    }

    #[test]
    fn split_collapse() {
        let mut state = SplitState::new(SplitSize::Ratio(0.5), 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        let area = buf.area;
        state.collapse_first();
        let (l, r) = SplitPane::new().render_split(area, &mut buf, &mut state);
        assert_eq!(l.width, 0);
        assert_eq!(r.width, 40);

        state.restore();
        let (l2, r2) = SplitPane::new().render_split(area, &mut buf, &mut state);
        assert!(l2.width > 0 && r2.width > 0);
    }
}
