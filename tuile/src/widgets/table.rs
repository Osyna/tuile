//! Data table with sortable columns, row/cell cursors, multi-select, filtering, zebra stripes,
//! header hover/click, column resize, scrollbars, and a simple key-value list variant.
//!
//! ```
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 80, 20);
//! # let mut buf = Buffer::empty(area);
//! let cols = vec![
//!     TableColumn::new("Name").width(Constraint::Fill(1)).sortable(true),
//!     TableColumn::new("CPU").width(Constraint::Length(6)).align(Alignment::Right).sortable(true),
//! ];
//! let rows = vec![
//!     TableRow::from(vec!["server-01", "45.2%"]),
//!     TableRow::from(vec!["server-02", "12.8%"]),
//! ];
//! let mut state = DataTableState::new();
//! DataTable::new(cols, rows).cursor(TableCursor::Row).render(area, &mut buf, &mut state);
//! ```

use std::collections::{BTreeSet, HashMap};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Constraint, Rect};
use ratatui_core::style::Style;
use ratatui_core::widgets::StatefulWidget;

use crate::core::{Hit, HitBox, Interactive, Outcome, is_press, mouse_in, mouse_pos, wheel_delta};
use crate::draw::{fill, put, put_aligned, put_centered, st, truncate};
use crate::layout::pad_trbl;
use crate::theme::{self, Theme, Variant};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

// column

/// One column definition: title, width constraint, alignment, sortability, and a unique key.
#[derive(Clone, Debug)]
pub struct TableColumn {
    pub title: String,
    pub width: Constraint,
    pub align: Alignment,
    pub sortable: bool,
    pub key: usize,
}

impl TableColumn {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: Constraint::Fill(1),
            align: Alignment::Left,
            sortable: false,
            key: 0,
        }
    }
    pub fn width(mut self, w: Constraint) -> Self {
        self.width = w;
        self
    }
    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }
    pub fn sortable(mut self, s: bool) -> Self {
        self.sortable = s;
        self
    }
    pub fn key(mut self, k: usize) -> Self {
        self.key = k;
        self
    }
}

// cell & row

/// One cell: text, optional style override, and a numeric sort key.
#[derive(Clone, Debug, Default)]
pub struct TableCell {
    pub text: String,
    pub style: Option<Style>,
    pub sort_key: Option<f64>,
}

impl From<&str> for TableCell {
    fn from(s: &str) -> Self {
        Self {
            text: s.to_string(),
            style: None,
            sort_key: None,
        }
    }
}
impl From<String> for TableCell {
    fn from(text: String) -> Self {
        Self {
            text,
            style: None,
            sort_key: None,
        }
    }
}
impl From<f64> for TableCell {
    fn from(v: f64) -> Self {
        Self {
            text: format!("{:.1}", v),
            style: None,
            sort_key: Some(v),
        }
    }
}

impl TableCell {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            sort_key: None,
        }
    }
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }
    pub fn sort_key(mut self, k: f64) -> Self {
        self.sort_key = Some(k);
        self
    }
}

/// One row: cells (one per column), optional variant for row colouring, and a disabled flag.
#[derive(Clone, Debug, Default)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub variant: Option<Variant>,
    pub disabled: bool,
}

impl<T: Into<TableCell>> From<Vec<T>> for TableRow {
    fn from(cells: Vec<T>) -> Self {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
            variant: None,
            disabled: false,
        }
    }
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            variant: None,
            disabled: false,
        }
    }
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = Some(v);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

