//! Select dropdown, combobox (editable filterable select), and multi-select with checkboxes.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 5);
//! # let mut buf = Buffer::empty(area);
//! let mut state = SelectState::new(&["Red", "Green", "Blue"]);
//! Select::new().placeholder("Pick a color").render(area, &mut buf, &mut state);
//! if state.selected().is_some() { /* ... */ }
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::*;
use crate::draw::{Border, fill, put, st};
use crate::fuzzy;
use crate::layout::{pad, popup_below};
use crate::theme::{self, Theme};
use crate::widgets::input::{Input, InputState};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

// ───────────────────────────── types ─────────────────────────────

#[derive(Clone, Debug)]
pub struct SelectOption {
    pub label: String,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(label: &str) -> Self {
        Self { label: label.to_string(), disabled: false }
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}

/// Builder for a select dropdown.
#[derive(Clone, Debug)]
pub struct Select {
    placeholder: String,
    max_visible: usize,
    allow_blank: bool,
    compact: bool,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

/// State for a select dropdown.
#[derive(Clone, Debug)]
pub struct SelectState {
    pub options: Vec<SelectOption>,
    pub selected: Option<usize>,
    pub open: bool,
    pub highlight: usize,
    pub scroll: usize,
    pub hit: HitBox,
    pub scrollbar_state: ScrollbarState,
    typeahead: String,
    typeahead_reset: Instant,
    option_hits: Vec<HitBox>,
    pub dropdown_area: Rect,
}

/// Builder for an editable combobox (filterable select).
#[derive(Clone, Debug)]
pub struct Combobox {
    placeholder: String,
    max_visible: usize,
    compact: bool,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

/// State for a combobox.
#[derive(Clone, Debug)]
pub struct ComboboxState {
    pub input: InputState,
    pub options: Vec<SelectOption>,
    pub selected: Option<usize>,
    pub open: bool,
    pub highlight: usize,
    pub scroll: usize,
    pub hit: HitBox,
    pub scrollbar_state: ScrollbarState,
    filtered: Vec<(usize, i32, Vec<usize>)>,
    option_hits: Vec<HitBox>,
    pub dropdown_area: Rect,
}

/// Builder for a multi-select with checkboxes.
#[derive(Clone, Debug)]
pub struct MultiSelect {
    placeholder: String,
    max_visible: usize,
    compact: bool,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

/// State for a multi-select.
#[derive(Clone, Debug)]
pub struct MultiSelectState {
    pub options: Vec<SelectOption>,
    pub selected: Vec<bool>,
    pub open: bool,
    pub highlight: usize,
    pub scroll: usize,
    pub hit: HitBox,
    pub scrollbar_state: ScrollbarState,
    option_hits: Vec<HitBox>,
    pub dropdown_area: Rect,
}

// ───────────────────────────── select builder ─────────────────────────────

impl Select {
    pub fn new() -> Self {
        Self {
            placeholder: "Select".to_string(),
            max_visible: 8,
            allow_blank: false,
            compact: false,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = s.to_string();
        self
    }

    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n;
        self
    }

