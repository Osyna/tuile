//! Log viewer with auto-follow, filtering and level badges.
//!
//! ```
//! # use tuile::prelude::*;
//! # use tuile::widgets::log::*;
//! # let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
//! # let mut state = LogViewState::new();
//! state.push(LogLevel::Info, "Server started");
//! LogView::new().render(Rect::new(0, 0, 40, 10), &mut buf, &mut state);
//! ```

use std::collections::VecDeque;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;

use crate::core::{HitBox, Interactive, Outcome, is_press, wheel_delta};
use crate::draw::{Border, fill, put, st};
use crate::runtime::local_hms;
use crate::theme::{self, Theme, Variant};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState};

// LogView

/// Log level for entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Success,
}

impl LogLevel {
    fn variant(self) -> Variant {
        match self {
            LogLevel::Trace => Variant::Default,
            LogLevel::Debug => Variant::Default,
            LogLevel::Info => Variant::Primary,
            LogLevel::Warn => Variant::Warning,
            LogLevel::Error => Variant::Error,
            LogLevel::Success => Variant::Success,
        }
    }

    fn label(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO ",
            LogLevel::Warn => "WARN ",
            LogLevel::Error => "ERROR",
            LogLevel::Success => "OK   ",
        }
    }
}

/// One log entry.
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub time: Option<(u32, u32, u32)>,
    pub level: LogLevel,
    pub text: String,
}

/// Filter mode for log entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFilter {
    Highlight,
    Only,
}

/// Log view state with ring buffer and auto-follow.
#[derive(Clone, Debug)]
pub struct LogViewState {
    pub lines: VecDeque<LogEntry>,
    pub follow: bool,
    pub scroll: usize,
    pub filter: String,
    pub filter_mode: LogFilter,
    pub vbar: ScrollbarState,
    pub hit: HitBox,
    pub max_lines: Option<usize>,
    timestamps: bool,
    level_column: bool,
    wrap_text: bool,
}

impl Default for LogViewState {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            follow: true,
            scroll: 0,
            filter: String::new(),
            filter_mode: LogFilter::Highlight,
            vbar: ScrollbarState::default(),
            hit: HitBox::default(),
            max_lines: None,
            timestamps: false,
            level_column: true,
            wrap_text: false,
        }
    }
}

impl LogViewState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, level: LogLevel, text: impl Into<String>) {
        let time = if self.timestamps {
            Some(local_hms())
        } else {
            None
        };
        self.lines.push_back(LogEntry {
            time,
            level,
            text: text.into(),
        });
        if let Some(max) = self.max_lines {
            while self.lines.len() > max {
                self.lines.pop_front();
            }
        }
        if self.follow {
            self.scroll = self.lines.len().saturating_sub(1);
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn set_timestamps(&mut self, v: bool) {
        self.timestamps = v;
    }

    pub fn set_level_column(&mut self, v: bool) {
        self.level_column = v;
    }

    pub fn set_wrap(&mut self, v: bool) {
        self.wrap_text = v;
    }

    pub fn set_filter(&mut self, f: impl Into<String>) {
        self.filter = f.into();
    }

    fn scroll_down(&mut self, n: usize, viewport: usize) {
        let content = self.visible_count();
        if content > viewport {
            self.scroll = (self.scroll + n).min(content.saturating_sub(viewport));
            self.follow = self.scroll + viewport >= content;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
        self.follow = false;
    }

    fn visible_count(&self) -> usize {
        if self.filter.is_empty() || self.filter_mode == LogFilter::Highlight {
            self.lines.len()
        } else {
            self.lines
                .iter()
                .filter(|e| e.text.to_lowercase().contains(&self.filter.to_lowercase()))
                .count()
        }
    }
}

impl Interactive for LogViewState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up => {
                self.scroll_up(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.scroll_down(1, 10);
                Outcome::Consumed
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
                Outcome::Consumed
            }
            KeyCode::PageDown => {
                self.scroll_down(10, 10);
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.scroll = 0;
                self.follow = false;
                Outcome::Consumed
            }
            KeyCode::End => {
                self.follow = true;
                self.scroll = self.lines.len().saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Char('f') => {
                self.follow = !self.follow;
                if self.follow {
                    self.scroll = self.lines.len().saturating_sub(1);
                }
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let out = self.vbar.handle_mouse(m);
        if out.is_consumed() {
            self.scroll = self.vbar.offset;
            self.follow = false;
            return out;
        }

        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                self.scroll_down(3, 10);
            } else {
                self.scroll_up(3);
            }
            return Outcome::Consumed;
        }

        Outcome::Ignored
    }
}

/// Log view widget with scrolling and filtering.
#[derive(Clone, Debug)]
pub struct LogView {
    timestamps: bool,
    level_column: bool,
    wrap_text: bool,
    max_lines: Option<usize>,
    border: Option<Border>,
    title: Option<String>,
    theme: Option<Theme>,
}

impl LogView {
    pub fn new() -> Self {
        Self {
            timestamps: false,
            level_column: true,
            wrap_text: false,
            max_lines: None,
            border: None,
            title: None,
            theme: None,
        }
    }

