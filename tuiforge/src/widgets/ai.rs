//! AI and LLM widgets: chat views, streaming text, tool calls, token usage gauges, diffs,
//! thinking indicators, approvals, and token heatmaps.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! use tuiforge::widgets::ai::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! let mut state = ChatState::default();
//! state.push(ChatMessage::new(Role::User, "Hello!"));
//! state.push(ChatMessage::new(Role::Assistant, "Hi! How can I help?"));
//! ChatView::new().bubbles(true).now(now).render(area, &mut buf, &mut state);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::anim::{blink, since};
use crate::core::{Hit, HitBox, Interactive, Outcome, is_press, wheel_delta};
use crate::draw::{Border, fill, put, put_centered, put_right, st, wrap};
use crate::theme::{self, Rgb, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState};
use crate::widgets::spinner::{SpinnerDef, spinners};
use crate::widgets::textarea::{TextArea, TextAreaState};

// ───────────────────────────── Role ─────────────────────────────

/// Role in a chat conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl Role {
    /// Role color from theme.
    pub fn color(self, th: &Theme) -> Rgb {
        match self {
            Role::User => th.primary,
            Role::Assistant => th.accent,
            Role::System => th.text_muted,
            Role::Tool => th.secondary,
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
            Role::Tool => "Tool",
        }
    }
}

// ───────────────────────────── ChatMessage ─────────────────────────────

/// One message in a chat conversation.
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: Role,
    pub author: Option<String>,
    pub text: String,
    pub time: Option<String>,
    pub streaming: bool,
}

impl ChatMessage {
    /// Create a message with role and text.
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self { role, author: None, text: text.into(), time: None, streaming: false }
    }

    /// Set author name.
    pub fn author(mut self, name: impl Into<String>) -> Self {
        self.author = Some(name.into());
        self
    }

    /// Set timestamp.
    pub fn time(mut self, t: impl Into<String>) -> Self {
        self.time = Some(t.into());
        self
    }

    /// Mark as streaming.
    pub fn streaming(mut self, v: bool) -> Self {
        self.streaming = v;
        self
    }
}

// ───────────────────────────── ChatState ─────────────────────────────

/// State for a chat view: messages, scroll position, follow mode.
#[derive(Clone, Debug, Default)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub scroll: usize,
    pub follow: bool,
    pub hit: HitBox,
    pub scrollbar_state: ScrollbarState,
}

impl ChatState {
    /// Create empty chat state.
    pub fn new() -> Self {
        Self { follow: true, ..Default::default() }
    }

    /// Add a message.
    pub fn push(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
    }

    /// Append text to the last message (for token streaming).
    pub fn append_text(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut() {
            last.text.push_str(text);
        }
    }

    /// Clear streaming flag on the last message.
    pub fn finish_stream(&mut self) {
        if let Some(last) = self.messages.last_mut() {
            last.streaming = false;
        }
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll = 0;
    }

    /// Scroll to end.
    pub fn scroll_to_end(&mut self) {
        self.follow = true;
    }

    /// Total rows the conversation takes at `width` with the default look (`ChatView::new()`).
    pub fn total_rows(&self, width: u16) -> usize {
        ChatView::new().rows(self, width).len()
    }
}

impl Interactive for ChatState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                let before = self.scroll;
                self.scroll = self.scroll.saturating_sub(1);
                self.follow = false;
                Outcome::changed_if(before != self.scroll)
            }
            KeyCode::Down => {
                let before = self.scroll;
                self.scroll = self.scroll.saturating_add(1);
                Outcome::changed_if(before != self.scroll)
            }
            KeyCode::PageUp => {
                let before = self.scroll;
                self.scroll = self.scroll.saturating_sub(10);
                self.follow = false;
                Outcome::changed_if(before != self.scroll)
            }
            KeyCode::PageDown => {
                let before = self.scroll;
                self.scroll = self.scroll.saturating_add(10);
                Outcome::changed_if(before != self.scroll)
            }
            KeyCode::Home => {
                let before = self.scroll;
                self.scroll = 0;
                self.follow = false;
                Outcome::changed_if(before != self.scroll)
            }
            KeyCode::End => {
                self.follow = true;
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        if let Some(d) = wheel_delta(&m) {
            let before = self.scroll;
            self.scroll = (self.scroll as i64 + d as i64 * 3).max(0) as usize;
            self.follow = d > 0 && self.follow; // scrolling up leaves follow mode; render re-arms it at the bottom
            out = Outcome::changed_if(before != self.scroll);
        }
        out | self.scrollbar_state.handle_mouse(m)
    }
}

// ───────────────────────────── ChatView ─────────────────────────────

/// Chat view with bubbles or full-width messages.
#[derive(Clone, Debug)]
pub struct ChatView {
    bubbles: bool,
    show_time: bool,
    focused: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
    max_width: Option<u16>,
}

impl ChatView {
    /// Create a chat view.
    pub fn new() -> Self {
        Self { bubbles: true, show_time: false, focused: false, now: None, theme: None, max_width: None }
    }

    /// Enable bubble mode (user messages right-aligned).
    pub fn bubbles(mut self, v: bool) -> Self {
        self.bubbles = v;
        self
    }

    /// Show timestamps.
    pub fn show_time(mut self, v: bool) -> Self {
        self.show_time = v;
        self
    }

    /// Set focus state.
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    /// Set animation time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Maximum bubble width.
    pub fn max_width(mut self, w: u16) -> Self {
        self.max_width = Some(w);
        self
    }
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}

