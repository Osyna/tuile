//! Vertical list with cursor, multi-select, fuzzy filtering, type-ahead, details, separators,
//! variants, scrollbar, and activation on Enter/double-click.
//!
//! ```
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 10);
//! # let mut buf = Buffer::empty(area);
//! let entries = vec![ListEntry::new("Item 1"), ListEntry::new("Item 2")];
//! let mut state = ListViewState::new();
//! ListView::new(entries).focused(true).render(area, &mut buf, &mut state);
//! ```

use std::collections::BTreeSet;
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::{
    Interactive, Look, Outcome, is_press, mouse_in, mouse_pos, plain_char, wheel_delta,
};
use crate::draw::{Border, fill, hline, put, put_right, st};
use crate::fuzzy;
use crate::layout::pad;
use crate::theme::{self, Theme, Variant};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

// entry

/// One list entry: label, optional detail/icon, disabled/separator flags, variant.
#[derive(Clone, Debug)]
pub struct ListEntry {
    pub label: String,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub disabled: bool,
    pub separator: bool,
    pub variant: Option<Variant>,
}

impl ListEntry {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            icon: None,
            disabled: false,
            separator: false,
            variant: None,
        }
    }

    pub fn detail(mut self, d: &str) -> Self {
        self.detail = Some(d.to_string());
        self
    }

    pub fn icon(mut self, i: &str) -> Self {
        self.icon = Some(i.to_string());
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn separator(mut self, s: bool) -> Self {
        self.separator = s;
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = Some(v);
        self
    }
}

impl From<&str> for ListEntry {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

// enums

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListDetail {
    #[default]
    Hidden,
    Right,
    Below,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListHighlight {
    #[default]
    Block,
    Bar,
    Underline,
}

// builder

/// Vertical list with entries, border, title, multi-select, details column, filter, and highlight style.
#[derive(Clone, Debug)]
pub struct ListView {
    entries: Vec<ListEntry>,
    multi_select: bool,
    details: ListDetail,
    border: Option<Border>,
    title: String,
    highlight: ListHighlight,
    filter: String,
    empty_text: String,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl ListView {
    pub fn new(entries: Vec<ListEntry>) -> Self {
        Self {
            entries,
            multi_select: false,
            details: ListDetail::default(),
            border: Some(Border::Round),
            title: String::new(),
            highlight: ListHighlight::default(),
            filter: String::new(),
            empty_text: "No items".to_string(),
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn multi_select(mut self, m: bool) -> Self {
        self.multi_select = m;
        self
    }

    pub fn details(mut self, d: ListDetail) -> Self {
        self.details = d;
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }

    pub fn highlight(mut self, h: ListHighlight) -> Self {
        self.highlight = h;
        self
    }

    pub fn filter(mut self, f: &str) -> Self {
        self.filter = f.to_string();
        self
    }

    pub fn empty_text(mut self, e: &str) -> Self {
        self.empty_text = e.to_string();
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

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

// state

/// List view state: cursor, selection set, scroll, hover, activation, type-ahead.
#[derive(Clone, Debug)]
pub struct ListViewState {
    pub cursor: usize,
    pub selected: BTreeSet<usize>,
    pub scroll: usize,
    pub hover: Option<usize>,
    pub hits: Rect,
    pub scrollbar: ScrollbarState,
    pub activated: Option<usize>,
    pub typeahead: String,
    pub typeahead_at: Option<Instant>,
    last_click: Option<(usize, Instant)>,
}

impl ListViewState {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            scroll: 0,
            selected: BTreeSet::new(),
            hover: None,
            hits: Rect::ZERO,
            scrollbar: ScrollbarState::default(),
            activated: None,
            typeahead: String::new(),
            typeahead_at: None,
            last_click: None,
        }
    }

    pub fn current(&self) -> usize {
        self.cursor
    }

    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }
}

impl Default for ListViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactive for ListViewState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    return Outcome::Changed;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = self.cursor.saturating_add(1);
                return Outcome::Changed;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if self.cursor != 0 {
                    self.cursor = 0;
                    return Outcome::Changed;
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.cursor = usize::MAX;
                return Outcome::Changed;
            }
            KeyCode::PageUp => {
                self.cursor = self.cursor.saturating_sub(10);
                return Outcome::Changed;
            }
            KeyCode::PageDown => {
                self.cursor = self.cursor.saturating_add(10);
                return Outcome::Changed;
            }
            KeyCode::Enter => {
                self.activated = Some(self.cursor);
                return Outcome::Changed;
            }
            KeyCode::Char(' ') => {
                // toggle in multi-select mode
                return Outcome::Changed;
            }
            KeyCode::Char(_c) => {
                if let Some(ch) = plain_char(&k) {
                    self.typeahead.push(ch);
                    self.typeahead_at = Some(Instant::now());
                    return Outcome::Changed;
                }
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        let mut out = Outcome::Ignored;

        // scrollbar
        out |= self.scrollbar.handle_mouse(m);
        self.scroll = self.scrollbar.offset;

        // hover
        if self.hits.contains(pos) {
            let row = (pos.y - self.hits.y) as usize + self.scroll;
            if self.hover != Some(row) {
                self.hover = Some(row);
                out = Outcome::Consumed;
            }

            // click
            if matches!(
                m.kind,
                ratatui::crossterm::event::MouseEventKind::Down(
                    ratatui::crossterm::event::MouseButton::Left
                )
            ) {
                self.cursor = row;
                let now = Instant::now();
                // double-click detection
                if let Some((last_row, last_time)) = self.last_click
                    && last_row == row
                    && now.duration_since(last_time).as_millis() < 400
                {
                    self.activated = Some(row);
                    self.last_click = None;
                    return Outcome::Changed;
                }
                self.last_click = Some((row, now));
                return Outcome::Changed;
            }
        } else {
            if self.hover.is_some() {
                self.hover = None;
                out = Outcome::Consumed;
            }
        }

        // wheel
        if let Some(delta) = wheel_delta(&m)
            && mouse_in(self.hits, &m)
        {
            if delta > 0 {
                self.scroll = self.scroll.saturating_add(1);
            } else {
                self.scroll = self.scroll.saturating_sub(1);
            }
            return Outcome::Consumed;
        }

        out
    }
}

// helpers

/// Returns indices of entries matching the filter (fuzzy).
pub fn visible_indices(entries: &[ListEntry], filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..entries.len()).collect();
    }
    fuzzy::rank(filter, entries.iter().map(|e| e.label.as_str()))
        .into_iter()
        .map(|(i, _, _)| i)
        .collect()
}

// render

impl StatefulWidget for ListView {
    type State = ListViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        fill(buf, area, bg);

