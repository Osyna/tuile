//! Modal dialogs: confirm, alert, prompt, with backdrop dimming and keyboard navigation.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 10);
//! # let mut buf = Buffer::empty(area);
//! # let mut state = ModalState::default();
//! Modal::confirm("Delete file?", "This cannot be undone").render(area, &mut buf, &mut state);
//! if let Some(result) = state.take_result() { /* 0 = cancel, 1 = confirm */ }
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;

use crate::anim::Easing;
use crate::core::{HitBox, Hit, Outcome, Interactive, is_press, is_activate};
use crate::draw::{fill, put, put_centered, wrap, Border, st, bold, blend_area};
use crate::layout::center;
use crate::theme::{self, Theme, Variant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalKind {
    Dialog,
    Sheet,
    Fullscreen,
}

/// Modal dialog builder.
pub struct Modal {
    title: String,
    body: String,
    buttons: Vec<(String, Variant)>,
    width: u16,
    kind: ModalKind,
    dim: f32,
    icon: Option<String>,
    close_on_escape: bool,
    close_on_backdrop: bool,
    cancel_index: usize,
    default_button: usize,
    prompt: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl Modal {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: String::new(),
            buttons: Vec::new(),
            width: 48,
            kind: ModalKind::Dialog,
            dim: 0.6,
            icon: None,
            close_on_escape: true,
            close_on_backdrop: true,
            cancel_index: 0,
            default_button: 0,
            prompt: false,
            now: None,
            theme: None,
        }
    }

    pub fn body(mut self, text: impl Into<String>) -> Self {
        self.body = text.into();
        self
    }

    pub fn buttons(mut self, btns: &[(&str, Variant)]) -> Self {
        self.buttons = btns.iter().map(|(s, v)| (s.to_string(), *v)).collect();
        self
    }

    pub fn width(mut self, w: u16) -> Self {
        self.width = w.clamp(24, 80);
        self
    }

    pub fn kind(mut self, k: ModalKind) -> Self {
        self.kind = k;
        self
    }

    pub fn dim(mut self, f: f32) -> Self {
        self.dim = f.clamp(0.0, 1.0);
        self
    }

    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(i.into());
        self
    }

    pub fn close_on_escape(mut self, v: bool) -> Self {
        self.close_on_escape = v;
        self
    }

    pub fn close_on_backdrop(mut self, v: bool) -> Self {
        self.close_on_backdrop = v;
        self
    }

    pub fn cancel_index(mut self, i: usize) -> Self {
        self.cancel_index = i;
        self
    }

    pub fn default_button(mut self, i: usize) -> Self {
        self.default_button = i;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn confirm(title: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(title).body(msg).buttons(&[("Cancel", Variant::Default), ("Confirm", Variant::Primary)]).cancel_index(0).default_button(1)
    }

    pub fn alert(title: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(title).body(msg).buttons(&[("OK", Variant::Primary)]).default_button(0)
    }

    /// Dialog with a single-line text field above the buttons; read `state.input_text`.
    pub fn prompt(title: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(title).body(msg).buttons(&[("Cancel", Variant::Default), ("OK", Variant::Primary)]).cancel_index(0).default_button(1).with_input(true)
    }

    /// Show a text field (prompt dialog); `Tab`/arrows move between field and buttons.
    pub fn with_input(mut self, v: bool) -> Self {
        self.prompt = v;
        self
    }
}

/// State for a modal: open/closed, focus, result, input text, cached config.
#[derive(Clone, Debug)]
pub struct ModalState {
    pub open: bool,
    focus: usize,
    pub result: Option<usize>,
    button_hits: Vec<HitBox>,
    backdrop_hit: HitBox,
    opened_at: Option<Instant>,
    pub input_text: String,
    input_cursor: usize,
    // config cached from the last render so event handling needs no builder
    button_count: usize,
    is_prompt: bool,
    cancel_index: usize,
    close_on_escape: bool,
    close_on_backdrop: bool,
    /// Set by `open()`, cleared by the first render, which applies `default_button`.
    fresh: bool,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            open: false,
            focus: 0,
            result: None,
            button_hits: Vec::new(),
            backdrop_hit: HitBox::default(),
            opened_at: None,
            input_text: String::new(),
            input_cursor: 0,
            button_count: 0,
            is_prompt: false,
            cancel_index: 0,
            close_on_escape: true,
            close_on_backdrop: true,
            fresh: false,
        }
    }
}

impl ModalState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, now: Instant) {
        self.open = true;
        self.opened_at = Some(now);
        self.result = None;
        self.fresh = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.input_text.clear();
        self.input_cursor = 0;
    }

    pub fn take_result(&mut self) -> Option<usize> {
        self.result.take()
    }


    fn next_button(&mut self) {
        let total = if self.is_prompt { self.button_count + 1 } else { self.button_count };
        if total > 0 {
            self.focus = (self.focus + 1) % total;
        }
    }

    fn prev_button(&mut self) {
        let total = if self.is_prompt { self.button_count + 1 } else { self.button_count };
        if total > 0 {
            self.focus = if self.focus == 0 { total - 1 } else { self.focus - 1 };
        }
    }

    fn press_button(&mut self, index: usize) {
        self.result = Some(index);
        self.open = false;
    }

}