/// One laid-out row of the conversation.
struct Row {
    /// Column offset inside the view and painted width (bubbles are narrower than the view).
    x: u16,
    w: u16,
    text: String,
    kind: RowKind,
    role: Role,
    /// Right-aligned secondary text (time) on author rows.
    right: Option<String>,
    /// Streaming caret after the text.
    caret: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Author,
    Text,
    Code,
    Blank,
}

/// Hard wrap that keeps indentation (for code); `wrap()` collapses leading spaces.
fn hard_wrap(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cw = 0;
    for ch in line.chars() {
        let c = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if cw + c > width && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            cw = 0;
        }
        cur.push(ch);
        cw += c;
    }
    out.push(cur);
    out
}

impl ChatView {
    /// Lay the conversation out as rows for `width`; the same list drives counting, scrolling
    /// and drawing so they can never disagree.
    fn rows(&self, state: &ChatState, width: u16) -> Vec<Row> {
        let mut rows = Vec::new();
        if width < 4 {
            return rows;
        }
        for (mi, msg) in state.messages.iter().enumerate() {
            if mi > 0 {
                rows.push(Row { x: 0, w: 0, text: String::new(), kind: RowKind::Blank, role: msg.role, right: None, caret: false });
            }
            // horizontal placement
            let (x, w) = match msg.role {
                Role::User if self.bubbles => {
                    let max_w = self.max_width.unwrap_or(width * 3 / 4).clamp(8, width);
                    let longest = msg.text.lines().map(|l| l.width()).max().unwrap_or(0) as u16 + 2;
                    let w = longest.max(msg.author.as_deref().unwrap_or("User").width() as u16 + 2).min(max_w);
                    (width - w, w)
                }
                Role::Assistant if self.bubbles => (0, width),
                _ => (0, width),
            };
            let author = msg.author.clone().unwrap_or_else(|| msg.role.label().to_string());
            let right = if self.show_time { msg.time.clone() } else { None };
            rows.push(Row { x, w, text: author, kind: RowKind::Author, role: msg.role, right, caret: false });

            // body: prose is word-wrapped, fenced code is hard-wrapped and keeps indentation
            let text_w = w.saturating_sub(2) as usize;
            let mut in_fence = false;
            for line in msg.text.lines() {
                if line.trim_start().starts_with("```") {
                    in_fence = !in_fence;
                    continue; // the fence itself is not shown; the code background marks it
                }
                let (kind, pieces) = if in_fence { (RowKind::Code, hard_wrap(line, text_w)) } else { (RowKind::Text, wrap(line, text_w)) };
                for piece in pieces {
                    rows.push(Row { x, w, text: piece, kind, role: msg.role, right: None, caret: false });
                }
            }
            if msg.streaming {
                match rows.last_mut() {
                    Some(last) if last.kind != RowKind::Author && last.text.width() + 1 < text_w => last.caret = true,
                    _ => rows.push(Row { x, w, text: String::new(), kind: RowKind::Text, role: msg.role, right: None, caret: true }),
                }
            }
        }
        rows
    }
}

impl StatefulWidget for ChatView {
    type State = ChatState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        if area.width < 4 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let rows = self.rows(state, area.width.saturating_sub(1)); // last column: scrollbar
        let total = rows.len();
        let viewport = area.height as usize;
        let max_scroll = total.saturating_sub(viewport);
        if state.follow || state.scroll >= max_scroll {
            state.scroll = max_scroll;
            state.follow = true;
        }
        let caret_on = blink(self.now.map(crate::anim::since).unwrap_or(0.0), 1.0);

        for (i, row) in rows.iter().enumerate().skip(state.scroll).take(viewport) {
            let y = area.y + (i - state.scroll) as u16;
            let color = row.role.color(&th);
            let (bg, fg) = match (row.role, row.kind, self.bubbles) {
                (_, RowKind::Blank, _) => continue,
                (Role::User, _, true) => (th.surface, th.text),
                (_, RowKind::Code, _) => (th.markdown_code_bg, th.text),
                (Role::System, _, _) | (Role::Tool, RowKind::Text, _) => (th.background, th.text_muted),
                _ => (th.background, th.text),
            };
            let line = Rect { x: area.x + row.x, y, width: row.w, height: 1 };
            fill(buf, line, bg);
            if row.role == Role::Assistant && self.bubbles {
                fill(buf, Rect { width: 1, ..line }, color); // painted role bar
            }
            let inner = Rect { x: line.x + 1, width: line.width.saturating_sub(2), ..line };
            match row.kind {
                RowKind::Author => {
                    let style = st(color, bg).add_modifier(Modifier::BOLD);
                    let text = if row.role == Role::Tool { format!("⚙ {}", row.text) } else { row.text.clone() };
                    if row.role == Role::System {
                        put_centered(buf, inner, &text, st(th.text_muted, bg).add_modifier(Modifier::BOLD));
                    } else {
                        put(buf, inner.x, y, &text, inner.width, style);
                    }
                    if let Some(r) = &row.right {
                        put_right(buf, inner, r, st(th.text_muted, bg));
                    }
                }
                RowKind::Text | RowKind::Code => {
                    let used = if row.role == Role::System {
                        put_centered(buf, inner, &row.text, st(fg, bg));
                        (inner.width + row.text.width() as u16) / 2
                    } else {
                        put(buf, inner.x, y, &row.text, inner.width, st(fg, bg))
                    };
                    if row.caret && caret_on {
                        put(buf, inner.x + used, y, "▌", 1, st(color, bg));
                    }
                }
                RowKind::Blank => {}
            }
        }

