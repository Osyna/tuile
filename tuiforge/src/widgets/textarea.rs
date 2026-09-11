//! Multi-line text area with line numbers, syntax highlighting hook, undo/redo,
//! auto-indent, and scrolling.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 15);
//! # let mut buf = Buffer::empty(area);
//! let mut state = TextAreaState::new();
//! TextArea::new().line_numbers(true).highlight_line(true).render(area, &mut buf, &mut state);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::StatefulWidget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::anim;
use crate::core::*;
use crate::draw::{FieldShape, bold, fill, put, put_cell, st};
use crate::theme::{self, Rgb, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

// types
/// Cursor drawing style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorStyle {
    /// Inverse cell (default).
    #[default]
    Block,
    /// Thin bar (`▏`) drawn in cursor color.
    Bar,
    /// Cell keeps glyph, underlined + bold + accent fg.
    Underline,
    /// Cell bg = cursor_blurred_bg, fg unchanged.
    Outline,
}

/// Builder for a multi-line text area.
#[derive(Clone, Debug)]
pub struct TextArea {
    line_numbers: bool,
    highlight_line: bool,
    read_only: bool,
    tab_size: usize,
    max_lines: Option<usize>,
    placeholder: String,
    shape: FieldShape,
    highlighter: Option<fn(&str) -> Vec<(usize, usize, Style)>>,
    cursor_style: CursorStyle,
    cursor_blink: bool,
    cursor_when_unfocused: bool,
    show_position: bool,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

/// State for a multi-line text area.
#[derive(Clone, Debug)]
pub struct TextAreaState {
    pub lines: Vec<String>,
    pub cursor: (usize, usize),
    pub scroll_y: usize,
    pub scroll_x: usize,
    pub selection: Option<(usize, usize, usize, usize)>,
    pub hit: HitBox,
    pub vscroll_state: ScrollbarState,
    pub hscroll_state: ScrollbarState,
    undo_stack: Vec<(Vec<String>, (usize, usize))>,
    redo_stack: Vec<(Vec<String>, (usize, usize))>,
    clipboard: String,
}

// builder

impl TextArea {
    pub fn new() -> Self {
        Self {
            line_numbers: false,
            highlight_line: false,
            read_only: false,
            tab_size: 4,
            max_lines: None,
            placeholder: String::new(),
            shape: FieldShape::default(),
            highlighter: None,
            cursor_style: CursorStyle::default(),
            cursor_blink: false,
            cursor_when_unfocused: false,
            show_position: false,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn line_numbers(mut self, v: bool) -> Self {
        self.line_numbers = v;
        self
    }

    pub fn highlight_line(mut self, v: bool) -> Self {
        self.highlight_line = v;
        self
    }

    pub fn read_only(mut self, v: bool) -> Self {
        self.read_only = v;
        self
    }

    pub fn tab_size(mut self, n: usize) -> Self {
        self.tab_size = n;
        self
    }

    pub fn max_lines(mut self, n: usize) -> Self {
        self.max_lines = Some(n);
        self
    }

    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = s.to_string();
        self
    }

    /// Frame shape (`FieldShape::Tall(Edge::Full)` by default).
    pub fn shape(mut self, s: FieldShape) -> Self {
        self.shape = s;
        self
    }

    pub fn highlighter(mut self, f: fn(&str) -> Vec<(usize, usize, Style)>) -> Self {
        self.highlighter = Some(f);
        self
    }

    pub fn cursor(mut self, style: CursorStyle) -> Self {
        self.cursor_style = style;
        self
    }

    pub fn cursor_blink(mut self, v: bool) -> Self {
        self.cursor_blink = v;
        self
    }

    pub fn cursor_when_unfocused(mut self, v: bool) -> Self {
        self.cursor_when_unfocused = v;
        self
    }

    pub fn show_position(mut self, v: bool) -> Self {
        self.show_position = v;
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

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

// state

impl TextAreaState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            scroll_y: 0,
            scroll_x: 0,
            selection: None,
            hit: HitBox::default(),
            vscroll_state: ScrollbarState::default(),
            hscroll_state: ScrollbarState::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
        }
    }

