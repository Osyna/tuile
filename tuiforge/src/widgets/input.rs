//! Single-line text input with block cursor, horizontal scrolling, selection, history,
//! validation, and autocomplete suggestions.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 3);
//! # let mut buf = Buffer::empty(area);
//! let mut state = InputState::new();
//! Input::new().placeholder("Name").max_len(32).focused(true).render(area, &mut buf, &mut state);
//! if state.take_submitted() { /* form submit */ }
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::StatefulWidget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::anim::{blink, elapsed};
use crate::core::*;
use crate::draw::{FieldShape, fill, put, st};
use crate::layout::pad;
use crate::theme::{self, Theme};

// types

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRestrict {
    None,
    Digits,
    Integer,
    Number,
    Alpha,
    Alnum,
    Custom,
}

/// Builder for a single-line text input.
#[derive(Clone, Debug)]
pub struct Input {
    placeholder: String,
    password: bool,
    max_len: Option<usize>,
    restrict: InputRestrict,
    // A `fn` pointer hook the caller must match exactly; an alias would hide the signature.
    #[allow(clippy::type_complexity)]
    validator: Option<fn(&str) -> Result<(), String>>,
    suggester: Option<fn(&str) -> Option<String>>,
    prefix: String,
    suffix: String,
    compact: bool,
    shape: FieldShape,
    tab_accepts: bool,
    show_error: bool,
    select_on_focus: bool,
    align: Alignment,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

/// State for a single-line input.
#[derive(Clone, Debug)]
pub struct InputState {
    pub value: String,
    pub cursor: usize,
    pub scroll: usize,
    pub selection: Option<(usize, usize)>,
    pub hit: HitBox,
    pub submitted: bool,
    history: Vec<String>,
    clipboard: String,
    last_edit: Instant,
    pub error: Option<String>,
    // A `fn` pointer hook the caller must match exactly; an alias would hide the signature.
    #[allow(clippy::type_complexity)]
    validator: Option<fn(&str) -> Result<(), String>>,
    suggester: Option<fn(&str) -> Option<String>>,
    last_click: Option<Instant>,
    restrict: InputRestrict,
    custom_filter: Option<fn(char) -> bool>,
}

// builder

impl Input {
    pub fn new() -> Self {
        Self {
            placeholder: String::new(),
            password: false,
            max_len: None,
            restrict: InputRestrict::None,
            validator: None,
            suggester: None,
            prefix: String::new(),
            suffix: String::new(),
            compact: false,
            shape: FieldShape::default(),
            tab_accepts: false,
            show_error: false,
            select_on_focus: false,
            align: Alignment::Left,
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

    pub fn password(mut self, v: bool) -> Self {
        self.password = v;
        self
    }

    pub fn max_len(mut self, n: usize) -> Self {
        self.max_len = Some(n);
        self
    }

    pub fn restrict(mut self, r: InputRestrict) -> Self {
        self.restrict = r;
        self
    }

    pub fn validator(mut self, f: fn(&str) -> Result<(), String>) -> Self {
        self.validator = Some(f);
        self
    }

    pub fn suggester(mut self, f: fn(&str) -> Option<String>) -> Self {
        self.suggester = Some(f);
        self
    }

    pub fn prefix(mut self, s: &str) -> Self {
        self.prefix = s.to_string();
        self
    }

    pub fn suffix(mut self, s: &str) -> Self {
        self.suffix = s.to_string();
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    /// Frame shape: `FieldShape::Tall(Edge::Full)` (Textual) by default; `Bars(Edge::Hair)`,
    /// `Rule`, `Prompt`… for omp-style fields.
    pub fn shape(mut self, s: FieldShape) -> Self {
        self.shape = s;
        self
    }

    pub fn tab_accepts(mut self, v: bool) -> Self {
        self.tab_accepts = v;
        self
    }

    pub fn show_error(mut self, v: bool) -> Self {
        self.show_error = v;
        self
    }

    pub fn select_on_focus(mut self, v: bool) -> Self {
        self.select_on_focus = v;
        self
    }

    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
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
        self.theme = Some(*th);
        self
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

// state

impl InputState {
    pub fn new() -> Self {
        Self::with_value("")
    }

    pub fn with_value(s: &str) -> Self {
        Self {
            value: s.to_string(),
            cursor: s.graphemes(true).count(),
            scroll: 0,
            selection: None,
            hit: HitBox::default(),
            submitted: false,
            history: Vec::new(),
            clipboard: String::new(),
            last_edit: Instant::now(),
            error: None,
            validator: None,
            suggester: None,
            last_click: None,
            restrict: InputRestrict::None,
            custom_filter: None,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, s: &str) {
        self.value = s.to_string();
        self.cursor = s.graphemes(true).count();
        self.selection = None;
        self.last_edit = Instant::now();
        self.validate();
    }

    pub fn take_submitted(&mut self) -> bool {
        std::mem::replace(&mut self.submitted, false)
    }

    pub fn select_all(&mut self) {
        let len = self.value.graphemes(true).count();
        if len > 0 {
            self.selection = Some((0, len));
        }
    }

    pub fn animating(&self, now: Instant) -> bool {
        // Always animate for cursor blink
        elapsed(self.last_edit, now) < 10.0
    }

    fn graphemes(&self) -> Vec<&str> {
        self.value.graphemes(true).collect()
    }

    fn validate(&mut self) {
        if let Some(validator) = self.validator {
            self.error = validator(&self.value).err();
        }
    }

    fn save_history(&mut self) {
        if !self.value.is_empty()
            && (self.history.is_empty() || self.history.last() != Some(&self.value))
        {
            self.history.push(self.value.clone());
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
    }

    fn insert_char(&mut self, c: char) -> bool {
        // Check restriction
        let allowed = match self.restrict {
            InputRestrict::None => true,
            InputRestrict::Digits => c.is_ascii_digit(),
            InputRestrict::Integer => c.is_ascii_digit() || (c == '-' && self.cursor == 0),
            InputRestrict::Number => {
                c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E'
            }
            InputRestrict::Alpha => c.is_alphabetic(),
            InputRestrict::Alnum => c.is_alphanumeric(),
            InputRestrict::Custom => {
                if let Some(f) = self.custom_filter {
                    f(c)
                } else {
                    true
                }
            }
        };
        if !allowed {
            return false;
        }

        self.delete_selection();
        let graphemes = self.graphemes();
        let mut new = String::new();
        for (i, g) in graphemes.iter().enumerate() {
            if i == self.cursor {
                new.push(c);
            }
            new.push_str(g);
        }
        if self.cursor >= graphemes.len() {
            new.push(c);
        }
        self.value = new;
        self.cursor += 1;
        self.last_edit = Instant::now();
        self.validate();
        true
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((a, b)) = self.selection {
            let (start, end) = if a < b { (a, b) } else { (b, a) };
            let graphemes = self.graphemes();
            self.value = graphemes
                .iter()
                .enumerate()
                .filter(|(i, _)| *i < start || *i >= end)
                .map(|(_, g)| *g)
                .collect();
            self.cursor = start;
            self.selection = None;
            self.last_edit = Instant::now();
            self.validate();
            return true;
        }
        false
    }

    fn delete_backward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let graphemes = self.graphemes();
        self.value = graphemes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.cursor - 1)
            .map(|(_, g)| *g)
            .collect();
        self.cursor = self.cursor.saturating_sub(1);
        self.last_edit = Instant::now();
        self.validate();
    }

    fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let graphemes = self.graphemes();
        if self.cursor >= graphemes.len() {
            return;
        }
        self.value = graphemes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.cursor)
            .map(|(_, g)| *g)
            .collect();
        self.last_edit = Instant::now();
        self.validate();
    }

    fn move_cursor(&mut self, delta: i32, extend: bool) {
        let len = self.value.graphemes(true).count();
        let new = (self.cursor as i32 + delta).max(0).min(len as i32) as usize;
        if extend {
            if let Some((anchor, _)) = self.selection {
                self.selection = Some((anchor, new));
            } else {
                self.selection = Some((self.cursor, new));
            }
        } else {
            self.selection = None;
        }
        self.cursor = new;
    }

    fn word_boundary(&self, pos: usize, forward: bool) -> usize {
        crate::core::word_boundary(&self.graphemes(), pos, forward)
    }

    fn accept_suggestion(&mut self) -> bool {
        if let Some(sg) = self.suggester
            && let Some(suggestion) = sg(&self.value)
        {
            self.value.push_str(&suggestion);
            self.cursor = self.value.graphemes(true).count();
            self.last_edit = Instant::now();
            self.validate();
            return true;
        }
        false
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactive for InputState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        let shift = k.modifiers.contains(KeyModifiers::SHIFT);

        // Enter submits
        if k.code == KeyCode::Enter {
            self.save_history();
            self.submitted = true;
            return Outcome::Changed;
        }

        // Tab accepts suggestion
        if k.code == KeyCode::Tab && !shift {
            if self.accept_suggestion() {
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }

        // Navigation
        match k.code {
            KeyCode::Left if ctrl(&k, 'b') || k.modifiers.contains(KeyModifiers::CONTROL) => {
                let pos = self.word_boundary(self.cursor, false);
                let delta = pos as i32 - self.cursor as i32;
                self.move_cursor(delta, shift);
                return Outcome::Consumed;
            }
            KeyCode::Right if ctrl(&k, 'f') || k.modifiers.contains(KeyModifiers::CONTROL) => {
                let pos = self.word_boundary(self.cursor, true);
                let delta = pos as i32 - self.cursor as i32;
                self.move_cursor(delta, shift);
                return Outcome::Consumed;
            }
            KeyCode::Left => {
                self.move_cursor(-1, shift);
                return Outcome::Consumed;
            }
            KeyCode::Right if !shift || self.selection.is_some() => {
                // Accept suggestion only at end with no selection
                if !shift
                    && self.cursor == self.value.graphemes(true).count()
                    && self.selection.is_none()
                    && self.accept_suggestion()
                {
                    return Outcome::Changed;
                }
                self.move_cursor(1, shift);
                return Outcome::Consumed;
            }
            KeyCode::Right => {
                self.move_cursor(1, shift);
                return Outcome::Consumed;
            }
            KeyCode::Home => {
                self.move_cursor(-(self.cursor as i32), shift);
                return Outcome::Consumed;
            }
            KeyCode::End => {
                let len = self.value.graphemes(true).count();
                self.move_cursor(len as i32 - self.cursor as i32, shift);
                return Outcome::Consumed;
            }
            _ => {}
        }

        // Ctrl shortcuts
        if ctrl(&k, 'a') {
            self.select_all();
            return Outcome::Consumed;
        }
        if ctrl(&k, 'e') {
            let len = self.value.graphemes(true).count();
            self.move_cursor(len as i32 - self.cursor as i32, false);
            return Outcome::Consumed;
        }
        if ctrl(&k, 'u') {
            self.selection = Some((0, self.cursor));
            self.delete_selection();
            return Outcome::Changed;
        }
        if ctrl(&k, 'k') {
            let len = self.value.graphemes(true).count();
            self.selection = Some((self.cursor, len));
            self.delete_selection();
            return Outcome::Changed;
        }
        if ctrl(&k, 'w') {
            let pos = self.word_boundary(self.cursor, false);
            self.selection = Some((pos, self.cursor));
            self.delete_selection();
            return Outcome::Changed;
        }
        if ctrl(&k, 'x') {
            if let Some((a, b)) = self.selection {
                let (start, end) = if a < b { (a, b) } else { (b, a) };
                let graphemes = self.graphemes();
                self.clipboard = graphemes[start..end].join("");
                self.delete_selection();
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        if ctrl(&k, 'c') {
            if let Some((a, b)) = self.selection {
                let (start, end) = if a < b { (a, b) } else { (b, a) };
                let graphemes = self.graphemes();
                self.clipboard = graphemes[start..end].join("");
                return Outcome::Consumed;
            }
            return Outcome::Consumed;
        }
        if ctrl(&k, 'v') {
            let text = self.clipboard.clone();
            for c in text.chars() {
                self.insert_char(c);
            }
            return if text.is_empty() {
                Outcome::Consumed
            } else {
                Outcome::Changed
            };
        }
        if ctrl(&k, 'z') {
            if let Some(prev) = self.history.pop() {
                self.value = prev;
                self.cursor = self.value.graphemes(true).count();
                self.selection = None;
                self.last_edit = Instant::now();
                self.validate();
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }

        // Alt shortcuts
        if alt(&k, 'b') {
            let pos = self.word_boundary(self.cursor, false);
            let delta = pos as i32 - self.cursor as i32;
            self.move_cursor(delta, shift);
            return Outcome::Consumed;
        }
        if alt(&k, 'f') {
            let pos = self.word_boundary(self.cursor, true);
            let delta = pos as i32 - self.cursor as i32;
            self.move_cursor(delta, shift);
            return Outcome::Consumed;
        }

        // Editing
        if k.code == KeyCode::Backspace {
            if k.modifiers.contains(KeyModifiers::CONTROL) {
                let pos = self.word_boundary(self.cursor, false);
                self.selection = Some((pos, self.cursor));
                self.delete_selection();
            } else {
                self.delete_backward();
            }
            return Outcome::Changed;
        }
        if k.code == KeyCode::Delete {
            self.delete_forward();
            return Outcome::Changed;
        }

        // Typing
        if let Some(c) = plain_char(&k)
            && self.insert_char(c)
        {
            return Outcome::Changed;
        }

        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let hit = self.hit.mouse(&m);
        if let Hit::Press = hit {
            // Map x position to cursor (requires render to have stored content rect)
            // For now just focus
            self.last_click = Some(Instant::now());
            return Outcome::Consumed;
        }
        if matches!(hit, Hit::HoverChanged | Hit::Click | Hit::Cancel) {
            return Outcome::Consumed;
        }
        Outcome::Ignored
    }
}

// render

impl StatefulWidget for Input {
    type State = InputState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.validator = self.validator;
        state.suggester = self.suggester;
        state.restrict = self.restrict;
        // prefilled or programmatically set values must show their validation state too
        state.validate();

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: state.hit.hover,
            enabled: self.enabled,
        };
        let now = self.now.unwrap_or_else(Instant::now);
        let cursor_visible = blink(elapsed(state.last_edit, now), 1.0);

        // Compact mode: 1 row no border
        if self.compact {
            state.hit.set_area(area);
            if area.width < 2 || area.height == 0 {
                return;
            }
            let bg = if look.focused {
                th.focus_bg()
            } else {
                th.surface
            };
            fill(buf, area, bg);
            let _fg = if look.enabled {
                th.text
            } else {
                th.text_disabled
            };

            let prefix_w = self.prefix.width() as u16;
            let suffix_w = self.suffix.width() as u16;
            let text_w = area.width.saturating_sub(prefix_w + suffix_w);

            if prefix_w > 0 {
                put(
                    buf,
                    area.x,
                    area.y,
                    &self.prefix,
                    prefix_w,
                    st(th.text_muted, bg),
                );
            }
            if suffix_w > 0 {
                put(
                    buf,
                    area.right().saturating_sub(suffix_w),
                    area.y,
                    &self.suffix,
                    suffix_w,
                    st(th.text_muted, bg),
                );
            }

            let text_x = area.x + prefix_w;
            if state.value.is_empty() {
                put(
                    buf,
                    text_x,
                    area.y,
                    &self.placeholder,
                    text_w,
                    st(th.text_muted, bg),
                );
                if look.focused
                    && cursor_visible
                    && let Some(cell) = buf.cell_mut((text_x, area.y))
                {
                    cell.set_style(st(th.cursor_fg, th.cursor_bg));
                }
            } else {
                render_text(
                    buf,
                    text_x,
                    area.y,
                    text_w as usize,
                    state,
                    &th,
                    cursor_visible,
                    self.password,
                    look.focused,
                );
            }
            return;
        }

        // Full mode: frame per `shape` (Textual tall by default), 1 content row
        let chrome = self.shape.vertical_chrome();
        if area.height < 1 + chrome || area.width < 7 {
            state.hit.set_area(Rect::default());
            return;
        }

        state.hit.set_area(area);
        let bg = if look.focused {
            th.focus_bg()
        } else {
            th.surface
        };
        fill(buf, area, bg);

        let border = if state.error.is_some() {
            th.error
        } else if look.focused {
            th.border
        } else {
            th.border_blurred
        };
        let frame = Rect {
            height: 1 + chrome,
            ..area
        };
        let extra = u16::from(matches!(
            self.shape,
            FieldShape::Tall(_) | FieldShape::Round
        ));
        let inner = pad(
            self.shape.draw(buf, frame, border, bg),
            self.shape.padding() + extra,
            0,
        );
        if inner.width == 0 {
            return;
        }

        let prefix_w = self.prefix.width() as u16;
        let suffix_w = self.suffix.width() as u16;
        let text_w = inner.width.saturating_sub(prefix_w + suffix_w);

        if prefix_w > 0 {
            put(
                buf,
                inner.x,
                inner.y,
                &self.prefix,
                prefix_w,
                st(th.text_muted, bg),
            );
        }
        if suffix_w > 0 {
            put(
                buf,
                inner.right().saturating_sub(suffix_w),
                inner.y,
                &self.suffix,
                suffix_w,
                st(th.text_muted, bg),
            );
        }

        let text_x = inner.x + prefix_w;
        let _fg = if look.enabled {
            th.text
        } else {
            th.text_disabled
        };

        if state.value.is_empty() {
            put(
                buf,
                text_x,
                inner.y,
                &self.placeholder,
                text_w,
                st(th.text_muted, bg),
            );
            if look.focused
                && cursor_visible
                && let Some(cell) = buf.cell_mut((text_x, inner.y))
            {
                cell.set_style(st(th.cursor_fg, th.cursor_bg));
            }
        } else {
            render_text(
                buf,
                text_x,
                inner.y,
                text_w as usize,
                state,
                &th,
                cursor_visible,
                self.password,
                look.focused,
            );
        }

        // Error message below
        if self.show_error
            && let Some(err) = &state.error
            && area.height > 3
        {
            put(
                buf,
                area.x + 3,
                area.bottom().saturating_sub(1),
                err,
                area.width.saturating_sub(6),
                st(th.error, th.background),
            );
        }

        // Ghost suggestion (never over the placeholder)
        if look.focused
            && !state.value.is_empty()
            && state.selection.is_none()
            && state.cursor == state.value.graphemes(true).count()
            && let Some(sg) = state.suggester
            && let Some(suggestion) = sg(&state.value)
        {
            let graphemes = state.value.graphemes(true);
            let cursor_col: usize = graphemes.map(|g| g.width()).sum();
            let offset = state.scroll;
            if cursor_col >= offset && cursor_col - offset < text_w as usize {
                let ghost_x = text_x + (cursor_col - offset) as u16;
                put(
                    buf,
                    ghost_x,
                    inner.y,
                    &suggestion,
                    text_w.saturating_sub((cursor_col - offset) as u16),
                    st(th.text_disabled, bg),
                );
            }
        }
    }
}

// The render path takes buffer, position, size and style separately: bundling them into a
// struct would cost an allocation per call.
#[allow(clippy::too_many_arguments)]
fn render_text(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    max_w: usize,
    state: &mut InputState,
    th: &Theme,
    cursor_visible: bool,
    password: bool,
    focused: bool,
) {
    let graphemes: Vec<&str> = state.value.graphemes(true).collect();
    let widths: Vec<usize> = graphemes
        .iter()
        .map(|g| if password { 1 } else { g.width() })
        .collect();
    let _total: usize = widths.iter().sum();
    let cursor_col: usize = widths.iter().take(state.cursor).sum();

    // Scroll to keep cursor visible
    if cursor_col + 1 > state.scroll + max_w {
        state.scroll = cursor_col + 1 - max_w;
    }
    if cursor_col < state.scroll {
        state.scroll = cursor_col;
    }

    let bg = if focused { th.focus_bg() } else { th.surface };
    let fg = th.text;

    let (sel_start, sel_end) = if let Some((a, b)) = state.selection {
        if a < b { (a, b) } else { (b, a) }
    } else {
        (usize::MAX, usize::MAX)
    };

    let mut col = 0;
    let mut px = x;
    for (i, (g, w)) in graphemes.iter().zip(widths.iter()).enumerate() {
        if col + w <= state.scroll {
            col += w;
            continue;
        }
        if col >= state.scroll + max_w {
            break;
        }
        if px >= x + max_w as u16 {
            break;
        }

        let in_selection = i >= sel_start && i < sel_end;
        let is_cursor = focused && cursor_visible && i == state.cursor;

        let style = if is_cursor {
            st(th.cursor_fg, th.cursor_bg)
        } else if in_selection {
            st(th.text, th.selection_bg)
        } else {
            st(fg, bg)
        };

        let display = if password { "•" } else { g };
        put(buf, px, y, display, *w as u16, style);
        px += *w as u16;
        col += w;
    }

    // Cursor past end
    if focused && cursor_visible && state.cursor >= graphemes.len() {
        let cursor_col: usize = widths.iter().sum();
        if cursor_col >= state.scroll
            && cursor_col < state.scroll + max_w
            && px < x + max_w as u16
            && let Some(cell) = buf.cell_mut((px, y))
        {
            cell.set_symbol(" ");
            cell.set_style(st(th.cursor_fg, th.cursor_bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_movement() {
        let mut s = InputState::with_value("hello");
        assert_eq!(s.cursor, 5);
        s.move_cursor(-2, false);
        assert_eq!(s.cursor, 3);
        s.move_cursor(10, false);
        assert_eq!(s.cursor, 5);
    }

    #[test]
    fn insert_delete() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        assert_eq!(s.value, "ab");
        s.delete_backward();
        assert_eq!(s.value, "a");
    }

    #[test]
    fn selection_delete() {
        let mut s = InputState::with_value("hello");
        s.selection = Some((1, 4));
        s.delete_selection();
        assert_eq!(s.value, "ho");
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn restrict_digits() {
        let mut s = InputState::new();
        s.restrict = InputRestrict::Digits;
        assert!(s.insert_char('5'));
        assert!(!s.insert_char('a'));
        assert_eq!(s.value, "5");
    }

    #[test]
    fn word_boundary() {
        let s = InputState::with_value("hello world");
        assert_eq!(s.word_boundary(5, true), 11);
        assert_eq!(s.word_boundary(7, false), 6);
    }
}
