//! Generic catalog: caller-defined columns, row marks, filter, cursor. Built on
//! [`crate::widgets::table::DataTable`]'s column machinery for aligned multi-column lists.
//!
//! [`crate::widgets::ai_agents::ModelPicker`], [`crate::widgets::ai_agents::SessionList`], and
//! [`crate::widgets::ai_agents::AgentTree`] are domain-specific presets; `Catalog` is the generic
//! widget for arbitrary catalogs (skills libraries, MCP servers, saved records).
//!
//! ```
//! use tuile::prelude::*;
//! use tuile::widgets::{Catalog, CatalogState, TableColumn, TableRow};
//! # let area = Rect::new(0, 0, 60, 20);
//! # let mut buf = Buffer::empty(area);
//! let cols = vec![TableColumn::new("Name").width(Constraint::Fill(1))];
//! let rows = vec![TableRow::from(vec!["server-01"]), TableRow::from(vec!["server-02"])];
//! let mut state = CatalogState::default();
//! Catalog::new(cols, rows).focused(true).render(area, &mut buf, &mut state);
//! ```

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Constraint, Rect};
use ratatui_core::style::Style;
use ratatui_core::widgets::StatefulWidget;

use crate::core::{Hit, HitBox, Interactive, MinSize, Outcome, is_press, wheel_delta};
use crate::draw::{self, fill, put, put_aligned, put_centered, st, truncate};
use crate::fuzzy;
use crate::layout::pad_trbl;
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};
use crate::widgets::table::{TableColumn, TableRow};

/// Mutable state for a catalog: cursor, filter, activation, hover, scroll.
#[derive(Clone, Debug, Default)]
pub struct CatalogState {
    pub cursor: usize,
    pub offset: usize,
    pub filter: String,
    pub hover_row: Option<usize>,
    activated: Option<usize>,
    order: Vec<usize>,
    dirty: bool,
    /// Row count `order` was built from; a mismatch means the caller's data changed under it.
    built_len: usize,
    hits: Vec<HitBox>,
    scrollbar: ScrollbarState,
    last_click: Option<(usize, Instant)>,
}

impl CatalogState {
    /// Creates a new catalog state.
    pub fn new() -> Self {
        Self::default()
    }

    /// True if an item was activated (Enter or double-click); drains on read.
    pub fn is_activated(&self) -> bool {
        self.activated.is_some()
    }

    /// Drains the activated row index; returns `None` if no activation since last drain.
    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }

    /// Marks the filter as changed, triggering a rebuild on next render.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `order` is stale when the filter changed or when the caller passed a different number of
    /// rows — including the first frame of a `Default` state, which would otherwise render the
    /// empty message over a full catalogue.
    fn stale(&self, rows: &[TableRow]) -> bool {
        self.dirty || self.built_len != rows.len()
    }

    fn rebuild_order(&mut self, rows: &[TableRow]) {
        self.order.clear();
        if self.filter.is_empty() {
            self.order.extend(0..rows.len());
        } else {
            for (i, row) in rows.iter().enumerate() {
                let haystack = row
                    .cells
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if fuzzy::fuzzy(&self.filter, &haystack).is_some() {
                    self.order.push(i);
                }
            }
        }
        self.dirty = false;
        self.built_len = rows.len();
    }
}

