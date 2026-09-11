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

use crate::anim::{blink, pulse, since};
use crate::core::{Hit, HitBox, Interactive, Outcome, is_press, wheel_delta};
use crate::draw::{
    Border, Edge, FieldShape, fill, put, put_centered, put_right, st, truncate, wrap,
};
use crate::theme::{self, Rgb, Theme};
use crate::widgets::charts::{Meter, MeterStyle};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState};
use crate::widgets::spinner::{SpinnerDef, spinners};
use crate::widgets::textarea::{TextArea, TextAreaState};

// Role

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

// ChatBlock

/// Block content inside a chat message: text, code, thinking, tool calls, dividers.
#[derive(Clone, Debug)]
pub enum ChatBlock {
    /// Plain text with inline markdown (`**bold**`, `` `code` ``, bullets).
    Text(String),
    /// Code block with optional language.
    Code { lang: Option<String>, text: String },
    /// Thinking block (expandable).
    Thinking {
        text: String,
        secs: f32,
        collapsed: bool,
        streaming: bool,
    },
    /// Tool call result.
    ToolCall {
        name: String,
        summary: String,
        status: ToolStatus,
        duration_ms: Option<u32>,
    },
    /// Horizontal divider with label.
    Divider(String),
}

// ChatMessage

/// One message in a chat conversation.
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: Role,
    pub author: Option<String>,
    pub text: String,
    pub time: Option<String>,
    pub streaming: bool,
    /// Rich blocks (text, code, thinking, tool calls). When empty, `text` is rendered as plain.
    pub blocks: Vec<ChatBlock>,
}

impl ChatMessage {
    /// Create a message with role and text.
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            author: None,
            text: text.into(),
            time: None,
            streaming: false,
            blocks: Vec::new(),
        }
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

    /// Add a block to the message.
    pub fn block(mut self, b: ChatBlock) -> Self {
        self.blocks.push(b);
        self
    }

    /// Set all blocks at once.
    pub fn with_blocks(mut self, blocks: Vec<ChatBlock>) -> Self {
        self.blocks = blocks;
        self
    }
}

// ChatState

/// State for a chat view: messages, scroll position, follow mode.
#[derive(Clone, Debug, Default)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub scroll: usize,
    pub follow: bool,
    pub hit: HitBox,
    pub scrollbar_state: ScrollbarState,
    /// Hovered row index (set from mouse moves when the view has `.hover(true)`).
    pub hover_row: Option<usize>,
    /// Thinking header rows from the last render: (row rect, msg_idx, block_idx).
    row_hits: Vec<(Rect, usize, usize)>,
    /// Rows below the viewport while not following.
    pub unread: usize,
    /// `▾ N new` pill from the last render; a click re-follows.
    pill_hit: HitBox,
    /// Full text being revealed by [`ChatState::stream_tick`].
    streaming_text: Option<String>,
    streaming_start: Option<Instant>,
}

