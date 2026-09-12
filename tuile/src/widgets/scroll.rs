//! Scrollable viewport with optional scrollbars, smooth tweened scrolling, and keyboard/wheel/drag.
//!
//! ```no_run
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 60, 15);
//! # let mut buf = Buffer::empty(area);
//! let mut state = ScrollViewState::new();
//! ScrollView::new().content_size(200, 100).smooth(true).render_with(area, &mut buf, &mut state, |cbuf, carea| {
//!     put(cbuf, 0, 0, "Content...", carea.width, Style::new());
//! });
//! ```

use std::time::{Duration, Instant};

use crate::anim::{Easing, Tween};
use crate::core::{MinSize, Outcome, is_press, wheel_delta};
use crate::draw::{Border, blit, st};
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{ScrollAxis, Scrollbar, ScrollbarState};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Rect};
use ratatui_core::style::Modifier;
use ratatui_core::widgets::StatefulWidget;

/// Scrollbar visibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollBars {
    /// Show only when content exceeds viewport.
    #[default]
    Auto,
    /// Always show.
    Always,
    /// Never show.
    Never,
}

/// State for a scroll view: offsets, animation, scrollbars.
#[derive(Clone, Debug, Default)]
pub struct ScrollViewState {
    /// Horizontal offset in cells.
    pub offset_x: usize,
    /// Vertical offset in cells.
    pub offset_y: usize,
    /// Animated vertical scroll.
    pub anim_y: Tween,
    /// Content size (width, height) in cells.
    pub content: (u16, u16),
    /// Last viewport rect.
    pub viewport: Rect,
    /// Vertical scrollbar state.
    pub vbar: ScrollbarState,
    /// Horizontal scrollbar state.
    pub hbar: ScrollbarState,
}

impl ScrollViewState {
    pub fn new() -> Self {
        Default::default()
    }

    /// Scroll to specific coordinates, optionally animated.
    pub fn scroll_to(&mut self, x: usize, y: usize, now: Instant, dur: Duration) {
        self.offset_x = x.min(self.content.0.saturating_sub(self.viewport.width) as usize);
        if dur.is_zero() {
            self.offset_y = y.min(self.content.1.saturating_sub(self.viewport.height) as usize);
            self.anim_y.set(self.offset_y as f32);
        } else {
            let target = y.min(self.content.1.saturating_sub(self.viewport.height) as usize) as f32;
            self.anim_y.go_with(target, now, dur, Easing::InOutCubic);
        }
    }

    /// Scroll by delta cells, optionally animated.
    pub fn scroll_by(&mut self, dx: i32, dy: i32, now: Instant, dur: Duration) {
        let new_x = (self.offset_x as i32 + dx).max(0) as usize;
        let new_y = (self.offset_y as i32 + dy).max(0) as usize;
        self.scroll_to(new_x, new_y, now, dur);
    }

    /// Ensure `rect` is visible (scroll minimally to bring it into view).
    pub fn scroll_into_view(&mut self, rect: Rect, now: Instant, dur: Duration) {
        let vp = self.viewport;
        let mut new_x = self.offset_x;
        let mut new_y = self.offset_y;

        if rect.x < self.offset_x as u16 {
            new_x = rect.x as usize;
        } else if rect.right() > (self.offset_x + vp.width as usize) as u16 {
            new_x = rect.right().saturating_sub(vp.width) as usize;
        }

        if rect.y < self.offset_y as u16 {
            new_y = rect.y as usize;
        } else if rect.bottom() > (self.offset_y + vp.height as usize) as u16 {
            new_y = rect.bottom().saturating_sub(vp.height) as usize;
        }

        self.scroll_to(new_x, new_y, now, dur);
    }

    /// True if scrolled to the bottom.
    pub fn at_bottom(&self) -> bool {
        self.offset_y >= self.content.1.saturating_sub(self.viewport.height) as usize
    }
    pub fn animating(&self, now: Instant) -> bool {
        self.anim_y.active(now)
    }