        if total > viewport {
            let sb_area = Rect { x: area.right() - 1, width: 1, ..area };
            Scrollbar::vertical(total, viewport).offset(state.scroll).theme(&th).render(sb_area, buf, &mut state.scrollbar_state);
        }
    }
}

// ───────────────────────────── StreamText ─────────────────────────────

/// Streaming text reveal animation.
#[derive(Clone, Debug)]
pub struct StreamText {
    text: String,
    elapsed: f32,
    cps: f32,
    caret: bool,
    theme: Option<Theme>,
}

impl StreamText {
    /// Create with text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), elapsed: 0.0, cps: 40.0, caret: true, theme: None }
    }

    /// Set elapsed time.
    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = e;
        self
    }

    /// Set time from Instant.
    pub fn now(self, now: Instant) -> Self {
        self.elapsed(now.elapsed().as_secs_f32())
    }

    /// Set chars per second.
    pub fn cps(mut self, c: f32) -> Self {
        self.cps = c;
        self
    }

    /// Show caret.
    pub fn caret(mut self, v: bool) -> Self {
        self.caret = v;
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Check if animation is done.
    pub fn done(&self) -> bool {
        (self.elapsed * self.cps) >= self.text.chars().count() as f32
    }
}

/// The prefix of `text` revealed `elapsed` seconds into a stream at `cps` chars per second.
pub fn revealed(text: &str, elapsed: f32, cps: f32) -> &str {
    let n = (elapsed.max(0.0) * cps).floor() as usize;
    match text.char_indices().nth(n) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

impl Widget for StreamText {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 1 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let rev = revealed(&self.text, self.elapsed, self.cps);
        let lines = wrap(rev, area.width as usize);
        let mut y = area.y;
        for line in lines.iter().take(area.height as usize) {
            put(buf, area.x, y, line, area.width, st(th.text, th.background));
            y = y.saturating_add(1);
        }

        if self.caret && !self.done() && y > area.y {
            let last_y = y.saturating_sub(1);
            let empty = String::new();
            let last_line = lines.last().unwrap_or(&empty);
            let x = area.x + last_line.width() as u16;
            if x < area.right() && blink(self.elapsed, 1.0) {
                put(buf, x, last_y, "▌", 1, st(th.primary, th.background));
            }
        }
    }
}

// ───────────────────────────── Thinking ─────────────────────────────

/// One-row "the model is working" indicator: spinner, label with a sweeping shimmer band,
/// optional detail and elapsed counter (`⠋ Thinking… reading 3 files (3.2s)`).
#[derive(Clone, Debug)]
pub struct Thinking {
    label: String,
    detail: Option<String>,
    spinner: &'static SpinnerDef,
    started: Option<Instant>,
    now: Option<Instant>,
    shimmer: bool,
    color: Option<Rgb>,
    theme: Option<Theme>,
}

impl Thinking {
    /// Create with label.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), detail: None, spinner: &spinners::DOTS, started: None, now: None, shimmer: true, color: None, theme: None }
    }

    /// Spinner from the catalog (default `spinners::DOTS`).
    pub fn spinner(mut self, def: &'static SpinnerDef) -> Self {
        self.spinner = def;
        self
    }

    /// Muted detail after the label.
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Start instant for the elapsed counter (no counter without it).
    pub fn started(mut self, t: Instant) -> Self {
        self.started = Some(t);
        self
    }

    /// Current instant; the phase is measured from [`crate::anim::EPOCH`].
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    /// Bright band sweeping across the label (default on).
    pub fn shimmer(mut self, v: bool) -> Self {
        self.shimmer = v;
        self
    }

    /// Spinner colour (default `th.primary`).
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for Thinking {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);
        let el = since(now);
        let bg = th.background;
        let color = self.color.unwrap_or(th.primary);

        let slot = self.spinner.width();
        put(buf, area.x, area.y, self.spinner.frame(el), slot, st(color, bg));
        let mut x = area.x + slot + 1;

        // label: a 4-cell bright band sweeps left→right every 1.6 s
        let label = format!("{}…", self.label);
        let chars: Vec<char> = label.chars().collect();
        let sweep = (el / 1.6).fract() * (chars.len() as f32 + 8.0) - 4.0;
        for (i, ch) in chars.iter().enumerate() {
            if x >= area.right() {
                break;
            }
            let fg = if self.shimmer {
                let d = (i as f32 - sweep).abs();
                th.text_muted.blend(th.text, (1.0 - d / 4.0).clamp(0.0, 1.0))
            } else {
                th.text
            };
            x += put(buf, x, area.y, ch.encode_utf8(&mut [0; 4]), 1, st(fg, bg));
        }

        let mut tail = String::new();
        if let Some(d) = &self.detail {
            tail.push(' ');
            tail.push_str(d);
        }
        if let Some(started) = self.started {
            tail.push_str(&format!(" ({:.1}s)", now.saturating_duration_since(started).as_secs_f32()));
        }
        if !tail.is_empty() && x < area.right() {
            put(buf, x, area.y, &tail, area.right() - x, st(th.text_muted, bg));
        }
    }
}

// ───────────────────────────── TokenUsage ─────────────────────────────

/// Token usage stats.
#[derive(Clone, Copy, Debug, Default)]
pub struct TokenUsage {
    pub prompt: u32,
    pub completion: u32,
    pub limit: u32,
}

impl TokenUsage {
    /// Total used tokens.
    pub fn used(self) -> u32 {
        self.prompt + self.completion
    }

    /// Fraction of limit used.
    pub fn fraction(self) -> f32 {
        if self.limit == 0 {
            return 0.0;
        }
        self.used() as f32 / self.limit as f32
    }
}