        // border
        let mut inner = area;
        if let Some(border) = self.border {
            let border_color = if self.focused {
                th.border
            } else {
                th.border_blurred
            };
            if !self.title.is_empty() {
                border.draw_titled(buf, area, border_color, bg, &self.title, Alignment::Left);
            } else {
                border.draw(buf, area, border_color, bg);
            }
            inner = pad(area, 1, 1);
        }

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // compute visible entries
        let visible = visible_indices(&self.entries, &self.filter);
        if visible.is_empty() {
            put(
                buf,
                inner.x + 1,
                inner.y,
                &self.empty_text,
                inner.width.saturating_sub(2),
                st(th.text_muted, bg),
            );
            return;
        }

        // clamp cursor
        state.cursor = state.cursor.min(visible.len().saturating_sub(1));

        // skip disabled/separator rows
        let selectable: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|&i| !self.entries[i].disabled && !self.entries[i].separator)
            .collect();
        if !selectable.is_empty()
            && self
                .entries
                .get(visible[state.cursor])
                .is_some_and(|e| e.disabled || e.separator)
        {
            // move to next selectable
            if let Some(&next) = selectable.iter().find(|&&i| i >= state.cursor) {
                state.cursor = visible.iter().position(|&x| x == next).unwrap_or(0);
            } else {
                state.cursor = visible
                    .iter()
                    .position(|&x| x == selectable[0])
                    .unwrap_or(0);
            }
        }

        // type-ahead
        if !state.typeahead.is_empty()
            && let Some(at) = state.typeahead_at
        {
            if Instant::now().duration_since(at).as_secs() > 1 {
                state.typeahead.clear();
            } else {
                // find next entry starting with typeahead
                let lower = state.typeahead.to_lowercase();
                if let Some(idx) = visible
                    .iter()
                    .skip(state.cursor + 1)
                    .find(|&&i| self.entries[i].label.to_lowercase().starts_with(&lower))
                {
                    state.cursor = visible
                        .iter()
                        .position(|&x| x == *idx)
                        .unwrap_or(state.cursor);
                }
            }
        }