// cursor modes

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableCursor {
    #[default]
    None,
    Row,
    Cell,
    Column,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableBorders {
    #[default]
    None,
    Horizontal,
    Vertical,
    All,
}

// state

/// Mutable state for a data table: cursor, selection, sort, filter, scroll, hover, resize.
#[derive(Clone, Debug, Default)]
pub struct DataTableState {
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub offset_y: usize,
    pub offset_x: usize,
    pub selected: BTreeSet<usize>,
    pub sort: Option<(usize, bool)>, // (col_idx, ascending)
    pub hover_row: Option<usize>,
    pub filter: String,
    pub activated: Option<usize>,
    order: Vec<usize>,
    dirty: bool,
    hits: Vec<HitBox>,
    scrollbar: ScrollbarState,
    col_widths: Vec<u16>,
    col_resize: Option<(usize, u16, u16)>, // (col, start_x, start_w)
    width_overrides: HashMap<usize, u16>,
}

impl DataTableState {
    pub fn new() -> Self {
        Self {
            dirty: true,
            ..Default::default()
        }
    }
    pub fn visible_len(&self) -> usize {
        self.order.len()
    }
    pub fn current_row(&self) -> Option<usize> {
        self.order.get(self.cursor_row).copied()
    }
    /// Row index (into the caller's rows) activated with Enter/double-click since the last call.
    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }
    pub fn set_filter(&mut self, f: &str) {
        if self.filter != f {
            self.filter = f.to_string();
            self.dirty = true;
        }
    }
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }
    fn rebuild_order(&mut self, rows: &[TableRow], cols: &[TableColumn]) {
        let query = self.filter.to_lowercase();
        let mut indices: Vec<usize> = (0..rows.len())
            .filter(|&i| {
                if query.is_empty() {
                    return true;
                }
                rows[i]
                    .cells
                    .iter()
                    .any(|c| c.text.to_lowercase().contains(&query))
            })
            .collect();

        if let Some((col, asc)) = self.sort
            && col < cols.len()
        {
            indices.sort_by(|&a, &b| {
                let ca = rows[a].cells.get(col);
                let cb = rows[b].cells.get(col);
                let ord = match (ca, cb) {
                    (Some(ca), Some(cb)) => {
                        if let (Some(ka), Some(kb)) = (ca.sort_key, cb.sort_key) {
                            ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            let ta = ca.text.to_lowercase();
                            let tb = cb.text.to_lowercase();
                            ta.cmp(&tb)
                        }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                };
                if asc { ord } else { ord.reverse() }
            });
        }
        self.order = indices;
        self.dirty = false;
    }
}

