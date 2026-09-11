//! Collapsible section with animated expand/collapse and accordion stacking.
//!
//! ```no_run
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! let mut state = CollapsibleState::new(false);
//! let used = Collapsible::new().title("Details").render_with(area, &mut buf, &mut state, 10, |inner, buf| {
//!     put(buf, inner.x, inner.y, "Content", inner.width, Style::new());
//! });
//! ```

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::anim::{Easing, Tween};
use crate::core::{Hit, HitBox, Outcome, is_activate, is_press};
use crate::draw::{Border, fill, put, st};
use crate::theme::{self, Theme};

/// Collapsible header style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollapsibleHeader {
    /// Plain text with marker.
    #[default]
    Plain,
    /// Panel-like background.
    Panel,
    /// Underline below header.
    Underline,
}

/// State for a collapsible section: open/closed, animation, hit tracking.
#[derive(Clone, Debug)]
pub struct CollapsibleState {
    /// True if expanded.
    pub open: bool,
    /// Animation tween (0.0 = collapsed, 1.0 = fully expanded).
    pub anim: Tween,
    /// Header HitBox.
    pub hit: HitBox,
    /// Animation duration.
    duration: Duration,
}

impl CollapsibleState {
    pub fn new(open: bool) -> Self {
        Self {
            open,
            anim: if open {
                Tween::new(1.0)
            } else {
                Tween::new(0.0)
            },
            hit: HitBox::default(),
            duration: Duration::from_millis(200),
        }
    }

    /// Toggle open/closed state, starting animation.
    pub fn toggle(&mut self, now: Instant) {
        self.open = !self.open;
        let target = if self.open { 1.0 } else { 0.0 };
        if self.duration.is_zero() {
            self.anim.set(target);
        } else {
            self.anim
                .go_with(target, now, self.duration, Easing::InOutCubic);
        }
    }

    /// True if animation is running.
    pub fn animating(&self, now: Instant) -> bool {
        self.anim.active(now)
    }

    /// Current animation factor (0..=1).
    fn factor(&self, now: Instant) -> f32 {
        self.anim.value(now).clamp(0.0, 1.0)
    }
}

impl Default for CollapsibleState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl crate::core::Interactive for CollapsibleState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        if is_activate(&k) {
            self.toggle(Instant::now());
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let h = self.hit.mouse(&m);
        if h == Hit::Press {
            self.toggle(Instant::now());
            Outcome::Changed
        } else if h == Hit::HoverChanged {
            Outcome::Consumed
        } else {
            Outcome::Ignored
        }
    }
}

/// Collapsible section builder.
#[derive(Clone, Debug)]
pub struct Collapsible {
    title: String,
    subtitle: Option<String>,
    markers: (&'static str, &'static str),
    border: Option<Border>,
    header_style: CollapsibleHeader,
    duration: Duration,
    focused: bool,
    theme: Option<Theme>,
}

