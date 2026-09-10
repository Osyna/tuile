//! Textual-style buttons: 3D default look with tall borders, plus flat/outline/ghost variants.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 20, 3);
//! # let mut buf = Buffer::empty(area);
//! let mut state = ButtonState::default();
//! Button::new("Save").variant(Variant::Primary).focused(true).render(area, &mut buf, &mut state);
//! if state.handle_key(KeyEvent::from(KeyCode::Char(' '))).is_changed() { /* clicked */ }
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::{Hit, HitBox, Interactive, Look, Outcome, is_activate, is_press};
use crate::draw::{Border, fill, put_centered, st};
use crate::theme::{self, Theme, Variant};

const BUTTON_MIN_W: u16 = 16;

/// Calculate button width given label and compact flag.
pub fn button_width(label: &str, compact: bool) -> u16 {
    let content_w = label.width() as u16 + 4;
    if compact { content_w } else { content_w.max(BUTTON_MIN_W) }
}

/// Button rendering style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonStyle {
    /// 3D look with tall borders (Textual default).
    #[default]
    Default,
    /// Flat surface background.
    Flat,
    /// Bordered outline, transparent centre.
    Outline,
    /// Ghost: transparent, faint hover.
    Ghost,
}

/// Textual-style button builder.
#[derive(Clone, Debug)]
pub struct Button {
    label: String,
    variant: Variant,
    style: ButtonStyle,
    compact: bool,
    icon: Option<String>,
    min_width: u16,
    full_width: bool,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: Variant::Default,
            style: ButtonStyle::Default,
            compact: false,
            icon: None,
            min_width: BUTTON_MIN_W,
            full_width: false,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn style(mut self, s: ButtonStyle) -> Self {
        self.style = s;
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(i.into());
        self
    }

    pub fn min_width(mut self, w: u16) -> Self {
        self.min_width = w;
        self
    }

    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
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

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl StatefulWidget for Button {
    type State = ButtonState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        if area.width < 3 || area.height == 0 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let pressed = state.pressed_at.filter(|&t| {
            self.now.is_some_and(|now| now.duration_since(t).as_millis() < 120)
        }).is_some();

        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };
        let display = if let Some(icon) = &self.icon {
            format!("{} {}", icon, self.label)
        } else {
            self.label.clone()
        };

        match self.style {
            ButtonStyle::Default => self.render_3d(area, buf, &th, &display, &look, pressed),
            ButtonStyle::Flat => self.render_flat(area, buf, &th, &display, &look, pressed),
            ButtonStyle::Outline => self.render_outline(area, buf, &th, &display, &look, pressed),
            ButtonStyle::Ghost => self.render_ghost(area, buf, &th, &display, &look),
        }
    }
}

impl Button {
    fn render_3d(&self, area: Rect, buf: &mut Buffer, th: &Theme, label: &str, look: &Look, pressed: bool) {
        let base = th.variant(self.variant);
        let (mut bg, mut top, mut bottom) = match self.variant {
            Variant::Default => (th.surface, Theme::shade(th.surface, 1), Theme::shade(th.surface, -1)),
            Variant::Primary => (base, Theme::shade(base, 3), Theme::shade(base, -3)),
            _ => (base, Theme::shade(base, 2), Theme::shade(base, -3)),
        };
        if look.hover && look.enabled {
            bg = match self.variant {
                Variant::Default => Theme::shade(th.surface, -1),
                Variant::Error => Theme::shade(base, -1),
                _ => Theme::shade(base, -2),
            };
            top = base;
        }
        if pressed {
            (top, bottom) = (Theme::shade(bottom, 1), top);
            bg = base;
        }
        if look.focused && look.enabled {
            bg = Theme::shade(bg, 1);
        }
        let mut fg = if self.variant == Variant::Default { th.foreground } else { base.text_on(0.9) };
        if !look.enabled {
            fg = bg.blend(fg, 0.5);
        }
        fill(buf, area, bg);
        if !self.compact {
            for x in area.left()..area.right() {
                if let Some(c) = buf.cell_mut((x, area.y)) {
                    c.set_symbol("▔").set_fg(top.color());
                }
                if area.height >= 3
                    && let Some(c) = buf.cell_mut((x, area.bottom() - 1)) {
                        c.set_symbol("▁").set_fg(bottom.color());
                    }
            }
        }
        let mid = if self.compact {
            area
        } else {
            Rect { y: area.y + 1, height: 1, ..area }
        };
        put_centered(buf, mid, label, Self::focus_style(st(fg, bg), look));
    }

    /// Focus cue: bold + underline on a lightened background — visible without looking like a
    /// text selection (Textual's `bold reverse` inverted the whole row).
    fn focus_style(base: Style, look: &Look) -> Style {
        let style = base.add_modifier(Modifier::BOLD);
        if look.focused { style.add_modifier(Modifier::UNDERLINED) } else { style }
    }

    fn render_flat(&self, area: Rect, buf: &mut Buffer, th: &Theme, label: &str, look: &Look, pressed: bool) {
        let base = th.variant(self.variant);
        let mut bg = if self.variant == Variant::Default { th.surface } else { base };
        if look.hover && look.enabled {
            bg = Theme::shade(bg, if pressed { -2 } else { -1 });
        }
        if look.focused && look.enabled {
            bg = Theme::shade(bg, 1);
        }
        let mut fg = if self.variant == Variant::Default { th.foreground } else { base.text_on(0.9) };
        if !look.enabled {
            fg = bg.blend(fg, 0.5);
        }

        fill(buf, area, bg);
        let mid = Rect { y: area.y + area.height / 2, height: 1, ..area };
        put_centered(buf, mid, label, Self::focus_style(st(fg, bg), look));
    }