impl ChatState {
    /// Create empty chat state.
    pub fn new() -> Self {
        Self {
            follow: true,
            ..Default::default()
        }
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

    /// Enable auto-scroll: next render scrolls to the last message.
    pub fn scroll_to_end(&mut self) {
        self.follow = true;
    }

    /// Toggle a thinking block collapsed state.
    pub fn toggle_thinking(&mut self, msg_idx: usize, block_idx: usize) {
        if let Some(msg) = self.messages.get_mut(msg_idx)
            && block_idx < msg.blocks.len()
            && let ChatBlock::Thinking { collapsed, .. } = &mut msg.blocks[block_idx]
        {
            *collapsed = !*collapsed;
        }
    }
    /// Push `msg` and reveal `full_text` into it over time with [`ChatState::stream_tick`]. The
    /// text lands in the last `Text` or `Thinking` block, or in `msg.text` when there are none.
    pub fn begin_stream(&mut self, mut msg: ChatMessage, full_text: String, now: Instant) {
        msg.streaming = true;
        self.messages.push(msg);
        self.streaming_text = Some(full_text);
        self.streaming_start = Some(now);
    }

    /// Reveal the streamed text at `cps` chars per second; `true` while still revealing. The
    /// message's `streaming` flag (and a streaming thinking block's) is cleared at the end.
    pub fn stream_tick(&mut self, now: Instant, cps: f32) -> bool {
        let (Some(start), Some(full)) = (self.streaming_start, self.streaming_text.as_deref())
        else {
            return false;
        };
        let elapsed = now.saturating_duration_since(start).as_secs_f32();
        let visible = revealed(full, elapsed, cps);
        let done = visible.len() >= full.len();
        if let Some(msg) = self.messages.last_mut() {
            let target = msg.blocks.iter_mut().rev().find_map(|b| match b {
                ChatBlock::Text(t) => Some((t, None)),
                ChatBlock::Thinking {
                    text, streaming, ..
                } => Some((text, Some(streaming))),
                _ => None,
            });
            match target {
                Some((text, streaming)) => {
                    text.clear();
                    text.push_str(visible);
                    if let Some(s) = streaming {
                        *s = !done;
                    }
                }
                None => {
                    msg.text.clear();
                    msg.text.push_str(visible);
                }
            }
            msg.streaming = !done;
        }
        if done {
            self.streaming_text = None;
            self.streaming_start = None;
        }
        !done
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
            KeyCode::End | KeyCode::Char('G') => {
                self.follow = true;
                self.unread = 0;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        use crate::core::{is_left_down, is_move, mouse_in, mouse_pos};
        if is_move(&m) {
            let before = self.hover_row;
            self.hover_row = mouse_in(self.hit.area, &m)
                .then(|| self.scroll + (mouse_pos(&m).y - self.hit.area.y) as usize);
            return Outcome::changed_if(before != self.hover_row);
        }
        if is_left_down(&m) {
            if self.pill_hit.area.width > 0 && mouse_in(self.pill_hit.area, &m) {
                self.follow = true;
                self.unread = 0;
                return Outcome::Changed;
            }
            let pos = mouse_pos(&m);
            if let Some(&(_, mi, bi)) = self.row_hits.iter().find(|(r, _, _)| r.contains(pos)) {
                self.toggle_thinking(mi, bi);
                return Outcome::Changed;
            }
        }
        let mut out = Outcome::Ignored;
        if let Some(d) = wheel_delta(&m) {
            let before = self.scroll;
            self.scroll = (self.scroll as i64 + d as i64 * 3).max(0) as usize;
            self.follow = d > 0 && self.follow;
            out = Outcome::changed_if(before != self.scroll);
        }
        out | self.scrollbar_state.handle_mouse(m)
    }
}

// ChatView

/// Chat view with bubbles or full-width messages. Assistant messages carry a role-coloured
/// bar on the left; its thickness is `.bar(Edge)` (thin by default, `Edge::Full` for a block).
#[derive(Clone, Debug)]
pub struct ChatView {
    bubbles: bool,
    show_time: bool,
    focused: bool,
    bar: Edge,
    now: Option<Instant>,
    theme: Option<Theme>,
    max_width: Option<u16>,
    compact: bool,
    hover: bool,
}

impl ChatView {
    /// Create a chat view.
    pub fn new() -> Self {
        Self {
            bubbles: true,
            show_time: false,
            focused: false,
            bar: Edge::Thin,
            now: None,
            theme: None,
            max_width: None,
            compact: false,
            hover: false,
        }
    }

    /// Enable bubble mode (user messages right-aligned).
    pub fn bubbles(mut self, v: bool) -> Self {
        self.bubbles = v;
        self
    }

    /// Thickness of the assistant role bar.
    pub fn bar(mut self, e: Edge) -> Self {
        self.bar = e;
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

    /// Compact mode (no blank rows between messages).
    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    /// Enable hover highlighting.
    pub fn hover(mut self, v: bool) -> Self {
        self.hover = v;
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

/// Inline style of a run inside a text row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Inline {
    Plain,
    Bold,
    Code,
    Heading,
}

/// Split one markdown-ish line into styled runs: `**bold**`, `` `code` ``, `#`/`##` headings,
/// `- `/`* ` bullets become `• `; numbered lists are kept as written.
fn inline_runs(line: &str) -> Vec<(String, Inline)> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    if let Some(rest) = trimmed
        .strip_prefix("## ")
        .or_else(|| trimmed.strip_prefix("# "))
    {
        return vec![(format!("{indent}{rest}"), Inline::Heading)];
    }
    let (prefix, body) = match trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        Some(rest) => (format!("{indent}• "), rest),
        None => (indent.to_string(), trimmed),
    };
    let mut runs: Vec<(String, Inline)> = Vec::new();
    if !prefix.is_empty() {
        runs.push((prefix, Inline::Plain));
    }
    let mut cur = String::new();
    let mut style = Inline::Plain;
    let mut it = body.chars().peekable();
    let flush = |cur: &mut String, style: Inline, runs: &mut Vec<(String, Inline)>| {
        if !cur.is_empty() {
            runs.push((std::mem::take(cur), style));
        }
    };
    while let Some(c) = it.next() {
        match c {
            '`' => {
                flush(&mut cur, style, &mut runs);
                style = if style == Inline::Code {
                    Inline::Plain
                } else {
                    Inline::Code
                };
            }
            '*' if style != Inline::Code && it.peek() == Some(&'*') => {
                it.next();
                flush(&mut cur, style, &mut runs);
                style = if style == Inline::Bold {
                    Inline::Plain
                } else {
                    Inline::Bold
                };
            }
            _ => cur.push(c),
        }
    }
    flush(&mut cur, style, &mut runs);
    if runs.is_empty() {
        runs.push((String::new(), Inline::Plain));
    }
    runs
}

/// Greedy word wrap over styled runs; every output row is a list of runs whose widths sum to
/// at most `width`.
fn wrap_runs(runs: &[(String, Inline)], width: usize) -> Vec<Vec<(String, Inline)>> {
    let width = width.max(1);
    // invariant: `rows` starts with one row and only ever grows, so `last_mut()` is always Some
    let mut rows: Vec<Vec<(String, Inline)>> = vec![Vec::new()];
    let mut used = 0usize;
    let push =
        |rows: &mut Vec<Vec<(String, Inline)>>, used: &mut usize, word: &str, style: Inline| {
            let w = word.width();
            if *used > 0 && *used + w > width {
                rows.push(Vec::new());
                *used = 0;
            }
            // a word longer than the row is hard-split
            let mut rest = word;
            while rest.width() > width {
                let mut cut = 0;
                let mut cw = 0;
                for (i, ch) in rest.char_indices() {
                    let c = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    if cw + c > width - *used {
                        break;
                    }
                    cw += c;
                    cut = i + ch.len_utf8();
                }
                if cut == 0 {
                    rows.push(Vec::new());
                    *used = 0;
                    continue;
                }
                rows.last_mut()
                    .expect("rows is never empty")
                    .push((rest[..cut].to_string(), style));
                rows.push(Vec::new());
                *used = 0;
                rest = &rest[cut..];
            }
            let row = rows.last_mut().expect("rows is never empty");
            let leading = *used == 0;
            let piece = if leading { rest.trim_start() } else { rest };
            if piece.is_empty() {
                return;
            }
            match row.last_mut() {
                Some((t, s)) if *s == style => t.push_str(piece),
                _ => row.push((piece.to_string(), style)),
            }
            *used += piece.width();
        };
    for (text, style) in runs {
        // keep the space attached to the word before it so runs re-join seamlessly
        let mut start = 0;
        let bytes = text.as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            if *b == b' ' {
                let word = &text[start..=i];
                push(&mut rows, &mut used, word, *style);
                start = i + 1;
            }
        }
        if start < text.len() {
            push(&mut rows, &mut used, &text[start..], *style);
        }
    }
    rows
}

/// One laid-out row of the conversation.
struct Row {
    /// Column offset inside the view and painted width (bubbles are narrower than the view).
    x: u16,
    w: u16,
    text: String,
    /// Styled runs for markdown text rows; empty for every other kind.
    runs: Vec<(String, Inline)>,
    kind: RowKind,
    role: Role,
    /// Right-aligned secondary text: time on author rows, language on the first code row,
    /// duration on tool rows.
    right: Option<String>,
    /// Streaming caret after the text.
    caret: bool,
    /// Tool status on tool rows.
    status: Option<ToolStatus>,
    /// Message and block index, for thinking-header clicks.
    msg_idx: usize,
    block_idx: usize,
}

impl Row {
    fn new(x: u16, w: u16, kind: RowKind, role: Role, msg_idx: usize, block_idx: usize) -> Self {
        Self {
            x,
            w,
            text: String::new(),
            runs: Vec::new(),
            kind,
            role,
            right: None,
            caret: false,
            status: None,
            msg_idx,
            block_idx,
        }
    }
    fn text(mut self, t: impl Into<String>) -> Self {
        self.text = t.into();
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Author,
    Text,
    Code,
    Blank,
    ThinkingHeader,
    ThinkingBody,
    ToolCall,
    Divider,
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
        let mut rows: Vec<Row> = Vec::new();
        if width < 4 {
            return rows;
        }
        for (mi, msg) in state.messages.iter().enumerate() {
            // a lone divider is a full-width rule, not a message
            if let [ChatBlock::Divider(label)] = msg.blocks.as_slice() {
                if mi > 0 && !self.compact {
                    rows.push(Row::new(0, 0, RowKind::Blank, msg.role, mi, 0));
                }
                rows.push(
                    Row::new(0, width, RowKind::Divider, msg.role, mi, 0).text(label.clone()),
                );
                continue;
            }
            if mi > 0 && !self.compact {
                rows.push(Row::new(0, 0, RowKind::Blank, msg.role, mi, 0));
            }
            // horizontal placement: user bubbles hug the right edge
            let (x, w) = if msg.role == Role::User && self.bubbles {
                let max_w = self.max_width.unwrap_or(width * 3 / 4).clamp(8, width);
                let longest = msg
                    .text
                    .lines()
                    .chain(
                        msg.blocks
                            .iter()
                            .filter_map(|b| match b {
                                ChatBlock::Text(t) => Some(t.as_str()),
                                _ => None,
                            })
                            .flat_map(|t| t.lines()),
                    )
                    .map(|l| l.width())
                    .max()
                    .unwrap_or(0) as u16
                    + 2;
                let author_w = msg.author.as_deref().unwrap_or("User").width() as u16 + 2;
                let w = longest.max(author_w).min(max_w);
                (width - w, w)
            } else {
                (0, width)
            };
            let author = msg
                .author
                .clone()
                .unwrap_or_else(|| msg.role.label().to_string());
            let mut head = Row::new(x, w, RowKind::Author, msg.role, mi, 0).text(author);
            head.right = if self.show_time {
                msg.time.clone()
            } else {
                None
            };
            rows.push(head);

            let text_w = w.saturating_sub(2) as usize;
            let first_body = rows.len();
            if msg.blocks.is_empty() {
                let mut in_fence = false;
                for line in msg.text.lines() {
                    if line.trim_start().starts_with("```") {
                        in_fence = !in_fence;
                        continue;
                    }
                    if in_fence {
                        for piece in hard_wrap(line, text_w) {
                            rows.push(Row::new(x, w, RowKind::Code, msg.role, mi, 0).text(piece));
                        }
                    } else {
                        for piece in wrap(line, text_w) {
                            rows.push(Row::new(x, w, RowKind::Text, msg.role, mi, 0).text(piece));
                        }
                    }
                }
            }
            for (bi, block) in msg.blocks.iter().enumerate() {
                match block {
                    ChatBlock::Text(text) => {
                        for line in text.lines() {
                            if line.trim().is_empty() {
                                rows.push(Row::new(x, w, RowKind::Text, msg.role, mi, bi));
                                continue;
                            }
                            for runs in wrap_runs(&inline_runs(line), text_w) {
                                let mut row = Row::new(x, w, RowKind::Text, msg.role, mi, bi);
                                row.runs = runs;
                                rows.push(row);
                            }
                        }
                        // a streaming text block that has not produced a line yet still gets a row
                        if text.is_empty() {
                            rows.push(Row::new(x, w, RowKind::Text, msg.role, mi, bi));
                        }
                    }
                    ChatBlock::Code { lang, text } => {
                        // header row carries the language chip; code lines never fight it
                        let mut head = Row::new(x, w, RowKind::Code, msg.role, mi, bi);
                        head.right = Some(lang.clone().unwrap_or_default());
                        rows.push(head);
                        for line in text.lines() {
                            for piece in hard_wrap(line, text_w.saturating_sub(1)) {
                                rows.push(
                                    Row::new(x, w, RowKind::Code, msg.role, mi, bi)
                                        .text(format!(" {piece}")),
                                );
                            }
                        }
                    }
                    ChatBlock::Thinking {
                        text,
                        secs,
                        collapsed,
                        streaming,
                    } => {
                        let header = if *streaming {
                            "▾ Thinking…".to_string()
                        } else if *collapsed {
                            format!("▸ Thought for {secs:.1}s")
                        } else {
                            format!("▾ Thought for {secs:.1}s")
                        };
                        let mut row =
                            Row::new(x, w, RowKind::ThinkingHeader, msg.role, mi, bi).text(header);
                        row.caret = *streaming;
                        rows.push(row);
                        if !*collapsed || *streaming {
                            for line in text.lines() {
                                for piece in wrap(line, text_w.saturating_sub(2)) {
                                    rows.push(
                                        Row::new(x, w, RowKind::ThinkingBody, msg.role, mi, bi)
                                            .text(format!("  {piece}")),
                                    );
                                }
                            }
                        }
                    }
                    ChatBlock::ToolCall {
                        name,
                        summary,
                        status,
                        duration_ms,
                    } => {
                        let mut row = Row::new(x, w, RowKind::ToolCall, msg.role, mi, bi);
                        row.status = Some(*status);
                        row.text = name.clone();
                        row.runs = vec![(summary.clone(), Inline::Plain)];
                        row.right = duration_ms.map(fmt_ms);
                        rows.push(row);
                    }
                    ChatBlock::Divider(label) => {
                        rows.push(
                            Row::new(x, w, RowKind::Divider, msg.role, mi, bi).text(label.clone()),
                        );
                    }
                }
            }
            if msg.streaming {
                // caret on the last text row, unless a thinking block is the one streaming
                let thinking = rows[first_body..].last().is_some_and(|r| {
                    matches!(r.kind, RowKind::ThinkingHeader | RowKind::ThinkingBody)
                });
                if !thinking {
                    match rows.last_mut() {
                        Some(last) if last.kind == RowKind::Text || last.kind == RowKind::Code => {
                            last.caret = true;
                        }
                        _ => {
                            let mut row = Row::new(x, w, RowKind::Text, msg.role, mi, 0);
                            row.caret = true;
                            rows.push(row);
                        }
                    }
                }
            }
        }
        rows
    }
}

/// `340ms`, `1.2s`, `1m 03s`.
pub fn fmt_ms(ms: u32) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f32 / 1000.0)
    } else {
        format!("{}m {:02}s", ms / 60_000, (ms / 1000) % 60)
    }
}