impl Interactive for ModalState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        if self.is_prompt && self.focus == self.button_count {
            // Editing input
            match k.code {
                KeyCode::Char(c) => {
                    self.input_text.insert(self.input_cursor, c);
                    self.input_cursor += 1;
                    return Outcome::Consumed;
                }
                KeyCode::Backspace => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                        self.input_text.remove(self.input_cursor);
                    }
                    return Outcome::Consumed;
                }
                KeyCode::Left => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                    }
                    return Outcome::Consumed;
                }
                KeyCode::Right => {
                    if self.input_cursor < self.input_text.len() {
                        self.input_cursor += 1;
                    }
                    return Outcome::Consumed;
                }
                KeyCode::Home => {
                    self.input_cursor = 0;
                    return Outcome::Consumed;
                }
                KeyCode::End => {
                    self.input_cursor = self.input_text.len();
                    return Outcome::Consumed;
                }
                KeyCode::Tab => {
                    self.next_button();
                    return Outcome::Consumed;
                }
                KeyCode::BackTab => {
                    self.prev_button();
                    return Outcome::Consumed;
                }
                KeyCode::Enter => {
                    self.press_button(1);
                    return Outcome::Changed;
                }
                KeyCode::Esc if self.close_on_escape => {
                    self.press_button(self.cancel_index);
                    return Outcome::Changed;
                }
                _ => return Outcome::Ignored,
            }
        }

        match k.code {
            KeyCode::Left => {
                self.prev_button();
                Outcome::Consumed
            }
            KeyCode::Right => {
                self.next_button();
                Outcome::Consumed
            }
            KeyCode::Tab => {
                self.next_button();
                Outcome::Consumed
            }
            KeyCode::BackTab => {
                self.prev_button();
                Outcome::Consumed
            }
            _ if is_activate(&k) => {
                self.press_button(self.focus);
                Outcome::Changed
            }
            KeyCode::Esc if self.close_on_escape => {
                self.press_button(self.cancel_index);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.button_hits.iter_mut().enumerate() {
            let h = hit.mouse(&m);
            if matches!(h, Hit::Press) {
                self.press_button(i);
                return Outcome::Changed;
            } else if hit.hover {
                self.focus = i;
            }
        }

        if self.close_on_backdrop {
            let h = self.backdrop_hit.mouse(&m);
            if matches!(h, Hit::Press) {
                self.press_button(self.cancel_index);
                return Outcome::Changed;
            }
        }

        Outcome::Ignored
    }
}