/// Format token count: `950`, `12.4k`, `200k`, `1.2M`.
pub fn fmt_tokens(n: u32) -> String {
    let scaled = |v: f32, unit: &str| {
        let s = format!("{v:.1}");
        format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
    };
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        scaled(n as f32 / 1000.0, "k")
    } else {
        scaled(n as f32 / 1_000_000.0, "M")
    }
}

// ───────────────────────────── ContextGauge ─────────────────────────────

/// Token usage gauge with segmented bar.
#[derive(Clone, Debug)]
pub struct ContextGauge {
    usage: TokenUsage,
    compact: bool,
    cost_usd: Option<f32>,
    label: String,
    theme: Option<Theme>,
}

impl ContextGauge {
    /// Create from usage.
    pub fn new(usage: TokenUsage) -> Self {
        Self { usage, compact: false, cost_usd: None, label: "context".to_string(), theme: None }
    }

    /// Compact mode (bar only).
    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    /// Add cost in USD.
    pub fn cost_usd(mut self, c: f32) -> Self {
        self.cost_usd = Some(c);
        self
    }

    /// Set label.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for ContextGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 4 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let frac = self.usage.fraction();
        
        let color = if frac >= 0.95 {
            th.error
        } else if frac >= 0.8 {
            th.warning
        } else {
            th.primary
        };

        if self.compact {
            // bar only
            let prompt_f = self.usage.prompt as f32 / self.usage.limit as f32;
            let used_f = self.usage.used() as f32 / self.usage.limit as f32;
            
            for i in 0..area.width {
                let cell_f = i as f32 / area.width as f32;
                let _next_f = (i + 1) as f32 / area.width as f32;
                
                if cell_f < prompt_f {
                    let c = if frac >= 0.95 { th.error } else if frac >= 0.8 { th.warning } else { th.primary };
                    fill(buf, Rect { x: area.x + i, y: area.y, width: 1, height: 1 }, c);
                } else if cell_f < used_f {
                    let c = if frac >= 0.95 { th.error } else if frac >= 0.8 { th.warning } else { th.accent };
                    fill(buf, Rect { x: area.x + i, y: area.y, width: 1, height: 1 }, c);
                } else {
                    fill(buf, Rect { x: area.x + i, y: area.y, width: 1, height: 1 }, th.surface);
                }
            }
        } else {
            // label + bar
            let mut label_text = format!("{} {} / {}  {:.0}%", 
                self.label, 
                fmt_tokens(self.usage.used()), 
                fmt_tokens(self.usage.limit),
                frac * 100.0
            );
            
            if let Some(cost) = self.cost_usd {
                label_text.push_str(&format!(" ${:.4}", cost));
            }
            
            put(buf, area.x, area.y, &label_text, area.width, st(th.text, th.background));
            
            if area.height > 1 {
                let bar_y = area.y + 1;
                let prompt_w = ((self.usage.prompt as f32 / self.usage.limit as f32) * area.width as f32).round() as u16;
                let used_w = ((self.usage.used() as f32 / self.usage.limit as f32) * area.width as f32).round() as u16;
                
                for i in 0..area.width {
                    if i < prompt_w {
                        fill(buf, Rect { x: area.x + i, y: bar_y, width: 1, height: 1 }, color);
                    } else if i < used_w {
                        let c = if frac >= 0.95 { th.error } else if frac >= 0.8 { th.warning } else { th.accent };
                        fill(buf, Rect { x: area.x + i, y: bar_y, width: 1, height: 1 }, c);
                    } else {
                        fill(buf, Rect { x: area.x + i, y: bar_y, width: 1, height: 1 }, th.surface);
                    }
                }
            }
        }
    }
}

// ───────────────────────────── ToolStatus ─────────────────────────────

/// Tool call execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolStatus {
    Pending,
    Running,
    Done,
    Failed,
}

// ───────────────────────────── ToolCall ─────────────────────────────

/// Tool call display widget.
#[derive(Clone, Debug)]
pub struct ToolCall {
    name: String,
    args: Vec<(String, String)>,
    status: ToolStatus,
    output: String,
    duration_ms: Option<u32>,
    max_output_lines: usize,
    collapsed: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl ToolCall {
    /// Create with name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { 
            name: name.into(), 
            args: Vec::new(), 
            status: ToolStatus::Pending, 
            output: String::new(),
            duration_ms: None,
            max_output_lines: 6,
            collapsed: false,
            now: None,
            theme: None,
        }
    }

    /// Set arguments.
    pub fn args(mut self, a: &[(impl AsRef<str>, impl AsRef<str>)]) -> Self {
        self.args = a.iter().map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string())).collect();
        self
    }

    /// Set status.
    pub fn status(mut self, s: ToolStatus) -> Self {
        self.status = s;
        self
    }

    /// Set output text.
    pub fn output(mut self, o: impl Into<String>) -> Self {
        self.output = o.into();
        self
    }

    /// Set duration.
    pub fn duration_ms(mut self, d: u32) -> Self {
        self.duration_ms = Some(d);
        self
    }

    /// Set max output lines.
    pub fn max_output_lines(mut self, n: usize) -> Self {
        self.max_output_lines = n;
        self
    }

    /// Set collapsed state.
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }

    /// Set time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Calculate height.
    pub fn height(&self, _width: u16) -> u16 {
        if self.collapsed {
            return 3; // border + header + border
        }
        let mut h = 3; // border + header + border
        if !self.output.is_empty() {
            let lines: Vec<&str> = self.output.lines().collect();
            h += lines.len().min(self.max_output_lines) as u16;
        }
        h
    }
}