impl StatefulWidget for ChatView {
    type State = ChatState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        state.row_hits.clear();
        state.pill_hit.set_area(Rect::ZERO);
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
            state.unread = 0;
        } else {
            state.unread = max_scroll - state.scroll;
        }
        let el = self.now.map(since).unwrap_or(0.0);
        let caret_on = blink(el, 1.0);

        for (i, row) in rows.iter().enumerate().skip(state.scroll).take(viewport) {
            let y = area.y + (i - state.scroll) as u16;
            let color = row.role.color(&th);
            let hovered = self.hover && state.hover_row == Some(i) && row.kind != RowKind::Blank;
            let (mut bg, fg) = match (row.role, row.kind, self.bubbles) {
                (_, RowKind::Blank, _) => continue,
                (_, RowKind::Code, _) => (th.markdown_code_bg, th.text),
                (Role::User, _, true) => (th.surface, th.text),
                (_, RowKind::ThinkingHeader | RowKind::ThinkingBody, _) => {
                    (th.background, th.text_muted)
                }
                (Role::System, _, _) | (Role::Tool, RowKind::Text, _) => {
                    (th.background, th.text_muted)
                }
                _ => (th.background, th.text),
            };
            if hovered && row.kind != RowKind::Code {
                bg = th.hover_bg;
            }
            let line = Rect {
                x: area.x + row.x,
                y,
                width: row.w,
                height: 1,
            };
            fill(buf, line, bg);
            if row.role == Role::Assistant && self.bubbles && row.kind != RowKind::Divider {
                self.bar.draw(buf, line.x, y, 1, false, color, bg);
            }
            let inner = Rect {
                x: line.x + 1,
                width: line.width.saturating_sub(2),
                ..line
            };

            match row.kind {
                RowKind::Author => {
                    let text = if row.role == Role::Tool {
                        format!("⊛ {}", row.text)
                    } else {
                        row.text.clone()
                    };
                    if row.role == Role::System {
                        put_centered(
                            buf,
                            inner,
                            &text,
                            st(th.text_muted, bg).add_modifier(Modifier::BOLD),
                        );
                    } else {
                        put(
                            buf,
                            inner.x,
                            y,
                            &text,
                            inner.width,
                            st(color, bg).add_modifier(Modifier::BOLD),
                        );
                    }
                    if let Some(r) = &row.right {
                        put_right(buf, inner, r, st(th.text_muted, bg));
                    }
                }
                RowKind::Text => {
                    let used = if row.runs.is_empty() {
                        if row.role == Role::System {
                            put_centered(buf, inner, &row.text, st(fg, bg));
                            (inner.width + row.text.width() as u16) / 2
                        } else {
                            put(buf, inner.x, y, &row.text, inner.width, st(fg, bg))
                        }
                    } else {
                        let mut x = inner.x;
                        for (text, style) in &row.runs {
                            if x >= inner.right() {
                                break;
                            }
                            let s = match style {
                                Inline::Plain => st(fg, bg),
                                Inline::Bold => st(th.text, bg).add_modifier(Modifier::BOLD),
                                Inline::Code => st(th.accent, th.markdown_code_bg),
                                Inline::Heading => st(color, bg).add_modifier(Modifier::BOLD),
                            };
                            x += put(buf, x, y, text, inner.right() - x, s);
                        }
                        x - inner.x
                    };
                    if row.caret && caret_on {
                        put(buf, inner.x + used, y, "▌", 1, st(color, bg));
                    }
                }
                RowKind::Code => {
                    let used = put(buf, inner.x, y, &row.text, inner.width, st(fg, bg));
                    if let Some(lang) = row.right.as_deref().filter(|l| !l.is_empty()) {
                        // painted language chip on the block's header row
                        let chip = format!(" {lang} ");
                        let cw = (chip.width() as u16).min(inner.width);
                        put(
                            buf,
                            inner.right() - cw,
                            y,
                            &chip,
                            cw,
                            st(th.accent, th.surface),
                        );
                    }
                    if row.caret && caret_on {
                        put(buf, inner.x + used, y, "▌", 1, st(color, bg));
                    }
                }
                RowKind::ThinkingHeader => {
                    let base = st(th.text_muted, bg).add_modifier(Modifier::ITALIC);
                    if row.caret {
                        // a 6-cell brighter window sweeps across the label while thinking streams
                        let chars: Vec<char> = row.text.chars().collect();
                        let n = chars.len() as f32;
                        let sweep = (el / 1.4).fract() * (n + 12.0) - 6.0;
                        let mut x = inner.x;
                        for (ci, ch) in chars.iter().enumerate() {
                            if x >= inner.right() {
                                break;
                            }
                            let d = (ci as f32 - sweep).abs();
                            let fg = th
                                .text_muted
                                .blend(th.text, (1.0 - d / 3.0).clamp(0.0, 1.0));
                            x += put(
                                buf,
                                x,
                                y,
                                ch.encode_utf8(&mut [0; 4]),
                                1,
                                st(fg, bg).add_modifier(Modifier::ITALIC),
                            );
                        }
                    } else {
                        put(buf, inner.x, y, &row.text, inner.width, base);
                    }
                    state.row_hits.push((line, row.msg_idx, row.block_idx));
                }
                RowKind::ThinkingBody => {
                    put(
                        buf,
                        inner.x,
                        y,
                        &row.text,
                        inner.width,
                        st(th.text_muted, bg).add_modifier(Modifier::ITALIC),
                    );
                }
                RowKind::ToolCall => {
                    let status = row.status.unwrap_or(ToolStatus::Pending);
                    let (glyph, gc): (&str, Rgb) = match status {
                        ToolStatus::Pending => ("○", th.text_muted),
                        ToolStatus::Running => (spinners::DOTS.frame(el), th.primary),
                        ToolStatus::Done => ("✓", th.success),
                        ToolStatus::Error => ("✗", th.error),
                    };
                    let right_w = row.right.as_ref().map_or(0, |r| r.width() as u16 + 1);
                    let mut x = inner.x;
                    x += put(buf, x, y, "⊛", 1, st(th.accent, bg));
                    x += put(buf, x, y, " ", 1, st(fg, bg));
                    x += put(buf, x, y, glyph, 1, st(gc, bg));
                    x += put(buf, x, y, " ", 1, st(fg, bg));
                    let name_style = st(th.text, bg).add_modifier(Modifier::BOLD);
                    x += put(
                        buf,
                        x,
                        y,
                        &row.text,
                        inner.right().saturating_sub(x + right_w),
                        name_style,
                    );
                    if let Some((summary, _)) = row.runs.first() {
                        let avail = inner.right().saturating_sub(x + right_w + 2) as usize;
                        if avail > 3 {
                            put(
                                buf,
                                x + 2,
                                y,
                                &truncate(summary, avail),
                                avail as u16,
                                st(th.text_muted, bg),
                            );
                        }
                    }
                    if let Some(r) = &row.right {
                        put_right(buf, inner, r, st(th.text_muted, bg));
                    }
                }
                RowKind::Divider => {
                    let label = if row.text.is_empty() {
                        String::new()
                    } else {
                        format!(" {} ", row.text)
                    };
                    let dash_w = inner.width.saturating_sub(label.width() as u16) as usize;
                    let left = dash_w / 2;
                    let text =
                        format!("{}{}{}", "─".repeat(left), label, "─".repeat(dash_w - left));
                    put(buf, inner.x, y, &text, inner.width, st(th.border, bg));
                }
                RowKind::Blank => {}
            }
        }

        if !state.follow && state.unread > 0 {
            // painted "N new" pill; a click re-follows
            let text = format!(" ▾ {} new ", state.unread);
            let w = (text.width() as u16).min(area.width);
            let pill = Rect {
                x: area.right().saturating_sub(w + 2),
                y: area.bottom() - 1,
                width: w,
                height: 1,
            };
            put(
                buf,
                pill.x,
                pill.y,
                &text,
                w,
                st(th.primary.text_on(0.9), th.primary).add_modifier(Modifier::BOLD),
            );
            state.pill_hit.set_area(pill);
        }

        if total > viewport {
            let sb_area = Rect {
                x: area.right() - 1,
                width: 1,
                ..area
            };
            Scrollbar::vertical(total, viewport)
                .offset(state.scroll)
                .theme(&th)
                .render(sb_area, buf, &mut state.scrollbar_state);
        }
    }
}