    /// Step animation (call on every frame when animating).
    fn step(&mut self, now: Instant) {
        if self.anim_y.active(now) {
            self.offset_y = self.anim_y.value(now) as usize;
        }
    }
}

impl crate::core::Interactive for ScrollViewState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        let page = self.viewport.height.saturating_sub(1) as i32;
        match k.code {
            KeyCode::Up => {
                self.scroll_by(0, -1, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.scroll_by(0, 1, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::Left => {
                self.scroll_by(-1, 0, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::Right => {
                self.scroll_by(1, 0, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::PageUp => {
                self.scroll_by(0, -page, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::PageDown => {
                self.scroll_by(0, page, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.scroll_to(self.offset_x, 0, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            KeyCode::End => {
                self.scroll_to(self.offset_x, usize::MAX, Instant::now(), Duration::ZERO);
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        if let Some(dy) = wheel_delta(&m) {
            self.scroll_by(0, -dy * 3, Instant::now(), Duration::from_millis(120));
            out = Outcome::Consumed;
        }

        let vb = self.vbar.handle_mouse(m);
        if vb.is_changed() {
            let new_y = (self.vbar.offset as f32 / self.vbar.max_offset().max(1) as f32
                * self.content.1.saturating_sub(self.viewport.height) as f32)
                as usize;
            self.offset_y = new_y;
            self.anim_y.set(new_y as f32);
            out |= Outcome::Consumed;
        } else if vb == Outcome::Consumed {
            out |= Outcome::Consumed;
        }

        let hb = self.hbar.handle_mouse(m);
        if hb.is_changed() {
            let new_x = (self.hbar.offset as f32 / self.hbar.max_offset().max(1) as f32
                * self.content.0.saturating_sub(self.viewport.width) as f32)
                as usize;
            self.offset_x = new_x;
            out |= Outcome::Consumed;
        } else if hb == Outcome::Consumed {
            out |= Outcome::Consumed;
        }

        out
    }
}

/// Scrollable viewport builder.
#[derive(Clone, Debug)]
pub struct ScrollView {
    content: (u16, u16),
    show_scrollbars: ScrollBars,
    smooth: bool,
    border: Option<Border>,
    title: Option<String>,
    theme: Option<Theme>,
}

impl ScrollView {
    pub fn new() -> Self {
        Self {
            content: (0, 0),
            show_scrollbars: ScrollBars::default(),
            smooth: false,
            border: None,
            title: None,
            theme: None,
        }
    }

    pub fn content_size(mut self, w: u16, h: u16) -> Self {
        // ponytail: clamp at 4000×4000 cells to avoid runaway memory
        self.content = (w.min(4000), h.min(4000));
        self
    }

    pub fn show_scrollbars(mut self, s: ScrollBars) -> Self {
        self.show_scrollbars = s;
        self
    }

    pub fn smooth(mut self, s: bool) -> Self {
        self.smooth = s;
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    /// Render with a closure that draws into the content buffer.
    pub fn render_with<F>(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut ScrollViewState,
        mut draw_fn: F,
    ) where
        F: FnMut(&mut Buffer, Rect),
    {
        let th = self.theme.unwrap_or_else(theme::current);

        // Border
        let inner = if let Some(bord) = self.border {
            if let Some(title) = &self.title {
                bord.draw_titled_with(
                    buf,
                    area,
                    th.border_blurred,
                    th.background,
                    title,
                    Alignment::Left,
                    st(th.text, th.background).add_modifier(Modifier::BOLD),
                );
            } else {
                bord.draw(buf, area, th.border, th.background);
            }
            bord.inner(area)
        } else {
            area
        };

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        state.content = self.content;

        // Determine scrollbar visibility
        let needs_vbar = self.content.1 > inner.height;
        let needs_hbar = self.content.0 > inner.width;
        let show_vbar = match self.show_scrollbars {
            ScrollBars::Always => true,
            ScrollBars::Never => false,
            ScrollBars::Auto => needs_vbar,
        };
        let show_hbar = match self.show_scrollbars {
            ScrollBars::Always => true,
            ScrollBars::Never => false,
            ScrollBars::Auto => needs_hbar,
        };

        let vbar_w = if show_vbar { 1 } else { 0 };
        let hbar_h = if show_hbar { 1 } else { 0 };

        let viewport = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width.saturating_sub(vbar_w),
            height: inner.height.saturating_sub(hbar_h),
        };
        state.viewport = viewport;

        // Step animation
        state.step(Instant::now());

        // Clamp offsets
        state.offset_x = state
            .offset_x
            .min(self.content.0.saturating_sub(viewport.width) as usize);
        state.offset_y = state
            .offset_y
            .min(self.content.1.saturating_sub(viewport.height) as usize);

        // Render content into offscreen buffer
        let content_area = Rect {
            x: 0,
            y: 0,
            width: self.content.0,
            height: self.content.1,
        };
        let mut content_buf = Buffer::empty(content_area);
        draw_fn(&mut content_buf, content_area);

        // Blit visible portion
        let src_rect = Rect {
            x: state.offset_x as u16,
            y: state.offset_y as u16,
            width: viewport.width,
            height: viewport.height,
        };
        blit(buf, viewport.as_position(), &content_buf, src_rect);

        // Scrollbars
        if show_vbar {
            let vbar_area = Rect {
                x: inner.right().saturating_sub(1),
                y: inner.y,
                width: 1,
                height: inner.height.saturating_sub(hbar_h),
            };
            state.vbar.offset = state.offset_y;
            Scrollbar::new(
                ScrollAxis::Vertical,
                self.content.1 as usize,
                viewport.height as usize,
            )
            .theme(&th)
            .render(vbar_area, buf, &mut state.vbar);
        }

        if show_hbar {
            let hbar_area = Rect {
                x: inner.x,
                y: inner.bottom().saturating_sub(1),
                width: inner.width.saturating_sub(vbar_w),
                height: 1,
            };
            state.hbar.offset = state.offset_x;
            Scrollbar::new(
                ScrollAxis::Horizontal,
                self.content.0 as usize,
                viewport.width as usize,
            )
            .theme(&th)
            .render(hbar_area, buf, &mut state.hbar);
        }
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl MinSize for ScrollView {
    /// Minimum viewport: 1 cell.
    fn min_size(&self) -> (u16, u16) {
        (1, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::buffer::Buffer;

    #[test]
    fn scroll_offset_clamp() {
        let mut state = ScrollViewState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        ScrollView::new().content_size(100, 50).render_with(
            buf.area,
            &mut buf,
            &mut state,
            |_, _| {},
        );
        state.scroll_to(200, 200, Instant::now(), Duration::ZERO);
        // the viewport shrinks by the scrollbar thickness, so clamp against the real viewport
        assert_eq!(state.offset_x, 100 - state.viewport.width as usize);
        assert_eq!(state.offset_y, 50 - state.viewport.height as usize);
    }

    #[test]
    fn scroll_into_view() {
        let mut state = ScrollViewState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        ScrollView::new().content_size(100, 50).render_with(
            buf.area,
            &mut buf,
            &mut state,
            |_, _| {},
        );

        let rect = Rect::new(50, 25, 5, 5);
        state.scroll_into_view(rect, Instant::now(), Duration::ZERO);
        // Should scroll to make rect visible
        assert!(state.offset_x + 20 >= rect.right() as usize);
        assert!(state.offset_y + 10 >= rect.bottom() as usize);
    }

    #[test]
    fn scroll_at_bottom() {
        let mut state = ScrollViewState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        ScrollView::new().content_size(100, 50).render_with(
            buf.area,
            &mut buf,
            &mut state,
            |_: &mut Buffer, _: Rect| {},
        );

        state.scroll_to(0, usize::MAX, Instant::now(), Duration::ZERO);
        assert!(state.at_bottom());
    }
}