impl Widget for ToolCall {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 3 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        
        let (glyph, color) = match self.status {
            ToolStatus::Pending => ("○", th.text_muted),
            ToolStatus::Running => {
                let now = self.now.unwrap_or_else(Instant::now);
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let idx = ((now.elapsed().as_secs_f32() * 10.0) as usize) % frames.len();
                (frames.get(idx).copied().unwrap_or("⠋"), th.primary)
            }
            ToolStatus::Done => ("✓", th.success),
            ToolStatus::Failed => ("✗", th.error),
        };

        Border::Round.draw(buf, area, color, th.background);
        let inner = Border::Round.inner(area);
        if inner.height == 0 {
            return;
        }

        let mut header = format!("{} {}", glyph, self.name);
        for (k, v) in &self.args {
            header.push_str(&format!(" {}={}", k, v));
        }
        if let Some(d) = self.duration_ms {
            let dur_str = format!("{}ms", d);
            put_right(buf, Rect { y: inner.y, height: 1, ..inner }, &dur_str, st(th.text_muted, th.background));
        }
        put(buf, inner.x, inner.y, &header, inner.width.saturating_sub(10), st(th.text, th.background));

        if !self.collapsed && !self.output.is_empty() && inner.height > 1 {
            let lines: Vec<&str> = self.output.lines().collect();
            let mut y = inner.y + 1;
            for (_i, line) in lines.iter().enumerate().take(self.max_output_lines) {
                if y >= inner.bottom() {
                    break;
                }
                put(buf, inner.x, y, line, inner.width, st(th.text_muted, th.background));
                y = y.saturating_add(1);
            }
            if lines.len() > self.max_output_lines && y < inner.bottom() {
                let more = format!("… +{} lines", lines.len() - self.max_output_lines);
                put(buf, inner.x, y, &more, inner.width, st(th.text_disabled, th.background));
            }
        }
    }
}

// ───────────────────────────── TokenHeat ─────────────────────────────

/// Token probability heatmap.
#[derive(Clone, Debug)]
pub struct TokenHeat {
    tokens: Vec<(String, f32)>,
    legend: bool,
    theme: Option<Theme>,
}

impl TokenHeat {
    /// Create from tokens and probabilities.
    pub fn new(tokens: &[(&str, f32)]) -> Self {
        Self { tokens: tokens.iter().map(|(t, p)| (t.to_string(), *p)).collect(), legend: false, theme: None }
    }

    /// Show legend.
    pub fn legend(mut self, v: bool) -> Self {
        self.legend = v;
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for TokenHeat {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        
        let mut y = area.y;
        let mut x = area.x;
        
        for (token, prob) in &self.tokens {
            let token_w = token.width() as u16;
            if x + token_w > area.right() {
                x = area.x;
                y = y.saturating_add(1);
                if y >= area.bottom() {
                    break;
                }
            }
            
            // prob → color: low=success (green), mid=warning (yellow), high=error (red)
            let bg = if prob < &0.33 {
                th.success.blend(th.warning, prob * 3.0)
            } else if prob < &0.67 {
                th.warning.blend(th.error, (prob - 0.33) * 3.0)
            } else {
                th.error
            };
            let blended_bg = th.background.blend(bg, 0.35);
            
            put(buf, x, y, token, token_w, st(th.text, blended_bg));
            x = x.saturating_add(token_w);
        }
        
        if self.legend && y + 1 < area.bottom() {
            y = y.saturating_add(1);
            let legend_text = "low ";
            put(buf, area.x, y, legend_text, area.width, st(th.text_muted, th.background));
            let mut lx = area.x + legend_text.width() as u16;
            
            for i in 0..8 {
                if lx >= area.right() {
                    break;
                }
                let p = i as f32 / 7.0;
                let bg = if p < 0.33 {
                    th.success.blend(th.warning, p * 3.0)
                } else if p < 0.67 {
                    th.warning.blend(th.error, (p - 0.33) * 3.0)
                } else {
                    th.error
                };
                fill(buf, Rect { x: lx, y, width: 1, height: 1 }, th.background.blend(bg, 0.35));
                lx = lx.saturating_add(1);
            }
            
            if lx < area.right() {
                put(buf, lx, y, " high", area.width.saturating_sub(lx - area.x), st(th.text_muted, th.background));
            }
        }
    }
}

// ───────────────────────────── DiffKind ─────────────────────────────

/// Diff line type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiffKind {
    Add,
    Del,
    Ctx,
    Hunk,
}

// ───────────────────────────── DiffLine ─────────────────────────────

/// One line in a diff.
#[derive(Clone, Debug)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

// ───────────────────────────── DiffView ─────────────────────────────

/// Unified diff viewer.
#[derive(Clone, Debug)]
pub struct DiffView {
    lines: Vec<DiffLine>,
    file: Option<String>,
    line_numbers: bool,
    scroll: usize,
    theme: Option<Theme>,
}

impl DiffView {
    /// Create from diff lines.
    pub fn new(lines: &[DiffLine]) -> Self {
        Self { lines: lines.to_vec(), file: None, line_numbers: false, scroll: 0, theme: None }
    }

    /// Parse a unified diff. `+`/`-`/` ` prefixes are stripped (the kind carries them), hunk
    /// headers are kept whole, `+++`/`---` file headers are skipped.
    pub fn parse(unified: &str) -> Vec<DiffLine> {
        unified
            .lines()
            .filter(|l| !(l.starts_with("+++") || l.starts_with("---")))
            .map(|l| {
                let (kind, text) = match l.as_bytes().first() {
                    Some(b'+') => (DiffKind::Add, &l[1..]),
                    Some(b'-') => (DiffKind::Del, &l[1..]),
                    Some(b'@') if l.starts_with("@@") => (DiffKind::Hunk, l),
                    Some(b' ') => (DiffKind::Ctx, &l[1..]),
                    _ => (DiffKind::Ctx, l),
                };
                DiffLine { kind, text: text.to_string() }
            })
            .collect()
    }