    pub fn timestamps(mut self, v: bool) -> Self {
        self.timestamps = v;
        self
    }
    pub fn level_column(mut self, v: bool) -> Self {
        self.level_column = v;
        self
    }
    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap_text = v;
        self
    }
    pub fn max_lines(mut self, n: usize) -> Self {
        self.max_lines = Some(n);
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
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for LogView {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for LogView {
    type State = LogViewState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 5 || area.height < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        state.hit.set_area(area);
        state.set_timestamps(self.timestamps);
        state.set_level_column(self.level_column);
        state.set_wrap(self.wrap_text);
        if let Some(max) = self.max_lines {
            state.max_lines = Some(max);
        }

        let mut inner = area;
        if let Some(border) = self.border {
            if let Some(title) = &self.title {
                border.draw_titled(buf, area, th.border, th.surface, title, Alignment::Left);
            } else {
                border.draw(buf, area, th.border, th.surface);
            }
            inner = Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            };
        }

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // Reserve scrollbar column
        let content_w = inner.width.saturating_sub(1);
        let _content_area = Rect {
            x: inner.x,
            y: inner.y,
            width: content_w,
            height: inner.height,
        };

        // Compute visible entries
        let entries: Vec<&LogEntry> =
            if state.filter.is_empty() || state.filter_mode == LogFilter::Highlight {
                state.lines.iter().collect()
            } else {
                state
                    .lines
                    .iter()
                    .filter(|e| e.text.to_lowercase().contains(&state.filter.to_lowercase()))
                    .collect()
            };

        let total = entries.len();
        let viewport = inner.height as usize;
        if state.follow && total > 0 {
            state.scroll = total.saturating_sub(viewport);
        }
        state.scroll = state.scroll.min(total.saturating_sub(1));

        // Render entries
        for (y, (_i, entry)) in (inner.y..).zip(entries.iter().enumerate().skip(state.scroll)) {
            if y >= inner.y + inner.height {
                break;
            }

            let mut x = inner.x;
            let variant = entry.level.variant();
            let fg = th.text_variant(variant);

            // timestamp
            if self.timestamps
                && let Some((h, m, s)) = entry.time
            {
                let ts = format!("{:02}:{:02}:{:02} ", h, m, s);
                put(
                    buf,
                    x,
                    y,
                    &ts,
                    ts.len() as u16,
                    st(th.text_muted, th.surface),
                );
                x += ts.len() as u16;
            }

            // level column
            if self.level_column {
                let level_badge = entry.level.label();
                put(
                    buf,
                    x,
                    y,
                    level_badge,
                    level_badge.len() as u16,
                    st(fg, th.surface).add_modifier(Modifier::BOLD),
                );
                x += level_badge.len() as u16 + 1;
            }

            // text (with filter highlight)
            let remain = inner.width.saturating_sub(x - inner.x).saturating_sub(1);
            if remain > 0 {
                let text = &entry.text;
                if !state.filter.is_empty() && state.filter_mode == LogFilter::Highlight {
                    // highlight matches
                    let lower_text = text.to_lowercase();
                    let lower_filter = state.filter.to_lowercase();
                    let mut last = 0;
                    let mut cx = x;
                    for mat in lower_text.match_indices(&lower_filter) {
                        let pre = &text[last..mat.0];
                        let pre_w = pre.len() as u16;
                        put(
                            buf,
                            cx,
                            y,
                            pre,
                            pre_w.min(remain.saturating_sub(cx - x)),
                            st(th.text, th.surface),
                        );
                        cx += pre_w;
                        let matched = &text[mat.0..mat.0 + state.filter.len()];
                        let mat_w = matched.len() as u16;
                        put(
                            buf,
                            cx,
                            y,
                            matched,
                            mat_w.min(remain.saturating_sub(cx - x)),
                            st(th.accent, th.surface).add_modifier(Modifier::BOLD),
                        );
                        cx += mat_w;
                        last = mat.0 + state.filter.len();
                        if cx >= x + remain {
                            break;
                        }
                    }
                    if last < text.len() && cx < x + remain {
                        let tail = &text[last..];
                        put(
                            buf,
                            cx,
                            y,
                            tail,
                            remain.saturating_sub(cx - x),
                            st(th.text, th.surface),
                        );
                    }
                } else {
                    let display = crate::draw::truncate(text, remain as usize);
                    put(buf, x, y, &display, remain, st(th.text, th.surface));
                }
            }
        }

        // Scrollbar
        let sb_area = Rect {
            x: inner.x + content_w,
            y: inner.y,
            width: 1,
            height: inner.height,
        };
        Scrollbar::vertical(total, viewport)
            .offset(state.scroll)
            .theme(&th)
            .render(sb_area, buf, &mut state.vbar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_ring_buffer() {
        let mut state = LogViewState::new();
        state.max_lines = Some(3);
        state.push(LogLevel::Info, "1");
        state.push(LogLevel::Info, "2");
        state.push(LogLevel::Info, "3");
        state.push(LogLevel::Info, "4");
        assert_eq!(state.lines.len(), 3);
        assert_eq!(state.lines[0].text, "2");
    }

    #[test]
    fn log_auto_follow() {
        let mut state = LogViewState::new();
        state.push(LogLevel::Info, "1");
        assert!(state.follow);
        state.scroll_up(1);
        assert!(!state.follow);
        state.push(LogLevel::Info, "2");
        assert!(!state.follow);
    }

    #[test]
    fn log_filter_only() {
        let mut state = LogViewState::new();
        state.push(LogLevel::Info, "hello");
        state.push(LogLevel::Warn, "world");
        state.push(LogLevel::Info, "hello again");
        state.filter = "hello".to_string();
        state.filter_mode = LogFilter::Only;
        assert_eq!(state.visible_count(), 2);
    }
}