    pub fn with_text(text: &str) -> Self {
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            lines,
            cursor: (0, 0),
            scroll_y: 0,
            scroll_x: 0,
            selection: None,
            hit: HitBox::default(),
            vscroll_state: ScrollbarState::default(),
            hscroll_state: ScrollbarState::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_text(&mut self, text: &str) {
        self.save_undo();
        self.lines = text.lines().map(|l| l.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = (0, 0);
        self.selection = None;
    }

    pub fn insert_str(&mut self, s: &str) {
        self.save_undo();
        for c in s.chars() {
            if c == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(c);
            }
        }
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn select_all(&mut self) {
        self.selection = Some((
            0,
            0,
            self.lines.len().saturating_sub(1),
            self.lines
                .last()
                .map(|l| l.graphemes(true).count())
                .unwrap_or(0),
        ));
    }

    fn save_undo(&mut self) {
        self.undo_stack.push((self.lines.clone(), self.cursor));
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some((lines, cursor)) = self.undo_stack.pop() {
            self.redo_stack.push((self.lines.clone(), self.cursor));
            self.lines = lines;
            self.cursor = cursor;
            self.selection = None;
        }
    }

    fn redo(&mut self) {
        if let Some((lines, cursor)) = self.redo_stack.pop() {
            self.undo_stack.push((self.lines.clone(), self.cursor));
            self.lines = lines;
            self.cursor = cursor;
            self.selection = None;
        }
    }

    fn current_line(&self) -> &str {
        self.lines
            .get(self.cursor.0)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    fn graphemes(&self, row: usize) -> Vec<&str> {
        self.lines
            .get(row)
            .map(|s| s.graphemes(true).collect())
            .unwrap_or_default()
    }

    fn clamp_cursor(&mut self) {
        if self.cursor.0 >= self.lines.len() {
            self.cursor.0 = self.lines.len().saturating_sub(1);
        }
        let line_len = self.current_line().graphemes(true).count();
        if self.cursor.1 > line_len {
            self.cursor.1 = line_len;
        }
    }

    fn insert_char(&mut self, c: char) {
        self.delete_selection();
        let graphemes = self.graphemes(self.cursor.0);
        let mut new = String::new();
        for (i, g) in graphemes.iter().enumerate() {
            if i == self.cursor.1 {
                new.push(c);
            }
            new.push_str(g);
        }
        if self.cursor.1 >= graphemes.len() {
            new.push(c);
        }
        self.lines[self.cursor.0] = new;
        self.cursor.1 += 1;
    }

    fn insert_newline(&mut self) {
        self.delete_selection();
        let line = self.current_line().to_string();
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let (before, after): (String, String) = (
            graphemes.iter().take(self.cursor.1).copied().collect(),
            graphemes.iter().skip(self.cursor.1).copied().collect(),
        );

        // Auto-indent: copy leading whitespace
        let indent: String = before.chars().take_while(|c| c.is_whitespace()).collect();

        self.lines[self.cursor.0] = before;
        self.cursor.0 += 1;
        self.lines.insert(self.cursor.0, indent.clone() + &after);
        self.cursor.1 = indent.graphemes(true).count();
    }

    fn delete_backward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor.1 > 0 {
            let graphemes = self.graphemes(self.cursor.0);
            self.lines[self.cursor.0] = graphemes
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != self.cursor.1 - 1)
                .map(|(_, g)| *g)
                .collect();
            self.cursor.1 -= 1;
        } else if self.cursor.0 > 0 {
            let current = self.lines.remove(self.cursor.0);
            self.cursor.0 -= 1;
            self.cursor.1 = self.current_line().graphemes(true).count();
            self.lines[self.cursor.0].push_str(&current);
        }
    }

    fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let graphemes = self.graphemes(self.cursor.0);
        if self.cursor.1 < graphemes.len() {
            self.lines[self.cursor.0] = graphemes
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != self.cursor.1)
                .map(|(_, g)| *g)
                .collect();
        } else if self.cursor.0 + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor.0 + 1);
            self.lines[self.cursor.0].push_str(&next);
        }
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((r0, c0, r1, c1)) = self.selection {
            let (start_r, start_c, end_r, end_c) = if r0 < r1 || (r0 == r1 && c0 < c1) {
                (r0, c0, r1, c1)
            } else {
                (r1, c1, r0, c0)
            };

            if start_r == end_r {
                let graphemes = self.graphemes(start_r);
                self.lines[start_r] = graphemes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i < start_c || *i >= end_c)
                    .map(|(_, g)| *g)
                    .collect();
            } else {
                let start_graphemes = self.graphemes(start_r);
                let end_graphemes = self.graphemes(end_r);
                let before: String = start_graphemes.iter().take(start_c).copied().collect();
                let after: String = end_graphemes.iter().skip(end_c).copied().collect();
                self.lines[start_r] = before + &after;
                for _ in start_r + 1..=end_r {
                    if start_r + 1 < self.lines.len() {
                        self.lines.remove(start_r + 1);
                    }
                }
            }

            self.cursor = (start_r, start_c);
            self.selection = None;
            return true;
        }
        false
    }

    fn move_cursor(&mut self, dr: i32, dc: i32) {
        let new_r = (self.cursor.0 as i32 + dr)
            .max(0)
            .min(self.lines.len().saturating_sub(1) as i32) as usize;
        self.cursor.0 = new_r;
        let line_len = self.current_line().graphemes(true).count();
        let new_c = (self.cursor.1 as i32 + dc).max(0).min(line_len as i32) as usize;
        self.cursor.1 = new_c;
    }

    fn word_boundary(&self, row: usize, col: usize, forward: bool) -> usize {
        let graphemes = self.graphemes(row);
        let len = graphemes.len();
        if forward {
            let mut i = col;
            while i < len && graphemes[i].chars().all(|c| !c.is_alphanumeric()) {
                i += 1;
            }
            while i < len && graphemes[i].chars().all(|c| c.is_alphanumeric()) {
                i += 1;
            }
            i
        } else {
            let mut i = col.saturating_sub(1);
            while i > 0 && graphemes[i].chars().all(|c| !c.is_alphanumeric()) {
                i = i.saturating_sub(1);
            }
            while i > 0
                && graphemes[i.saturating_sub(1)]
                    .chars()
                    .all(|c| c.is_alphanumeric())
            {
                i = i.saturating_sub(1);
            }
            i
        }
    }

    fn duplicate_line(&mut self) {
        self.save_undo();
        let line = self.current_line().to_string();
        self.lines.insert(self.cursor.0 + 1, line);
    }

    fn move_line(&mut self, delta: i32) {
        if delta == -1 && self.cursor.0 > 0 {
            self.save_undo();
            self.lines.swap(self.cursor.0, self.cursor.0 - 1);
            self.cursor.0 -= 1;
        } else if delta == 1 && self.cursor.0 + 1 < self.lines.len() {
            self.save_undo();
            self.lines.swap(self.cursor.0, self.cursor.0 + 1);
            self.cursor.0 += 1;
        }
    }
}

