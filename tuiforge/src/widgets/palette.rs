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

use crate::core::{HitBox, Hit, Outcome, Interactive, is_press, is_left_down, mouse_pos, wheel_delta};
use crate::draw::{fill, put, hline, truncate, Border, st, bold, blend_area};
use crate::fuzzy;
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState};
use unicode_width::UnicodeWidthStr;

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
    /// Frame rect from the last render; presses outside it close the palette.
    panel: Rect,
    /// Result rows that fit in the last render (variable row heights).
    visible: usize,
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

    fn scroll_by(&mut self, delta: i32) {
        // ponytail: `visible` is the last frame's count, close enough with mixed row heights
        let max = self.results.len().saturating_sub(self.visible.max(1));
        self.scroll = (self.scroll as i64 + delta as i64).clamp(0, max as i64) as usize;
        let last = (self.scroll + self.visible.max(1)).saturating_sub(1);
        self.highlight = self.highlight.clamp(self.scroll, last.max(self.scroll));
    }

    fn select(&mut self) {
        if !self.results.is_empty() && self.highlight < self.results.len() {
            let item_idx = self.results[self.highlight].0;
            self.selected = Some(item_idx);
            self.open = false;
        }
    }

    /// Byte offset of char index `ci` in the query (cursor is a char index).
    fn byte_at(&self, ci: usize) -> usize {
        self.query.char_indices().nth(ci).map_or(self.query.len(), |(b, _)| b)
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
                self.cursor = self.query.chars().count();
                Outcome::Consumed
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.query.chars().count());
                Outcome::Consumed
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    let b = self.byte_at(self.cursor);
                    self.query.remove(b);
                    self.recompute_results();
                }
                Outcome::Consumed
            }
            KeyCode::Char(c) => {
                let b = self.byte_at(self.cursor);
                self.query.insert(b, c);
                self.cursor += 1;
                self.recompute_results();
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        // rows first: a press on a result must select it, not fall through to the backdrop
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
            self.scroll_by(delta);
            return Outcome::Consumed;
        }

        let sb_out = self.scrollbar_state.handle_mouse(m);
        if sb_out.is_changed() || sb_out.is_consumed() {
            self.scroll_by(self.scrollbar_state.offset as i32 - self.scroll as i32);
            return sb_out;
        }

        if is_left_down(&m) && !self.panel.contains(mouse_pos(&m)) {
            self.close();
            return Outcome::Changed;
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

        let lo = area.width.min(40);
        let hi = (area.width * 9 / 10).min(100).max(lo);
        let w = area.width.saturating_sub(4).clamp(lo, hi);
        let x = area.x + (area.width - w) / 2;
        let y = area.y + 1;

        // Frame rows: border, input, separator, body…, footer, border.
        const CHROME: u16 = 5;
        let avail = area.height.saturating_sub(2).min(area.height * 4 / 5); // popup, not a full-screen list
        if avail <= CHROME || w < 8 {
            return;
        }
        let max_body = avail - CHROME;

        // Rows that fit from `start`, honouring group headers and two-line entries.
        let fit = |start: usize| -> usize {
            let mut used = 0u16;
            let mut shown = 0;
            let mut last_group: Option<&str> = None;
            for (i, &(item_idx, _, _)) in state.results.iter().enumerate().skip(start) {
                let item = &state.items[item_idx];
                let mut per = if item.hint.is_some() { 2 } else { 1 };
                if item.group.is_some() && item.group.as_deref() != last_group {
                    per += 1 + u16::from(i > start);
                    last_group = item.group.as_deref();
                }
                if used + per > max_body {
                    break;
                }
                used += per;
                shown += 1;
            }
            shown
        };

        // Keep the highlight inside the window.
        if state.highlight < state.scroll {
            state.scroll = state.highlight;
        }
        let mut shown = fit(state.scroll);
        while shown > 0 && state.highlight >= state.scroll + shown && state.scroll < state.highlight {
            state.scroll += 1;
            shown = fit(state.scroll);
        }
        state.visible = shown;
        let start = state.scroll;

        // Body height for the rows we will draw.
        let mut body_h = 0u16;
        let mut last_group: Option<&str> = None;
        for (i, &(item_idx, _, _)) in state.results.iter().enumerate().skip(start).take(shown) {
            let item = &state.items[item_idx];
            if item.group.is_some() && item.group.as_deref() != last_group {
                body_h += 1 + u16::from(i > start);
                last_group = item.group.as_deref();
            }
            body_h += if item.hint.is_some() { 2 } else { 1 };
        }
        let body_h = body_h.max(1);

        let panel = Rect { x, y, width: w, height: body_h + CHROME };
        state.panel = panel;
        fill(buf, panel, th.surface);
        Border::Tall.draw(buf, panel, th.border, th.surface);
        let inner_x = x + 1;
        let inner_w = w - 2;

        // Input strip
        let input_bg = th.surface.blend(th.foreground, 0.05);
        fill(buf, Rect { x: inner_x, y: y + 1, width: inner_w, height: 1 }, input_bg);
        put(buf, inner_x + 1, y + 1, ">", 1, bold(st(th.primary, input_bg)));
        let text_x = inner_x + 3;
        let text_w = inner_w.saturating_sub(4);
        if state.query.is_empty() {
            put(buf, text_x, y + 1, "Search for commands…", text_w, st(th.text_muted, input_bg));
        } else {
            put(buf, text_x, y + 1, &state.query, text_w, st(th.text, input_bg));
        }
        let cursor_x = text_x + state.query[..state.byte_at(state.cursor)].width() as u16;
        if cursor_x < text_x + text_w {
            let under = state.query.chars().nth(state.cursor).map_or(" ".to_string(), |c| c.to_string());
            put(buf, cursor_x, y + 1, &under, 1, st(th.cursor_fg, th.cursor_bg));
        }
        hline(buf, inner_x, y + 2, inner_w, "─", st(th.border_blurred, th.surface));

        // Footer
        let footer_y = panel.bottom() - 2;
        let hint = "↑↓ navigate • enter run • esc close";
        let hint = truncate(hint, inner_w as usize);
        put(buf, inner_x + (inner_w.saturating_sub(hint.width() as u16)) / 2, footer_y, &hint, inner_w, st(th.text_muted, th.surface).add_modifier(Modifier::DIM));

        // Body
        let body_y = y + 3;
        state.row_hits.clear();
        if shown == 0 {
            put(buf, inner_x + 2, body_y, "No matches", inner_w.saturating_sub(2), st(th.text_muted, th.surface));
            return;
        }
        let has_sb = state.results.len() > shown;
        let row_w = inner_w - u16::from(has_sb);

        let mut row_y = body_y;
        let mut last_group: Option<&str> = None;
        for (row, &(item_idx, _score, ref positions)) in state.results.iter().enumerate().skip(start).take(shown) {
            let item = &state.items[item_idx];

            if item.group.is_some() && item.group.as_deref() != last_group {
                if row > start {
                    row_y += 1;
                }
                if let Some(g) = &item.group {
                    put(buf, inner_x + 2, row_y, g, row_w.saturating_sub(2), st(th.text_muted, th.surface).add_modifier(Modifier::DIM));
                    row_y += 1;
                }
                last_group = item.group.as_deref();
            }

            let per = if item.hint.is_some() { 2 } else { 1 };
            let row_area = Rect { x: inner_x, y: row_y, width: row_w, height: per };
            let selected = row == state.highlight;
            let (fg, bg) = if selected { (th.cursor_fg, th.cursor_bg) } else { (th.foreground, th.surface) };
            let muted = if selected { fg.blend(bg, 0.3) } else { th.text_muted };
            fill(buf, row_area, bg);

            let mut title_x = inner_x + 2;
            if let Some(icon_str) = &item.icon {
                put(buf, title_x, row_y, icon_str, 2, st(fg, bg));
                title_x += 3;
            }

            // Shortcut on the right; title truncates before it.
            let right = row_area.right().saturating_sub(2);
            let mut title_end = right;
            if let Some(sc) = &item.shortcut {
                let sc_w = sc.width() as u16;
                put(buf, right.saturating_sub(sc_w), row_y, sc, sc_w, st(muted, bg));
                title_end = right.saturating_sub(sc_w + 1);
            }
            let title = truncate(&item.title, title_end.saturating_sub(title_x) as usize);
            let spans: Vec<Span> = title
                .chars()
                .enumerate()
                .map(|(ci, ch)| {
                    let style = if positions.contains(&ci) {
                        bold(st(if selected { th.accent.lighten(0.3) } else { th.accent }, bg)).add_modifier(Modifier::UNDERLINED)
                    } else {
                        st(fg, bg)
                    };
                    Span::styled(ch.to_string(), style)
                })
                .collect();
            buf.set_line(title_x, row_y, &Line::from(spans), title_end.saturating_sub(title_x));

            if let Some(h) = &item.hint {
                let hw = right.saturating_sub(title_x);
                put(buf, title_x, row_y + 1, &truncate(h, hw as usize), hw, st(muted, bg));
            }

            let mut hit = HitBox::default();
            hit.set_area(row_area);
            state.row_hits.push(hit);
            row_y += per;
        }

        if has_sb {
            let sb_area = Rect { x: panel.right() - 2, y: body_y, width: 1, height: body_h };
            Scrollbar::vertical(state.results.len(), shown).offset(state.scroll).theme(&th).render(sb_area, buf, &mut state.scrollbar_state);
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

    #[test]
    fn palette_row_click_selects_item() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
        let mut state = CommandPaletteState::new();
        state.set_items(&[PaletteItem::new("Alpha"), PaletteItem::new("Beta")]);
        state.open();
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        CommandPalette::new().render(buf.area, &mut buf, &mut state);
        let row = state.row_hits[1].area;
        let press = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: row.x + 2, row: row.y, modifiers: KeyModifiers::NONE };
        assert_eq!(state.handle_mouse(press), Outcome::Changed);
        assert_eq!(state.take_selected(), Some(1));
        // a press on the dimmed backdrop closes without selecting
        state.open();
        CommandPalette::new().render(buf.area, &mut buf, &mut state);
        let outside = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 0, row: 19, modifiers: KeyModifiers::NONE };
        state.handle_mouse(outside);
        assert!(!state.open);
        assert_eq!(state.take_selected(), None);
    }
}