    /// Set file name.
    pub fn file(mut self, f: impl Into<String>) -> Self {
        self.file = Some(f.into());
        self
    }

    /// Show line numbers.
    pub fn line_numbers(mut self, v: bool) -> Self {
        self.line_numbers = v;
        self
    }

    /// Set scroll offset.
    pub fn scroll(mut self, s: usize) -> Self {
        self.scroll = s;
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Count additions and deletions.
    pub fn stats(lines: &[DiffLine]) -> (usize, usize) {
        let adds = lines.iter().filter(|l| l.kind == DiffKind::Add).count();
        let dels = lines.iter().filter(|l| l.kind == DiffKind::Del).count();
        (adds, dels)
    }
}

/// `@@ -12,3 +12,8 @@` → `(12, 12)`.
fn hunk_start(header: &str) -> Option<(usize, usize)> {
    let mut it = header.split_whitespace().skip(1);
    let old = it.next()?.trim_start_matches('-').split(',').next()?.parse().ok()?;
    let new = it.next()?.trim_start_matches('+').split(',').next()?.parse().ok()?;
    Some((old, new))
}

impl Widget for DiffView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 4 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let mut y = area.y;

        if let Some(file) = &self.file {
            put(buf, area.x, y, file, area.width, st(th.text, th.background).add_modifier(Modifier::BOLD));
            y += 1;
        }

        // line numbers: new-side for additions/context, old-side for deletions
        let (mut old, mut new) = (1usize, 1usize);
        let mut numbered: Vec<(Option<usize>, &DiffLine)> = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            let n = match line.kind {
                DiffKind::Hunk => {
                    if let Some((o, n)) = hunk_start(&line.text) {
                        (old, new) = (o, n);
                    }
                    None
                }
                DiffKind::Add => {
                    new += 1;
                    Some(new - 1)
                }
                DiffKind::Del => {
                    old += 1;
                    Some(old - 1)
                }
                DiffKind::Ctx => {
                    old += 1;
                    new += 1;
                    Some(new - 1)
                }
            };
            numbered.push((n, line));
        }
        let num_w = if self.line_numbers { numbered.iter().filter_map(|(n, _)| *n).max().unwrap_or(0).to_string().len() as u16 + 1 } else { 0 };
        let text_x = area.x + num_w + 2;
        let text_w = area.width.saturating_sub(num_w + 2);

        for (n, line) in numbered.into_iter().skip(self.scroll) {
            if y >= area.bottom() {
                break;
            }
            let (glyph, fg, bg) = match line.kind {
                DiffKind::Add => ("+", th.text, th.background.blend(th.success, 0.15)),
                DiffKind::Del => ("-", th.text, th.background.blend(th.error, 0.15)),
                DiffKind::Ctx => (" ", th.text, th.background),
                DiffKind::Hunk => (" ", th.text_muted, th.surface),
            };
            fill(buf, Rect { x: area.x, y, width: area.width, height: 1 }, bg);
            if line.kind == DiffKind::Hunk {
                put(buf, area.x + 1, y, &line.text, area.width.saturating_sub(1), st(fg, bg));
            } else {
                if let Some(n) = n.filter(|_| num_w > 0) {
                    put(buf, area.x, y, &format!("{n:>w$}", w = num_w as usize - 1), num_w, st(th.text_muted, bg));
                }
                put(buf, area.x + num_w, y, glyph, 1, st(fg, bg));
                put(buf, text_x, y, &line.text, text_w, st(fg, bg));
            }
            y += 1;
        }
    }
}

// ───────────────────────────── ComposerState ─────────────────────────────

/// State for prompt composer.
#[derive(Clone, Debug, Default)]
pub struct ComposerState {
    pub editor: TextAreaState,
    pub hit: HitBox,
    pub submitted: Option<String>,
}

impl ComposerState {
    /// Take submitted text.
    pub fn take_submitted(&mut self) -> Option<String> {
        self.submitted.take()
    }
}

impl Interactive for ComposerState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        
        // Enter without shift → submit
        if k.code == KeyCode::Enter && !k.modifiers.contains(KeyModifiers::SHIFT) {
            let text = self.editor.lines.join("\n");
            if !text.trim().is_empty() {
                self.submitted = Some(text);
                self.editor.lines = vec![String::new()];
                self.editor.cursor = (0, 0);
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        
        // Shift+Enter or Ctrl+J → newline
        if (k.code == KeyCode::Enter && k.modifiers.contains(KeyModifiers::SHIFT))
            || (k.code == KeyCode::Char('j') && k.modifiers.contains(KeyModifiers::CONTROL)) {
            // insert newline
            let (row, col) = self.editor.cursor;
            if row < self.editor.lines.len() {
                let line = &mut self.editor.lines[row];
                let tail = line[col..].to_string();
                line.truncate(col);
                self.editor.lines.insert(row + 1, tail);
                self.editor.cursor = (row + 1, 0);
            }
            return Outcome::Consumed;
        }
        
        // forward to editor
        self.editor.handle_key(k)
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        self.editor.handle_mouse(m)
    }
}

// ───────────────────────────── PromptComposer ─────────────────────────────

