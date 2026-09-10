//! Command palette with fuzzy search: Textual/VS Code style picker.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 20);
//! # let mut buf = Buffer::empty(area);
//! # let items = vec![];
//! # let mut palette = CommandPaletteState::default();
//! palette.set_items(&items);
//! CommandPalette::new().render(area, &mut buf, &mut palette);
//! if let Some(idx) = palette.take_selected() { /* run items[idx] */ }
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;

use crate::core::{HitBox, Hit, Outcome, Interactive, is_press, wheel_delta};
use crate::draw::{fill, put, put_right, Border, st, bold, blend_area};
use crate::fuzzy;
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

/// A single palette entry.
#[derive(Clone, Debug)]
pub struct PaletteItem {
    pub title: String,
    pub hint: Option<String>,
    pub group: Option<String>,
    pub shortcut: Option<String>,
    pub icon: Option<String>,
}

impl PaletteItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), hint: None, group: None, shortcut: None, icon: None }
    }

    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }

    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(i.into());
        self
    }
}

/// State for the command palette: query, results, selection.
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct CommandPaletteState {
    pub open: bool,
    items: Vec<PaletteItem>,
    query: String,
    cursor: usize,
    highlight: usize,
    scroll: usize,
    results: Vec<(usize, i32, Vec<usize>)>,
    backdrop_hit: HitBox,
    input_hit: HitBox,
    row_hits: Vec<HitBox>,
    scrollbar_state: ScrollbarState,
    pub selected: Option<usize>,
    last_query: String,
}


impl CommandPaletteState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.cursor = 0;
        self.highlight = 0;
        self.scroll = 0;
        self.selected = None;
        self.recompute_results();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn set_items(&mut self, items: &[PaletteItem]) {
        self.items = items.to_vec();
        let titles: Vec<&str> = self.items.iter().map(|item| item.title.as_str()).collect();
        self.results = fuzzy::rank(&self.query, titles);
        self.highlight = 0;
        self.scroll = 0;
    }

    pub fn take_selected(&mut self) -> Option<usize> {
        self.selected.take()
    }

    pub fn items_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn recompute_results(&mut self) {
        if self.query != self.last_query {
            let titles: Vec<&str> = self.items.iter().map(|item| item.title.as_str()).collect();
            self.results = fuzzy::rank(&self.query, titles);
            self.last_query = self.query.clone();
            self.highlight = 0;
            self.scroll = 0;
        }
    }

    fn move_up(&mut self) {
        if self.highlight > 0 {
            self.highlight -= 1;
        }
    }

    fn move_down(&mut self) {
        if !self.results.is_empty() && self.highlight < self.results.len() - 1 {
            self.highlight += 1;
        }
    }

    fn select(&mut self) {
        if !self.results.is_empty() && self.highlight < self.results.len() {
            let item_idx = self.results[self.highlight].0;
            self.selected = Some(item_idx);
            self.open = false;
        }
    }
}
impl Interactive for CommandPaletteState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        match k.code {
            KeyCode::Esc => {
                self.close();
                Outcome::Changed
            }
            KeyCode::Enter => {
                self.select();
                Outcome::Changed
            }
            KeyCode::Up => {
                self.move_up();
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.move_down();
                Outcome::Consumed
            }
            KeyCode::PageUp => {
                for _ in 0..5 {
                    self.move_up();
                }
                Outcome::Consumed
            }
            KeyCode::PageDown => {
                for _ in 0..5 {
                    self.move_down();
                }
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.cursor = 0;
                Outcome::Consumed
            }
            KeyCode::End => {
                self.cursor = self.query.len();
                Outcome::Consumed
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                Outcome::Consumed
            }
            KeyCode::Right => {
                if self.cursor < self.query.len() {
                    self.cursor += 1;
                }
                Outcome::Consumed
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.query.remove(self.cursor);
                    self.recompute_results();
                }
                Outcome::Consumed
            }
            KeyCode::Char(c) => {
                self.query.insert(self.cursor, c);
                self.cursor += 1;
                self.recompute_results();
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let backdrop_hit = self.backdrop_hit.mouse(&m);
        if matches!(backdrop_hit, Hit::Press) {
            self.close();
            return Outcome::Changed;
        }

        for (i, hit) in self.row_hits.iter_mut().enumerate() {
            let h = hit.mouse(&m);
            if hit.hover {
                self.highlight = self.scroll + i;
            }
            if matches!(h, Hit::Press) {
                self.highlight = self.scroll + i;
                self.select();
                return Outcome::Changed;
            }
        }

        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                self.move_down();
            } else {
                self.move_up();
            }
            return Outcome::Consumed;
        }

        let sb_out = self.scrollbar_state.handle_mouse(m);
        if sb_out.is_changed() || sb_out.is_consumed() {
            self.scroll = self.scrollbar_state.offset;
            self.highlight = keep_visible(self.scroll, self.highlight, 8);
            return sb_out;
        }

        Outcome::Ignored
    }
}