impl Collapsible {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            subtitle: None,
            markers: ("▸", "▾"),
            border: None,
            header_style: CollapsibleHeader::default(),
            duration: Duration::from_millis(200),
            focused: false,
            theme: None,
        }
    }

    /// Focused header: bold, cursor colours.
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    pub fn markers(mut self, closed: &'static str, open: &'static str) -> Self {
        self.markers = (closed, open);
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn header_style(mut self, h: CollapsibleHeader) -> Self {
        self.header_style = h;
        self
    }

    pub fn duration(mut self, ms: u64) -> Self {
        self.duration = Duration::from_millis(ms);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    /// Render with a closure that draws content. Returns total height used.
    pub fn render_with<F>(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut CollapsibleState,
        content_height: u16,
        mut draw_fn: F,
    ) -> u16
    where
        F: FnMut(Rect, &mut Buffer),
    {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.width == 0 || area.height == 0 {
            return 0;
        }

        state.duration = self.duration;
        let now = Instant::now();
        let factor = state.factor(now);

        // Header
        let marker = if state.open {
            self.markers.1
        } else {
            self.markers.0
        };
        let header_text = if let Some(sub) = &self.subtitle {
            if !sub.is_empty() {
                format!("{} {} · {}", marker, self.title, sub)
            } else {
                format!("{} {}", marker, self.title)
            }
        } else {
            format!("{} {}", marker, self.title)
        };

        let header_bg = match self.header_style {
            CollapsibleHeader::Plain => th.background,
            CollapsibleHeader::Panel => th.panel,
            CollapsibleHeader::Underline => th.background,
        };

        let header_area = Rect { height: 1, ..area };
        state.hit.set_area(header_area);
        let (header_fg, header_bg) = if self.focused {
            (th.cursor_fg, th.cursor_bg)
        } else if state.hit.hover {
            (th.text, th.hover_bg)
        } else {
            (th.text, header_bg)
        };
        fill(buf, header_area, header_bg);
        let style = if self.focused || state.open {
            st(header_fg, header_bg).add_modifier(Modifier::BOLD)
        } else {
            st(header_fg, header_bg)
        };
        put(buf, area.x, area.y, &header_text, area.width, style);

        if self.header_style == CollapsibleHeader::Underline && area.height > 1 {
            for x in area.x..area.right() {
                if let Some(c) = buf.cell_mut((x, area.y + 1)) {
                    c.set_symbol("─")
                        .set_fg(th.border_blurred.color())
                        .set_bg(th.background.color());
                }
            }
        }

        let header_h = match self.header_style {
            CollapsibleHeader::Underline => 2,
            _ => 1,
        };

        if factor == 0.0 {
            return header_h;
        }

        // Content area with animated height
        let max_content_h = area.height.saturating_sub(header_h);
        let visible_h = (content_height.min(max_content_h) as f32 * factor).ceil() as u16;

        if visible_h == 0 {
            return header_h;
        }

        let content_area = Rect {
            x: area.x,
            y: area.y + header_h,
            width: area.width,
            height: visible_h,
        };

        // Border around content
        let inner = if let Some(bord) = self.border {
            bord.draw(buf, content_area, th.border, th.background);
            Rect {
                x: content_area.x + 1,
                y: content_area.y + 1,
                width: content_area.width.saturating_sub(2),
                height: content_area.height.saturating_sub(2),
            }
        } else {
            content_area
        };

        if inner.width > 0 && inner.height > 0 {
            draw_fn(inner, buf);
        }

        header_h + visible_h
    }
}

impl Default for Collapsible {
    fn default() -> Self {
        Self::new()
    }
}

/// Accordion: stack of collapsibles with optional exclusive mode.
#[derive(Clone, Debug)]
pub struct AccordionState {
    /// Individual collapsible states.
    pub states: Vec<CollapsibleState>,
    /// Only one open at a time.
    pub exclusive: bool,
    focus: usize,
}

impl AccordionState {
    pub fn new(count: usize, exclusive: bool) -> Self {
        Self {
            states: (0..count).map(|_| CollapsibleState::new(false)).collect(),
            exclusive,
            focus: 0,
        }
    }

    /// Toggle item `i`, closing others if exclusive.
    pub fn toggle(&mut self, i: usize, now: Instant) {
        if i >= self.states.len() {
            return;
        }

        if self.exclusive {
            for (j, s) in self.states.iter_mut().enumerate() {
                if j == i || s.open {
                    s.toggle(now);
                }
            }
        } else {
            self.states[i].toggle(now);
        }
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.states.iter().any(|s| s.animating(now))
    }

    /// Header index that keyboard input targets.
    pub fn focus(&self) -> usize {
        self.focus
    }
}

impl crate::core::Interactive for AccordionState {
    /// `↑`/`↓` move between headers, `Enter`/`Space` toggle the focused one.
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) || self.states.is_empty() {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                self.focus = self.focus.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.focus = (self.focus + 1).min(self.states.len() - 1);
                Outcome::Consumed
            }
            _ if is_activate(&k) => {
                self.toggle(self.focus, Instant::now());
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        let mut clicked = None;
        for (i, s) in self.states.iter_mut().enumerate() {
            match s.hit.mouse(&m) {
                Hit::Press => clicked = Some(i),
                Hit::HoverChanged => out |= Outcome::Consumed,
                _ => {}
            }
        }
        if let Some(i) = clicked {
            self.focus = i;
            self.toggle(i, Instant::now());
            out = Outcome::Changed;
        }
        out
    }
}

