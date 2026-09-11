//! Tooltips that appear on hover after a delay.
//!
//! ```
//! use tuile::prelude::*;
//! # let now = Instant::now();
//! # let bounds = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(bounds);
//! let mut state = TooltipState::new();
//! let button = HitBox { area: Rect::new(0, 0, 20, 1), hover: true, ..Default::default() };
//! // feed the hovered widget's hit box every frame; the tooltip appears after the delay
//! state.track(button.hover, button.area, now);
//! Tooltip::new("Help text").max_width(30).delay(400).render_overlay(&mut buf, bounds, &mut state, now);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::draw::{Border, fill, put, st, wrap};
use crate::layout::popup_below;
use crate::theme::{self, Theme};

/// State tracking hover timing and anchor position for a tooltip.
#[derive(Clone, Debug, Default)]
pub struct TooltipState {
    hovered_since: Option<Instant>,
    pub anchor: Rect,
    pub visible: bool,
}

impl TooltipState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call this every frame with hover state and the widget's rect. Shows after `delay`.
    pub fn track(&mut self, hover: bool, anchor: Rect, now: Instant) {
        if hover {
            if self.hovered_since.is_none() {
                self.hovered_since = Some(now);
            }
            self.anchor = anchor;
        } else {
            self.hovered_since = None;
            self.visible = false;
        }
    }

    pub fn update_visibility(&mut self, now: Instant, delay_ms: u64) {
        if let Some(since) = self.hovered_since {
            let elapsed = now.saturating_duration_since(since);
            self.visible = elapsed.as_millis() >= delay_ms as u128;
        } else {
            self.visible = false;
        }
    }
}

/// Tooltip overlay with wrapped text in a bordered box.
pub struct Tooltip {
    text: String,
    max_width: u16,
    delay: u64,
    theme: Option<Theme>,
}

impl Tooltip {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            max_width: 30,
            delay: 400,
            theme: None,
        }
    }

    pub fn max_width(mut self, w: u16) -> Self {
        self.max_width = w;
        self
    }

    pub fn delay(mut self, ms: u64) -> Self {
        self.delay = ms;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Render the tooltip overlay if visible. Call this at the end of the frame.
    pub fn render_overlay(
        self,
        buf: &mut Buffer,
        bounds: Rect,
        state: &mut TooltipState,
        now: Instant,
    ) {
        state.update_visibility(now, self.delay);
        if !state.visible || state.anchor.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let inner_w = self.max_width.saturating_sub(4);
        let lines = wrap(&self.text, inner_w as usize);
        let h = (lines.len() as u16 + 2).min(20);
        let w = self.max_width.min(bounds.width);

        let area = popup_below(state.anchor, w, h, bounds);
        if area.is_empty() {
            return;
        }

        let bg = th.panel;
        fill(buf, area, bg);
        Border::Round.draw(buf, area, th.border, bg);

        let text_x = area.x + 2;
        let text_w = area.width.saturating_sub(4);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y + 1 + i as u16;
            if y >= area.bottom().saturating_sub(1) {
                break;
            }
            put(buf, text_x, y, line, text_w, st(th.text, bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_delay() {
        let mut state = TooltipState::new();
        let now = Instant::now();
        let anchor = Rect::new(10, 10, 5, 1);
        state.track(true, anchor, now);
        state.update_visibility(now, 400);
        assert!(!state.visible);
        state.update_visibility(now + std::time::Duration::from_millis(500), 400);
        assert!(state.visible);
    }

    #[test]
    fn tooltip_hides_on_leave() {
        let mut state = TooltipState::new();
        let now = Instant::now();
        let anchor = Rect::new(10, 10, 5, 1);
        state.track(true, anchor, now);
        state.update_visibility(now + std::time::Duration::from_millis(500), 400);
        assert!(state.visible);
        state.track(false, anchor, now);
        assert!(!state.visible);
    }
}