impl StatefulWidget for Modal {
    type State = ModalState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.open || area.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);

        // Backdrop
        blend_area(buf, area, th.background, self.dim);
        state.backdrop_hit.set_area(area);

        let is_prompt = self.prompt;
        // Cache config for `Interactive` before any early return (the open animation's first
        // frames return early) so keys work from the very first event.
        state.button_count = self.buttons.len();
        state.is_prompt = is_prompt;
        state.cancel_index = self.cancel_index;
        state.close_on_escape = self.close_on_escape;
        state.close_on_backdrop = self.close_on_backdrop;
        if state.fresh {
            state.fresh = false;
            let slots = self.buttons.len() + usize::from(is_prompt);
            state.focus = if is_prompt { self.buttons.len() } else { self.default_button.min(slots.saturating_sub(1)) };
        }

        let width = match self.kind {
            ModalKind::Dialog => self.width.min(area.width),
            ModalKind::Sheet => area.width.saturating_sub(8).max(self.width.min(area.width)),
            ModalKind::Fullscreen => area.width.saturating_sub(4),
        };
        let inner_w = width.saturating_sub(6);
        let body_lines = wrap(&self.body, inner_w as usize);
        let prompt_h = if is_prompt { 4 } else { 0 };
        let buttons_h = if self.buttons.is_empty() { 0 } else { 4 };
        let icon_h = if self.icon.is_some() { 2 } else { 0 };
        let h = (2 + 1 + icon_h + 1 + body_lines.len() as u16 + if body_lines.is_empty() { 0 } else { 1 } + prompt_h + buttons_h + 1).min(area.height);

        let modal_area = match self.kind {
            ModalKind::Dialog => center(area, width, h),
            // anchored to the bottom edge, wide, keeps the page context visible above
            ModalKind::Sheet => Rect { x: area.x + (area.width - width) / 2, y: area.bottom().saturating_sub(h), width, height: h },
            ModalKind::Fullscreen => crate::layout::pad(area, 2, 1),
        };
        if modal_area.is_empty() {
            return;
        }

        let bg = th.surface;
        fill(buf, modal_area, bg);

        let border_color = if let Some((_, v)) = self.buttons.last() {
            th.variant(*v)
        } else {
            th.primary
        };
        Border::Thick.draw(buf, modal_area, border_color, bg);

        // Animate scale
        let scale_p = if let Some(opened) = state.opened_at {
            let age = now.saturating_duration_since(opened).as_secs_f32();
            Easing::OutCubic.apply((age / 0.12).min(1.0))
        } else {
            1.0
        };

        if scale_p < 0.99 {
            let shrink = ((1.0 - scale_p) * h as f32 / 2.0) as u16;
            let shrunk = Rect {
                y: modal_area.y + shrink,
                height: modal_area.height.saturating_sub(shrink * 2),
                ..modal_area
            };
            if shrunk.height < 3 {
                return;
            }
            blend_area(buf, modal_area, bg, 0.8);
            fill(buf, shrunk, bg);
            Border::Thick.draw(buf, shrunk, border_color, bg);
        }

        let cx = modal_area.x + 3;
        let mut cy = modal_area.y + 2;

        if let Some(icon_str) = &self.icon {
            put_centered(buf, Rect { x: cx, y: cy, width: inner_w, height: 1 }, icon_str, st(border_color, bg));
            cy += 2;
        }

        put(buf, cx, cy, &self.title, inner_w, bold(st(th.text, bg)));
        cy += 2;

        for line in &body_lines {
            put(buf, cx, cy, line, inner_w, st(th.text_muted, bg));
            cy += 1;
        }
        if !body_lines.is_empty() {
            cy += 1;
        }

        if is_prompt {
            let input_area = Rect { x: cx, y: cy, width: inner_w, height: 1 };
            let cursor_x = state.input_cursor.min(state.input_text.len());
            let display_text = if state.input_text.is_empty() { "..." } else { &state.input_text };
            
            fill(buf, input_area, th.focus_bg());
            Border::Tall.draw(buf, Rect { x: cx - 1, y: cy - 1, width: inner_w + 2, height: 3 }, th.border, th.focus_bg());
            
            put(buf, cx, cy, display_text, inner_w, st(th.text, th.focus_bg()));
            
            if state.focus == self.buttons.len() {
                let cursor_char = if cursor_x < state.input_text.len() {
                    state.input_text.chars().nth(cursor_x).unwrap_or(' ')
                } else {
                    ' '
                };
                put(buf, cx + cursor_x as u16, cy, &cursor_char.to_string(), 1, st(th.cursor_fg, th.cursor_bg));
            }
            
        }

        state.button_hits.clear();
        if !self.buttons.is_empty() {
            let by = modal_area.bottom().saturating_sub(4);
            let mut bx = modal_area.right().saturating_sub(3);

            for (i, (label, variant)) in self.buttons.iter().enumerate().rev() {
                let bw = (label.len() as u16 + 4).max(16);
                bx = bx.saturating_sub(bw);
                let br = Rect { x: bx, y: by, width: bw, height: 3 };

                draw_button(buf, br, &th, label, *variant, state.focus == i);
                state.button_hits.insert(0, {
                    let mut h = HitBox::default();
                    h.set_area(br);
                    h
                });

                bx = bx.saturating_sub(2);
            }
        }

    }
}

fn draw_button(buf: &mut Buffer, area: Rect, th: &Theme, label: &str, variant: Variant, focused: bool) {
    if area.height < 3 || area.width < 4 {
        return;
    }

    let (bg, fg) = if focused {
        (th.variant(variant), th.background)
    } else {
        (th.surface.blend(th.variant(variant), 0.2), th.text)
    };

    fill(buf, area, bg);
    
    // 3D effect
    let top_color = bg.lighten(0.15);
    let bottom_color = bg.darken(0.15);
    
    for x in area.left()..area.right() {
        put(buf, x, area.top(), "▔", 1, st(top_color, bg));
        put(buf, x, area.bottom() - 1, "▁", 1, st(bottom_color, bg));
    }

    let style = if focused {
        bold(st(fg, bg)).add_modifier(Modifier::REVERSED)
    } else {
        st(fg, bg)
    };

    put_centered(buf, Rect { y: area.y + 1, height: 1, ..area }, label, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(modal: Modal) -> ModalState {
        let mut state = ModalState::new();
        state.open(Instant::now());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        modal.render(buf.area, &mut buf, &mut state);
        state
    }

    #[test]
    fn modal_button_navigation_wraps_and_reports_result() {
        let mut state = opened(Modal::new("Test").buttons(&[("Cancel", Variant::Default), ("OK", Variant::Primary)]));
        assert_eq!(state.focus, 0);
        state.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(state.focus, 1);
        state.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(state.focus, 0);
        state.handle_key(KeyEvent::from(KeyCode::Right));
        state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(state.take_result(), Some(1));
        // Esc reports the cancel index
        let mut state = opened(Modal::confirm("Sure?", "really").cancel_index(0));
        state.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(state.take_result(), Some(0));
    }

    #[test]
    fn modal_prompt_edits_text_when_field_focused() {
        let mut state = opened(Modal::prompt("Rename", "New name"));
        // the field sits after the two buttons in the focus ring
        state.focus = 2;
        state.handle_key(KeyEvent::from(KeyCode::Char('a')));
        state.handle_key(KeyEvent::from(KeyCode::Char('b')));
        state.handle_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(state.input_text, "a");
        assert_eq!(state.input_cursor, 1);
    }
}