        // scrollbar area
        let sb_area = Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height);
        let list_area = Rect::new(
            inner.x,
            inner.y,
            inner.width.saturating_sub(1),
            inner.height,
        );

        // scroll to cursor
        state.scroll = keep_visible(state.scroll, state.cursor, list_area.height as usize);

        state.hits = list_area;

        // render rows
        for row_idx in 0..list_area.height as usize {
            let entry_idx = row_idx + state.scroll;
            if entry_idx >= visible.len() {
                break;
            }
            let i = visible[entry_idx];
            let entry = &self.entries[i];
            let y = list_area.y + row_idx as u16;

            let is_cursor = entry_idx == state.cursor;
            let is_hover = state.hover == Some(entry_idx);
            let is_selected = state.selected.contains(&i);

            let _look = Look {
                focused: self.focused && is_cursor,
                hover: is_hover,
                enabled: !entry.disabled,
            };

            let row_bg = if is_cursor && self.focused {
                th.cursor_bg
            } else if is_hover {
                th.hover_bg
            } else if is_selected {
                th.selection_bg
            } else if let Some(v) = entry.variant {
                th.variant(v).blend(bg, 0.15)
            } else {
                bg
            };

            let row_fg = if entry.disabled {
                th.text_disabled
            } else if is_cursor && self.focused {
                th.cursor_fg
            } else if let Some(v) = entry.variant {
                th.text_variant(v)
            } else {
                th.text
            };

            // highlight style
            let hl_area = match self.highlight {
                ListHighlight::Block => Rect::new(list_area.x, y, list_area.width, 1),
                ListHighlight::Bar => Rect::new(list_area.x, y, 2, 1),
                ListHighlight::Underline => Rect::new(list_area.x, y, list_area.width, 1),
            };

            if matches!(self.highlight, ListHighlight::Block | ListHighlight::Bar) {
                fill(buf, hl_area, row_bg);
            }

            if entry.separator {
                hline(
                    buf,
                    list_area.x,
                    y,
                    list_area.width,
                    "─",
                    st(th.text_muted.blend(bg, 0.5), bg),
                );
                continue;
            }

            // multi-select checkbox: a painted 3-cell button (same look as `Checkbox`)
            let mut x = list_area.x;
            if self.multi_select {
                let btn = if is_cursor && self.focused {
                    th.cursor_bg
                } else {
                    th.panel
                };
                let mark_fg = if is_selected { th.text_success } else { btn };
                put(buf, x, y, "   ", 3, st(btn, btn));
                put(
                    buf,
                    x + 1,
                    y,
                    if is_selected { "X" } else { " " },
                    1,
                    st(mark_fg, btn).add_modifier(Modifier::BOLD),
                );
                x += 4;
            }

            // icon
            if let Some(icon) = &entry.icon {
                put(buf, x, y, icon, 2, st(row_fg, row_bg));
                x += 2;
            }

            // label
            let label_w = list_area.right().saturating_sub(x);
            let mut style = st(row_fg, row_bg);
            if is_cursor && !entry.disabled {
                style = style.add_modifier(Modifier::BOLD);
            }

            // fuzzy highlight
            if !self.filter.is_empty() {
                if let Some((_, positions)) = fuzzy::fuzzy(&self.filter, &entry.label) {
                    let label_chars: Vec<char> = entry.label.chars().collect();
                    let mut cx = x;
                    for (ci, ch) in label_chars.iter().enumerate() {
                        let mut ch_style = style;
                        if positions.contains(&ci) {
                            ch_style = st(th.accent, row_bg).add_modifier(Modifier::BOLD);
                        }
                        let ch_str = ch.to_string();
                        put(buf, cx, y, &ch_str, 1, ch_style);
                        cx += ch_str.width() as u16;
                        if cx >= list_area.right() {
                            break;
                        }
                    }
                } else {
                    put(buf, x, y, &entry.label, label_w, style);
                }
            } else {
                put(buf, x, y, &entry.label, label_w, style);
            }

            // detail
            if let Some(detail) = &entry.detail {
                match self.details {
                    ListDetail::Right => {
                        put_right(
                            buf,
                            Rect::new(list_area.x, y, list_area.width, 1),
                            detail,
                            st(th.text_muted, row_bg),
                        );
                    }
                    ListDetail::Below => {
                        // would need two rows per entry
                    }
                    ListDetail::Hidden => {}
                }
            }

            // underline
            if matches!(self.highlight, ListHighlight::Underline) && is_cursor {
                for x in list_area.x..list_area.right() {
                    if buf.area.contains((x, y).into()) {
                        let cell = &mut buf[(x, y)];
                        cell.modifier.insert(Modifier::UNDERLINED);
                    }
                }
            }
        }

        // scrollbar
        Scrollbar::vertical(visible.len(), list_area.height as usize)
            .offset(state.scroll)
            .theme(&th)
            .render(sb_area, buf, &mut state.scrollbar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_visible_indices_filters() {
        let entries = vec![
            ListEntry::new("apple"),
            ListEntry::new("banana"),
            ListEntry::new("apricot"),
        ];
        let vis = visible_indices(&entries, "ap");
        assert_eq!(vis.len(), 2);
        assert!(vis.contains(&0));
        assert!(vis.contains(&2));
    }

    #[test]
    fn list_cursor_wraps_on_keys() {
        let mut state = ListViewState::new();
        assert_eq!(
            state.handle_key(KeyEvent::new(
                KeyCode::Down,
                ratatui::crossterm::event::KeyModifiers::NONE
            )),
            Outcome::Changed
        );
        assert_eq!(state.cursor, 1);
        assert_eq!(
            state.handle_key(KeyEvent::new(
                KeyCode::Up,
                ratatui::crossterm::event::KeyModifiers::NONE
            )),
            Outcome::Changed
        );
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn list_activates_on_enter() {
        let mut state = ListViewState::new();
        state.cursor = 5;
        assert_eq!(
            state.handle_key(KeyEvent::new(
                KeyCode::Enter,
                ratatui::crossterm::event::KeyModifiers::NONE
            )),
            Outcome::Changed
        );
        assert_eq!(state.take_activated(), Some(5));
    }
}