/// Cursor style for streaming text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StreamCursor {
    /// Block cursor `▌`.
    #[default]
    Block,
    /// Bar cursor `▏`.
    Bar,
    /// Underline on last char.
    Underline,
    /// No cursor.
    None,
}

// StreamText
pub struct StreamText {
    text: String,
    elapsed: f32,
    cps: f32,
    caret: bool,
    cursor: StreamCursor,
    fade: bool,
    word_mode: bool,
    theme: Option<Theme>,
}

impl StreamText {
    /// Create with text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            elapsed: 0.0,
            cps: 40.0,
            caret: true,
            cursor: StreamCursor::Block,
            fade: false,
            word_mode: false,
            theme: None,
        }
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

    /// Set cursor style.
    pub fn cursor(mut self, c: StreamCursor) -> Self {
        self.cursor = c;
        self
    }

    /// Fade newest chars from muted to text color.
    pub fn fade(mut self, v: bool) -> Self {
        self.fade = v;
        self
    }

    /// Reveal whole words at once.
    pub fn word_mode(mut self, v: bool) -> Self {
        self.word_mode = v;
        self
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

/// Like [`revealed`] but never stops mid-word: the cut advances to the next whitespace.
pub fn revealed_words(text: &str, elapsed: f32, cps: f32) -> &str {
    let head = revealed(text, elapsed, cps);
    if head.len() >= text.len() {
        return text;
    }
    match text[head.len()..].find(char::is_whitespace) {
        Some(off) => &text[..head.len() + off],
        None => text,
    }
}

impl Widget for StreamText {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 1 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.background;
        let rev = if self.word_mode {
            revealed_words(&self.text, self.elapsed, self.cps)
        } else {
            revealed(&self.text, self.elapsed, self.cps)
        };
        let total = rev.chars().count();
        // newest 12 chars fade in from muted to text colour by age
        const FADE: usize = 12;
        let lines = wrap(rev, area.width as usize);
        let mut seen = 0usize;
        let mut end = (area.x, area.y);
        for (y, line) in (area.y..area.bottom()).zip(lines.iter()) {
            let mut x = area.x;
            if self.fade && !self.done() {
                for ch in line.chars() {
                    let age = total.saturating_sub(seen + 1);
                    let t = if age >= FADE {
                        1.0
                    } else {
                        age as f32 / FADE as f32
                    };
                    let fg = th.text_muted.blend(th.text, t);
                    x += put(
                        buf,
                        x,
                        y,
                        ch.encode_utf8(&mut [0; 4]),
                        area.right().saturating_sub(x),
                        st(fg, bg),
                    );
                    seen += 1;
                }
                // the wrap dropped one space per line break
                seen += 1;
            } else {
                x += put(buf, x, y, line, area.width, st(th.text, bg));
            }
            end = (x, y);
        }
        if self.caret && !self.done() && blink(self.elapsed, 1.0) {
            let (x, y) = end;
            match self.cursor {
                StreamCursor::Block if x < area.right() => {
                    put(buf, x, y, "▌", 1, st(th.primary, bg));
                }
                StreamCursor::Bar if x < area.right() => {
                    put(buf, x, y, "▏", 1, st(th.primary, bg));
                }
                StreamCursor::Underline if x > area.x => {
                    if let Some(cell) = buf.cell_mut((x - 1, y)) {
                        cell.modifier |= Modifier::UNDERLINED;
                    }
                }
                _ => {}
            }
        }
    }
}

// TypingIndicator