/// Prompt composer with model indicator and hints.
#[derive(Clone, Debug)]
pub struct PromptComposer {
    model: String,
    attachments: Vec<String>,
    focused: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl PromptComposer {
    /// Create composer.
    pub fn new() -> Self {
        Self { model: String::new(), attachments: Vec::new(), focused: false, now: None, theme: None }
    }

    /// Set model name.
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = m.into();
        self
    }

    /// Set attachments.
    pub fn attachments(mut self, a: &[impl AsRef<str>]) -> Self {
        self.attachments = a.iter().map(|s| s.as_ref().to_string()).collect();
        self
    }

    /// Set focus.
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    /// Set time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for PromptComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for PromptComposer {
    type State = ComposerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        if area.width < 8 || area.height < 4 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        // the editor draws the Tall frame (focus colour included); one hint row sits under it
        let editor_area = Rect { height: area.height - 1, ..area };
        TextArea::new()
            .placeholder("Message…")
            .focused(self.focused)
            .now(self.now.unwrap_or_else(Instant::now))
            .theme(&th)
            .render(editor_area, buf, &mut state.editor);

        let y = area.bottom() - 1;
        let bg = th.background;
        fill(buf, Rect { y, height: 1, ..area }, bg);

        // right: token estimate (~4 chars per token)
        let chars: usize = state.editor.lines.iter().map(|l| l.chars().count()).sum();
        let tokens = format!("~{} tokens", chars.div_ceil(4));
        let right_w = put_right(buf, Rect { x: area.x, y, width: area.width, height: 1 }, &tokens, st(th.text_disabled, bg));
        let mut x = area.x + 1;
        let limit = area.right().saturating_sub(right_w + 2);

        // left, in priority order, each only if it fits whole: model pill, send hint, attachments, newline hint
        let mut place = |text: &str, style: ratatui::style::Style, x: &mut u16| {
            let w = text.width() as u16;
            if *x + w <= limit {
                put(buf, *x, y, text, w, style);
                *x += w + 2;
            }
        };
        if !self.model.is_empty() {
            place(&format!(" {} ", self.model), st(th.text_primary, th.surface), &mut x);
        }
        place("⏎ send", st(th.text_muted, bg), &mut x);
        for attach in &self.attachments {
            place(&format!("⌘ {attach}"), st(th.text_muted, bg), &mut x);
        }
        place("⇧⏎ newline", st(th.text_muted, bg), &mut x);
    }
}

// ───────────────────────────── ApprovalChoice ─────────────────────────────

/// Approval choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApprovalChoice {
    Once,
    Always,
    Deny,
}

// ───────────────────────────── ApprovalState ─────────────────────────────

/// State for approval dialog.
#[derive(Clone, Debug, Default)]
pub struct ApprovalState {
    pub choice: Option<ApprovalChoice>,
    pub focus: usize,
    pub hits: [HitBox; 3],
}

impl ApprovalState {
    /// Take choice.
    pub fn take_choice(&mut self) -> Option<ApprovalChoice> {
        self.choice.take()
    }
}

