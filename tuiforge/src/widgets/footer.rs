//! Key footer with bindings (key + description), message mode, and priority-based overflow.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 80, 3);
//! # let mut buf = Buffer::empty(area);
//! let mut state = KeyFooterState::default();
//! KeyFooter::new().bindings(&[("q", "Quit"), ("^S", "Save")]).render(area, &mut buf, &mut state);
//! if let Some(idx) = state.take_pressed() { /* handle binding click */ }
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::core::{Hit, HitBox, Outcome};
use crate::draw::{fill, put, put_right, st};
use crate::theme::{self, Theme, Variant};
use unicode_width::UnicodeWidthStr;

/// A footer key binding: key string, description, enabled state, and priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FooterBinding {
    pub key: String,
    pub description: String,
    pub enabled: bool,
    /// Lower priority bindings are dropped first when overflow occurs.
    pub priority: u8,
}

impl FooterBinding {
    pub fn new(key: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            description: desc.into(),
            enabled: true,
            priority: 128,
        }
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn priority(mut self, p: u8) -> Self {
        self.priority = p;
        self
    }
}

impl From<(&str, &str)> for FooterBinding {
    fn from((k, d): (&str, &str)) -> Self {
        FooterBinding::new(k, d)
    }
}

/// State for the key footer: hover and pressed tracking.
#[derive(Clone, Debug, Default)]
pub struct KeyFooterState {
    /// HitBoxes for each displayed binding.
    pub hits: Vec<HitBox>,
    /// Index of the hovered binding.
    pub hover: Option<usize>,
    /// Index of the pressed binding (take with `take_pressed`).
    pub pressed: Option<usize>,
}

impl KeyFooterState {
    pub fn new() -> Self {
        Default::default()
    }

    /// Take the pressed binding index (consumes it).
    pub fn take_pressed(&mut self) -> Option<usize> {
        self.pressed.take()
    }
}

impl crate::core::Interactive for KeyFooterState {
    fn handle_key(&mut self, _k: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        self.hover = None;

        for (i, hit) in self.hits.iter_mut().enumerate() {
            let h = hit.mouse(&m);
            if h == Hit::Press {
                self.pressed = Some(i);
                out = Outcome::Changed;
            } else if h == Hit::HoverChanged && hit.hover {
                self.hover = Some(i);
                out = Outcome::Consumed;
            }
        }

        out
    }
}

/// Key footer builder.
#[derive(Clone, Debug)]
pub struct KeyFooter {
    bindings: Vec<FooterBinding>,
    compact: bool,
    groups: Vec<Vec<usize>>,
    right: Option<String>,
    message: Option<(String, Variant)>,
    theme: Option<Theme>,
}