/// Command palette overlay widget.
pub struct CommandPalette {
    theme: Option<Theme>,
    now: Option<Instant>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self { theme: None, now: None }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for CommandPalette {
    type State = CommandPaletteState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.open || area.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);

        // Backdrop
        blend_area(buf, area, th.background, 0.5);
        state.backdrop_hit.set_area(area);

        let w = area.width.saturating_sub(4).clamp(40, (area.width * 9 / 10).min(100));
        let x = area.x + (area.width - w) / 2;
        let y = area.y + 2;

        // Input box
        let input_area = Rect { x, y, width: w, height: 3 };
        let input_bg = th.surface.blend(th.foreground, 0.05);
        fill(buf, input_area, input_bg);
        Border::Tall.draw(buf, input_area, th.border, input_bg);

        let prompt_x = x + 1;
        put(buf, prompt_x, y + 1, ">", 1, bold(st(th.primary, input_bg)));

        let text_x = prompt_x + 2;
        let text_w = w.saturating_sub(4);
        let display_text = if state.query.is_empty() { "Search for commands…" } else { &state.query };
        let text_style = if state.query.is_empty() {
            st(th.text_muted, input_bg)
        } else {
            st(th.text, input_bg)
        };
        put(buf, text_x, y + 1, display_text, text_w, text_style);

        // Cursor
        if !state.query.is_empty() {
            let cursor_x = text_x + state.cursor as u16;
            if cursor_x < input_area.right().saturating_sub(1) {
                let cursor_char = if state.cursor < state.query.len() {
                    state.query.chars().nth(state.cursor).unwrap_or(' ')
                } else {
                    ' '
                };
                put(buf, cursor_x, y + 1, &cursor_char.to_string(), 1, st(th.cursor_fg, th.cursor_bg));
            }
        }

        state.input_hit.set_area(input_area);

        // Results list
        let max_rows = 8;
        let shown = state.results.len().min(max_rows);
        if shown == 0 {
            let empty_area = Rect { x, y: y + 4, width: w, height: 3 };
            fill(buf, empty_area, th.surface);
            Border::Tall.draw(buf, empty_area, th.border, th.surface);
            put(buf, x + 2, y + 5, "No matches", w.saturating_sub(4), st(th.text_muted, th.surface));
            return;
        }

        // Calculate list height accounting for variable row heights
        let mut total_h = 0u16;
        let mut last_group: Option<String> = None;
        for (i, &(item_idx, _, _)) in state.results.iter().enumerate().take(shown) {
            let item = &state.items[item_idx];
            if item.group.is_some() && item.group != last_group {
                if i > 0 { total_h += 1; } // group gap
                total_h += 1; // group header line
                last_group = item.group.clone();
            }
            total_h += if item.hint.is_some() { 2 } else { 1 };
        }

        let list_h = total_h + 3; // +2 for border, +1 for footer
        let list_area = Rect { x, y: y + 4, width: w, height: list_h };
        fill(buf, list_area, th.surface);
        Border::Tall.draw(buf, list_area, th.border, th.surface);

        state.scroll = keep_visible(state.scroll, state.highlight, shown);
        let start = state.scroll;

        state.row_hits.clear();
        last_group = None;
        let mut row_y = list_area.y + 1;