/// Three-dot typing animation (traveling wave pulse).
pub struct TypingIndicator {
    label: String,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl TypingIndicator {
    /// Create with default label.
    pub fn new() -> Self {
        Self {
            label: "Assistant is typing".to_string(),
            now: None,
            theme: None,
        }
    }

    /// Set label text.
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = s.into();
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
}

impl Default for TypingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TypingIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 1 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let elapsed = self.now.map(since).unwrap_or(0.0);

        let mut x = area.x;
        for i in 0..3 {
            let phase = elapsed - i as f32 * 0.15;
            let t = pulse(phase, 0.9);
            let color = th.text_muted.blend(th.primary, t);
            put(buf, x, area.y, "●", 1, st(color, th.background));
            x = x.saturating_add(1);
            if x >= area.right() {
                return;
            }
            put(buf, x, area.y, " ", 1, st(th.text, th.background));
            x = x.saturating_add(1);
            if x >= area.right() {
                return;
            }
        }

        let label_style = st(th.text_muted, th.background);
        put(
            buf,
            x,
            area.y,
            &self.label,
            area.right().saturating_sub(x),
            label_style,
        );
    }
}

// Thinking

/// One-row "the model is working" indicator: spinner, label with a sweeping shimmer band,
/// optional detail and elapsed counter (`⠋ Thinking… reading 3 files (3.2s)`).
#[derive(Clone, Debug)]
pub struct Thinking {
    label: String,
    detail: Option<String>,
    spinner: &'static SpinnerDef,
    started: Option<Instant>,
    now: Option<Instant>,
    elapsed_secs: Option<f32>,
    show_elapsed: bool,
    shimmer: bool,
    color: Option<Rgb>,
    theme: Option<Theme>,
}

impl Thinking {
    /// Create with label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            spinner: &spinners::DOTS,
            started: None,
            now: None,
            elapsed_secs: None,
            show_elapsed: false,
            shimmer: true,
            color: None,
            theme: None,
        }
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

    /// Set elapsed time explicitly.
    pub fn elapsed(mut self, secs: f32) -> Self {
        self.elapsed_secs = Some(secs);
        self
    }

    /// Append elapsed time label `(N.Ns)`.
    pub fn elapsed_label(mut self, v: bool) -> Self {
        self.show_elapsed = v;
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
        put(
            buf,
            area.x,
            area.y,
            self.spinner.frame(el),
            slot,
            st(color, bg),
        );
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
                th.text_muted
                    .blend(th.text, (1.0 - d / 4.0).clamp(0.0, 1.0))
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
        let secs = self.elapsed_secs.or_else(|| {
            self.started
                .map(|s| now.saturating_duration_since(s).as_secs_f32())
        });
        if let Some(secs) = secs.filter(|_| self.show_elapsed || self.started.is_some()) {
            tail.push_str(&format!(" ({secs:.1}s)"));
        }
        if !tail.is_empty() && x < area.right() {
            put(
                buf,
                x,
                area.y,
                &tail,
                area.right() - x,
                st(th.text_muted, bg),
            );
        }
    }
}

// TokenUsage

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

// ContextGauge

/// Token usage gauge: `context 15.6k / 200k  8%  $0.0123` over a bar. The bar is a [`Meter`],
/// so every meter style works (`Block` default, `Line`, `Segments`, btop `Blocks`/`Dots`) and a
/// gradient can replace the warning/error thresholds.
#[derive(Clone, Debug)]
pub struct ContextGauge<'a> {
    usage: TokenUsage,
    compact: bool,
    cost_usd: Option<f32>,
    label: String,
    style: MeterStyle,
    gradient: Option<&'a [Rgb]>,
    theme: Option<Theme>,
}

impl<'a> ContextGauge<'a> {
    /// Create from usage.
    pub fn new(usage: TokenUsage) -> Self {
        Self {
            usage,
            compact: false,
            cost_usd: None,
            label: "context".to_string(),
            style: MeterStyle::Block,
            gradient: None,
            theme: None,
        }
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

    /// Bar style (any [`MeterStyle`]; `Block` splits prompt/completion, the others show usage).
    pub fn style(mut self, s: MeterStyle) -> Self {
        self.style = s;
        self
    }

    /// Colour stops for the bar (replaces the 80 % warning / 95 % error thresholds).
    pub fn gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.gradient = Some(stops);
        self
    }

    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for ContextGauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 4 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let frac = self.usage.fraction();
        let level = if frac >= 0.95 {
            th.error
        } else if frac >= 0.8 {
            th.warning
        } else {
            th.primary
        };

        let bar_y = if self.compact {
            area.y
        } else {
            let mut text = format!(
                "{} {} / {}  {:.0}%",
                self.label,
                fmt_tokens(self.usage.used()),
                fmt_tokens(self.usage.limit),
                frac * 100.0
            );
            if let Some(cost) = self.cost_usd {
                text.push_str(&format!(" ${cost:.4}"));
            }
            put(
                buf,
                area.x,
                area.y,
                &text,
                area.width,
                st(th.text, th.background),
            );
            if area.height < 2 {
                return;
            }
            area.y + 1
        };
        let bar = Rect {
            x: area.x,
            y: bar_y,
            width: area.width,
            height: 1,
        };

        let mut meter = Meter::new()
            .value(frac.min(1.0))
            .style(self.style)
            .color(level)
            .theme(&th);
        if let Some(g) = self.gradient {
            meter = meter.gradient(g);
        }
        meter.render(bar, buf);

        // the painted style also shows where the prompt ends and the completion begins
        if self.style == MeterStyle::Block && self.gradient.is_none() && self.usage.limit > 0 {
            let prompt_w = ((self.usage.prompt as f32 / self.usage.limit as f32)
                * area.width as f32)
                .round() as u16;
            let used_w = ((self.usage.used() as f32 / self.usage.limit as f32) * area.width as f32)
                .round() as u16;
            let completion = if frac >= 0.8 { level } else { th.accent };
            fill(
                buf,
                Rect {
                    x: area.x + prompt_w.min(area.width),
                    y: bar_y,
                    width: used_w
                        .saturating_sub(prompt_w)
                        .min(area.width.saturating_sub(prompt_w)),
                    height: 1,
                },
                completion,
            );
        }
    }
}

// ToolStatus

/// Tool call execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolStatus {
    Pending,
    Running,
    Done,
    Error,
}

// ToolCall

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
        self.args = a
            .iter()
            .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
            .collect();
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

    /// Rows this block needs: 3 when collapsed, otherwise 3 plus one row per output line.
    /// Independent of width, so the caller can budget layout before it knows the column.
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
            ToolStatus::Error => ("✗", th.error),
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
            put_right(
                buf,
                Rect {
                    y: inner.y,
                    height: 1,
                    ..inner
                },
                &dur_str,
                st(th.text_muted, th.background),
            );
        }
        put(
            buf,
            inner.x,
            inner.y,
            &header,
            inner.width.saturating_sub(10),
            st(th.text, th.background),
        );

        if !self.collapsed && !self.output.is_empty() && inner.height > 1 {
            let lines: Vec<&str> = self.output.lines().collect();
            let mut y = inner.y + 1;
            for (_i, line) in lines.iter().enumerate().take(self.max_output_lines) {
                if y >= inner.bottom() {
                    break;
                }
                put(
                    buf,
                    inner.x,
                    y,
                    line,
                    inner.width,
                    st(th.text_muted, th.background),
                );
                y = y.saturating_add(1);
            }
            if lines.len() > self.max_output_lines && y < inner.bottom() {
                let more = format!("… +{} lines", lines.len() - self.max_output_lines);
                put(
                    buf,
                    inner.x,
                    y,
                    &more,
                    inner.width,
                    st(th.text_disabled, th.background),
                );
            }
        }
    }
}