impl Interactive for CatalogState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.order.len() {
                    self.cursor += 1;
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::Home => {
                if self.cursor != 0 {
                    self.cursor = 0;
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::End => {
                let end = self.order.len().saturating_sub(1);
                if self.cursor != end {
                    self.cursor = end;
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::PageUp => {
                if self.cursor > 0 {
                    self.cursor = self.cursor.saturating_sub(10);
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::PageDown => {
                let end = self.order.len().saturating_sub(1);
                if self.cursor < end {
                    self.cursor = (self.cursor + 10).min(end);
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::Enter => {
                if !self.order.is_empty() {
                    self.activated = Some(self.order[self.cursor]);
                    Outcome::Submitted
                } else {
                    Outcome::Ignored
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;

        if self.hits.len() > 1 {
            let body_hits = &mut self.hits[1..];
            for (i, hit) in body_hits.iter_mut().enumerate() {
                match hit.mouse(&m) {
                    Hit::Press if !self.order.is_empty() => {
                        self.cursor = i;
                        let now = Instant::now();
                        // Double-click detection
                        if let Some((last_row, last_time)) = self.last_click
                            && last_row == i
                            && now.duration_since(last_time).as_millis() < 400
                        {
                            self.activated = Some(self.order[i]);
                            self.last_click = None;
                            return Outcome::Submitted;
                        }
                        self.last_click = Some((i, now));
                        out |= Outcome::Changed;
                    }
                    Hit::HoverChanged if hit.hover => {
                        self.hover_row = Some(i);
                        out |= Outcome::Consumed;
                    }
                    _ => {}
                }
            }
        }

        if let Some(d) = wheel_delta(&m) {
            self.offset = self.offset.saturating_add_signed(-d as isize);
            out |= Outcome::Consumed;
        }

        out
    }
}

/// Generic catalog: caller-defined columns, row marks, filter, cursor.
#[derive(Clone, Debug)]
pub struct Catalog {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    show_marks: bool,
    show_filter: bool,
    column_gap: u16,
    padding: u16,
    empty_text: String,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Catalog {
    /// Creates a new catalog with the given columns and rows.
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow>) -> Self {
        Self {
            columns,
            rows,
            show_marks: false,
            show_filter: false,
            column_gap: 2,
            padding: 1,
            empty_text: "No items".to_string(),
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    /// Shows a mark column (┃) for marked/selected rows.
    pub fn show_marks(mut self, s: bool) -> Self {
        self.show_marks = s;
        self
    }

    /// Shows a filter row at the top.
    pub fn show_filter(mut self, s: bool) -> Self {
        self.show_filter = s;
        self
    }

    /// Gap between columns in cells.
    pub fn column_gap(mut self, g: u16) -> Self {
        self.column_gap = g;
        self
    }

    /// Padding around the content.
    pub fn padding(mut self, p: u16) -> Self {
        self.padding = p;
        self
    }

    /// Text shown when no rows are present.
    pub fn empty_text(mut self, t: impl Into<String>) -> Self {
        self.empty_text = t.into();
        self
    }

    /// Sets the focused state.
    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    /// Sets the enabled state.
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    /// Sets the theme.
    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

impl MinSize for Catalog {
    /// Config-dependent: `(width, height)` in cells.
    fn min_size(&self) -> (u16, u16) {
        let header_h = 1;
        let filter_h = if self.show_filter { 1 } else { 0 };
        let padding_h = self.padding * 2;
        let total_h = header_h + filter_h + padding_h;
        (10, total_h.max(2))
    }
}

impl StatefulWidget for Catalog {
    type State = CatalogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);

        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            state.hits.clear();
            return;
        }

        if state.stale(&self.rows) {
            state.rebuild_order(&self.rows);
        }

        state.cursor = state.cursor.min(state.order.len().saturating_sub(1));

        let content = if self.padding > 0 {
            pad_trbl(area, self.padding, self.padding, self.padding, self.padding)
        } else {
            area
        };

        if content.width < 2 || content.height < 1 {
            state.hits.clear();
            return;
        }

        let filter_h = if self.show_filter { 1 } else { 0 };
        let header_h = 1;
        let body_h = content
            .height
            .saturating_sub(header_h)
            .saturating_sub(filter_h);
        let visible_rows = state.order.len().min(body_h as usize);

        state.offset = keep_visible(state.offset, state.cursor, visible_rows);

        // Compute column widths (reuse DataTable's solving)
        let total_w = content.width;
        let mut col_widths = vec![0u16; self.columns.len()];
        let mut fill_indices = vec![];
        let mut used = 0u16;

        for (i, col) in self.columns.iter().enumerate() {
            let w = match col.width {
                Constraint::Length(l) => l,
                Constraint::Min(m) => m,
                Constraint::Max(m) => m.min(total_w.saturating_sub(used)),
                Constraint::Percentage(p) => (total_w as f32 * p as f32 / 100.0) as u16,
                Constraint::Fill(_) => {
                    fill_indices.push(i);
                    0
                }
                _ => 10,
            };
            col_widths[i] = w;
            if !fill_indices.contains(&i) {
                used += w + if i > 0 { self.column_gap } else { 0 };
            }
        }

        let remaining = total_w.saturating_sub(used);
        if !fill_indices.is_empty() {
            let per_fill = remaining / fill_indices.len() as u16;
            for &i in &fill_indices {
                col_widths[i] = per_fill;
            }
        }

        // Allocate hits
        state.hits.clear();
        state.hits.resize(1 + visible_rows, HitBox::default());

        // Filter row
        if self.show_filter {
            let filter_y = content.y;
            fill(
                buf,
                Rect::new(content.x, filter_y, content.width, 1),
                th.panel,
            );
            let filter_text = if state.filter.is_empty() {
                "Filter…".to_string()
            } else {
                state.filter.clone()
            };
            put(
                buf,
                content.x,
                filter_y,
                &truncate(&filter_text, content.width as usize),
                content.width,
                st(
                    if state.filter.is_empty() {
                        th.text_muted
                    } else {
                        th.text
                    },
                    th.panel,
                ),
            );
        }

        // Header
        let header_y = content.y + filter_h;
        let mut x = content.x;
        for (col, &w) in self.columns.iter().zip(&col_widths) {
            if x >= content.x + content.width {
                break;
            }
            let col_w = w.min(content.x + content.width - x);
            put(
                buf,
                x,
                header_y,
                &truncate(&col.title, col_w as usize),
                col_w,
                st(th.text_muted, th.surface),
            );
            x += col_w + self.column_gap;
        }

        // Body rows
        let body_y = header_y + 1;
        for row_offset in 0..visible_rows {
            let vis_idx = state.offset + row_offset;
            if vis_idx >= state.order.len() {
                break;
            }

            let row_idx = state.order[vis_idx];
            let row = &self.rows[row_idx];

            let y = body_y + row_offset as u16;
            let is_cursor = vis_idx == state.cursor;
            let is_hover = state.hover_row == Some(row_offset);

            let bg = if is_cursor && self.focused {
                th.cursor_bg
            } else if is_cursor {
                th.cursor_blurred_bg
            } else if is_hover {
                th.hover_bg
            } else {
                th.surface
            };

            let fg = if row.disabled {
                th.text_disabled
            } else if is_cursor && self.focused {
                th.cursor_fg
            } else if let Some(v) = row.variant {
                th.text_variant(v)
            } else {
                th.text
            };

            let row_rect = Rect {
                x: content.x,
                y,
                width: content.width,
                height: 1,
            };
            fill(buf, row_rect, bg);

            if row_offset + 1 < state.hits.len() {
                state.hits[row_offset + 1].set_area(row_rect);
            }

            if self.show_marks && row.variant.is_some() {
                put(buf, content.x, y, "┃", 1, st(th.accent, bg));
            }

            let mut x = content.x;
            for (col_i, (col, &w)) in self.columns.iter().zip(&col_widths).enumerate() {
                if x >= content.x + content.width {
                    break;
                }
                let cell_w = w.min(content.x + content.width - x);
                let cell = row.cells.get(col_i).cloned().unwrap_or_default();

                let mut cell_style = Style::new().fg(fg.color()).bg(bg.color());
                if !(is_cursor && self.focused)
                    && let Some(s) = cell.style
                {
                    cell_style = cell_style.patch(Style { bg: None, ..s });
                }

                let align = if cell.sort_key.is_some() {
                    Alignment::Right
                } else {
                    col.align
                };
                let cell_rect = Rect {
                    x,
                    y,
                    width: cell_w,
                    height: 1,
                };
                put_aligned(
                    buf,
                    cell_rect,
                    &truncate(&cell.text, cell_w as usize),
                    align,
                    cell_style,
                );

                x += cell_w + self.column_gap;
            }
        }

        // Scrollbar
        if state.order.len() > visible_rows {
            let sb_area = Rect {
                x: content.x + content.width - 1,
                y: body_y,
                width: 1,
                height: body_h,
            };
            Scrollbar::vertical(state.order.len(), visible_rows)
                .offset(state.offset)
                .theme(&th)
                .render(sb_area, buf, &mut state.scrollbar);
        }

        // Empty state
        if state.order.is_empty() {
            let msg_y = body_y + body_h / 2;
            if msg_y < body_y + body_h {
                put_centered(
                    buf,
                    Rect {
                        x: content.x,
                        y: msg_y,
                        width: content.width,
                        height: 1,
                    },
                    &self.empty_text,
                    st(th.text_muted, th.surface),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Constraint;

    fn painted(buf: &Buffer) -> bool {
        buf.content().iter().any(|c| c.symbol() != " ")
    }

    #[test]
    fn draws_at_its_minimum_and_refuses_visibly_below_it() {
        let cols = vec![TableColumn::new("Name").width(Constraint::Fill(1))];
        let rows = vec![TableRow::from(vec!["server-01"])];
        let catalog = Catalog::new(cols.clone(), rows.clone());
        let (w, h) = catalog.min_size();
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        catalog.render(buf.area, &mut buf, &mut CatalogState::default());
        assert!(painted(&buf), "should draw at its stated minimum");

        // One cell short on whichever axis can shrink
        let (sw, sh) = if h > 1 { (w, h - 1) } else { (w - 1, h) };
        let mut buf = Buffer::empty(Rect::new(0, 0, sw, sh));
        Catalog::new(cols, rows).render(buf.area, &mut buf, &mut CatalogState::default());
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one cell short must refuse visibly, not silently draw nothing"
        );
    }

    #[test]
    fn different_column_sets_render_their_own_headers_and_align_values() {
        // Two unrelated catalogues
        let cols1 = vec![
            TableColumn::new("Name").width(Constraint::Length(10)),
            TableColumn::new("Count")
                .width(Constraint::Length(6))
                .align(Alignment::Right),
        ];
        let rows1 = vec![
            TableRow::from(vec!["server-01", "42"]),
            TableRow::from(vec!["server-02", "100"]),
        ];

        let cols2 = vec![
            TableColumn::new("Tool").width(Constraint::Length(15)),
            TableColumn::new("Status").width(Constraint::Length(8)),
        ];
        let rows2 = vec![
            TableRow::from(vec!["aisandbox", "enabled"]),
            TableRow::from(vec!["context-mode", "enabled"]),
        ];

        let mut buf1 = Buffer::empty(Rect::new(0, 0, 30, 10));
        Catalog::new(cols1, rows1).render(buf1.area, &mut buf1, &mut CatalogState::default());

        let mut buf2 = Buffer::empty(Rect::new(0, 0, 30, 10));
        Catalog::new(cols2, rows2).render(buf2.area, &mut buf2, &mut CatalogState::default());

        // Both painted
        assert!(painted(&buf1));
        assert!(painted(&buf2));

        // Headers differ (different column titles)
        let content1 = buf1
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        let content2 = buf2
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            content1.contains("Name"),
            "catalog 1 should have Name column"
        );
        assert!(
            content1.contains("Count"),
            "catalog 1 should have Count column"
        );
        assert!(
            content2.contains("Tool"),
            "catalog 2 should have Tool column"
        );
        assert!(
            content2.contains("Status"),
            "catalog 2 should have Status column"
        );
    }

    #[test]
    fn a_fresh_state_renders_the_rows_not_the_empty_message() {
        let cols = [TableColumn::new("Name").width(Constraint::Fill(1))];
        let rows = [TableRow::from(vec!["alpha"]), TableRow::from(vec!["beta"])];
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
        // no mark_dirty, no rebuild: exactly what a caller writes on the first frame
        let mut state = CatalogState::new();
        Catalog::new(cols.to_vec(), rows.to_vec()).render(buf.area, &mut buf, &mut state);

        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("alpha"), "{content:?}");
        assert!(
            !content.contains("No items"),
            "a catalogue with rows must never show the empty message"
        );

        // and it follows the caller's data when the row count changes
        let more = [
            TableRow::from(vec!["alpha"]),
            TableRow::from(vec!["beta"]),
            TableRow::from(vec!["gamma"]),
        ];
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
        Catalog::new(cols.to_vec(), more.to_vec()).render(buf.area, &mut buf, &mut state);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("gamma"), "{content:?}");
    }

    #[test]
    fn cursor_clamps_at_both_ends() {
        let _cols = [TableColumn::new("Name").width(Constraint::Fill(1))];
        let _rows = [
            TableRow::from(vec!["a"]),
            TableRow::from(vec!["b"]),
            TableRow::from(vec!["c"]),
        ];
        let mut state = CatalogState {
            cursor: 0,
            ..Default::default()
        };

        // Up at the top
        let out = state.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(out, Outcome::Ignored);
        assert_eq!(state.cursor, 0);

        // Move to end
        state.cursor = 2;
        let out = state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(out, Outcome::Ignored);
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn filter_narrows_visible_rows() {
        let cols = [TableColumn::new("Name").width(Constraint::Fill(1))];
        let rows = [
            TableRow::from(vec!["apple"]),
            TableRow::from(vec!["banana"]),
            TableRow::from(vec!["apricot"]),
        ];
        let mut state = CatalogState {
            filter: "ap".to_string(),
            ..Default::default()
        };
        state.mark_dirty();

        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 10));
        Catalog::new(cols.to_vec(), rows.to_vec()).render(buf.area, &mut buf, &mut state);

        // Should match apple and apricot
        assert_eq!(state.order.len(), 2);
        assert_eq!(state.order[0], 0); // apple
        assert_eq!(state.order[1], 2); // apricot
    }

    #[test]
    fn enter_returns_submitted_and_take_activated_drains_once() {
        let _cols = [TableColumn::new("Name").width(Constraint::Fill(1))];
        let rows = [TableRow::from(vec!["item"]), TableRow::from(vec!["two"])];
        let mut state = CatalogState::default();
        state.rebuild_order(&rows);
        state.cursor = 1;

        let out = state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(out, Outcome::Submitted);
        assert!(state.is_activated());

        let activated = state.take_activated();
        assert_eq!(activated, Some(1));

        let activated2 = state.take_activated();
        assert_eq!(activated2, None);
    }
}