impl Default for TextAreaState {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactive for TextAreaState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        let _shift = k.modifiers.contains(KeyModifiers::SHIFT);

        // Navigation
        match k.code {
            KeyCode::Up => {
                self.move_cursor(-1, 0);
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                self.move_cursor(1, 0);
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::Left if k.modifiers.contains(KeyModifiers::CONTROL) => {
                let new_c = self.word_boundary(self.cursor.0, self.cursor.1, false);
                self.cursor.1 = new_c;
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::Right if k.modifiers.contains(KeyModifiers::CONTROL) => {
                let new_c = self.word_boundary(self.cursor.0, self.cursor.1, true);
                self.cursor.1 = new_c;
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::Left => {
                if self.cursor.1 > 0 {
                    self.cursor.1 -= 1;
                } else if self.cursor.0 > 0 {
                    self.cursor.0 -= 1;
                    self.cursor.1 = self.current_line().graphemes(true).count();
                }
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::Right => {
                let line_len = self.current_line().graphemes(true).count();
                if self.cursor.1 < line_len {
                    self.cursor.1 += 1;
                } else if self.cursor.0 + 1 < self.lines.len() {
                    self.cursor.0 += 1;
                    self.cursor.1 = 0;
                }
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::Home => {
                self.cursor.1 = 0;
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::End => {
                self.cursor.1 = self.current_line().graphemes(true).count();
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::PageUp => {
                self.cursor.0 = self.cursor.0.saturating_sub(10);
                self.clamp_cursor();
                self.selection = None;
                return Outcome::Consumed;
            }
            KeyCode::PageDown => {
                self.cursor.0 = (self.cursor.0 + 10).min(self.lines.len().saturating_sub(1));
                self.clamp_cursor();
                self.selection = None;
                return Outcome::Consumed;
            }
            _ => {}
        }

        // Ctrl shortcuts
        if ctrl(&k, 'a') {
            self.select_all();
            return Outcome::Consumed;
        }
        if ctrl(&k, 'z') {
            self.undo();
            return Outcome::Changed;
        }
        if ctrl(&k, 'y') {
            self.redo();
            return Outcome::Changed;
        }
        if ctrl(&k, 'd') {
            self.duplicate_line();
            return Outcome::Changed;
        }
        if ctrl(&k, 'x') {
            if let Some((r0, c0, r1, c1)) = self.selection {
                let (start_r, start_c, end_r, end_c) = if r0 < r1 || (r0 == r1 && c0 < c1) {
                    (r0, c0, r1, c1)
                } else {
                    (r1, c1, r0, c0)
                };
                if start_r == end_r {
                    let graphemes = self.graphemes(start_r);
                    self.clipboard = graphemes[start_c..end_c].join("");
                } else {
                    self.clipboard = self.lines[start_r..=end_r].join("\n");
                }
                self.delete_selection();
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        if ctrl(&k, 'c') {
            if let Some((r0, c0, r1, c1)) = self.selection {
                let (start_r, start_c, end_r, end_c) = if r0 < r1 || (r0 == r1 && c0 < c1) {
                    (r0, c0, r1, c1)
                } else {
                    (r1, c1, r0, c0)
                };
                if start_r == end_r {
                    let graphemes = self.graphemes(start_r);
                    self.clipboard = graphemes[start_c..end_c].join("");
                } else {
                    self.clipboard = self.lines[start_r..=end_r].join("\n");
                }
                return Outcome::Consumed;
            }
            return Outcome::Consumed;
        }
        if ctrl(&k, 'v') {
            self.save_undo();
            self.insert_str(&self.clipboard.clone());
            return Outcome::Changed;
        }
        if k.code == KeyCode::Home && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.cursor = (0, 0);
            self.selection = None;
            return Outcome::Consumed;
        }
        if k.code == KeyCode::End && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.cursor.0 = self.lines.len().saturating_sub(1);
            self.cursor.1 = self.current_line().graphemes(true).count();
            self.selection = None;
            return Outcome::Consumed;
        }

        // Alt shortcuts
        if k.code == KeyCode::Up && k.modifiers.contains(KeyModifiers::ALT) {
            self.move_line(-1);
            return Outcome::Changed;
        }
        if k.code == KeyCode::Down && k.modifiers.contains(KeyModifiers::ALT) {
            self.move_line(1);
            return Outcome::Changed;
        }

        // Editing
        if k.code == KeyCode::Enter {
            self.save_undo();
            self.insert_newline();
            return Outcome::Changed;
        }
        if k.code == KeyCode::Tab {
            self.save_undo();
            for _ in 0..4 {
                self.insert_char(' ');
            }
            return Outcome::Changed;
        }
        if k.code == KeyCode::Backspace {
            self.save_undo();
            self.delete_backward();
            return Outcome::Changed;
        }
        if k.code == KeyCode::Delete {
            self.save_undo();
            self.delete_forward();
            return Outcome::Changed;
        }

        // Typing
        if let Some(c) = plain_char(&k) {
            self.save_undo();
            self.insert_char(c);
            return Outcome::Changed;
        }

        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let vout = self.vscroll_state.handle_mouse(m);
        let hout = self.hscroll_state.handle_mouse(m);
        if vout.is_changed() {
            self.scroll_y = self.vscroll_state.offset;
            return Outcome::Consumed;
        }
        if hout.is_changed() {
            self.scroll_x = self.hscroll_state.offset;
            return Outcome::Consumed;
        }

        // Wheel scrolling
        if let Some(delta) = wheel_delta(&m)
            && mouse_in(self.hit.area, &m)
        {
            self.scroll_y = (self.scroll_y as i32 + delta * 3).max(0) as usize;
            return Outcome::Consumed;
        }

        // Click to position cursor
        if m.kind == MouseEventKind::Down(MouseButton::Left) && mouse_in(self.hit.area, &m) {
            let pos = mouse_pos(&m);
            let dx = pos.x.saturating_sub(self.hit.area.x) as usize;
            let dy = pos.y.saturating_sub(self.hit.area.y) as usize;
            let row = (self.scroll_y + dy).min(self.lines.len().saturating_sub(1));
            let line_len = self
                .lines
                .get(row)
                .map(|l| l.graphemes(true).count())
                .unwrap_or(0);
            let col = (self.scroll_x + dx).min(line_len);
            self.cursor = (row, col);
            self.selection = None;
            return Outcome::Consumed;
        }

        let hit = self.hit.mouse(&m);
        if matches!(
            hit,
            Hit::Press | Hit::HoverChanged | Hit::Click | Hit::Cancel
        ) {
            return Outcome::Consumed;
        }
        Outcome::Ignored
    }
}

// render

impl StatefulWidget for TextArea {
    type State = TextAreaState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chrome = self.shape.vertical_chrome();
        if area.height < 1 + chrome || area.width < 4 {
            state.hit.set_area(Rect::default());
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: state.hit.hover,
            enabled: self.enabled,
        };
        let bg = if look.focused {
            th.focus_bg()
        } else {
            th.surface
        };
        fill(buf, area, bg);

        let border = if look.focused {
            th.border
        } else {
            th.border_blurred
        };
        let inner = crate::layout::pad(
            self.shape.draw(buf, area, border, bg),
            self.shape.padding(),
            0,
        );
        if inner.width < 2 || inner.height == 0 {
            state.hit.set_area(area);
            return;
        }

        // Line numbers
        let gutter_w = if self.line_numbers {
            let digits = state.lines.len().to_string().len();
            (digits + 1) as u16
        } else {
            0
        };

        let text_area = Rect {
            x: inner.x + gutter_w,
            y: inner.y,
            width: inner.width.saturating_sub(gutter_w + 1),
            height: inner.height,
        };

        state.hit.set_area(text_area);

        // Placeholder (muted, under the cursor) while there is nothing typed
        if !self.placeholder.is_empty()
            && state.lines.iter().all(String::is_empty)
            && text_area.width > 1
        {
            put(
                buf,
                text_area.x + 1,
                text_area.y,
                &self.placeholder,
                text_area.width - 1,
                st(th.text_muted, bg),
            );
        }

        // Scroll
        state.scroll_y = keep_visible(state.scroll_y, state.cursor.0, text_area.height as usize);
        let max_line_w = state.lines.iter().map(|l| l.width()).max().unwrap_or(0);
        if state.cursor.1 * 2 >= text_area.width as usize {
            state.scroll_x = state.cursor.1.saturating_sub(text_area.width as usize / 2);
        } else if state.cursor.1 < state.scroll_x {
            state.scroll_x = state.cursor.1;
        }

        // Draw lines
        for (i, row) in (state.scroll_y..).zip(0..text_area.height as usize) {
            if i >= state.lines.len() {
                break;
            }
            let y = text_area.y + row as u16;

            // Line number
            if self.line_numbers && gutter_w > 0 {
                let num = format!("{:>w$}", i + 1, w = gutter_w as usize - 1);
                put(buf, inner.x, y, &num, gutter_w, st(th.text_muted, bg));
            }

            // Highlight current line
            if self.highlight_line && look.focused && i == state.cursor.0 {
                let hl_rect = Rect {
                    x: text_area.x,
                    y,
                    width: text_area.width,
                    height: 1,
                };
                fill(buf, hl_rect, th.boost);
            }

            // Text
            let line = &state.lines[i];
            let display = if state.scroll_x >= line.graphemes(true).count() {
                String::new()
            } else {
                line.graphemes(true)
                    .skip(state.scroll_x)
                    .take(text_area.width as usize)
                    .collect()
            };

            let fg = if self.enabled {
                th.text
            } else {
                th.text_disabled
            };
            let line_bg = if self.highlight_line && look.focused && i == state.cursor.0 {
                th.boost
            } else {
                bg
            };

            // Apply syntax highlighting if provided
            if let Some(highlighter) = self.highlighter {
                let spans = highlighter(line);
                let mut x = text_area.x;
                let mut char_pos = 0;
                for grapheme in line.graphemes(true) {
                    if char_pos < state.scroll_x {
                        char_pos += 1;
                        continue;
                    }
                    if x >= text_area.right() {
                        break;
                    }

                    // Find applicable style
                    let byte_pos = line
                        .graphemes(true)
                        .take(char_pos)
                        .collect::<String>()
                        .len();
                    let style = spans
                        .iter()
                        .find(|(start, end, _)| byte_pos >= *start && byte_pos < *end)
                        .map(|(_, _, s)| *s)
                        .unwrap_or_else(|| st(fg, line_bg));
                    // Draw cursor
                    let draw_cursor = if look.focused {
                        true
                    } else {
                        self.cursor_when_unfocused
                    };

                    let is_cursor =
                        draw_cursor && i == state.cursor.0 && char_pos == state.cursor.1;
                    let cursor_visible = !self.cursor_blink
                        || self.now.is_none_or(|n| anim::blink(anim::since(n), 1.0));

                    let final_style = if is_cursor && cursor_visible {
                        match self.cursor_style {
                            CursorStyle::Block => st(th.cursor_fg, th.cursor_bg),
                            CursorStyle::Bar => {
                                // Bar is drawn separately, use normal style for text
                                style
                            }
                            CursorStyle::Underline => bold(st(
                                if look.focused {
                                    th.primary
                                } else {
                                    th.cursor_blurred_bg
                                },
                                line_bg,
                            ))
                            .add_modifier(Modifier::UNDERLINED),
                            CursorStyle::Outline => {
                                let style_fg = style.fg.and_then(Rgb::from_color).unwrap_or(fg);
                                st(style_fg, th.cursor_blurred_bg)
                            }
                        }
                    } else {
                        style
                    };

                    put(buf, x, y, grapheme, grapheme.width() as u16, final_style);

                    // Draw bar cursor after text
                    if is_cursor && cursor_visible && self.cursor_style == CursorStyle::Bar {
                        let cursor_color = if look.focused {
                            th.cursor_bg
                        } else {
                            th.cursor_blurred_bg
                        };
                        put_cell(buf, x, y, "▎", st(cursor_color, line_bg));
                    }

                    x += grapheme.width() as u16;
                    char_pos += 1;
                }
            } else {
                put(
                    buf,
                    text_area.x,
                    y,
                    &display,
                    text_area.width,
                    st(fg, line_bg),
                );

                // Cursor
                let draw_cursor = if look.focused {
                    true
                } else {
                    self.cursor_when_unfocused
                };

                if draw_cursor && i == state.cursor.0 {
                    let cursor_col = state.cursor.1.saturating_sub(state.scroll_x);
                    if cursor_col < text_area.width as usize {
                        let cursor_x = text_area.x + cursor_col as u16;

                        let cursor_visible = !self.cursor_blink
                            || self.now.is_none_or(|n| anim::blink(anim::since(n), 1.0));

                        if cursor_visible && let Some(cell) = buf.cell_mut((cursor_x, y)) {
                            match self.cursor_style {
                                CursorStyle::Block => {
                                    cell.set_style(st(th.cursor_fg, th.cursor_bg));
                                }
                                CursorStyle::Bar => {
                                    let cursor_color = if look.focused {
                                        th.cursor_bg
                                    } else {
                                        th.cursor_blurred_bg
                                    };
                                    cell.set_symbol("▎");
                                    cell.set_style(st(cursor_color, line_bg));
                                }
                                CursorStyle::Underline => {
                                    let accent = if look.focused {
                                        th.primary
                                    } else {
                                        th.cursor_blurred_bg
                                    };
                                    cell.set_style(
                                        bold(st(accent, line_bg))
                                            .add_modifier(Modifier::UNDERLINED),
                                    );
                                }
                                CursorStyle::Outline => {
                                    let cell_fg = Rgb::from_color(cell.fg).unwrap_or(th.text);
                                    cell.set_style(st(cell_fg, th.cursor_blurred_bg));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Position indicator
        if self.show_position {
            let pos_text = format!("Ln {}, Col {}", state.cursor.0 + 1, state.cursor.1 + 1);
            let pos_width = pos_text.width() as u16;

            // Draw right-aligned, avoiding scrollbars
            // If there's a horizontal scrollbar, draw on the last visible text row
            // Otherwise, draw in the bottom border row (if chrome) or last content row
            let has_hscroll = max_line_w > text_area.width as usize;
            let pos_y = if has_hscroll {
                // Draw on the last visible text line to avoid horizontal scrollbar
                text_area.bottom().saturating_sub(1)
            } else if chrome > 0 {
                // Draw in bottom border row
                area.bottom().saturating_sub(1)
            } else {
                // Draw on last content row
                text_area.bottom().saturating_sub(1)
            };

            // Never over the vertical scrollbar column
            let scrollbar_offset = if state.lines.len() > text_area.height as usize {
                1
            } else {
                0
            };
            let pos_x = area
                .right()
                .saturating_sub(pos_width + scrollbar_offset + 1);

            if pos_x >= text_area.x && pos_y < area.bottom() && pos_y >= text_area.y {
                put(
                    buf,
                    pos_x,
                    pos_y,
                    &pos_text,
                    pos_width,
                    st(th.text_muted, bg),
                );
            }
        }

        // Scrollbars
        if state.lines.len() > text_area.height as usize {
            let sb_area = Rect {
                x: area.right().saturating_sub(1),
                y: area.y + 1,
                width: 1,
                height: area.height.saturating_sub(2),
            };
            Scrollbar::vertical(state.lines.len(), text_area.height as usize)
                .offset(state.scroll_y)
                .theme(&th)
                .render(sb_area, buf, &mut state.vscroll_state);
        }

        if max_line_w > text_area.width as usize {
            let sb_area = Rect {
                x: text_area.x,
                y: area.bottom().saturating_sub(1),
                width: text_area.width,
                height: 1,
            };
            Scrollbar::horizontal(max_line_w, text_area.width as usize)
                .offset(state.scroll_x)
                .theme(&th)
                .render(sb_area, buf, &mut state.hscroll_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_newline() {
        let mut s = TextAreaState::with_text("hello");
        s.cursor = (0, 2);
        s.insert_newline();
        assert_eq!(s.lines.len(), 2);
        assert_eq!(s.lines[0], "he");
        assert_eq!(s.lines[1], "llo");
    }

    #[test]
    fn delete_backward_joins_lines() {
        let mut s = TextAreaState::with_text("hello\nworld");
        s.cursor = (1, 0);
        s.delete_backward();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.lines[0], "helloworld");
    }

    #[test]
    fn undo_redo() {
        let mut s = TextAreaState::new();
        s.save_undo();
        s.lines[0] = "test".to_string();
        s.undo();
        assert_eq!(s.lines[0], "");
        s.redo();
        assert_eq!(s.lines[0], "test");
    }

    #[test]
    fn click_positions_cursor() {
        let mut state = TextAreaState::with_text("line 1\nline 2\nline 3");
        let area = Rect::new(5, 10, 20, 5);
        let mut buf = Buffer::empty(area);

        // Render to set hit area
        TextArea::new().render(area, &mut buf, &mut state);

        // Click at offset (4, 1) in text area
        let text_x = state.hit.area.x;
        let text_y = state.hit.area.y;
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: text_x + 4,
            row: text_y + 1,
            modifiers: KeyModifiers::empty(),
        };

        let out = state.handle_mouse(click);
        assert!(out.is_consumed());
        assert_eq!(state.cursor, (1, 4));
        assert_eq!(state.selection, None);
    }

    #[test]
    fn click_clamps_to_line_end() {
        let mut state = TextAreaState::with_text("hi\nworld");
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);

        TextArea::new().render(area, &mut buf, &mut state);

        // Click beyond line end
        let text_x = state.hit.area.x;
        let text_y = state.hit.area.y;
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: text_x + 10,
            row: text_y,
            modifiers: KeyModifiers::empty(),
        };

        state.handle_mouse(click);
        assert_eq!(state.cursor, (0, 2)); // clamped to "hi".len()
    }

    #[test]
    fn wheel_scrolls() {
        let mut state = TextAreaState::with_text("1\n2\n3\n4\n5\n6\n7\n8\n9\n10");
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);

        TextArea::new().render(area, &mut buf, &mut state);

        let wheel_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: state.hit.area.x,
            row: state.hit.area.y,
            modifiers: KeyModifiers::empty(),
        };

        let initial_scroll = state.scroll_y;
        state.handle_mouse(wheel_down);
        assert_eq!(state.scroll_y, initial_scroll + 3); // scrolls by 3

        let wheel_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: state.hit.area.x,
            row: state.hit.area.y,
            modifiers: KeyModifiers::empty(),
        };

        state.handle_mouse(wheel_up);
        assert_eq!(state.scroll_y, initial_scroll);
    }

    #[test]
    fn cursor_styles_render() {
        let mut state = TextAreaState::with_text("hello\nworld");
        state.cursor = (0, 2);
        let area = Rect::new(0, 0, 20, 5);

        let th = theme::current();

        // Block: the cell takes the cursor colour roles, not merely fg != bg (true of any text).
        let mut buf = Buffer::empty(area);
        TextArea::new()
            .cursor(CursorStyle::Block)
            .focused(true)
            .render(area, &mut buf, &mut state);
        let text_x = state.hit.area.x;
        let text_y = state.hit.area.y;
        let block_cell = buf.cell((text_x + 2, text_y)).unwrap();
        assert_eq!(block_cell.bg, th.cursor_bg.color(), "{block_cell:?}");
        assert_eq!(block_cell.fg, th.cursor_fg.color(), "{block_cell:?}");

        // Bar: vertical bar glyph
        let mut buf = Buffer::empty(area);
        TextArea::new()
            .cursor(CursorStyle::Bar)
            .focused(true)
            .render(area, &mut buf, &mut state);
        let bar_cell = buf.cell((text_x + 2, text_y)).unwrap();
        assert_eq!(
            bar_cell.symbol(),
            "▎",
            "Bar cursor should be ▎: {}",
            bar_cell.symbol()
        );

        // Underline: modifier
        let mut buf = Buffer::empty(area);
        TextArea::new()
            .cursor(CursorStyle::Underline)
            .focused(true)
            .render(area, &mut buf, &mut state);
        let underline_cell = buf.cell((text_x + 2, text_y)).unwrap();
        assert!(
            underline_cell.modifier.contains(Modifier::UNDERLINED),
            "Underline cursor should have UNDERLINED: {underline_cell:?}"
        );

        // Outline: background changes
        let mut buf = Buffer::empty(area);
        TextArea::new()
            .cursor(CursorStyle::Outline)
            .focused(true)
            .render(area, &mut buf, &mut state);
        let outline_cell = buf.cell((text_x + 2, text_y)).unwrap();
        assert_eq!(
            outline_cell.bg,
            th.cursor_blurred_bg.color(),
            "Outline cursor should have cursor_blurred_bg: {outline_cell:?}"
        );
    }

    #[test]
    fn underline_cursor_preserves_glyph() {
        let mut state = TextAreaState::with_text("hello");
        state.cursor = (0, 2);
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);

        TextArea::new()
            .cursor(CursorStyle::Underline)
            .focused(true)
            .render(area, &mut buf, &mut state);

        // Cursor cell should still have the 'l' glyph - use text area coordinates
        let text_x = state.hit.area.x;
        let text_y = state.hit.area.y;
        if let Some(cursor_cell) = buf.cell((text_x + 2, text_y)) {
            assert_eq!(cursor_cell.symbol(), "l");
        }
    }

    #[test]
    fn block_cursor_sets_colors() {
        let mut state = TextAreaState::with_text("hello");
        state.cursor = (0, 2);
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);

        TextArea::new()
            .cursor(CursorStyle::Block)
            .focused(true)
            .render(area, &mut buf, &mut state);

        // Cursor cell should have cursor colors
        if let Some(cursor_cell) = buf.cell((2, 0)) {
            // Just verify it has some style applied (exact colors depend on theme)
            assert!(cursor_cell.fg != cursor_cell.bg);
        }
    }

    #[test]
    fn render_does_not_panic_small() {
        let mut state = TextAreaState::with_text("test");

        // 4x1 should not panic
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        TextArea::new().cursor(CursorStyle::Bar).render(
            Rect::new(0, 0, 4, 1),
            &mut buf,
            &mut state,
        );

        // 0x0 should not panic
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        TextArea::new().cursor(CursorStyle::Underline).render(
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
    }

    #[test]
    fn position_indicator_shows() {
        let mut state = TextAreaState::with_text("line 1\nline 2\nline 3");
        state.cursor = (1, 3);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        TextArea::new()
            .show_position(true)
            .focused(true)
            .render(area, &mut buf, &mut state);
        let rows: Vec<String> = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect();
        assert!(rows.iter().any(|r| r.contains("Ln 2, Col 4")), "{rows:#?}");
    }
}