// TokenHeat

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
        Self {
            tokens: tokens.iter().map(|(t, p)| (t.to_string(), *p)).collect(),
            legend: false,
            theme: None,
        }
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
            put(
                buf,
                area.x,
                y,
                legend_text,
                area.width,
                st(th.text_muted, th.background),
            );
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
                fill(
                    buf,
                    Rect {
                        x: lx,
                        y,
                        width: 1,
                        height: 1,
                    },
                    th.background.blend(bg, 0.35),
                );
                lx = lx.saturating_add(1);
            }

            if lx < area.right() {
                put(
                    buf,
                    lx,
                    y,
                    " high",
                    area.width.saturating_sub(lx - area.x),
                    st(th.text_muted, th.background),
                );
            }
        }
    }
}

// DiffKind

/// Diff line type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiffKind {
    Add,
    Del,
    Ctx,
    Hunk,
}

// DiffLine

/// One line in a diff.
#[derive(Clone, Debug)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

// DiffView

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
        Self {
            lines: lines.to_vec(),
            file: None,
            line_numbers: false,
            scroll: 0,
            theme: None,
        }
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
                DiffLine {
                    kind,
                    text: text.to_string(),
                }
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
    let old = it
        .next()?
        .trim_start_matches('-')
        .split(',')
        .next()?
        .parse()
        .ok()?;
    let new = it
        .next()?
        .trim_start_matches('+')
        .split(',')
        .next()?
        .parse()
        .ok()?;
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
            put(
                buf,
                area.x,
                y,
                file,
                area.width,
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
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
        let num_w = if self.line_numbers {
            numbered
                .iter()
                .filter_map(|(n, _)| *n)
                .max()
                .unwrap_or(0)
                .to_string()
                .len() as u16
                + 1
        } else {
            0
        };
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
            fill(
                buf,
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                bg,
            );
            if line.kind == DiffKind::Hunk {
                put(
                    buf,
                    area.x + 1,
                    y,
                    &line.text,
                    area.width.saturating_sub(1),
                    st(fg, bg),
                );
            } else {
                if let Some(n) = n.filter(|_| num_w > 0) {
                    put(
                        buf,
                        area.x,
                        y,
                        &format!("{n:>w$}", w = num_w as usize - 1),
                        num_w,
                        st(th.text_muted, bg),
                    );
                }
                put(buf, area.x + num_w, y, glyph, 1, st(fg, bg));
                put(buf, text_x, y, &line.text, text_w, st(fg, bg));
            }
            y += 1;
        }
    }
}

// ComposerState

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

        // Shift+Enter or Ctrl+J → newline (the editor's Enter path is grapheme-safe)
        if (k.code == KeyCode::Enter && k.modifiers.contains(KeyModifiers::SHIFT))
            || (k.code == KeyCode::Char('j') && k.modifiers.contains(KeyModifiers::CONTROL))
        {
            return self.editor.handle_key(KeyEvent::from(KeyCode::Enter));
        }

        // forward to editor
        self.editor.handle_key(k)
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        self.editor.handle_mouse(m)
    }
}

// PromptComposer

/// Prompt composer: a text field in one of the omp composer shapes (thin side bars by
/// default) with a hint row - model pill, send/newline keys, attachments, token estimate.
#[derive(Clone, Debug)]
pub struct PromptComposer {
    model: String,
    placeholder: String,
    attachments: Vec<String>,
    shape: FieldShape,
    focused: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl PromptComposer {
    /// Create composer.
    pub fn new() -> Self {
        Self {
            model: String::new(),
            placeholder: "Message…".to_string(),
            attachments: Vec::new(),
            shape: FieldShape::Bars(Edge::Thin),
            focused: false,
            now: None,
            theme: None,
        }
    }

    /// Set model name.
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = m.into();
        self
    }

    /// Placeholder shown while empty.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    /// Field frame: `FieldShape::Bars(Edge::Thin)` by default; `Bar(Edge::Hair)`, `Rule`,
    /// `Round`, `Prompt`, `Tall(..)`, `None`.
    pub fn shape(mut self, s: FieldShape) -> Self {
        self.shape = s;
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
        if area.width < 8 || area.height < 2 + self.shape.vertical_chrome() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        // the editor draws the frame (focus colour included); one hint row sits under it
        let editor_area = Rect {
            height: area.height - 1,
            ..area
        };
        TextArea::new()
            .placeholder(&self.placeholder)
            .shape(self.shape)
            .focused(self.focused)
            .now(self.now.unwrap_or_else(Instant::now))
            .theme(&th)
            .render(editor_area, buf, &mut state.editor);

        let y = area.bottom() - 1;
        let bg = th.background;
        fill(
            buf,
            Rect {
                y,
                height: 1,
                ..area
            },
            bg,
        );

        // right: token estimate (~4 chars per token)
        let chars: usize = state.editor.lines.iter().map(|l| l.chars().count()).sum();
        let tokens = format!("~{} tokens", chars.div_ceil(4));
        let right_w = put_right(
            buf,
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
            &tokens,
            st(th.text_disabled, bg),
        );
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
            place(
                &format!(" {} ", self.model),
                st(th.text_primary, th.surface),
                &mut x,
            );
        }
        place("⏎ send", st(th.text_muted, bg), &mut x);
        for attach in &self.attachments {
            place(&format!("⌘ {attach}"), st(th.text_muted, bg), &mut x);
        }
        place("⇧⏎ newline", st(th.text_muted, bg), &mut x);
    }
}

// ApprovalChoice

/// User choice from approval prompt: once, always, or deny.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApprovalChoice {
    Once,
    Always,
    Deny,
}

// ApprovalState

/// State for approval dialog.
#[derive(Clone, Debug, Default)]
pub struct ApprovalState {
    pub choice: Option<ApprovalChoice>,
    /// Focused button: 0 once, 1 always, 2 deny.
    pub focus: usize,
    pub hits: [HitBox; 3],
    /// Set by a `.danger(true)` render: `Always` is not offered.
    pub no_always: bool,
}

impl ApprovalState {
    /// Return and clear the user's choice, if set.
    pub fn take_choice(&mut self) -> Option<ApprovalChoice> {
        self.choice.take()
    }

