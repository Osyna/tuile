//! Tab bars with multiple styles (Underline with sliding animation, Boxed, Pills, Segmented,
//! Minimal) plus `TabbedContent` for tab+content layout. Closable tabs, keyboard navigation,
//! horizontal scroll with overflow indicators.
//!
//! ```
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 40, 5);
//! # let mut buf = Buffer::empty(area);
//! let mut state = TabBarState::new(0);
//! TabBar::new(vec!["Home", "Settings", "Help"].into_iter().map(TabItem::new).collect::<Vec<_>>())
//!     .style(TabStyle::Underline)
//!     .focused(true)
//!     .render(area, &mut buf, &mut state);
//! ```

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Rect};
use ratatui_core::style::Modifier;
use ratatui_core::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::anim::Tween;
use crate::core::{Interactive, Look, Outcome, ctrl, is_press, mouse_pos, wheel_delta};
use crate::draw::{Border, fill, hline, put, put_centered, st};
use crate::layout::pad;
use crate::theme::{self, Theme};

// items

/// One tab item: label, optional icon/badge, closable/disabled flags.
#[derive(Clone, Debug)]
pub struct TabItem {
    pub label: String,
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub closable: bool,
    pub disabled: bool,
}

impl TabItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            badge: None,
            closable: false,
            disabled: false,
        }
    }
    pub fn icon(mut self, i: &str) -> Self {
        self.icon = Some(i.to_string());
        self
    }
    pub fn badge(mut self, b: &str) -> Self {
        self.badge = Some(b.to_string());
        self
    }
    pub fn closable(mut self, c: bool) -> Self {
        self.closable = c;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

impl From<&str> for TabItem {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

// style

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabStyle {
    #[default]
    Underline,
    Boxed,
    Pills,
    Segmented,
    Minimal,
}

// builder

/// Tab bar builder: multiple styles, closable tabs, focus ring, animated underline.
#[derive(Clone, Debug)]
pub struct TabBar {
    items: Vec<TabItem>,
    style: TabStyle,
    align: Alignment,
    duration: Duration,
    fill: bool,
    padding: u16,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl TabBar {
    pub fn new(items: impl Into<Vec<TabItem>>) -> Self {
        Self {
            items: items.into(),
            style: TabStyle::default(),
            align: Alignment::Left,
            duration: Duration::from_millis(200),
            fill: false,
            padding: 1,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn style(mut self, s: TabStyle) -> Self {
        self.style = s;
        self
    }

    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }

    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = d;
        self
    }

    pub fn fill(mut self, f: bool) -> Self {
        self.fill = f;
        self
    }

    pub fn padding(mut self, p: u16) -> Self {
        self.padding = p;
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

impl<T: Into<Vec<TabItem>>> From<T> for TabBar {
    fn from(items: T) -> Self {
        Self::new(items)
    }
}

// state

/// Tab bar state: active index, hover, sliding underline tweens, scroll, hit boxes, closed tab.
#[derive(Clone, Debug)]
pub struct TabBarState {
    pub active: usize,
    pub hover: Option<usize>,
    pub hl_start: Tween,
    pub hl_width: Tween,
    pub scroll: u16,
    pub hits: Vec<Rect>,
    pub close_hits: Vec<Rect>,
    pub closed: Option<usize>,
}

impl TabBarState {
    pub fn new(active: usize) -> Self {
        Self {
            active,
            hover: None,
            hl_start: Tween::new(0.0),
            hl_width: Tween::new(0.0),
            scroll: 0,
            hits: Vec::new(),
            close_hits: Vec::new(),
            closed: None,
        }
    }

    /// Select tab `i`. The underline and pill tweens retarget on the next render, which is
    /// where the actual tab geometry is known.
    pub fn set_active(&mut self, i: usize) {
        self.active = i;
    }

    pub fn take_closed(&mut self) -> Option<usize> {
        self.closed.take()
    }
}

impl Default for TabBarState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Interactive for TabBarState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Left => {
                if self.active > 0 {
                    self.active -= 1;
                    return Outcome::Changed;
                }
            }
            KeyCode::Right => {
                self.active += 1;
                return Outcome::Changed;
            }
            KeyCode::Home => {
                if self.active != 0 {
                    self.active = 0;
                    return Outcome::Changed;
                }
            }
            KeyCode::End => {
                self.active = usize::MAX;
                return Outcome::Changed;
            }
            KeyCode::Char(c @ '1'..='9') => {
                let i = (c as usize) - ('1' as usize);
                if i != self.active {
                    self.active = i;
                    return Outcome::Changed;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                return Outcome::Changed;
            }
            KeyCode::Char('w') if ctrl(&k, 'w') => {
                self.closed = Some(self.active);
                return Outcome::Changed;
            }
            KeyCode::Delete => {
                self.closed = Some(self.active);
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        let mut out = Outcome::Ignored;

        // hover
        let mut hover = None;
        for (i, r) in self.hits.iter().enumerate() {
            if r.contains(pos) {
                hover = Some(i);
                break;
            }
        }
        if hover != self.hover {
            self.hover = hover;
            out = Outcome::Consumed;
        }

        // click tab
        if matches!(
            m.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) && let Some(i) = hover
        {
            // check close button
            if i < self.close_hits.len() && self.close_hits[i].contains(pos) {
                self.closed = Some(i);
                return Outcome::Changed;
            }
            if i != self.active {
                self.active = i;
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }

        // wheel
        if let Some(delta) = wheel_delta(&m)
            && (self.hits.iter().any(|r| r.contains(pos)) || hover.is_some())
        {
            if delta > 0 {
                self.active = self.active.saturating_add(1);
            } else {
                self.active = self.active.saturating_sub(1);
            }
            return Outcome::Changed;
        }

        out
    }
}

// render

impl StatefulWidget for TabBar {
    type State = TabBarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || self.items.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        let now = self.now.unwrap_or_else(Instant::now);

        // clamp active
        state.active = state.active.min(self.items.len().saturating_sub(1));

        // skip disabled tabs when wrapping
        let items_vec: Vec<(usize, &TabItem)> = self.items.iter().enumerate().collect();
        let enabled: Vec<usize> = items_vec
            .iter()
            .filter(|(_, t)| !t.disabled)
            .map(|(i, _)| *i)
            .collect();
        if !enabled.is_empty() && self.items.get(state.active).is_some_and(|t| t.disabled) {
            // move to next enabled
            state.active = enabled
                .iter()
                .copied()
                .find(|&i| i >= state.active)
                .unwrap_or(enabled[0]);
        }

        match self.style {
            TabStyle::Underline => render_underline(self, area, buf, state, &th, bg, now),
            TabStyle::Boxed => render_boxed(self, area, buf, state, &th, bg),
            TabStyle::Pills => render_pills(self, area, buf, state, &th, bg),
            TabStyle::Segmented => render_segmented(self, area, buf, state, &th, bg),
            TabStyle::Minimal => render_minimal(self, area, buf, state, &th, bg),
        }
    }
}

// underline style

fn render_underline(
    builder: TabBar,
    area: Rect,
    buf: &mut Buffer,
    state: &mut TabBarState,
    th: &Theme,
    bg: crate::theme::Rgb,
    now: Instant,
) {
    fill(buf, area, bg);
    if area.height < 2 {
        return;
    }

    let items = &builder.items;
    state.hits.clear();
    state.close_hits.clear();

    // compute tab ranges
    let mut ranges = Vec::new();
    let mut x = area.x;
    for item in items.iter() {
        let mut w = item.label.width() as u16 + 2; // padding 0 1
        if item.icon.is_some() {
            w += 2;
        }
        if item.badge.is_some() {
            w += 4;
        }
        if item.closable {
            w += 2;
        }
        ranges.push((x, w));
        x += w;
    }

    // render tabs
    for (i, (item, (tx, tw))) in items.iter().zip(&ranges).enumerate() {
        let is_active = i == state.active;
        let is_hover = state.hover == Some(i);
        let _look = Look {
            focused: builder.focused && is_active,
            hover: is_hover,
            enabled: !item.disabled,
        };

        let style = if is_active {
            st(th.text, bg).add_modifier(Modifier::BOLD)
        } else if is_hover && !item.disabled {
            st(th.text, bg)
        } else if item.disabled {
            st(th.text_disabled, bg)
        } else {
            st(th.text_muted, bg)
        };

        let mut label = format!(" {} ", item.label);
        if let Some(icon) = &item.icon {
            label = format!("{} {}", icon, label);
        }
        if let Some(badge) = &item.badge {
            label = format!("{} {}", label, badge);
        }

        let label_w = put(buf, *tx, area.y, &label, *tw, style);
        state.hits.push(Rect::new(*tx, area.y, label_w, 1));

        if item.closable {
            let close_x = tx + label_w;
            if close_x < area.right() {
                put(buf, close_x, area.y, "×", 1, st(th.text_muted, bg));
                state.close_hits.push(Rect::new(close_x, area.y, 1, 1));
            }
        } else {
            state.close_hits.push(Rect::ZERO);
        }
    }

    // underline row - track is transparent (background color)
    let y = area.y + 1;
    // No track drawing - leave it as background, only highlight the active span below

    // animated highlight
    if state.active < ranges.len() {
        let (tx, tw) = ranges[state.active];
        let target_start = (tx - area.x) as f32;
        let target_width = tw as f32;

        if state.hl_start.target() != target_start {
            state.hl_start.go(target_start, now, builder.duration);
        }
        if state.hl_width.target() != target_width {
            state.hl_width.go(target_width, now, builder.duration);
        }

        let s = state.hl_start.value(now);
        let w = state.hl_width.value(now);
        let e = s + w;

        let s_cell = s.floor() as u16;
        let e_cell = e.ceil() as u16;

        for x in s_cell..e_cell.min(area.width) {
            let fx = x as f32;
            let gx = area.x + x;
            if !buf.area.contains((gx, y).into()) {
                continue;
            }
            if fx + 0.5 < s || fx + 0.5 > e {
                continue;
            }
            let sym = if fx < s {
                "╺"
            } else if fx + 1.0 > e {
                "╸"
            } else {
                "━"
            };
            if let Some(c) = buf.cell_mut((gx, y)) {
                c.set_symbol(sym).set_fg(th.accent.color());
            }
        }
    }
}

// boxed style

fn render_boxed(
    builder: TabBar,
    area: Rect,
    buf: &mut Buffer,
    state: &mut TabBarState,
    th: &Theme,
    bg: crate::theme::Rgb,
) {
    fill(buf, area, bg);
    if area.height < 3 {
        return;
    }

    state.hits.clear();
    state.close_hits.clear();

    // baseline row
    let base_y = area.y + 2;
    for bx in area.left()..area.right() {
        put(buf, bx, base_y, "─", 1, st(th.border, bg));
    }

    let line = st(th.border, bg);
    let mut x = area.x;
    for (i, item) in builder.items.iter().enumerate() {
        let is_active = i == state.active;
        let is_hover = state.hover == Some(i);

        // `│ [icon ]label [× ]│`: two side columns, one cell of padding each side
        let mut content = String::from(" ");
        if let Some(icon) = &item.icon {
            content.push_str(icon);
            content.push(' ');
        }
        content.push_str(&item.label);
        content.push(' ');
        if item.closable {
            content.push_str("× ");
        }
        let w = content.width() as u16 + 2;
        if x + w > area.right() {
            break;
        }

        let style = if is_active {
            st(th.text, bg).add_modifier(Modifier::BOLD)
        } else if is_hover && !item.disabled {
            st(th.text, th.hover_bg)
        } else if item.disabled {
            st(th.text_disabled, bg)
        } else {
            st(th.text_muted, bg)
        };

        let label_y = area.y + 1;
        if is_active {
            // ╭────╮ / │ … │ / ╯    ╰ : the frame opens into the content below
            put(buf, x, area.y, "╭", 1, line);
            hline(buf, x + 1, area.y, w - 2, "─", line);
            put(buf, x + w - 1, area.y, "╮", 1, line);
            put(buf, x, label_y, "│", 1, line);
            put(buf, x + w - 1, label_y, "│", 1, line);
            put(buf, x, base_y, "╯", 1, line);
            hline(buf, x + 1, base_y, w - 2, " ", st(bg, bg));
            put(buf, x + w - 1, base_y, "╰", 1, line);
        }
        put(buf, x + 1, label_y, &content, w - 2, style);
        if item.closable {
            put(
                buf,
                x + w - 3,
                label_y,
                "×",
                1,
                style.fg(th.text_muted.color()),
            );
        }

        state.hits.push(Rect::new(x, label_y, w, 1));
        state.close_hits.push(if item.closable {
            Rect::new(x + w - 3, label_y, 1, 1)
        } else {
            Rect::ZERO
        });

        x += w;
    }
}

// pills style

fn render_pills(
    builder: TabBar,
    area: Rect,
    buf: &mut Buffer,
    state: &mut TabBarState,
    th: &Theme,
    bg: crate::theme::Rgb,
) {
    fill(buf, area, bg);
    state.hits.clear();
    state.close_hits.clear();

    let mut x = area.x;
    for (i, item) in builder.items.iter().enumerate() {
        let is_active = i == state.active;
        let is_hover = state.hover == Some(i);

        let mut w = item.label.width() as u16 + 2;
        if item.icon.is_some() {
            w += 2;
        }
        if item.closable {
            w += 2;
        }

        if x + w + 1 > area.right() {
            break;
        }

        let (fg, pill_bg) = if is_active {
            (th.cursor_fg, th.primary)
        } else if is_hover && !item.disabled {
            (th.text, th.hover_bg)
        } else if item.disabled {
            (th.text_disabled, th.panel)
        } else {
            (th.text_muted, th.panel)
        };

        fill(buf, Rect::new(x, area.y, w, 1), pill_bg);
        let mut label = format!(" {}", item.label);
        if let Some(icon) = &item.icon {
            label = format!("{} {}", icon, item.label);
        }
        put(buf, x, area.y, &label, w, st(fg, pill_bg));

        if item.closable {
            put(buf, x + w - 2, area.y, "×", 1, st(th.text_muted, pill_bg));
        }

        state.hits.push(Rect::new(x, area.y, w, 1));
        state.close_hits.push(if item.closable {
            Rect::new(x + w - 2, area.y, 1, 1)
        } else {
            Rect::ZERO
        });

        x += w + 1;
    }
}

// segmented style

fn render_segmented(
    builder: TabBar,
    area: Rect,
    buf: &mut Buffer,
    state: &mut TabBarState,
    th: &Theme,
    bg: crate::theme::Rgb,
) {
    fill(buf, area, bg);
    if area.width < 2 || area.height == 0 {
        return;
    }

    Border::Tall.draw(buf, area, th.border, bg);
    let inner = pad(area, 1, 0);
    fill(buf, inner, th.panel);

    state.hits.clear();
    state.close_hits.clear();

    let n = builder.items.len();
    if n == 0 {
        return;
    }

    let seg_w = inner.width / n as u16;
    let mut x = inner.x;

    for (i, item) in builder.items.iter().enumerate() {
        let is_active = i == state.active;
        let is_hover = state.hover == Some(i);

        let w = if i == n - 1 { inner.right() - x } else { seg_w };

        let (fg, seg_bg) = if is_active {
            (th.cursor_fg, th.cursor_bg)
        } else if is_hover && !item.disabled {
            (th.text, th.hover_bg)
        } else if item.disabled {
            (th.text_disabled, th.panel)
        } else {
            (th.text_muted, th.panel)
        };

        fill(buf, Rect::new(x, inner.y, w, 1), seg_bg);
        let label = if let Some(icon) = &item.icon {
            format!("{} {}", icon, item.label)
        } else {
            item.label.clone()
        };
        put_centered(buf, Rect::new(x, inner.y, w, 1), &label, st(fg, seg_bg));

        state.hits.push(Rect::new(x, inner.y, w, 1));
        state.close_hits.push(Rect::ZERO);

        if i < n - 1 {
            put(buf, x + w, inner.y, "│", 1, st(th.border, th.panel));
        }

        x += w;
    }
}

// minimal style

fn render_minimal(
    builder: TabBar,
    area: Rect,
    buf: &mut Buffer,
    state: &mut TabBarState,
    th: &Theme,
    bg: crate::theme::Rgb,
) {
    fill(buf, area, bg);
    state.hits.clear();
    state.close_hits.clear();

    let mut x = area.x;
    for (i, item) in builder.items.iter().enumerate() {
        let is_active = i == state.active;
        let is_hover = state.hover == Some(i);

        if is_active {
            put(buf, x, area.y, "┃", 1, st(th.accent, bg));
            x += 2;
        }

        let style = if is_active {
            st(th.text, bg).add_modifier(Modifier::BOLD)
        } else if is_hover && !item.disabled {
            st(th.text, bg)
        } else if item.disabled {
            st(th.text_disabled, bg)
        } else {
            st(th.text_muted, bg)
        };

        let label = if let Some(icon) = &item.icon {
            format!("{} {}", icon, item.label)
        } else {
            item.label.clone()
        };

        let w = label.width() as u16;
        if x + w > area.right() {
            break;
        }

        put(buf, x, area.y, &label, w, style);
        state.hits.push(Rect::new(
            x - if is_active { 2 } else { 0 },
            area.y,
            w + if is_active { 2 } else { 0 },
            1,
        ));
        state.close_hits.push(Rect::ZERO);

        x += w + 2;
    }
}

// TabbedContent

/// Helper: tab bar + bordered content area.
#[derive(Clone, Debug)]
pub struct TabbedContent {
    bar: TabBar,
    bordered: bool,
}

impl TabbedContent {
    pub fn new(items: impl Into<Vec<TabItem>>) -> Self {
        Self {
            bar: TabBar::new(items),
            bordered: false,
        }
    }

    pub fn style(mut self, s: TabStyle) -> Self {
        self.bar = self.bar.style(s);
        self
    }

    pub fn bordered(mut self, b: bool) -> Self {
        self.bordered = b;
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.bar = self.bar.focused(f);
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.bar = self.bar.enabled(e);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.bar = self.bar.now(n);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.bar = self.bar.theme(t);
        self
    }

    /// Render the tab bar and return the content rect.
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut TabBarState) -> Rect {
        if area.height < 2 {
            return Rect::ZERO;
        }
        let bar_h = if matches!(self.bar.style, TabStyle::Underline) {
            2
        } else {
            1
        };
        let bar_area = Rect::new(area.x, area.y, area.width, bar_h);
        let th = self.bar.theme.unwrap_or_else(theme::current);
        self.bar.render(bar_area, buf, state);

        let content_area = Rect::new(
            area.x,
            area.y + bar_h,
            area.width,
            area.height.saturating_sub(bar_h),
        );
        if self.bordered && content_area.width >= 2 && content_area.height >= 2 {
            Border::Round.draw(buf, content_area, th.border, th.surface);
            pad(content_area, 1, 1)
        } else {
            content_area
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_bar_switches_on_arrows() {
        let mut state = TabBarState::new(0);
        assert_eq!(
            state.handle_key(KeyEvent::new(
                KeyCode::Right,
                crossterm::event::KeyModifiers::NONE
            )),
            Outcome::Changed
        );
        assert_eq!(state.active, 1);
        assert_eq!(
            state.handle_key(KeyEvent::new(
                KeyCode::Left,
                crossterm::event::KeyModifiers::NONE
            )),
            Outcome::Changed
        );
        assert_eq!(state.active, 0);
    }

    #[test]
    fn tab_bar_clamps_active() {
        let items: Vec<TabItem> = vec!["A".into(), "B".into()];
        let mut state = TabBarState::new(5);
        let bar = TabBar::new(items.clone());
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
        bar.render(buf.area, &mut buf, &mut state);
        assert_eq!(state.active, 1);
    }

    #[test]
    fn tabbed_content_returns_content_rect() {
        let mut state = TabBarState::new(0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        let content = TabbedContent::new(vec!["A".into(), "B".into()])
            .bordered(true)
            .render(buf.area, &mut buf, &mut state);
        assert!(content.height < buf.area.height);
        assert!(content.y > buf.area.y);
    }
}