impl Interactive for ApprovalState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        
        match k.code {
            KeyCode::Char('y') | KeyCode::Enter if self.focus == 0 => {
                self.choice = Some(ApprovalChoice::Once);
                Outcome::Changed
            }
            KeyCode::Char('a') => {
                self.choice = Some(ApprovalChoice::Always);
                Outcome::Changed
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.choice = Some(ApprovalChoice::Deny);
                Outcome::Changed
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.focus = (self.focus + 1) % 3;
                Outcome::Consumed
            }
            KeyCode::Enter => {
                self.choice = Some(match self.focus {
                    0 => ApprovalChoice::Once,
                    1 => ApprovalChoice::Always,
                    _ => ApprovalChoice::Deny,
                });
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.hits.iter_mut().enumerate() {
            if let Hit::Click = hit.mouse(&m) {
                self.choice = Some(match i {
                    0 => ApprovalChoice::Once,
                    1 => ApprovalChoice::Always,
                    _ => ApprovalChoice::Deny,
                });
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }
}

// ───────────────────────────── Approval ─────────────────────────────

/// Approval dialog widget.
#[derive(Clone, Debug)]
pub struct Approval {
    title: String,
    detail: Option<String>,
    focused: bool,
    theme: Option<Theme>,
}

impl Approval {
    /// Create with title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), detail: None, focused: false, theme: None }
    }

    /// Set detail text.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    /// Set detail text.
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Set focus.
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Rows the card needs at `width`: frame, title, wrapped detail, a gap, the button row.
    pub fn height(&self, width: u16) -> u16 {
        let detail = self.detail.as_ref().map_or(0, |d| wrap(d, width.saturating_sub(2) as usize).len() as u16);
        2 + 1 + detail + 1 + 1
    }
}

impl StatefulWidget for Approval {
    type State = ApprovalState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 12 || area.height < 5 {
            return;
        }
        
        let th = self.theme.unwrap_or_else(theme::current);
        Border::Round.draw(buf, area, th.warning, th.background);
        let inner = Border::Round.inner(area);
        
        if inner.height < 2 {
            return;
        }
        
        let mut y = inner.y;
        put(buf, inner.x, y, &self.title, inner.width, st(th.text, th.background).add_modifier(Modifier::BOLD));
        y = y.saturating_add(1);
        
        if let Some(ref detail) = self.detail {
            let lines = wrap(detail, inner.width as usize);
            for line in lines {
                if y >= inner.bottom().saturating_sub(2) {
                    break;
                }
                put(buf, inner.x, y, &line, inner.width, st(th.text_muted, th.background));
                y = y.saturating_add(1);
            }
        }
        
        // buttons
        let button_y = inner.bottom().saturating_sub(1);
        let specs = [
            ("Allow once", "y", ApprovalChoice::Once, th.success),
            ("Always", "a", ApprovalChoice::Always, th.primary),
            ("Deny", "n", ApprovalChoice::Deny, th.error),
        ];
        
        // three flat buttons, evenly spaced; the label is padded so the whole pill is painted
        let cell_w = inner.width / 3;
        for (i, (label, key, _, color)) in specs.iter().enumerate() {
            let focused = self.focused && state.focus == i;
            let bg = if focused { Theme::shade(*color, 1) } else { *color };
            let text = format!(" {label}  {key} ");
            let w = (text.width() as u16).min(cell_w.max(1));
            let x = inner.x + i as u16 * cell_w + cell_w.saturating_sub(w) / 2;
            let btn = Rect { x, y: button_y, width: w, height: 1 };
            state.hits[i].set_area(btn);
            let mut style = st(bg.text_on(0.9), bg);
            if focused {
                style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }
            fill(buf, btn, bg);
            put(buf, btn.x, btn.y, &text, w, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_append_text_grows_last_message() {
        let mut state = ChatState::new();
        state.push(ChatMessage::new(Role::Assistant, "Hello"));
        state.append_text(" world");
        assert_eq!(state.messages[0].text, "Hello world");
    }

    #[test]
    fn chat_finish_stream_clears_flag() {
        let mut state = ChatState::new();
        state.push(ChatMessage::new(Role::Assistant, "Test").streaming(true));
        assert!(state.messages[0].streaming);
        state.finish_stream();
        assert!(!state.messages[0].streaming);
    }

    #[test]
    fn stream_text_revealed_boundaries() {
        assert_eq!(revealed("hello", 0.0, 10.0), "");
        assert_eq!(revealed("hello", 0.3, 10.0), "hel");
        assert_eq!(revealed("hello", 10.0, 10.0), "hello");
    }

    #[test]
    fn diff_view_parse_produces_correct_kinds() {
        let diff = "+added\n-deleted\n context\n@@ hunk @@";
        let lines = DiffView::parse(diff);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].kind, DiffKind::Add);
        assert_eq!(lines[1].kind, DiffKind::Del);
        assert_eq!(lines[2].kind, DiffKind::Ctx);
        assert_eq!(lines[3].kind, DiffKind::Hunk);
    }

    #[test]
    fn diff_stats_count_adds_and_dels() {
        let lines = vec![
            DiffLine { kind: DiffKind::Add, text: "+a".to_string() },
            DiffLine { kind: DiffKind::Add, text: "+b".to_string() },
            DiffLine { kind: DiffKind::Del, text: "-c".to_string() },
        ];
        let (adds, dels) = DiffView::stats(&lines);
        assert_eq!(adds, 2);
        assert_eq!(dels, 1);
    }

    #[test]
    fn token_usage_fraction_and_fmt() {
        let usage = TokenUsage { prompt: 100, completion: 50, limit: 200 };
        assert_eq!(usage.used(), 150);
        assert!((usage.fraction() - 0.75).abs() < 0.01);
        assert_eq!(fmt_tokens(950), "950");
        assert_eq!(fmt_tokens(12400), "12.4k");
    }

    #[test]
    fn approval_key_a_selects_always() {
        let mut state = ApprovalState::default();
        let k = KeyEvent::from(KeyCode::Char('a'));
        let out = state.handle_key(k);
        assert!(out.is_changed());
        assert_eq!(state.choice, Some(ApprovalChoice::Always));
    }

    #[test]
    fn approval_take_choice_clears() {
        let mut state = ApprovalState { choice: Some(ApprovalChoice::Once), ..Default::default() };
        assert_eq!(state.take_choice(), Some(ApprovalChoice::Once));
        assert_eq!(state.choice, None);
    }

    #[test]
    fn widgets_render_without_panic_at_60x16() {
        let area = Rect::new(0, 0, 60, 16);
        let mut buf = Buffer::empty(area);
        let mut chat_state = ChatState::new();
        ChatView::new().render(area, &mut buf, &mut chat_state);
        StreamText::new("test").render(area, &mut buf);
        Thinking::new("Loading").render(area, &mut buf);
        ContextGauge::new(TokenUsage::default()).render(area, &mut buf);
        ToolCall::new("test").render(area, &mut buf);
        let tokens = [("a", 0.5f32)];
        TokenHeat::new(&tokens).render(area, &mut buf);
        DiffView::new(&[][..]).render(area, &mut buf);
        let mut composer_state = ComposerState::default();
        PromptComposer::new().render(area, &mut buf, &mut composer_state);
        let mut approval_state = ApprovalState::default();
        Approval::new("Test").render(area, &mut buf, &mut approval_state);
    }

    #[test]
    fn widgets_render_without_panic_at_20x3() {
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        let mut chat_state = ChatState::new();
        ChatView::new().render(area, &mut buf, &mut chat_state);
        StreamText::new("test").render(area, &mut buf);
        Thinking::new("Loading").render(area, &mut buf);
        ContextGauge::new(TokenUsage::default()).render(area, &mut buf);
        ToolCall::new("test").render(area, &mut buf);
        let tokens = [("a", 0.5f32)];
        TokenHeat::new(&tokens).render(area, &mut buf);
        let diff: Vec<DiffLine> = vec![];
        DiffView::new(&diff).render(area, &mut buf);
        let mut composer_state = ComposerState::default();
        PromptComposer::new().render(area, &mut buf, &mut composer_state);
        let mut approval_state = ApprovalState::default();
        Approval::new("Test").render(area, &mut buf, &mut approval_state);
    }
}