    fn choice_at(&self, i: usize) -> ApprovalChoice {
        match i {
            0 => ApprovalChoice::Once,
            1 if !self.no_always => ApprovalChoice::Always,
            _ => ApprovalChoice::Deny,
        }
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
            KeyCode::Char('a') if !self.no_always => {
                self.choice = Some(ApprovalChoice::Always);
                Outcome::Changed
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.choice = Some(ApprovalChoice::Deny);
                Outcome::Changed
            }
            KeyCode::Right | KeyCode::Tab => {
                self.focus = (self.focus + 1) % 3;
                if self.no_always && self.focus == 1 {
                    self.focus = 2;
                }
                Outcome::Consumed
            }
            KeyCode::Left => {
                self.focus = (self.focus + 2) % 3;
                if self.no_always && self.focus == 1 {
                    self.focus = 0;
                }
                Outcome::Consumed
            }
            KeyCode::Enter => {
                self.choice = Some(self.choice_at(self.focus));
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for i in 0..3 {
            if let Hit::Click = self.hits[i].mouse(&m) {
                self.choice = Some(self.choice_at(i));
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }
}

/// Approval visual style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ApprovalStyle {
    /// Card layout (current default).
    #[default]
    Card,
    /// Single-row inline buttons.
    Inline,
    /// Full-width banner.
    Banner,
}

// Approval

/// Approval dialog widget.
#[derive(Clone, Debug)]
pub struct Approval {
    title: String,
    detail: Option<String>,
    command: Option<String>,
    style: ApprovalStyle,
    danger: bool,
    focused: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl Approval {
    /// Create with title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: None,
            command: None,
            style: ApprovalStyle::Card,
            danger: false,
            focused: false,
            now: None,
            theme: None,
        }
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

    /// Set approval style.
    pub fn style(mut self, s: ApprovalStyle) -> Self {
        self.style = s;
        self
    }

    /// Show command preview.
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Mark as dangerous (error colors, no Always button).
    pub fn danger(mut self, v: bool) -> Self {
        self.danger = v;
        self
    }

    /// Set animation time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
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

    /// Rows this approval needs at `width`: Inline 1; Banner 2; Card = frame + title + wrapped
    /// detail + command row + gap + buttons.
    pub fn height(&self, width: u16) -> u16 {
        match self.style {
            ApprovalStyle::Inline => 1,
            ApprovalStyle::Banner => 2,
            ApprovalStyle::Card => {
                let inner_w = width.saturating_sub(2) as usize;
                let title_w = self.title.width() + if self.danger { 2 } else { 0 };
                let title = if title_w > inner_w { 2 } else { 1 };
                let detail = self
                    .detail
                    .as_ref()
                    .map_or(0, |d| wrap(d, inner_w).len() as u16);
                2 + title + detail + u16::from(self.command.is_some()) + 1 + 1
            }
        }
    }

    /// Accent colour: warning, or a slowly pulsing error tone for dangerous requests.
    fn accent(&self, th: &Theme) -> Rgb {
        if self.danger {
            let el = self.now.map(since).unwrap_or(0.0);
            th.error.blend(th.border, pulse(el, 1.4) * 0.6)
        } else {
            th.warning
        }
    }

    /// Paint the choice buttons into `row` (right-aligned when `right`), recording hits.
    fn buttons(
        &self,
        row: Rect,
        buf: &mut Buffer,
        th: &Theme,
        state: &mut ApprovalState,
        right: bool,
        compact: bool,
    ) {
        let specs: [(&str, &str, Rgb); 3] = [
            (if compact { "once" } else { "Allow once" }, "y", th.success),
            (if compact { "always" } else { "Always" }, "a", th.primary),
            (if compact { "deny" } else { "Deny" }, "n", th.error),
        ];
        let visible: Vec<usize> = (0..3).filter(|&i| !(self.danger && i == 1)).collect();
        let texts: Vec<String> = visible
            .iter()
            .map(|&i| {
                let (label, key, _) = specs[i];
                if compact {
                    format!(" [{key}] {label} ")
                } else {
                    format!(" {label}  {key} ")
                }
            })
            .collect();
        let total: u16 =
            texts.iter().map(|t| t.width() as u16).sum::<u16>() + (texts.len() as u16 - 1) * 2;
        let mut x = if right {
            row.right().saturating_sub(total)
        } else if total < row.width {
            row.x + (row.width - total) / 2
        } else {
            row.x
        };
        for hit in state.hits.iter_mut() {
            hit.set_area(Rect::ZERO);
        }
        for (&i, text) in visible.iter().zip(&texts) {
            let w = (text.width() as u16).min(row.right().saturating_sub(x));
            if w == 0 {
                break;
            }
            let focused = self.focused && state.focus == i;
            let bg = if focused {
                Theme::shade(specs[i].2, 1)
            } else {
                specs[i].2
            };
            let btn = Rect {
                x,
                y: row.y,
                width: w,
                height: 1,
            };
            state.hits[i].set_area(btn);
            let mut style = st(bg.text_on(0.9), bg);
            if focused {
                style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }
            fill(buf, btn, bg);
            put(buf, btn.x, btn.y, text, w, style);
            x += w + 2;
        }
    }
}

impl StatefulWidget for Approval {
    type State = ApprovalState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.no_always = self.danger;
        if state.no_always && state.focus == 1 {
            state.focus = 0;
        }
        if area.width < 12 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let accent = self.accent(&th);
        match self.style {
            ApprovalStyle::Inline => {
                // `◔ title   [y] once [a] always [n] deny`
                let row = Rect { height: 1, ..area };
                fill(buf, row, th.background);
                let glyph = if self.danger { "▲" } else { "◔" };
                let mut x = area.x;
                x += put(buf, x, area.y, glyph, 1, st(accent, th.background));
                x += 1;
                // reserve the buttons' width on the right
                let btn_w: u16 = if self.danger { 26 } else { 38 };
                let title_w = area.width.saturating_sub(x - area.x + btn_w + 2);
                put(
                    buf,
                    x,
                    area.y,
                    &truncate(&self.title, title_w as usize),
                    title_w,
                    st(th.text, th.background).add_modifier(Modifier::BOLD),
                );
                self.buttons(row, buf, &th, state, true, true);
            }
            ApprovalStyle::Banner => {
                if area.height < 2 {
                    return;
                }
                let tint = accent.blend(th.background, 0.8);
                let band = Rect { height: 2, ..area };
                fill(buf, band, tint);
                Edge::Full.draw(buf, area.x, area.y, 2, false, accent, tint);
                let text_w = area.width.saturating_sub(3);
                put(
                    buf,
                    area.x + 2,
                    area.y,
                    &truncate(&self.title, text_w as usize),
                    text_w,
                    st(th.text, tint).add_modifier(Modifier::BOLD),
                );
                // second row: command/detail left, compact buttons right
                let btn_w: u16 = if self.danger { 26 } else { 38 };
                let sub_w = area.width.saturating_sub(btn_w + 4);
                let sub = self
                    .command
                    .as_deref()
                    .or(self.detail.as_deref())
                    .unwrap_or("");
                put(
                    buf,
                    area.x + 2,
                    area.y + 1,
                    &truncate(sub, sub_w as usize),
                    sub_w,
                    st(th.text_muted, tint),
                );
                let row = Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width.saturating_sub(1),
                    height: 1,
                };
                self.buttons(row, buf, &th, state, true, true);
            }
            ApprovalStyle::Card => {
                if area.height < 5 {
                    return;
                }
                let bg = th.background;
                Border::Round.draw(buf, area, accent, bg);
                let inner = Border::Round.inner(area);
                let mut y = inner.y;
                let glyph = if self.danger { "▲ " } else { "" };
                let title = format!("{glyph}{}", self.title);
                for line in wrap(&title, inner.width as usize).iter().take(2) {
                    put(
                        buf,
                        inner.x,
                        y,
                        line,
                        inner.width,
                        st(th.text, bg).add_modifier(Modifier::BOLD),
                    );
                    y += 1;
                }
                let button_y = inner.bottom() - 1;
                let cmd_rows = u16::from(self.command.is_some());
                if let Some(detail) = &self.detail {
                    for line in wrap(detail, inner.width as usize) {
                        if y + cmd_rows + 1 >= button_y {
                            break;
                        }
                        put(buf, inner.x, y, &line, inner.width, st(th.text_muted, bg));
                        y += 1;
                    }
                }
                if let Some(cmd) = &self.command
                    && y + 1 < button_y
                {
                    let row = Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    };
                    fill(buf, row, th.markdown_code_bg);
                    put(
                        buf,
                        inner.x + 1,
                        y,
                        &format!(
                            "❯ {}",
                            truncate(cmd, inner.width.saturating_sub(3) as usize)
                        ),
                        inner.width.saturating_sub(1),
                        st(th.text, th.markdown_code_bg),
                    );
                }
                let row = Rect {
                    x: inner.x,
                    y: button_y,
                    width: inner.width,
                    height: 1,
                };
                self.buttons(row, buf, &th, state, false, false);
            }
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
            DiffLine {
                kind: DiffKind::Add,
                text: "+a".to_string(),
            },
            DiffLine {
                kind: DiffKind::Add,
                text: "+b".to_string(),
            },
            DiffLine {
                kind: DiffKind::Del,
                text: "-c".to_string(),
            },
        ];
        let (adds, dels) = DiffView::stats(&lines);
        assert_eq!(adds, 2);
        assert_eq!(dels, 1);
    }

    #[test]
    fn token_usage_fraction_and_fmt() {
        let usage = TokenUsage {
            prompt: 100,
            completion: 50,
            limit: 200,
        };
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
        let mut state = ApprovalState {
            choice: Some(ApprovalChoice::Once),
            ..Default::default()
        };
        assert_eq!(state.take_choice(), Some(ApprovalChoice::Once));
        assert_eq!(state.choice, None);
    }

    fn rich_message() -> ChatMessage {
        ChatMessage::new(Role::Assistant, "")
            .block(ChatBlock::Thinking {
                text: "weigh options".into(),
                secs: 2.5,
                collapsed: true,
                streaming: false,
            })
            .block(ChatBlock::Text(
                "# Plan\n- add **retries**\n- run `cargo test`".into(),
            ))
            .block(ChatBlock::ToolCall {
                name: "bash".into(),
                summary: "cargo test".into(),
                status: ToolStatus::Done,
                duration_ms: Some(340),
            })
            .block(ChatBlock::Code {
                lang: Some("rust".into()),
                text: "fn main() {}\n".into(),
            })
            .block(ChatBlock::Divider("done".into()))
    }

    #[test]
    fn plain_message_rows_unchanged_by_blocks_feature() {
        let mut a = ChatState::new();
        a.push(ChatMessage::new(Role::Assistant, "hello\nworld"));
        // author row + two text rows, independent of the (empty) block list
        assert_eq!(a.total_rows(40), 3);
        assert!(a.messages[0].blocks.is_empty());
    }

    #[test]
    fn inline_markup_splits_runs_and_bullets() {
        let runs = inline_runs("- add **retries** to `fetch`");
        assert_eq!(runs[0], ("• ".to_string(), Inline::Plain));
        assert!(runs.contains(&("retries".to_string(), Inline::Bold)));
        assert!(runs.contains(&("fetch".to_string(), Inline::Code)));
        assert_eq!(
            inline_runs("## Title")[0],
            ("Title".to_string(), Inline::Heading)
        );
        // wrapping keeps every row within the width and never loses text
        let rows = wrap_runs(&inline_runs("one two **three four** five six seven"), 10);
        assert!(
            rows.iter()
                .all(|r| r.iter().map(|(t, _)| t.width()).sum::<usize>() <= 10)
        );
        let joined: String = rows
            .iter()
            .flat_map(|r| r.iter().map(|(t, _)| t.trim_end()))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(joined, "one two three four five six seven");
    }

    #[test]
    fn stream_tick_reveals_monotonically_and_finishes() {
        let t0 = Instant::now();
        let mut s = ChatState::new();
        let msg = ChatMessage::new(Role::Assistant, "").block(ChatBlock::Text(String::new()));
        s.begin_stream(msg, "hello world".into(), t0);
        assert!(s.stream_tick(t0 + std::time::Duration::from_millis(500), 10.0));
        let ChatBlock::Text(t) = &s.messages[0].blocks[0] else {
            panic!()
        };
        assert_eq!(t, "hello");
        assert!(s.messages[0].streaming);
        assert!(!s.stream_tick(t0 + std::time::Duration::from_secs(5), 10.0));
        let ChatBlock::Text(t) = &s.messages[0].blocks[0] else {
            panic!()
        };
        assert_eq!(t, "hello world");
        assert!(!s.messages[0].streaming);
        assert!(
            !s.stream_tick(t0 + std::time::Duration::from_secs(9), 10.0),
            "idle after completion"
        );
    }

    #[test]
    fn clicking_thinking_header_toggles_it() {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        let area = Rect::new(0, 0, 60, 12);
        let mut buf = Buffer::empty(area);
        let mut s = ChatState::new();
        s.push(rich_message());
        ChatView::new().render(area, &mut buf, &mut s);
        let (hit, mi, bi) = s.row_hits[0];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x + 1,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        };
        assert!(s.handle_mouse(click).is_changed());
        let ChatBlock::Thinking { collapsed, .. } = &s.messages[mi].blocks[bi] else {
            panic!()
        };
        assert!(!collapsed);
        // expanded: the body row now exists
        let rows_after = s.total_rows(59);
        s.toggle_thinking(mi, bi);
        assert!(rows_after > s.total_rows(59));
    }

    #[test]
    fn approval_inline_fits_one_row_and_danger_hides_always() {
        let area = Rect::new(0, 0, 70, 1);
        let mut buf = Buffer::empty(area);
        let mut st_ = ApprovalState::default();
        Approval::new("Allow bash?")
            .style(ApprovalStyle::Inline)
            .render(area, &mut buf, &mut st_);
        assert!(
            st_.hits
                .iter()
                .all(|h| h.area.height == 1 && h.area.right() <= 70)
        );
        assert!(st_.hits[1].area.width > 0);
        let mut d = ApprovalState::default();
        Approval::new("rm -rf")
            .danger(true)
            .render(Rect::new(0, 0, 40, 6), &mut buf, &mut d);
        assert_eq!(d.hits[1].area.width, 0, "Always is not offered");
        assert!(
            d.handle_key(KeyEvent::from(KeyCode::Char('a')))
                .is_ignored()
        );
        d.handle_key(KeyEvent::from(KeyCode::Right));
        assert_eq!(d.focus, 2, "focus skips the hidden button");
    }

    #[test]
    fn revealed_words_stops_at_boundaries() {
        assert_eq!(revealed_words("hello big world", 0.3, 10.0), "hello");
        assert_eq!(revealed_words("hello big world", 0.7, 10.0), "hello big");
        assert_eq!(revealed_words("hello", 9.0, 10.0), "hello");
    }

    #[test]
    fn every_widget_survives_all_sizes() {
        for (w, h) in [
            (1, 1),
            (3, 2),
            (10, 3),
            (20, 3),
            (60, 16),
            (130, 42),
            (250, 70),
        ] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            let now = Instant::now();
            let mut chat = ChatState::new();
            chat.push(ChatMessage::new(Role::User, "hi"));
            chat.push(rich_message());
            chat.push(
                ChatMessage::new(Role::Assistant, "")
                    .block(ChatBlock::Thinking {
                        text: "x".into(),
                        secs: 0.0,
                        collapsed: false,
                        streaming: true,
                    })
                    .streaming(true),
            );
            chat.follow = false;
            ChatView::new()
                .hover(true)
                .show_time(true)
                .now(now)
                .render(area, &mut buf, &mut chat);
            ChatView::new()
                .bubbles(false)
                .compact(true)
                .render(area, &mut buf, &mut chat);
            for cursor in [
                StreamCursor::Block,
                StreamCursor::Bar,
                StreamCursor::Underline,
                StreamCursor::None,
            ] {
                StreamText::new("streaming some text here")
                    .elapsed(0.4)
                    .cursor(cursor)
                    .fade(true)
                    .word_mode(true)
                    .render(area, &mut buf);
            }
            TypingIndicator::new().now(now).render(area, &mut buf);
            Thinking::new("Loading")
                .elapsed(4.2)
                .elapsed_label(true)
                .now(now)
                .render(area, &mut buf);
            ContextGauge::new(TokenUsage::default()).render(area, &mut buf);
            ToolCall::new("test").render(area, &mut buf);
            let tokens = [("a", 0.5f32)];
            TokenHeat::new(&tokens).render(area, &mut buf);
            DiffView::new(&[][..]).render(area, &mut buf);
            let mut composer_state = ComposerState::default();
            PromptComposer::new().render(area, &mut buf, &mut composer_state);
            for style in [
                ApprovalStyle::Card,
                ApprovalStyle::Inline,
                ApprovalStyle::Banner,
            ] {
                let mut a = ApprovalState::default();
                Approval::new("Test")
                    .detail("d")
                    .command("cargo test")
                    .style(style)
                    .danger(true)
                    .now(now)
                    .focused(true)
                    .render(area, &mut buf, &mut a);
            }
        }
    }
}