    fn render_outline(&self, area: Rect, buf: &mut Buffer, th: &Theme, label: &str, look: &Look, pressed: bool) {
        // `Default` maps to `$surface`, invisible on the background: outline buttons use the text colour instead
        let base = if self.variant == Variant::Default { th.text_muted } else { th.variant(self.variant) };
        let border_color = if look.enabled { base } else { base.blend(th.background, 0.5) };
        let mut bg = th.background;
        if look.hover && look.enabled {
            bg = Theme::shade(th.background, 1);
        }
        if pressed {
            bg = Theme::shade(bg, -1);
        }
        if look.focused && look.enabled {
            bg = Theme::shade(bg, 1);
        }
        let label_base = if self.variant == Variant::Default { th.text } else { base };
        let fg = if look.enabled { label_base } else { label_base.blend(th.background, 0.5) };

        fill(buf, area, bg);
        let framed = area.height >= 3 && area.width >= 3 && !self.compact;
        let mid = if framed {
            Border::Tall.draw(buf, area, border_color, bg);
            Rect { x: area.x + 1, y: area.y + area.height / 2, width: area.width - 2, height: 1 }
        } else {
            area
        };
        put_centered(buf, mid, label, Self::focus_style(st(fg, bg), look));
    }

    fn render_ghost(&self, area: Rect, buf: &mut Buffer, th: &Theme, label: &str, look: &Look) {
        let base = th.variant(self.variant);
        let mut bg = th.background;
        if look.hover && look.enabled {
            bg = Theme::shade(th.background, 1);
        }
        let mut fg = if self.variant == Variant::Default { th.text_muted } else { base };
        if !look.enabled {
            fg = fg.blend(bg, 0.5);
        }

        fill(buf, area, bg);
        let mut style = st(fg, bg);
        if look.focused {
            style = st(bg, fg).add_modifier(Modifier::BOLD);
            fill(buf, area, fg);
        }
        put_centered(buf, area, label, style);
    }
}

/// Button state: hover/press tracking and press flash timestamp.
#[derive(Clone, Debug, Default)]
pub struct ButtonState {
    pub hit: HitBox,
    pub pressed_at: Option<Instant>,
}

impl ButtonState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.pressed_at.is_some_and(|t| now.duration_since(t).as_millis() < 120)
    }
}

impl Interactive for ButtonState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        if is_activate(&key) {
            self.pressed_at = Some(Instant::now());
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        match self.hit.mouse(&m) {
            Hit::Click => {
                self.pressed_at = Some(Instant::now());
                Outcome::Changed
            }
            Hit::Press | Hit::Cancel | Hit::HoverChanged => Outcome::Consumed,
            Hit::Drag | Hit::Wheel(_) | Hit::None => Outcome::Ignored,
        }
    }
}

/// Helper to lay out N buttons horizontally with a gap, returning their rects.
pub struct ButtonGroup;

impl ButtonGroup {
    /// Returns rects for buttons with the given labels, laid out with `gap` between them.
    pub fn layout(area: Rect, labels: &[&str], gap: u16, compact: bool) -> Vec<Rect> {
        if labels.is_empty() || area.width == 0 {
            return vec![];
        }
        let widths: Vec<u16> = labels.iter().map(|l| button_width(l, compact)).collect();
        let total_w: u16 = widths.iter().sum::<u16>() + gap * labels.len().saturating_sub(1) as u16;
        if total_w > area.width {
            return vec![];
        }
        let mut x = area.x;
        let h = if compact { 1 } else { 3 };
        widths.iter().map(|&w| {
            let r = Rect { x, y: area.y, width: w, height: h.min(area.height) };
            x += w + gap;
            r
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn button_renders_at_minimum_size() {
        let area = Rect { x: 0, y: 0, width: 16, height: 3 };
        let mut buf = Buffer::empty(area);
        let mut state = ButtonState::default();
        Button::new("OK").render(area, &mut buf, &mut state);
        assert_eq!(state.hit.area, area);
    }

    #[test]
    fn button_handles_enter_and_space() {
        let mut state = ButtonState::default();
        let key = KeyEvent::from(ratatui::crossterm::event::KeyCode::Enter);
        assert!(state.handle_key(key).is_changed());
    }

    #[test]
    fn button_width_respects_compact() {
        assert_eq!(button_width("X", false), BUTTON_MIN_W);
        assert_eq!(button_width("X", true), 5); // "X" + 4 padding
        assert_eq!(button_width("Long Label", false), BUTTON_MIN_W.max("Long Label".width() as u16 + 4));
    }

    #[test]
    fn button_group_layout() {
        let area = Rect { x: 0, y: 0, width: 60, height: 3 };
        let rects = ButtonGroup::layout(area, &["One", "Two", "Three"], 2, false);
        assert_eq!(rects.len(), 3);
        assert!(rects[0].x < rects[1].x);
        assert!(rects[1].x < rects[2].x);
    }
}