impl Interactive for DataTableState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let old = (self.cursor_row, self.cursor_col);
        match key.code {
            KeyCode::Up => self.cursor_row = self.cursor_row.saturating_sub(1),
            KeyCode::Down if self.cursor_row + 1 < self.order.len() => self.cursor_row += 1,
            KeyCode::Left => self.cursor_col = self.cursor_col.saturating_sub(1),
            KeyCode::Right => self.cursor_col += 1,
            KeyCode::Home => self.cursor_row = 0,
            KeyCode::End if !self.order.is_empty() => self.cursor_row = self.order.len() - 1,
            KeyCode::PageUp => self.cursor_row = self.cursor_row.saturating_sub(10),
            KeyCode::PageDown => {
                self.cursor_row = (self.cursor_row + 10).min(self.order.len().saturating_sub(1))
            }
            KeyCode::Enter => {
                if let Some(r) = self.current_row() {
                    self.activated = Some(r);
                    return Outcome::Changed;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(r) = self.current_row() {
                    if self.selected.contains(&r) {
                        self.selected.remove(&r);
                    } else {
                        self.selected.insert(r);
                    }
                    return Outcome::Changed;
                }
            }
            KeyCode::Char('s') => {
                if let Some((col, asc)) = self.sort {
                    if col == self.cursor_col {
                        self.sort = if asc { Some((col, false)) } else { None };
                    } else {
                        self.sort = Some((self.cursor_col, true));
                    }
                } else {
                    self.sort = Some((self.cursor_col, true));
                }
                self.dirty = true;
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        }
        if (self.cursor_row, self.cursor_col) != old {
            Outcome::Consumed
        } else {
            Outcome::Ignored
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        out |= self.scrollbar.handle_mouse(m);

        if self.col_resize.is_some() {
            match m.kind {
                crossterm::event::MouseEventKind::Drag(_) => {
                    if let Some((col, start_x, start_w)) = self.col_resize {
                        let delta = (m.column as i32) - (start_x as i32);
                        let new_w = (start_w as i32 + delta).max(3) as u16;
                        self.width_overrides.insert(col, new_w);
                        return Outcome::Consumed;
                    }
                }
                crossterm::event::MouseEventKind::Up(_) => {
                    self.col_resize = None;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }

        // Header hit: sort or resize
        if let Some(hdr) = self.hits.first_mut()
            && let Hit::Press = hdr.mouse(&m)
        {
            let pos = mouse_pos(&m);
            for (i, &w) in self.col_widths.iter().enumerate() {
                if i >= self.hits.len() - 1 {
                    break;
                }
                let hit = &self.hits[i + 1];
                let a = hit.area;
                if a.x > 0 && pos.x + 1 == a.x && pos.y == a.y {
                    // Boundary: start resize
                    self.col_resize = Some((i, m.column, w));
                    return Outcome::Consumed;
                }
                if mouse_in(a, &m) {
                    // Header click: toggle sort
                    if let Some((col, asc)) = self.sort {
                        if col == i {
                            self.sort = if asc { Some((col, false)) } else { None };
                        } else {
                            self.sort = Some((i, true));
                        }
                    } else {
                        self.sort = Some((i, true));
                    }
                    self.dirty = true;
                    return Outcome::Changed;
                }
            }
        }

        // Body row hit
        if self.hits.len() > 1 {
            let body_hits = &mut self.hits[1..];
            for (i, hit) in body_hits.iter_mut().enumerate() {
                match hit.mouse(&m) {
                    Hit::Press => {
                        self.cursor_row = i;
                        return Outcome::Changed;
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
            self.offset_y = self.offset_y.saturating_add_signed(-d as isize);
            out |= Outcome::Consumed;
        }
        out
    }
}

// widget

/// Data table with sortable headers, cursor, selection, zebra stripes, scrollbars.
#[derive(Clone, Debug)]
pub struct DataTable {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    cursor: TableCursor,
    zebra: bool,
    header: bool,
    borders: TableBorders,
    column_gap: u16,
    padding: u16,
    empty_text: String,
    footer: Vec<String>,
    multi_select: bool,
    fixed_columns: usize,
    // A `fn` pointer hook the caller must match exactly; an alias would hide the signature.
    #[allow(clippy::type_complexity)]
    cell_style: Option<fn(usize, usize, &TableCell, &Theme) -> Option<Style>>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
    show_row_numbers: bool,
}

impl DataTable {
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow>) -> Self {
        Self {
            columns,
            rows,
            cursor: TableCursor::None,
            zebra: false,
            header: true,
            borders: TableBorders::None,
            column_gap: 2,
            padding: 1,
            empty_text: "No data".to_string(),
            footer: vec![],
            multi_select: false,
            fixed_columns: 0,
            cell_style: None,
            focused: false,
            enabled: true,
            theme: None,
            show_row_numbers: false,
        }
    }
    pub fn cursor(mut self, c: TableCursor) -> Self {
        self.cursor = c;
        self
    }
    pub fn zebra(mut self, z: bool) -> Self {
        self.zebra = z;
        self
    }
    pub fn header(mut self, h: bool) -> Self {
        self.header = h;
        self
    }
    pub fn borders(mut self, b: TableBorders) -> Self {
        self.borders = b;
        self
    }
    pub fn column_gap(mut self, g: u16) -> Self {
        self.column_gap = g;
        self
    }
    pub fn padding(mut self, p: u16) -> Self {
        self.padding = p;
        self
    }
    pub fn empty_text(mut self, t: impl Into<String>) -> Self {
        self.empty_text = t.into();
        self
    }
    pub fn footer(mut self, f: Vec<String>) -> Self {
        self.footer = f;
        self
    }
    pub fn multi_select(mut self, m: bool) -> Self {
        self.multi_select = m;
        self
    }
    pub fn fixed_columns(mut self, n: usize) -> Self {
        self.fixed_columns = n;
        self
    }
    pub fn cell_style(mut self, f: fn(usize, usize, &TableCell, &Theme) -> Option<Style>) -> Self {
        self.cell_style = Some(f);
        self
    }
    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
    pub fn show_row_numbers(mut self, s: bool) -> Self {
        self.show_row_numbers = s;
        self
    }
}

impl StatefulWidget for DataTable {
    type State = DataTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.width < 4 || area.height < 2 {
            return;
        }

        if state.dirty {
            state.rebuild_order(&self.rows, &self.columns);
        }

        state.cursor_col = state.cursor_col.min(self.columns.len().saturating_sub(1));
        state.cursor_row = state.cursor_row.min(state.order.len().saturating_sub(1));

        let content = if self.padding > 0 {
            pad_trbl(area, self.padding, self.padding, self.padding, self.padding)
        } else {
            area
        };
        if content.width < 2 || content.height < 1 {
            return;
        }

        let header_h = if self.header { 1 } else { 0 };
        let body_h = content.height.saturating_sub(header_h);
        let visible_rows = state.order.len().min(body_h as usize);

        state.offset_y = keep_visible(state.offset_y, state.cursor_row, visible_rows);

        // Compute column widths
        let total_w = content.width;
        let mut col_widths = vec![0u16; self.columns.len()];
        let mut fill_indices = vec![];
        let mut used = 0u16;

        for (i, col) in self.columns.iter().enumerate() {
            let w = state
                .width_overrides
                .get(&i)
                .copied()
                .unwrap_or_else(|| match col.width {
                    Constraint::Length(l) => l,
                    Constraint::Min(m) => m,
                    Constraint::Max(m) => m.min(total_w.saturating_sub(used)),
                    Constraint::Percentage(p) => (total_w as f32 * p as f32 / 100.0) as u16,
                    Constraint::Fill(_) => {
                        fill_indices.push(i);
                        0
                    }
                    _ => 10,
                });
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

        state.col_widths = col_widths.clone();

        // Allocate hits
        state.hits.clear();
        state.hits.resize(1 + visible_rows, HitBox::default());

        // Header
        if self.header {
            let mut x = content.x;
            let y = content.y;
            for (i, (col, &w)) in self.columns.iter().zip(&col_widths).enumerate() {
                if x >= content.x + content.width {
                    break;
                }
                let cell_w = w.min(content.x + content.width - x);
                let area = Rect {
                    x,
                    y,
                    width: cell_w,
                    height: 1,
                };
                state.hits[0].set_area(area);
                if i + 1 < state.hits.len() {
                    state.hits[i + 1].set_area(area);
                }

                let hover = state.hits[0].hover;
                let bg = if hover { th.hover_bg } else { th.panel };
                fill(buf, area, bg);

                let mut title = col.title.clone();
                if let Some((sort_col, asc)) = state.sort
                    && sort_col == i
                {
                    title.push_str(if asc { " ▲" } else { " ▼" });
                }
                let title_s = st(th.text, bg).add_modifier(ratatui_core::style::Modifier::BOLD);
                put(
                    buf,
                    x,
                    y,
                    &truncate(&title, cell_w as usize),
                    cell_w,
                    title_s,
                );
                x += cell_w + self.column_gap;
            }
        }

        // Body
        let body_y = content.y + header_h;
        for row_offset in 0..visible_rows {
            let vis_idx = state.offset_y + row_offset;
            if vis_idx >= state.order.len() {
                break;
            }
            let row_idx = state.order[vis_idx];
            let row = &self.rows[row_idx];

            let y = body_y + row_offset as u16;
            let is_cursor = self.cursor == TableCursor::Row && vis_idx == state.cursor_row;
            let is_selected = self.multi_select && state.selected.contains(&row_idx);
            let is_hover = state.hover_row == Some(row_offset);

            let bg = if is_cursor && self.focused {
                th.cursor_bg
            } else if is_cursor {
                th.cursor_blurred_bg
            } else if is_hover {
                th.hover_bg
            } else if self.zebra && row_idx % 2 == 1 {
                th.boost
            } else {
                th.surface
            };

            // block cursor uses the contrast text; otherwise row variant tints the whole row
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

            if is_selected {
                put(buf, content.x, y, "┃", 1, st(th.accent, bg));
            }

            let mut x = content.x;
            for (col_i, (col, &w)) in self.columns.iter().zip(&col_widths).enumerate() {
                if x >= content.x + content.width {
                    break;
                }
                let cell_w = w.min(content.x + content.width - x);
                let cell = row.cells.get(col_i).cloned().unwrap_or_default();

                let cell_bg = bg;
                // precedence: cursor row (readability) > cell_style callback > TableCell::style > row/default
                let mut cell_style = Style::new().fg(fg.color()).bg(cell_bg.color());
                if !(is_cursor && self.focused) {
                    if let Some(s) = cell.style {
                        cell_style = cell_style.patch(Style { bg: None, ..s });
                    }
                    if let Some(f) = self.cell_style
                        && let Some(s) = f(row_idx, col_i, &cell, &th)
                    {
                        cell_style = cell_style.patch(Style { bg: None, ..s });
                    }
                } else if let Some(s) = cell.style {
                    // keep bold/italic on the cursor row, drop colours
                    cell_style = cell_style.add_modifier(s.add_modifier);
                }

                let is_cell_cursor = self.cursor == TableCursor::Cell
                    && vis_idx == state.cursor_row
                    && col_i == state.cursor_col;
                let final_style = if is_cell_cursor {
                    cell_style.add_modifier(ratatui_core::style::Modifier::REVERSED)
                } else {
                    cell_style
                };

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
                    final_style,
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
                .offset(state.offset_y)
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

// key-value list

/// Simple two-column list for definition-style content (label: value).
pub struct KeyValueList {
    items: Vec<(String, String)>,
    gap: u16,
    theme: Option<Theme>,
}

impl KeyValueList {
    pub fn new(items: Vec<(String, String)>) -> Self {
        Self {
            items,
            gap: 2,
            theme: None,
        }
    }
    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

impl ratatui_core::widgets::Widget for KeyValueList {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        if area.width < 2 || area.height == 0 {
            return;
        }
        let max_label_w = self.items.iter().map(|(l, _)| l.len()).max().unwrap_or(0) as u16;
        let label_w = max_label_w.min(area.width / 3);

        for (i, (label, value)) in self.items.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            put(
                buf,
                area.x,
                y,
                &truncate(label, label_w as usize),
                label_w,
                st(th.text_muted, th.surface).add_modifier(ratatui_core::style::Modifier::BOLD),
            );
            let val_x = area.x + label_w + self.gap;
            let val_w = area.width.saturating_sub(label_w + self.gap);
            put(
                buf,
                val_x,
                y,
                &truncate(value, val_w as usize),
                val_w,
                st(th.text, th.surface),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_numeric() {
        let cols = vec![TableColumn::new("Val").sortable(true)];
        let rows = vec![
            TableRow::from(vec![TableCell::from(3.5)]),
            TableRow::from(vec![TableCell::from(1.2)]),
            TableRow::from(vec![TableCell::from(10.0)]),
        ];
        let mut state = DataTableState::new();
        state.sort = Some((0, true));
        state.rebuild_order(&rows, &cols);
        assert_eq!(state.order, vec![1, 0, 2]);
        state.sort = Some((0, false));
        state.rebuild_order(&rows, &cols);
        assert_eq!(state.order, vec![2, 0, 1]);
    }

    #[test]
    fn filter_preserves_cursor() {
        let cols = vec![TableColumn::new("Name")];
        let rows = vec![
            TableRow::from(vec!["apple"]),
            TableRow::from(vec!["banana"]),
            TableRow::from(vec!["apricot"]),
        ];
        let mut state = DataTableState::new();
        state.rebuild_order(&rows, &cols);
        state.cursor_row = 1;
        state.set_filter("ap");
        state.rebuild_order(&rows, &cols);
        assert_eq!(state.visible_len(), 2);
        assert!(state.cursor_row < state.visible_len());
    }

    #[test]
    fn keep_visible_scrolling() {
        let offset = keep_visible(0, 5, 10);
        assert_eq!(offset, 0);
        let offset = keep_visible(0, 12, 10);
        assert_eq!(offset, 3);
        let offset = keep_visible(10, 5, 10);
        assert_eq!(offset, 5);
    }

    /// A list taller than its area shows the rows that fit. It used to draw nothing at all, which
    /// reads on screen as "there is no data" rather than "there is more data".
    #[test]
    fn key_value_list_clips_instead_of_vanishing() {
        use ratatui_core::widgets::Widget;
        let items: Vec<(String, String)> = (0..11)
            .map(|i| (format!("k{i}"), format!("v{i}")))
            .collect();
        let area = Rect::new(0, 0, 20, 6);
        let mut buf = Buffer::empty(area);
        KeyValueList::new(items).render(area, &mut buf);
        let first = buf.content().iter().map(|c| c.symbol()).collect::<String>();
        assert!(first.contains("k0"), "the first row must be drawn");
        assert!(first.contains("k5"), "every row that fits must be drawn");
        assert!(
            !first.contains("k6"),
            "a row past the area must not be drawn"
        );
    }

    /// Degenerate areas draw nothing rather than panicking, as the widget contract requires.
    #[test]
    fn key_value_list_survives_a_tiny_area() {
        use ratatui_core::widgets::Widget;
        for (w, h) in [(0, 0), (1, 1), (2, 1), (20, 0)] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(Rect::new(0, 0, w.max(1), h.max(1)));
            KeyValueList::new(vec![("a".into(), "b".into())]).render(area, &mut buf);
        }
    }
}