    pub fn allow_blank(mut self, v: bool) -> Self {
        self.allow_blank = v;
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn now(mut self, t: Instant) -> Self {
        self.now = Some(t);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────────── select state ─────────────────────────────

impl SelectState {
    pub fn new(options: &[&str]) -> Self {
        Self {
            options: options.iter().map(|s| SelectOption::new(s)).collect(),
            selected: None,
            open: false,
            highlight: 0,
            scroll: 0,
            hit: HitBox::default(),
            scrollbar_state: ScrollbarState::default(),
            typeahead: String::new(),
            typeahead_reset: Instant::now(),
            option_hits: Vec::new(),
            dropdown_area: Rect::default(),
        }
    }

    pub fn with_options(options: Vec<SelectOption>) -> Self {
        Self {
            options,
            selected: None,
            open: false,
            highlight: 0,
            scroll: 0,
            hit: HitBox::default(),
            scrollbar_state: ScrollbarState::default(),
            typeahead: String::new(),
            typeahead_reset: Instant::now(),
            option_hits: Vec::new(),
            dropdown_area: Rect::default(),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_label(&self) -> Option<&str> {
        self.selected.and_then(|i| self.options.get(i).map(|o| o.label.as_str()))
    }

    pub fn set_selected(&mut self, idx: Option<usize>) {
        if idx.is_none() || idx.map(|i| i < self.options.len()).unwrap_or(false) {
            self.selected = idx;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn typeahead_match(&mut self, now: Instant) {
        if now.duration_since(self.typeahead_reset).as_millis() > 800 {
            self.typeahead.clear();
        }
        self.typeahead_reset = now;

        if self.typeahead.is_empty() {
            return;
        }

        let query = self.typeahead.to_lowercase();
        for (i, opt) in self.options.iter().enumerate().skip(self.highlight + 1) {
            if opt.label.to_lowercase().starts_with(&query) {
                self.highlight = i;
                return;
            }
        }
        for (i, opt) in self.options.iter().enumerate().take(self.highlight + 1) {
            if opt.label.to_lowercase().starts_with(&query) {
                self.highlight = i;
                return;
            }
        }
    }
}

impl Interactive for SelectState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        if !self.open {
            if is_activate(&k) || k.code == KeyCode::Down {
                self.open = true;
                self.highlight = self.selected.unwrap_or(0);
                return Outcome::Consumed;
            }
            return Outcome::Ignored;
        }

        // Open state
        match k.code {
            KeyCode::Esc => {
                self.open = false;
                return Outcome::Consumed;
            }
            KeyCode::Enter => {
                if !self.options.is_empty() && !self.options[self.highlight].disabled {
                    self.selected = Some(self.highlight);
                    self.open = false;
                    return Outcome::Changed;
                }
                return Outcome::Consumed;
            }
            KeyCode::Up => {
                if self.highlight > 0 {
                    self.highlight -= 1;
                    while self.highlight > 0 && self.options[self.highlight].disabled {
                        self.highlight -= 1;
                    }
                }
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                if self.highlight + 1 < self.options.len() {
                    self.highlight += 1;
                    while self.highlight + 1 < self.options.len() && self.options[self.highlight].disabled {
                        self.highlight += 1;
                    }
                }
                return Outcome::Consumed;
            }
            KeyCode::Home => {
                self.highlight = 0;
                return Outcome::Consumed;
            }
            KeyCode::End => {
                self.highlight = self.options.len().saturating_sub(1);
                return Outcome::Consumed;
            }
            KeyCode::PageUp => {
                self.highlight = self.highlight.saturating_sub(5);
                return Outcome::Consumed;
            }
            KeyCode::PageDown => {
                self.highlight = (self.highlight + 5).min(self.options.len().saturating_sub(1));
                return Outcome::Consumed;
            }
            _ => {}
        }

        // Typeahead
        if let Some(c) = plain_char(&k) {
            self.typeahead.push(c);
            self.typeahead_match(Instant::now());
            return Outcome::Consumed;
        }

        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if self.open {
            let sb_out = self.scrollbar_state.handle_mouse(m);
            if sb_out.is_changed() {
                self.scroll = self.scrollbar_state.offset;
                return Outcome::Consumed;
            }

            for (i, hit) in self.option_hits.iter_mut().enumerate() {
                let h = hit.mouse(&m);
                if let Hit::Press = h {
                    let idx = i + self.scroll;
                    if idx < self.options.len() && !self.options[idx].disabled {
                        self.selected = Some(idx);
                        self.open = false;
                        return Outcome::Changed;
                    }
                }
                if matches!(h, Hit::HoverChanged) {
                    self.highlight = i + self.scroll;
                }
            }

            if wheel_delta(&m).is_some() && mouse_in(self.dropdown_area, &m) {
                let delta = wheel_delta(&m).unwrap();
                self.scroll = (self.scroll as i32 - delta).max(0) as usize;
                return Outcome::Consumed;
            }

            // Click outside closes
            if is_left_down(&m) && !mouse_in(self.dropdown_area, &m) && !mouse_in(self.hit.area, &m) {
                self.open = false;
                return Outcome::Consumed;
            }
        }

        let hit = self.hit.mouse(&m);
        if let Hit::Press = hit {
            self.open = !self.open;
            if self.open {
                self.highlight = self.selected.unwrap_or(0);
            }
            return Outcome::Consumed;
        }

        if matches!(hit, Hit::HoverChanged | Hit::Click | Hit::Cancel) {
            return Outcome::Consumed;
        }

        Outcome::Ignored
    }
}

// ───────────────────────────── select render ─────────────────────────────

impl StatefulWidget for Select {
    type State = SelectState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };

        let label = state.selected_label().map(str::to_string);
        if self.compact {
            render_select_compact(area, buf, &mut state.hit, &th, look, label.as_deref(), &self.placeholder, state.open);
        } else {
            render_field(area, buf, &mut state.hit, &th, look, label.as_deref(), &self.placeholder, state.open);
        }
    }
}

impl SelectState {
    /// Render the dropdown overlay. Call this at the end of your page's draw.
    pub fn render_overlay(&mut self, buf: &mut Buffer, bounds: Rect, theme: &Theme, max_visible: usize) {
        if !self.open || self.options.is_empty() {
            return;
        }

        let visible = max_visible.min(self.options.len());
        let h = visible as u16 + 2;
        let w = self.options.iter().map(|o| o.label.width()).max().unwrap_or(10).min(60) as u16 + 4;

        let dropdown = popup_below(self.hit.area, w, h, bounds);
        self.dropdown_area = dropdown;

        self.scroll = keep_visible(self.scroll, self.highlight, visible);

        render_option_list(buf, dropdown, &self.options, self.highlight, self.scroll, visible, theme, &mut self.option_hits, &mut self.scrollbar_state);
    }
}

fn render_select_compact(area: Rect, buf: &mut Buffer, hit: &mut HitBox, th: &Theme, look: Look, label: Option<&str>, placeholder: &str, open: bool) {
    hit.set_area(area);
    if area.width < 4 || area.height == 0 {
        return;
    }

    let bg = if look.focused { th.focus_bg() } else { th.surface };
    fill(buf, area, bg);

    let fg = if look.enabled { th.text } else { th.text_disabled };
    let arrow = if open { "▲" } else { "▼" };

    put(buf, area.x, area.y, label.unwrap_or(placeholder), area.width.saturating_sub(2), st(fg, bg));
    put(buf, area.right().saturating_sub(2), area.y, arrow, 1, st(th.text_muted, bg));
}

/// The 3-row `Tall` field shared by Select and MultiSelect: label or placeholder, arrow.
fn render_field(area: Rect, buf: &mut Buffer, hit: &mut HitBox, th: &Theme, look: Look, label: Option<&str>, placeholder: &str, open: bool) {
    if area.height < 3 || area.width < 8 {
        hit.set_area(Rect::default());
        return;
    }

    hit.set_area(area);
    let bg = if look.focused { th.focus_bg() } else { th.surface };
    fill(buf, area, bg);

    let border = if look.focused { th.border } else { th.border_blurred };
    Border::Tall.draw(buf, area, border, bg);

    let inner = pad(area, 3, 1);
    let fg = if look.enabled { th.text } else { th.text_disabled };
    let label_fg = if label.is_some() { fg } else { th.text_muted };
    let arrow = if open { "▲" } else { "▼" };

    put(buf, inner.x, inner.y, label.unwrap_or(placeholder), inner.width.saturating_sub(2), st(label_fg, bg));
    put(buf, inner.right().saturating_sub(2), inner.y, arrow, 1, st(th.text_muted, bg));
}

fn render_option_list(
    buf: &mut Buffer,
    area: Rect,
    options: &[SelectOption],
    highlight: usize,
    scroll: usize,
    visible: usize,
    th: &Theme,
    hits: &mut Vec<HitBox>,
    scrollbar_state: &mut ScrollbarState,
) {
    let bg = th.surface.blend(th.text, 0.05);
    fill(buf, area, bg);
    Border::Tall.draw(buf, area, th.border, bg);

    let inner = pad(area, 1, 1);
    hits.clear();

    for (row, idx) in (scroll..).zip(0..visible) {
        if row >= options.len() {
            break;
        }
        let y = inner.y + idx as u16;
        let row_rect = Rect { x: inner.x, y, width: inner.width, height: 1 };

        let opt = &options[row];
        let is_highlight = row == highlight;
        let (fg, row_bg, bold) = if opt.disabled {
            (th.text_disabled, bg, false)
        } else if is_highlight {
            fill(buf, row_rect, th.cursor_bg);
            (th.cursor_fg, th.cursor_bg, true)
        } else {
            (th.text, bg, false)
        };

        let mut style = st(fg, row_bg);
        if bold {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }

        put(buf, row_rect.x + 1, y, &opt.label, row_rect.width.saturating_sub(2), style);

        let mut hit = HitBox::default();
        hit.set_area(row_rect);
        hits.push(hit);
    }

    if options.len() > visible {
        let sb_area = Rect { x: area.right().saturating_sub(1), y: area.y + 1, width: 1, height: area.height.saturating_sub(2) };
        Scrollbar::vertical(options.len(), visible).offset(scroll).theme(th).render(sb_area, buf, scrollbar_state);
    }
}

// ───────────────────────────── combobox ─────────────────────────────

impl Combobox {
    pub fn new() -> Self {
        Self {
            placeholder: "Type to filter".to_string(),
            max_visible: 8,
            compact: false,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = s.to_string();
        self
    }

    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n;
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn now(mut self, t: Instant) -> Self {
        self.now = Some(t);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Combobox {
    fn default() -> Self {
        Self::new()
    }
}

impl ComboboxState {
    pub fn new(options: &[&str]) -> Self {
        Self {
            input: InputState::new(),
            options: options.iter().map(|s| SelectOption::new(s)).collect(),
            selected: None,
            open: false,
            highlight: 0,
            scroll: 0,
            hit: HitBox::default(),
            scrollbar_state: ScrollbarState::default(),
            filtered: Vec::new(),
            option_hits: Vec::new(),
            dropdown_area: Rect::default(),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_label(&self) -> Option<&str> {
        self.selected.and_then(|i| self.options.get(i).map(|o| o.label.as_str()))
    }

    pub fn value(&self) -> &str {
        self.input.value()
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn update_filter(&mut self) {
        let query = self.input.value();
        if query.is_empty() {
            self.filtered = self.options.iter().enumerate().map(|(i, _)| (i, 0, Vec::new())).collect();
        } else {
            let labels: Vec<&str> = self.options.iter().map(|o| o.label.as_str()).collect();
            self.filtered = fuzzy::rank(query, labels);
        }
        if !self.filtered.is_empty() {
            self.highlight = 0;
        }
    }

    /// Render the dropdown overlay.
    pub fn render_overlay(&mut self, buf: &mut Buffer, bounds: Rect, theme: &Theme, max_visible: usize) {
        if !self.open || self.filtered.is_empty() {
            return;
        }

        let visible = max_visible.min(self.filtered.len());
        let h = visible as u16 + 2;
        let w = self.options.iter().map(|o| o.label.width()).max().unwrap_or(10).min(60) as u16 + 4;

        let dropdown = popup_below(self.hit.area, w, h, bounds);
        self.dropdown_area = dropdown;

        self.scroll = keep_visible(self.scroll, self.highlight, visible);

        let bg = theme.surface.blend(theme.text, 0.05);
        fill(buf, dropdown, bg);
        Border::Tall.draw(buf, dropdown, theme.border, bg);

        let inner = pad(dropdown, 1, 1);
        self.option_hits.clear();

        for (row, idx) in (self.scroll..).zip(0..visible) {
            if row >= self.filtered.len() {
                break;
            }
            let y = inner.y + idx as u16;
            let row_rect = Rect { x: inner.x, y, width: inner.width, height: 1 };

            let (orig_idx, _score, positions) = &self.filtered[row];
            let opt = &self.options[*orig_idx];
            let is_highlight = row == self.highlight;

            let (fg, row_bg, bold) = if opt.disabled {
                (theme.text_disabled, bg, false)
            } else if is_highlight {
                fill(buf, row_rect, theme.cursor_bg);
                (theme.cursor_fg, theme.cursor_bg, true)
            } else {
                (theme.text, bg, false)
            };

            let mut style = st(fg, row_bg);
            if bold {
                style = style.add_modifier(ratatui::style::Modifier::BOLD);
            }

            // Highlight matched characters
            let mut x = row_rect.x + 1;
            for (i, ch) in opt.label.chars().enumerate() {
                let matched = positions.contains(&i);
                let s = if matched { style.add_modifier(ratatui::style::Modifier::UNDERLINED) } else { style };
                let mut buf_ch = [0u8; 4];
                put(buf, x, y, ch.encode_utf8(&mut buf_ch), 1, s);
                x += 1;
                if x >= row_rect.right().saturating_sub(1) {
                    break;
                }
            }

            let mut hit = HitBox::default();
            hit.set_area(row_rect);
            self.option_hits.push(hit);
        }

        if self.filtered.len() > visible {
            let sb_area = Rect { x: dropdown.right().saturating_sub(1), y: dropdown.y + 1, width: 1, height: dropdown.height.saturating_sub(2) };
            Scrollbar::vertical(self.filtered.len(), visible).offset(self.scroll).theme(theme).render(sb_area, buf, &mut self.scrollbar_state);
        }
    }
}

impl Interactive for ComboboxState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        // Down opens or moves into list
        if k.code == KeyCode::Down {
            if !self.open {
                self.open = true;
                self.update_filter();
                return Outcome::Consumed;
            } else if self.highlight + 1 < self.filtered.len() {
                self.highlight += 1;
                return Outcome::Consumed;
            }
        }

        // In list navigation
        if self.open {
            match k.code {
                KeyCode::Up => {
                    if self.highlight > 0 {
                        self.highlight -= 1;
                    }
                    return Outcome::Consumed;
                }
                KeyCode::Esc => {
                    self.open = false;
                    return Outcome::Consumed;
                }
                KeyCode::Enter => {
                    if !self.filtered.is_empty() {
                        let (orig_idx, _, _) = self.filtered[self.highlight];
                        if !self.options[orig_idx].disabled {
                            self.selected = Some(orig_idx);
                            self.input.set_value(&self.options[orig_idx].label);
                            self.open = false;
                            return Outcome::Changed;
                        }
                    }
                    return Outcome::Consumed;
                }
                KeyCode::PageUp => {
                    self.highlight = self.highlight.saturating_sub(5);
                    return Outcome::Consumed;
                }
                KeyCode::PageDown => {
                    self.highlight = (self.highlight + 5).min(self.filtered.len().saturating_sub(1));
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }

        // Forward to input
        let out = self.input.handle_key(k);
        if out.is_changed() || out.is_consumed() {
            self.update_filter();
            if !self.open && !self.input.value().is_empty() {
                self.open = true;
            }
        }
        out
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if self.open {
            let sb_out = self.scrollbar_state.handle_mouse(m);
            if sb_out.is_changed() {
                self.scroll = self.scrollbar_state.offset;
                return Outcome::Consumed;
            }

            for (i, hit) in self.option_hits.iter_mut().enumerate() {
                let h = hit.mouse(&m);
                if let Hit::Press = h {
                    let list_idx = i + self.scroll;
                    if list_idx < self.filtered.len() {
                        let (orig_idx, _, _) = self.filtered[list_idx];
                        if !self.options[orig_idx].disabled {
                            self.selected = Some(orig_idx);
                            self.input.set_value(&self.options[orig_idx].label);
                            self.open = false;
                            return Outcome::Changed;
                        }
                    }
                }
                if matches!(h, Hit::HoverChanged) {
                    self.highlight = i + self.scroll;
                }
            }

            if wheel_delta(&m).is_some() && mouse_in(self.dropdown_area, &m) {
                let delta = wheel_delta(&m).unwrap();
                self.scroll = (self.scroll as i32 - delta).max(0) as usize;
                return Outcome::Consumed;
            }

            if is_left_down(&m) && !mouse_in(self.dropdown_area, &m) && !mouse_in(self.hit.area, &m) {
                self.open = false;
                return Outcome::Consumed;
            }
        }

        self.input.handle_mouse(m)
    }
}

impl StatefulWidget for Combobox {
    type State = ComboboxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.clone().unwrap_or_else(theme::current);
        state.hit.set_area(area);

        Input::new()
            .placeholder(&self.placeholder)
            .compact(self.compact)
            .focused(self.focused)
            .enabled(self.enabled)
            .now(self.now.unwrap_or_else(Instant::now))
            .theme(&th)
            .render(area, buf, &mut state.input);
    }
}

// ───────────────────────────── multiselect ─────────────────────────────

impl MultiSelect {
    pub fn new() -> Self {
        Self {
            placeholder: "Select multiple".to_string(),
            max_visible: 8,
            compact: false,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = s.to_string();
        self
    }

    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n;
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn now(mut self, t: Instant) -> Self {
        self.now = Some(t);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for MultiSelect {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiSelectState {
    pub fn new(options: &[&str]) -> Self {
        let len = options.len();
        Self {
            options: options.iter().map(|s| SelectOption::new(s)).collect(),
            selected: vec![false; len],
            open: false,
            highlight: 0,
            scroll: 0,
            hit: HitBox::default(),
            scrollbar_state: ScrollbarState::default(),
            option_hits: Vec::new(),
            dropdown_area: Rect::default(),
        }
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i).collect()
    }

    pub fn summary(&self) -> String {
        let count = self.selected.iter().filter(|&&b| b).count();
        if count == 0 {
            "None selected".to_string()
        } else if count == 1 {
            self.options.iter().zip(self.selected.iter()).find(|(_, b)| **b).map(|(o, _)| o.label.clone()).unwrap_or_default()
        } else {
            format!("{} selected", count)
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Render the dropdown overlay.
    pub fn render_overlay(&mut self, buf: &mut Buffer, bounds: Rect, theme: &Theme, max_visible: usize) {
        if !self.open || self.options.is_empty() {
            return;
        }

        let visible = max_visible.min(self.options.len());
        let h = visible as u16 + 2;
        let w = self.options.iter().map(|o| o.label.width() + 4).max().unwrap_or(14).min(60) as u16 + 4;

        let dropdown = popup_below(self.hit.area, w, h, bounds);
        self.dropdown_area = dropdown;

        self.scroll = keep_visible(self.scroll, self.highlight, visible);

        let bg = theme.surface.blend(theme.text, 0.05);
        fill(buf, dropdown, bg);
        Border::Tall.draw(buf, dropdown, theme.border, bg);

        let inner = pad(dropdown, 1, 1);
        self.option_hits.clear();

        for (row, idx) in (self.scroll..).zip(0..visible) {
            if row >= self.options.len() {
                break;
            }
            let y = inner.y + idx as u16;
            let row_rect = Rect { x: inner.x, y, width: inner.width, height: 1 };

            let opt = &self.options[row];
            let is_highlight = row == self.highlight;
            let checked = self.selected.get(row).copied().unwrap_or(false);

            let (fg, row_bg, bold) = if opt.disabled {
                (theme.text_disabled, bg, false)
            } else if is_highlight {
                fill(buf, row_rect, theme.cursor_bg);
                (theme.cursor_fg, theme.cursor_bg, true)
            } else {
                (theme.text, bg, false)
            };

            let mut style = st(fg, row_bg);
            if bold {
                style = style.add_modifier(ratatui::style::Modifier::BOLD);
            }

            let check = if checked { "[✓]" } else { "[ ]" };
            put(buf, row_rect.x + 1, y, check, 3, style);
            put(buf, row_rect.x + 5, y, &opt.label, row_rect.width.saturating_sub(6), style);

            let mut hit = HitBox::default();
            hit.set_area(row_rect);
            self.option_hits.push(hit);
        }

        if self.options.len() > visible {
            let sb_area = Rect { x: dropdown.right().saturating_sub(1), y: dropdown.y + 1, width: 1, height: dropdown.height.saturating_sub(2) };
            Scrollbar::vertical(self.options.len(), visible).offset(self.scroll).theme(theme).render(sb_area, buf, &mut self.scrollbar_state);
        }
    }
}

impl Interactive for MultiSelectState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        if !self.open {
            if is_activate(&k) || k.code == KeyCode::Down {
                self.open = true;
                return Outcome::Consumed;
            }
            return Outcome::Ignored;
        }

        match k.code {
            KeyCode::Esc => {
                self.open = false;
                return Outcome::Consumed;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.highlight < self.options.len() && !self.options[self.highlight].disabled {
                    self.selected[self.highlight] = !self.selected[self.highlight];
                    return Outcome::Changed;
                }
                return Outcome::Consumed;
            }
            KeyCode::Up => {
                if self.highlight > 0 {
                    self.highlight -= 1;
                }
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                if self.highlight + 1 < self.options.len() {
                    self.highlight += 1;
                }
                return Outcome::Consumed;
            }
            KeyCode::Home => {
                self.highlight = 0;
                return Outcome::Consumed;
            }
            KeyCode::End => {
                self.highlight = self.options.len().saturating_sub(1);
                return Outcome::Consumed;
            }
            KeyCode::PageUp => {
                self.highlight = self.highlight.saturating_sub(5);
                return Outcome::Consumed;
            }
            KeyCode::PageDown => {
                self.highlight = (self.highlight + 5).min(self.options.len().saturating_sub(1));
                return Outcome::Consumed;
            }
            _ => {}
        }

        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if self.open {
            let sb_out = self.scrollbar_state.handle_mouse(m);
            if sb_out.is_changed() {
                self.scroll = self.scrollbar_state.offset;
                return Outcome::Consumed;
            }

            for (i, hit) in self.option_hits.iter_mut().enumerate() {
                let h = hit.mouse(&m);
                if let Hit::Press = h {
                    let idx = i + self.scroll;
                    if idx < self.options.len() && !self.options[idx].disabled {
                        self.selected[idx] = !self.selected[idx];
                        return Outcome::Changed;
                    }
                }
                if matches!(h, Hit::HoverChanged) {
                    self.highlight = i + self.scroll;
                }
            }

            if wheel_delta(&m).is_some() && mouse_in(self.dropdown_area, &m) {
                let delta = wheel_delta(&m).unwrap();
                self.scroll = (self.scroll as i32 - delta).max(0) as usize;
                return Outcome::Consumed;
            }

            if is_left_down(&m) && !mouse_in(self.dropdown_area, &m) && !mouse_in(self.hit.area, &m) {
                self.open = false;
                return Outcome::Consumed;
            }
        }

        let hit = self.hit.mouse(&m);
        if let Hit::Press = hit {
            self.open = !self.open;
            return Outcome::Consumed;
        }

        if matches!(hit, Hit::HoverChanged | Hit::Click | Hit::Cancel) {
            return Outcome::Consumed;
        }

        Outcome::Ignored
    }
}

impl StatefulWidget for MultiSelect {
    type State = MultiSelectState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };

        let summary = state.summary();
        let label = if state.selected.iter().any(|&b| b) { Some(summary.as_str()) } else { None };
        if self.compact {
            render_select_compact(area, buf, &mut state.hit, &th, look, label, &self.placeholder, state.open);
        } else {
            render_field(area, buf, &mut state.hit, &th, look, label, &self.placeholder, state.open);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_navigation() {
        let mut s = SelectState::new(&["A", "B", "C"]);
        s.open = true;
        s.highlight = 1;
        s.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(s.highlight, 2);
    }

    #[test]
    fn combobox_filter() {
        let mut s = ComboboxState::new(&["Apple", "Banana", "Apricot"]);
        s.input.set_value("ap");
        s.update_filter();
        assert_eq!(s.filtered.len(), 2);
    }

    #[test]
    fn multiselect_toggle() {
        let mut s = MultiSelectState::new(&["X", "Y", "Z"]);
        s.open = true;
        s.highlight = 1;
        s.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(s.selected[1]);
    }

    #[test]
    fn multiselect_rendered_field_opens_on_click() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
        let mut s = MultiSelectState::new(&["X", "Y"]);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
        MultiSelect::new().render(Rect::new(0, 0, 30, 3), &mut buf, &mut s);
        let press = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 5, row: 1, modifiers: KeyModifiers::NONE };
        assert_eq!(s.handle_mouse(press), Outcome::Consumed);
        assert!(s.open, "click inside the rendered field must open the dropdown");
    }
}