        for (row, &(item_idx, _score, ref positions)) in state.results.iter().enumerate().skip(start).take(shown) {
            let item = &state.items[item_idx];

            // Group header
            if item.group.is_some() && item.group != last_group {
                if row > start {
                    row_y += 1; // gap before new group
                }
                if let Some(g) = &item.group {
                    put(buf, x + 2, row_y, g, w.saturating_sub(4), st(th.text_muted, th.surface).add_modifier(Modifier::DIM));
                    row_y += 1;
                }
                last_group = item.group.clone();
            }

            let per = if item.hint.is_some() { 2 } else { 1 };
            if row_y + per > list_area.bottom().saturating_sub(2) { // leave room for footer
                break;
            }

            let row_area = Rect { x: x + 1, y: row_y, width: w - 2, height: per };
            let selected = row == state.highlight;
            let (fg, bg) = if selected {
                (th.cursor_fg, th.cursor_bg)
            } else {
                (th.foreground, th.surface)
            };

            fill(buf, row_area, bg);

            // Icon + Title with matched chars highlighted
            let mut title_x = x + 3;
            if let Some(icon_str) = &item.icon {
                put(buf, title_x, row_y, icon_str, 2, st(fg, bg));
                title_x += 3;
            }

            let mut spans = Vec::new();
            for (ci, ch) in item.title.chars().enumerate() {
                let mut style = st(fg, bg);
                if positions.contains(&ci) {
                    style = bold(st(if selected { th.accent.lighten(0.3) } else { th.accent }, bg)).add_modifier(Modifier::UNDERLINED);
                }
                spans.push(Span::styled(ch.to_string(), style));
            }
            let title_w = w.saturating_sub(title_x - x).saturating_sub(3);
            buf.set_line(title_x, row_y, &Line::from(spans), title_w);

            // Hint on second line (if present)
            if let Some(hint) = &item.hint {
                let hint_fg = if selected { fg.blend(bg, 0.3) } else { th.text_muted };
                put(buf, x + 3, row_y + 1, hint, w.saturating_sub(6), st(hint_fg, bg));
            }

            // Shortcut on right
            if let Some(sc) = &item.shortcut {
                put_right(buf, Rect { x: x + 3, y: row_y, width: w.saturating_sub(6), height: 1 }, sc, st(th.text_muted, bg));
            }

            let mut hit = HitBox::default();
            hit.set_area(row_area);
            state.row_hits.push(hit);

            row_y += per;
        }

        // Footer hint INSIDE the box
        let footer_y = list_area.bottom().saturating_sub(1);
        let hint = "↑↓ navigate • enter run • esc close";
        put(buf, x + (w.saturating_sub(hint.len() as u16)) / 2, footer_y, hint, w, st(th.text_muted, th.surface).add_modifier(Modifier::DIM));

        // Scrollbar
        if state.results.len() > max_rows {
            let sb_area = Rect { x: list_area.right().saturating_sub(1), y: list_area.y, width: 1, height: list_area.height };
            Scrollbar::vertical(state.results.len(), shown).offset(state.scroll).render(sb_area, buf, &mut state.scrollbar_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_fuzzy_search() {
        let mut state = CommandPaletteState::new();
        state.set_items(&[
            PaletteItem::new("Open File"),
            PaletteItem::new("Save File"),
            PaletteItem::new("Close Window"),
        ]);
        state.query = "sf".to_string();
        state.recompute_results();
        assert_eq!(state.results.len(), 1);
        assert_eq!(state.items[state.results[0].0].title, "Save File");
    }

    #[test]
    fn palette_navigation() {
        let mut state = CommandPaletteState::new();
        state.set_items(&[PaletteItem::new("A"), PaletteItem::new("B"), PaletteItem::new("C")]);
        state.recompute_results();
        assert_eq!(state.highlight, 0);
        state.move_down();
        assert_eq!(state.highlight, 1);
        state.move_down();
        assert_eq!(state.highlight, 2);
        state.move_down();
        assert_eq!(state.highlight, 2);
        state.move_up();
        assert_eq!(state.highlight, 1);
    }
}