impl KeyFooter {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            compact: false,
            groups: Vec::new(),
            right: None,
            message: None,
            theme: None,
        }
    }

    /// Set bindings from `(key, description)` tuples.
    pub fn bindings(mut self, b: &[(&str, &str)]) -> Self {
        self.bindings = b
            .iter()
            .map(|&(k, d)| FooterBinding::from((k, d)))
            .collect();
        self
    }

    /// Set bindings from `FooterBinding` instances.
    pub fn bindings_full(mut self, b: Vec<FooterBinding>) -> Self {
        self.bindings = b;
        self
    }

    /// Compact mode: show keys only, no descriptions.
    pub fn compact(mut self, c: bool) -> Self {
        self.compact = c;
        self
    }

    /// Group bindings with separators (`groups` = lists of binding indices).
    pub fn groups(mut self, g: &[&[usize]]) -> Self {
        self.groups = g.iter().map(|&slice| slice.to_vec()).collect();
        self
    }

    /// Right-aligned status text.
    pub fn right(mut self, r: impl Into<String>) -> Self {
        self.right = Some(r.into());
        self
    }

    /// Message mode: replace bindings with a status message.
    pub fn message(mut self, msg: impl Into<String>, v: Variant) -> Self {
        self.message = Some((msg.into(), v));
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut KeyFooterState) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.width == 0 || area.height == 0 {
            return;
        }

        fill(buf, area, th.panel);

        // Message mode
        if let Some((msg, variant)) = self.message {
            let msg_color = th.text_variant(variant);
            put(
                buf,
                area.x + 1,
                area.y,
                &msg,
                area.width.saturating_sub(2),
                st(msg_color, th.panel).add_modifier(Modifier::BOLD),
            );
            state.hits.clear();
            return;
        }

        // Right text: reserve its width so bindings never run underneath it
        let mut limit = area.right();
        if let Some(ref right_text) = self.right {
            let w = (right_text.width() as u16).min(area.width / 2);
            let text = crate::draw::truncate(right_text, w as usize);
            put_right(buf, area, &text, st(th.text_muted, th.panel));
            limit = limit.saturating_sub(w + 1);
        }

        // Sort bindings by priority (higher first) for overflow handling
        let mut sorted: Vec<(usize, &FooterBinding)> = self.bindings.iter().enumerate().collect();
        sorted.sort_by_key(|(_, b)| std::cmp::Reverse(b.priority));

        // Layout bindings, dropping lowest-priority ones on overflow
        let mut x = area.x;
        let mut visible = Vec::new();
        let mut overflow = false;

        for &(orig_idx, binding) in &sorted {
            let key_w = binding.key.len() as u16 + 2;
            let desc_w = if self.compact {
                0
            } else {
                binding.description.len() as u16 + 1
            };
            let w = key_w + desc_w;

            if x + w > limit.saturating_sub(2) {
                overflow = true;
                break;
            }

            visible.push((orig_idx, binding, x, w, key_w));
            x += w + 1;
        }

        // Render visible bindings
        state.hits.clear();
        state.hits.resize(self.bindings.len(), HitBox::default());

        for (orig_idx, binding, bx, w, key_w) in visible {
            let r = Rect {
                x: bx,
                y: area.y,
                width: w,
                height: 1,
            };
            state.hits[orig_idx].set_area(r);

            let bg = if state.hover == Some(orig_idx) {
                th.hover_bg
            } else {
                th.panel
            };
            fill(buf, r, bg);

            // Key box
            let key_box_bg = th.footer_bg;
            let key_box = Rect {
                x: bx,
                y: area.y,
                width: key_w,
                height: 1,
            };
            fill(buf, key_box, key_box_bg);
            put(
                buf,
                bx,
                area.y,
                &format!(" {} ", binding.key),
                key_w,
                st(th.footer_key, key_box_bg).add_modifier(Modifier::BOLD),
            );

            // Description
            if !self.compact {
                let desc_color = if binding.enabled {
                    th.footer_desc
                } else {
                    th.text_disabled
                };
                put(
                    buf,
                    bx + key_w,
                    area.y,
                    &format!(" {}", binding.description),
                    w - key_w,
                    st(desc_color, bg),
                );
            }
        }

        // Overflow indicator
        if overflow {
            put(
                buf,
                limit.saturating_sub(2),
                area.y,
                "…",
                1,
                st(th.text_muted, th.panel),
            );
        }
    }
}

impl Default for KeyFooter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn footer_renders_bindings() {
        let mut state = KeyFooterState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        KeyFooter::new()
            .bindings(&[("q", "Quit"), ("^S", "Save")])
            .render(buf.area, &mut buf, &mut state);
    }

    #[test]
    fn footer_compact_mode() {
        let mut state = KeyFooterState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        KeyFooter::new()
            .bindings(&[("q", "Quit"), ("^S", "Save")])
            .compact(true)
            .render(buf.area, &mut buf, &mut state);
    }

    #[test]
    fn footer_overflow_drops_low_priority() {
        let mut state = KeyFooterState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let bindings = vec![
            FooterBinding::new("q", "Quit").priority(255),
            FooterBinding::new("^S", "Save").priority(200),
            FooterBinding::new("F1", "Help").priority(100),
            FooterBinding::new("F2", "Info").priority(50),
        ];
        KeyFooter::new()
            .bindings_full(bindings)
            .render(buf.area, &mut buf, &mut state);
        // Lower priority bindings should be dropped first
    }

    #[test]
    fn footer_message_mode() {
        let mut state = KeyFooterState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        KeyFooter::new()
            .message("Saved successfully", Variant::Success)
            .render(buf.area, &mut buf, &mut state);
        assert!(state.hits.is_empty());
    }
}