impl Default for AccordionState {
    fn default() -> Self {
        Self::new(3, false)
    }
}

/// Accordion builder (renders multiple collapsibles stacked).
#[derive(Clone, Debug)]
pub struct Accordion {
    exclusive: bool,
    gap: u16,
    titles: Vec<String>,
    header_style: CollapsibleHeader,
    border: Option<Border>,
    focused: bool,
    theme: Option<Theme>,
}

impl Accordion {
    pub fn new() -> Self {
        Self {
            exclusive: false,
            gap: 0,
            titles: Vec::new(),
            header_style: CollapsibleHeader::Plain,
            border: None,
            focused: false,
            theme: None,
        }
    }

    /// Whether the accordion has keyboard focus (highlights the focused header).
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn exclusive(mut self, e: bool) -> Self {
        self.exclusive = e;
        self
    }

    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }

    /// Section titles; missing ones fall back to `Item N`.
    pub fn titles(mut self, titles: &[&str]) -> Self {
        self.titles = titles.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn header_style(mut self, h: CollapsibleHeader) -> Self {
        self.header_style = h;
        self
    }

    /// Border drawn around each open section's content.
    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    /// Render with a closure that draws each item. `heights` = content heights per item.
    pub fn render_with<F>(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut AccordionState,
        heights: &[u16],
        mut draw_fn: F,
    ) where
        F: FnMut(usize, Rect, &mut Buffer),
    {
        state.exclusive = self.exclusive;
        let th = self.theme.unwrap_or_else(theme::current);
        let mut y = area.y;
        for (i, (item_state, &content_h)) in state.states.iter_mut().zip(heights.iter()).enumerate()
        {
            if y >= area.bottom() {
                break;
            }
            let item_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: area.bottom().saturating_sub(y),
            };
            let title = self
                .titles
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("Item {}", i + 1));
            let mut c = Collapsible::new()
                .title(title)
                .header_style(self.header_style)
                .focused(self.focused && state.focus == i)
                .theme(&th);
            if let Some(b) = self.border {
                c = c.border(b);
            }
            let used = c.render_with(item_area, buf, item_state, content_h, |inner, b| {
                draw_fn(i, inner, b);
            });
            y += used + self.gap;
        }
    }
}

impl Default for Accordion {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsible_toggle() {
        let mut state = CollapsibleState::new(false);
        assert!(!state.open);
        state.toggle(Instant::now());
        assert!(state.open);
    }

    #[test]
    fn collapsible_animation_factor() {
        let mut state = CollapsibleState::new(false);
        let now = Instant::now();
        state.duration = Duration::ZERO;
        state.toggle(now);
        assert_eq!(state.factor(now), 1.0);
    }

    #[test]
    fn accordion_exclusive() {
        let mut state = AccordionState::new(3, true);
        let now = Instant::now();
        state.toggle(0, now);
        assert!(state.states[0].open);
        state.toggle(1, now);
        assert!(!state.states[0].open);
        assert!(state.states[1].open);
    }

    #[test]
    fn accordion_non_exclusive() {
        let mut state = AccordionState::new(3, false);
        let now = Instant::now();
        state.toggle(0, now);
        state.toggle(1, now);
        assert!(state.states[0].open);
        assert!(state.states[1].open);
    }
}
