//! AI harness tools: timeline, shell blocks, code blocks, edit previews, change sets, JSON trees.
//!
//! ```no_run
//! use tuile::prelude::*;
//! use tuile::widgets::ai_tools::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! let mut state = ToolTimelineState::default();
//! state.steps.push(ToolStep::new("read").status(ToolStatus::Done));
//! ToolTimeline::new().now(now).render(area, &mut buf, &mut state);
//! ```

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::{StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::anim::{elapsed, since};
use crate::core::{Highlighter, Hit, HitBox, Interactive, MinSize, Outcome, is_press, wheel_delta};
use crate::draw::{
    Border, Edge, bold, fill, hbar, put, put_highlighted, put_right, refuse, st, truncate,
    truncate_start,
};
use crate::theme::{self, Theme, Variant};
use crate::widgets::ai::{DiffKind, DiffLine, ToolStatus};
use crate::widgets::spinner::spinners;
use crate::widgets::{Scrollbar, ScrollbarState};

// ToolStep

/// One step in a tool execution timeline.
#[derive(Clone, Debug)]
pub struct ToolStep {
    pub name: String,
    pub summary: String,
    pub status: ToolStatus,
    pub started: Option<Instant>,
    pub duration: Option<Duration>,
    pub output: Vec<String>,
    pub depth: u8,
    pub expanded: bool,
}

impl ToolStep {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            summary: String::new(),
            status: ToolStatus::Pending,
            started: None,
            duration: None,
            output: Vec::new(),
            depth: 0,
            expanded: false,
        }
    }

    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = s.into();
        self
    }

    pub fn status(mut self, s: ToolStatus) -> Self {
        self.status = s;
        self
    }

    pub fn started(mut self, i: Instant) -> Self {
        self.started = Some(i);
        self
    }

    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = Some(d);
        self
    }

    pub fn output(mut self, lines: Vec<String>) -> Self {
        self.output = lines;
        self
    }

    pub fn depth(mut self, d: u8) -> Self {
        self.depth = d;
        self
    }

    pub fn expanded(mut self, e: bool) -> Self {
        self.expanded = e;
        self
    }
}

// ToolTimelineState

/// State for [`ToolTimeline`].
#[derive(Clone, Debug, Default)]
pub struct ToolTimelineState {
    pub steps: Vec<ToolStep>,
    pub cursor: usize,
    pub scroll: u16,
    pub hits: Vec<HitBox>,
    focused: bool,
}

impl ToolTimelineState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self, idx: usize) {
        if idx < self.steps.len() {
            self.steps[idx].expanded = !self.steps[idx].expanded;
        }
    }

    pub fn expand_all(&mut self) {
        for step in &mut self.steps {
            step.expanded = true;
        }
    }

    pub fn collapse_all(&mut self) {
        for step in &mut self.steps {
            step.expanded = false;
        }
    }

    pub fn animating(&self, _now: Instant) -> bool {
        self.steps.iter().any(|s| s.status == ToolStatus::Running)
    }
}

impl Interactive for ToolTimelineState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    return Outcome::Consumed;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor + 1 < self.steps.len() {
                    self.cursor += 1;
                    return Outcome::Consumed;
                }
            }
            KeyCode::PageUp => {
                self.cursor = self.cursor.saturating_sub(10);
                return Outcome::Consumed;
            }
            KeyCode::PageDown => {
                self.cursor = (self.cursor + 10).min(self.steps.len().saturating_sub(1));
                return Outcome::Consumed;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle(self.cursor);
                return Outcome::Changed;
            }
            KeyCode::Char('e') => {
                self.expand_all();
                return Outcome::Changed;
            }
            KeyCode::Char('c') => {
                self.collapse_all();
                return Outcome::Changed;
            }
            _ => {}
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if let Some(delta) = wheel_delta(&m) {
            self.scroll = self.scroll.saturating_add_signed(-delta as i16);
            return Outcome::Consumed;
        }
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    self.toggle(i);
                    return Outcome::Changed;
                }
                Hit::HoverChanged => return Outcome::Consumed,
                _ => {}
            }
        }
        Outcome::Ignored
    }
}

// ToolTimeline

/// Tool execution timeline with nested steps, live elapsed time, expandable output.
#[derive(Clone, Debug)]
pub struct ToolTimeline {
    theme: Option<Theme>,
    focused: bool,
    now: Option<Instant>,
    max_output_rows: u16,
}

impl ToolTimeline {
    pub fn new() -> Self {
        Self {
            theme: None,
            focused: false,
            now: None,
            max_output_rows: 6,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn max_output_rows(mut self, n: u16) -> Self {
        self.max_output_rows = n;
        self
    }
}

impl Default for ToolTimeline {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ToolTimeline {
    type State = ToolTimelineState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        state.focused = self.focused;
        state.hits.clear();
        state.hits.resize(state.steps.len(), HitBox::default());

        if area.height == 0 {
            return;
        }

        let mut y = area.y;
        let max_y = area.bottom();
        let mut visible_idx = 0;

        for (step_idx, step) in state.steps.iter().enumerate() {
            if visible_idx < state.scroll as usize {
                visible_idx += 1;
                if step.expanded {
                    visible_idx += (step.output.len() + 1).min(self.max_output_rows as usize);
                }
                continue;
            }

            if y >= max_y {
                break;
            }

            // Step row
            let row_rect = Rect::new(area.x, y, area.width, 1);
            state.hits[step_idx].set_area(row_rect);

            let is_cursor = step_idx == state.cursor;
            let bg = if is_cursor {
                if self.focused { th.cursor_bg } else { th.panel }
            } else {
                th.background
            };
            fill(buf, row_rect, bg);

            let mut x = area.x;
            let w = area.width as usize;

            // Nesting guides: one 2-cell column per depth; `└` closes the last sibling,
            // `│` continues an ancestor whose later siblings still follow
            let steps = &state.steps;
            let later = |depth: u8| {
                steps[step_idx + 1..]
                    .iter()
                    .take_while(|s| s.depth >= depth)
                    .any(|s| s.depth == depth)
            };
            for d in 1..=step.depth {
                if x >= area.right() {
                    break;
                }
                let guide = if d == step.depth {
                    if later(d) { "├" } else { "└" }
                } else if later(d) {
                    "│"
                } else {
                    " "
                };
                put(buf, x, y, guide, 1, st(th.border, bg));
                x += 2;
            }

            // Status glyph - always in its own column after guides
            let (glyph, glyph_color) = match step.status {
                ToolStatus::Pending => ("○", th.text_muted),
                ToolStatus::Running => {
                    let now = self.now.unwrap_or_else(Instant::now);
                    let frame = spinners::DOTS.frame(since(now));
                    (frame, th.primary)
                }
                ToolStatus::Done => ("✓", th.success),
                ToolStatus::Error => ("✗", th.error),
            };
            if x < area.right() {
                put(buf, x, y, glyph, 1, st(glyph_color, bg));
                x += 2; // 1 for glyph + 1 for spacing
            }

            // Name + summary
            let _name_w = step.name.width();
            let _summary_w = step.summary.width();
            let dur_text = match step.status {
                ToolStatus::Running => {
                    let now = self.now.unwrap_or_else(Instant::now);
                    if let Some(started) = step.started {
                        format!(" {}", fmt_ms(elapsed(started, now)))
                    } else {
                        String::new()
                    }
                }
                _ => {
                    if let Some(d) = step.duration {
                        format!(" {}", fmt_ms(d.as_secs_f32()))
                    } else {
                        String::new()
                    }
                }
            };
            let dur_w = dur_text.width();
            let avail = (area.right().saturating_sub(x) as usize).saturating_sub(dur_w);

            let name_style = bold(st(th.text, bg));
            let summary_style = st(th.text_muted, bg);

            if avail > 0 {
                let name = truncate(&step.name, avail);
                let used = put(buf, x, y, &name, avail as u16, name_style) as usize;
                let rest = avail.saturating_sub(used + 2);
                if !step.summary.is_empty() && rest > 3 {
                    put(
                        buf,
                        x + used as u16 + 2,
                        y,
                        &truncate(&step.summary, rest),
                        rest as u16,
                        summary_style,
                    );
                }
            }

            // Duration
            if !dur_text.is_empty() {
                put_right(
                    buf,
                    Rect::new(area.x, y, area.width, 1),
                    &dur_text,
                    st(th.text_muted, bg),
                );
            }

            y += 1;
            visible_idx += 1;

            // Output rows
            if step.expanded && !step.output.is_empty() && y < max_y {
                let out_rows = step.output.len().min(self.max_output_rows as usize);
                for i in 0..out_rows {
                    if y >= max_y {
                        break;
                    }
                    let line = &step.output[i];
                    let indent = (step.depth as usize + 2).min(w);
                    let x_start = area.x + indent as u16;
                    let avail = area.width.saturating_sub(indent as u16);
                    let text = truncate(line, avail as usize);
                    put(buf, x_start, y, &text, avail, st(th.text_muted, bg));
                    y += 1;
                }

                // "+N lines" row
                if step.output.len() > out_rows && y < max_y {
                    let indent = (step.depth as usize + 2).min(w);
                    let x_start = area.x + indent as u16;
                    let avail = area.width.saturating_sub(indent as u16);
                    let more = format!("… +{} lines", step.output.len() - out_rows);
                    put(buf, x_start, y, &more, avail, st(th.text_muted, bg));
                    y += 1;
                }
                visible_idx += out_rows + 1;
            }
        }

        // Scrollbar
        let total_rows: usize = state
            .steps
            .iter()
            .map(|s| {
                let mut r = 1;
                if s.expanded && !s.output.is_empty() {
                    r += s.output.len().min(self.max_output_rows as usize) + 1;
                }
                r
            })
            .sum();
        if total_rows > area.height as usize {
            let mut sb_state = ScrollbarState::default();
            Scrollbar::vertical(total_rows, area.height as usize)
                .offset(state.scroll as usize)
                .theme(&th)
                .render(area, buf, &mut sb_state);
        }
    }
}

// ShellBlock

/// Shell command block with streaming output.
#[derive(Clone, Debug)]
pub struct ShellBlock<'a> {
    theme: Option<Theme>,
    command: &'a str,
    cwd: Option<&'a str>,
    output: &'a [(bool, String)],
    exit_code: Option<i32>,
    duration: Option<Duration>,
    running: bool,
    collapsed: bool,
    max_rows: u16,
    elapsed: Option<f32>,
    lps: f32,
    now: Option<Instant>,
}

impl<'a> ShellBlock<'a> {
    pub fn new() -> Self {
        Self {
            theme: None,
            command: "",
            cwd: None,
            output: &[] as &[(bool, String)],
            exit_code: None,
            duration: None,
            running: false,
            collapsed: false,
            max_rows: 20,
            elapsed: None,
            lps: f32::INFINITY,
            now: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn command(mut self, c: &'a str) -> Self {
        self.command = c;
        self
    }

    pub fn cwd(mut self, c: Option<&'a str>) -> Self {
        self.cwd = c;
        self
    }

    pub fn output(mut self, o: &'a [(bool, String)]) -> Self {
        self.output = o;
        self
    }

    pub fn exit_code(mut self, e: Option<i32>) -> Self {
        self.exit_code = e;
        self
    }

    pub fn duration(mut self, d: Option<Duration>) -> Self {
        self.duration = d;
        self
    }

    pub fn running(mut self, r: bool) -> Self {
        self.running = r;
        self
    }

    pub fn collapsed(mut self, c: bool) -> Self {
        self.collapsed = c;
        self
    }

    pub fn max_rows(mut self, m: u16) -> Self {
        self.max_rows = m;
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn lps(mut self, l: f32) -> Self {
        self.lps = l;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    /// `(lines revealed so far, lines shown)` given the stream clock and the collapsed cap.
    fn counts(&self) -> (usize, usize) {
        let revealed = if self.lps.is_infinite() || self.lps <= 0.0 {
            self.output.len()
        } else {
            ((self.elapsed.unwrap_or(0.0) * self.lps) as usize).min(self.output.len())
        };
        let shown = if self.collapsed {
            revealed.min(self.max_rows as usize)
        } else {
            revealed
        };
        (revealed, shown)
    }

    /// Rows the block takes right now: header + shown lines (+ the `… +N lines` row).
    pub fn height(&self) -> u16 {
        let (revealed, shown) = self.counts();
        1 + shown as u16 + u16::from(self.collapsed && revealed > shown)
    }
}

impl Default for ShellBlock<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ShellBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.height == 0 {
            return;
        }

        let mut y = area.y;

        // Header row: ❯ command [duration] [cwd] [status pill | spinner]
        let header_bg = th.background;
        fill(buf, Rect::new(area.x, y, area.width, 1), header_bg);

        let mut x = area.x;
        put(buf, x, y, "❯", 2, bold(st(th.primary, header_bg)));
        x += 2;

        // Calculate right side widths (from right to left)
        let mut right_w = 0u16;

        // Status (spinner or pill), measured exactly as drawn
        let pill_text = self.exit_code.map(|c| format!("exit {c}"));
        let status_w = if self.running {
            2
        } else {
            pill_text.as_ref().map_or(0, |t| t.width() as u16 + 2)
        };
        right_w += status_w;

        // Duration
        let dur_text = if let Some(d) = self.duration {
            format!(" {}", fmt_ms(d.as_secs_f32()))
        } else {
            String::new()
        };
        let dur_w = dur_text.width() as u16;
        if dur_w > 0 {
            right_w += dur_w + 1; // +1 for gap
        }

        // cwd
        let cwd_w = self.cwd.map(|c| c.width() as u16 + 1).unwrap_or(0); // +1 for gap
        if cwd_w > 0 {
            right_w += cwd_w;
        }

        // Command (truncated to fit)
        let cmd_avail = area.width.saturating_sub(2 + right_w);
        if cmd_avail > 0 {
            let cmd_text = truncate(self.command, cmd_avail as usize);
            put(
                buf,
                x,
                y,
                &cmd_text,
                cmd_avail,
                bold(st(th.text, header_bg)),
            );
        }

        // Draw right side (from right to left)
        let mut right_x = area.right();

        // Status pill / spinner (rightmost)
        if self.running {
            let now = self.now.unwrap_or_else(Instant::now);
            let frame = spinners::DOTS.frame(since(now));
            right_x = right_x.saturating_sub(2);
            put(buf, right_x, y, frame, 2, st(th.primary, header_bg));
        } else if let (Some(code), Some(pill_text)) = (self.exit_code, &pill_text) {
            let pill_bg = th.variant(if code == 0 {
                Variant::Success
            } else {
                Variant::Error
            });
            let pill_w = pill_text.width() as u16 + 2;
            right_x = right_x.saturating_sub(pill_w);
            if right_x >= area.x {
                fill(buf, Rect::new(right_x, y, pill_w, 1), pill_bg);
                put(
                    buf,
                    right_x + 1,
                    y,
                    pill_text,
                    pill_w - 2,
                    st(pill_bg.text_on(1.0), pill_bg),
                );
            }
        }

        // Duration
        if !dur_text.is_empty() {
            right_x = right_x.saturating_sub(1); // gap
            right_x = right_x.saturating_sub(dur_w);
            put(
                buf,
                right_x,
                y,
                &dur_text,
                dur_w,
                st(th.text_muted, header_bg),
            );
        }

        // cwd
        if let Some(cwd) = self.cwd
            && cwd_w > 0
        {
            right_x = right_x.saturating_sub(1); // gap
            let cwd_text = truncate(cwd, (cwd_w - 1) as usize);
            right_x = right_x.saturating_sub(cwd_text.width() as u16);
            put(
                buf,
                right_x,
                y,
                &cwd_text,
                cwd_text.width() as u16,
                st(th.text_muted, header_bg),
            );
        }

        y += 1;

        // Body rows: only as tall as the lines shown (plus the "+N lines" row)
        if y < area.bottom() && !self.output.is_empty() {
            let (revealed_count, display_count) = self.counts();
            let more_row = u16::from(self.collapsed && revealed_count > display_count);
            let body_h = (display_count as u16 + more_row).min(area.bottom().saturating_sub(y));
            let body_rect = Rect::new(area.x, y, area.width, body_h);
            let code_bg = th.markdown_code_bg;
            fill(buf, body_rect, code_bg);

            // Rail
            let rail_color = if self.running {
                th.primary
            } else if let Some(code) = self.exit_code {
                if code == 0 { th.success } else { th.error }
            } else {
                th.border
            };
            Edge::Thin.draw(buf, area.x, y, body_h, false, rail_color, code_bg);

            for i in 0..display_count.min(body_h as usize) {
                if i >= self.output.len() {
                    break;
                }
                let (is_stderr, line) = &self.output[i];
                let x_start = area.x + 2;
                let avail = area.width.saturating_sub(2);
                let text = truncate(line, avail as usize);
                let fg = if *is_stderr {
                    th.error.blend(th.text, 0.7)
                } else {
                    th.text
                };
                put(buf, x_start, y, &text, avail, st(fg, code_bg));
                y += 1;
            }

            // "+N lines" row
            if self.collapsed && revealed_count > display_count && y < area.bottom() {
                let more = format!("… +{} lines", revealed_count - display_count);
                let x_start = area.x + 2;
                let avail = area.width.saturating_sub(2);
                put(buf, x_start, y, &more, avail, st(th.text_muted, code_bg));
            }
        }
    }
}

// CodeBlock

/// Code block with language, path, line numbers, syntax highlighting.
#[derive(Clone, Debug)]
pub struct CodeBlock<'a> {
    theme: Option<Theme>,
    lang: &'a str,
    path: &'a str,
    text: &'a str,
    line_numbers: bool,
    start_line: usize,
    wrap: bool,
    caret: bool,
    now: Option<Instant>,
    // A `fn` pointer hook the caller must match exactly; an alias would hide the signature.
    highlighter: Option<Highlighter>,
}

impl<'a> CodeBlock<'a> {
    pub fn new() -> Self {
        Self {
            theme: None,
            lang: "",
            path: "",
            text: "",
            line_numbers: true,
            start_line: 1,
            wrap: false,
            caret: false,
            now: None,
            highlighter: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn lang(mut self, l: &'a str) -> Self {
        self.lang = l;
        self
    }

    pub fn path(mut self, p: &'a str) -> Self {
        self.path = p;
        self
    }

    pub fn text(mut self, t: &'a str) -> Self {
        self.text = t;
        self
    }

    pub fn line_numbers(mut self, v: bool) -> Self {
        self.line_numbers = v;
        self
    }

    pub fn start_line(mut self, n: usize) -> Self {
        self.start_line = n;
        self
    }

    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap = v;
        self
    }

    pub fn caret(mut self, v: bool) -> Self {
        self.caret = v;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn highlighter(mut self, h: Highlighter) -> Self {
        self.highlighter = Some(h);
        self
    }
}

impl Default for CodeBlock<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for CodeBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.height == 0 {
            return;
        }

        let mut y = area.y;

        // Header: lang chip + path + line count
        let header_bg = th.background;
        fill(buf, Rect::new(area.x, y, area.width, 1), header_bg);

        let mut x = area.x;
        if !self.lang.is_empty() {
            let chip_bg = th.surface;
            let chip_fg = th.accent;
            let chip_w = self.lang.width() as u16 + 2;
            fill(buf, Rect::new(x, y, chip_w, 1), chip_bg);
            put(
                buf,
                x + 1,
                y,
                self.lang,
                chip_w.saturating_sub(2),
                st(chip_fg, chip_bg),
            );
            x += chip_w + 1;
        }

        let lines: Vec<&str> = self.text.lines().collect();
        let count_text = format!("{} lines", lines.len());
        let count_w = count_text.width() as u16;
        let path_avail = area.right().saturating_sub(x).saturating_sub(count_w + 1);

        if path_avail > 0 && !self.path.is_empty() {
            let path_text = truncate(self.path, path_avail as usize);
            put(
                buf,
                x,
                y,
                &path_text,
                path_avail,
                st(th.text_muted, header_bg),
            );
        }

        put_right(
            buf,
            Rect::new(area.x, y, area.width, 1),
            &count_text,
            st(th.text_muted, header_bg),
        );

        y += 1;

        // Body
        if y < area.bottom() {
            let body_h = area.bottom().saturating_sub(y);
            let body_rect = Rect::new(area.x, y, area.width, body_h);
            let code_bg = th.markdown_code_bg;
            fill(buf, body_rect, code_bg);

            let gutter_w = if self.line_numbers {
                let max_line = self.start_line + lines.len();
                format!("{}", max_line).len() as u16 + 1
            } else {
                0
            };

            let code_x = area.x + gutter_w + if gutter_w > 0 { 1 } else { 0 };
            let code_w = area
                .width
                .saturating_sub(gutter_w + if gutter_w > 0 { 1 } else { 0 });

            for (i, line) in lines.iter().enumerate() {
                if y >= area.bottom() {
                    break;
                }

                // Line number
                if self.line_numbers && gutter_w > 0 {
                    let num = format!(
                        "{:>width$}",
                        self.start_line + i,
                        width = gutter_w as usize - 1
                    );
                    put(
                        buf,
                        area.x,
                        y,
                        &num,
                        gutter_w - 1,
                        st(th.text_disabled, code_bg),
                    );
                    put(
                        buf,
                        area.x + gutter_w - 1,
                        y,
                        "│",
                        1,
                        st(th.border, code_bg),
                    );
                }

                // Line text
                let text = truncate(line, code_w as usize);
                if let Some(highlighter) = self.highlighter {
                    let ranges = highlighter(line);
                    put_highlighted(buf, code_x, y, &text, code_w, st(th.text, code_bg), &ranges);
                } else {
                    put(buf, code_x, y, &text, code_w, st(th.text, code_bg));
                }

                y += 1;
            }

            // Caret
            if self.caret && !lines.is_empty() {
                let last_y =
                    (area.y + 1 + lines.len() as u16 - 1).min(area.bottom().saturating_sub(1));
                let last_line = lines.last().unwrap_or(&"");
                let caret_x = code_x + last_line.width() as u16;
                if caret_x < area.right() && last_y < area.bottom() {
                    let now = self.now.unwrap_or_else(Instant::now);
                    let blink = ((since(now) * 2.0) as u32).is_multiple_of(2);
                    if blink {
                        put(buf, caret_x, last_y, "▌", 1, st(th.cursor_fg, code_bg));
                    }
                }
            }
        }
    }
}

// EditDecision

/// User decision for an edit preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditDecision {
    Accept,
    Reject,
    Edit,
}

// EditPreviewState

/// State for [`EditPreview`].
#[derive(Clone, Debug, Default)]
pub struct EditPreviewState {
    pub focus: usize,
    pub decision: Option<EditDecision>,
    pub hits: [HitBox; 3],
}

impl EditPreviewState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_decision(&mut self) -> Option<EditDecision> {
        self.decision.take()
    }
}

impl Interactive for EditPreviewState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Char('a') => {
                self.decision = Some(EditDecision::Accept);
                Outcome::Changed
            }
            KeyCode::Char('r') => {
                self.decision = Some(EditDecision::Reject);
                Outcome::Changed
            }
            KeyCode::Char('e') => {
                self.decision = Some(EditDecision::Edit);
                Outcome::Changed
            }
            KeyCode::Left => {
                if self.focus > 0 {
                    self.focus -= 1;
                }
                Outcome::Consumed
            }
            KeyCode::Right => {
                if self.focus < 2 {
                    self.focus += 1;
                }
                Outcome::Consumed
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.decision = Some(match self.focus {
                    0 => EditDecision::Accept,
                    1 => EditDecision::Reject,
                    _ => EditDecision::Edit,
                });
                Outcome::Changed
            }
            KeyCode::Esc => {
                self.decision = Some(EditDecision::Reject);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.focus = i;
                    self.decision = Some(match i {
                        0 => EditDecision::Accept,
                        1 => EditDecision::Reject,
                        _ => EditDecision::Edit,
                    });
                    return Outcome::Changed;
                }
                Hit::HoverChanged => return Outcome::Consumed,
                _ => {}
            }
        }
        Outcome::Ignored
    }
}

// EditPreview

/// Edit preview with animated reveal and decision buttons.
#[derive(Clone, Debug)]
pub struct EditPreview<'a> {
    theme: Option<Theme>,
    path: &'a str,
    lines: &'a [DiffLine],
    started: Option<Instant>,
    now: Option<Instant>,
    lps: f32,
    side_by_side: bool,
    accept_text: Option<String>,
    reject_text: Option<String>,
    edit_text: Option<String>,
}

impl<'a> EditPreview<'a> {
    pub fn new() -> Self {
        Self {
            theme: None,
            path: "",
            lines: &[] as &[DiffLine],
            started: None,
            now: None,
            lps: 24.0,
            side_by_side: false,
            accept_text: None,
            reject_text: None,
            edit_text: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn path(mut self, p: &'a str) -> Self {
        self.path = p;
        self
    }

    pub fn lines(mut self, l: &'a [DiffLine]) -> Self {
        self.lines = l;
        self
    }

    pub fn started(mut self, s: Instant) -> Self {
        self.started = Some(s);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn lps(mut self, l: f32) -> Self {
        self.lps = l;
        self
    }

    pub fn side_by_side(mut self, v: bool) -> Self {
        self.side_by_side = v;
        self
    }

    /// Accept button text override. Default is `[a] Accept`.
    pub fn accept_text(mut self, s: impl Into<String>) -> Self {
        self.accept_text = Some(s.into());
        self
    }

    /// Reject button text override. Default is `[r] Reject`.
    pub fn reject_text(mut self, s: impl Into<String>) -> Self {
        self.reject_text = Some(s.into());
        self
    }

    /// Edit button text override. Default is `[e] Edit`.
    pub fn edit_text(mut self, s: impl Into<String>) -> Self {
        self.edit_text = Some(s.into());
        self
    }
}

impl MinSize for EditPreview<'_> {
    /// Needs 2 rows: header + footer. Body is optional.
    fn min_size(&self) -> (u16, u16) {
        (8, 2)
    }
}

impl Default for EditPreview<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for EditPreview<'_> {
    type State = EditPreviewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);

        if refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let mut y = area.y;

        // Header: path + stats
        let added = self
            .lines
            .iter()
            .filter(|l| l.kind == DiffKind::Add)
            .count();
        let removed = self
            .lines
            .iter()
            .filter(|l| l.kind == DiffKind::Del)
            .count();
        let stats = format!("+{} −{}", added, removed);
        let stats_w = stats.width() as u16 + 1;

        let header_bg = th.background;
        fill(buf, Rect::new(area.x, y, area.width, 1), header_bg);

        let path_avail = area.width.saturating_sub(stats_w);
        let path_text = truncate(self.path, path_avail as usize);
        put(
            buf,
            area.x,
            y,
            &path_text,
            path_avail,
            bold(st(th.text, header_bg)),
        );

        put_right(
            buf,
            Rect::new(area.x, y, area.width, 1),
            &stats,
            st(th.text_muted, header_bg),
        );

        y += 1;

        // Body: diff lines
        let body_h = area.bottom().saturating_sub(y).saturating_sub(1);
        if body_h > 0 {
            let elapsed_val = if let (Some(started), Some(now)) = (self.started, self.now) {
                elapsed(started, now)
            } else {
                f32::INFINITY
            };

            let revealed_count = if elapsed_val.is_infinite() || self.lps <= 0.0 {
                self.lines.len()
            } else {
                ((elapsed_val * self.lps) as usize).min(self.lines.len())
            };

            for i in 0..revealed_count.min(body_h as usize) {
                if i >= self.lines.len() {
                    break;
                }
                let line = &self.lines[i];
                let glyph = match line.kind {
                    DiffKind::Add => "+",
                    DiffKind::Del => "-",
                    DiffKind::Ctx => " ",
                    DiffKind::Hunk => " ",
                };
                let (fg, bg) = match line.kind {
                    DiffKind::Add => (th.success, th.success.blend(th.background, 0.85)),
                    DiffKind::Del => (th.error, th.error.blend(th.background, 0.85)),
                    DiffKind::Ctx => (th.text_muted, th.background),
                    DiffKind::Hunk => (th.text_muted, th.surface),
                };

                // Brighten recently revealed lines
                let age = elapsed_val - (i as f32 / self.lps);
                let is_fresh = age < 0.3;
                let final_fg = if is_fresh { fg.blend(th.text, 0.5) } else { fg };

                fill(buf, Rect::new(area.x, y, area.width, 1), bg);
                put(buf, area.x, y, glyph, 1, st(final_fg, bg));
                let text = truncate(&line.text, (area.width.saturating_sub(2)) as usize);
                put(
                    buf,
                    area.x + 2,
                    y,
                    &text,
                    area.width.saturating_sub(2),
                    st(final_fg, bg),
                );
                y += 1;
            }
        }

        // Footer: action buttons
        if area.bottom() > y {
            let footer_y = area.bottom() - 1;
            fill(
                buf,
                Rect::new(area.x, footer_y, area.width, 1),
                th.background,
            );

            let buttons = [
                (
                    self.accept_text.as_deref().unwrap_or("[a] Accept"),
                    Variant::Success,
                    EditDecision::Accept,
                ),
                (
                    self.reject_text.as_deref().unwrap_or("[r] Reject"),
                    Variant::Error,
                    EditDecision::Reject,
                ),
                (
                    self.edit_text.as_deref().unwrap_or("[e] Edit"),
                    Variant::Default,
                    EditDecision::Edit,
                ),
            ];

            let mut x = area.x;
            for (i, (label, variant, _)) in buttons.iter().enumerate() {
                let is_focused = i == state.focus;
                let w = label.width() as u16 + 2;
                let btn_bg = if is_focused {
                    th.variant(*variant)
                } else {
                    th.variant(*variant).blend(th.background, 0.6)
                };
                let btn_fg = btn_bg.text_on(1.0);
                if x + w <= area.right() {
                    let btn_rect = Rect::new(x, footer_y, w, 1);
                    state.hits[i].set_area(btn_rect);
                    fill(buf, btn_rect, btn_bg);
                    put(
                        buf,
                        x + 1,
                        footer_y,
                        label,
                        w.saturating_sub(2),
                        st(btn_fg, btn_bg),
                    );
                    x += w + 1;
                }
            }
        }
    }
}

// ChangeKind

/// File change kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed(/* from offset in path */ usize),
}

// FileChange

/// One file in a change set.
#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    pub added: u32,
    pub removed: u32,
}

impl FileChange {
    pub fn new(path: impl Into<String>, kind: ChangeKind) -> Self {
        Self {
            path: path.into(),
            kind,
            added: 0,
            removed: 0,
        }
    }

    pub fn added(mut self, n: u32) -> Self {
        self.added = n;
        self
    }

    pub fn removed(mut self, n: u32) -> Self {
        self.removed = n;
        self
    }
}

// ChangeSetState

/// State for [`ChangeSet`].
#[derive(Clone, Debug, Default)]
pub struct ChangeSetState {
    pub files: Vec<FileChange>,
    pub cursor: usize,
    pub scroll: u16,
    pub hits: Vec<HitBox>,
    pub activated: Option<usize>,
}

impl ChangeSetState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }
}

impl Interactive for ChangeSetState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    return Outcome::Consumed;
                }
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.files.len() {
                    self.cursor += 1;
                    return Outcome::Consumed;
                }
            }
            KeyCode::Enter => {
                self.activated = Some(self.cursor);
                return Outcome::Submitted;
            }
            _ => {}
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if let Some(delta) = wheel_delta(&m) {
            self.scroll = self.scroll.saturating_add_signed(-delta as i16);
            return Outcome::Consumed;
        }
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    self.activated = Some(i);
                    return Outcome::Submitted;
                }
                Hit::HoverChanged => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }
        Outcome::Ignored
    }
}

// ChangeSet

/// File change set with stat bars.
#[derive(Clone, Debug)]
pub struct ChangeSet {
    theme: Option<Theme>,
    footer: bool,
}

impl ChangeSet {
    pub fn new() -> Self {
        Self {
            theme: None,
            footer: false,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn footer(mut self, v: bool) -> Self {
        self.footer = v;
        self
    }
}

impl Default for ChangeSet {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ChangeSet {
    type State = ChangeSetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);

        state.hits.clear();
        state.hits.resize(state.files.len(), HitBox::default());

        if area.height == 0 {
            return;
        }

        let max_change = state
            .files
            .iter()
            .map(|f| f.added.max(f.removed))
            .max()
            .unwrap_or(1);

        let footer_h = if self.footer { 1 } else { 0 };
        let list_h = area.height.saturating_sub(footer_h);

        let mut y = area.y;
        for (i, file) in state.files.iter().enumerate() {
            if i < state.scroll as usize {
                continue;
            }
            if y >= area.y + list_h {
                break;
            }

            let is_cursor = i == state.cursor;
            let bg = if is_cursor {
                th.cursor_bg
            } else {
                th.background
            };

            let row_rect = Rect::new(area.x, y, area.width, 1);
            state.hits[i].set_area(row_rect);
            fill(buf, row_rect, bg);

            // Badge
            let (badge, badge_variant) = match file.kind {
                ChangeKind::Added => ("A", Variant::Success),
                ChangeKind::Modified => ("M", Variant::Warning),
                ChangeKind::Deleted => ("D", Variant::Error),
                ChangeKind::Renamed(_) => ("R", Variant::Secondary),
            };
            let badge_bg = th.variant(badge_variant);
            let badge_fg = badge_bg.text_on(1.0);
            fill(buf, Rect::new(area.x, y, 1, 1), badge_bg);
            put(buf, area.x, y, badge, 1, st(badge_fg, badge_bg));

            // Stats + bar
            let stats = format!("+{} −{}", file.added, file.removed);
            let stats_w = stats.width() as u16;
            let bar_w = 8;
            let right_w = stats_w + 1 + bar_w;

            // Path
            let path_avail = area.width.saturating_sub(2 + right_w);
            let path_text = truncate_start(&file.path, path_avail as usize);
            put(buf, area.x + 2, y, &path_text, path_avail, st(th.text, bg));

            // Stats
            let stats_x = area.right().saturating_sub(right_w);
            put(buf, stats_x, y, &stats, stats_w, st(th.text_muted, bg));

            // Bar
            let bar_x = area.right().saturating_sub(bar_w);
            let green_frac = (file.added as f32 / max_change as f32).min(1.0);
            let red_frac = (file.removed as f32 / max_change as f32).min(1.0);
            let green_cells =
                (green_frac * bar_w as f32)
                    .ceil()
                    .max(if file.added > 0 { 1.0 } else { 0.0 }) as u16;
            let red_cells =
                (red_frac * bar_w as f32)
                    .ceil()
                    .max(if file.removed > 0 { 1.0 } else { 0.0 }) as u16;

            for i in 0..bar_w {
                let cell_x = bar_x + i;
                if cell_x >= area.right() {
                    break;
                }
                let color = if i < green_cells {
                    th.success
                } else if i < green_cells + red_cells {
                    th.error
                } else {
                    th.panel
                };
                fill(buf, Rect::new(cell_x, y, 1, 1), color);
            }

            y += 1;
        }

        // Footer
        if self.footer && footer_h > 0 {
            let footer_y = area.bottom() - 1;
            fill(
                buf,
                Rect::new(area.x, footer_y, area.width, 1),
                th.background,
            );
            let total_added: u32 = state.files.iter().map(|f| f.added).sum();
            let total_removed: u32 = state.files.iter().map(|f| f.removed).sum();
            let footer_text = format!(
                "{} files  +{} −{}",
                state.files.len(),
                total_added,
                total_removed
            );
            put(
                buf,
                area.x,
                footer_y,
                &footer_text,
                area.width,
                st(th.text_muted, th.background),
            );
        }
    }
}

// Json

/// Simple JSON value for tree rendering.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Parse JSON text; returns `Err("line:col: message")` on failure.
    pub fn parse(s: &str) -> Result<Json, String> {
        let mut p = Parser::new(s);
        let val = p.value()?;
        p.skip_ws();
        if p.pos < p.text.len() {
            return Err(p.error("unexpected trailing content"));
        }
        Ok(val)
    }
}

struct Parser {
    text: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Self {
            text: s.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn error(&self, msg: &str) -> String {
        format!("{}:{}: {}", self.line, self.col, msg)
    }

    fn peek(&self) -> Option<char> {
        self.text.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        if self.peek() == Some(expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", expected)))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some('n') => self.null(),
            Some('t') | Some('f') => self.bool(),
            Some('"') => self.string().map(Json::Str),
            Some('[') => self.array(),
            Some('{') => self.object(),
            Some(c) if c.is_ascii_digit() || c == '-' => self.number(),
            _ => Err(self.error("expected value")),
        }
    }

    fn null(&mut self) -> Result<Json, String> {
        for c in ['n', 'u', 'l', 'l'] {
            self.expect(c)?;
        }
        Ok(Json::Null)
    }

    fn bool(&mut self) -> Result<Json, String> {
        let peek = self.peek();
        if peek == Some('t') {
            for c in ['t', 'r', 'u', 'e'] {
                self.expect(c)?;
            }
            Ok(Json::Bool(true))
        } else {
            for c in ['f', 'a', 'l', 's', 'e'] {
                self.expect(c)?;
            }
            Ok(Json::Bool(false))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        let peek = self.peek();
        if peek == Some('-') {
            self.advance();
        }
        if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            return Err(self.error("expected digit"));
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.advance();
        }
        if self.peek() == Some('.') {
            self.advance();
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.advance();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.advance();
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }
        }
        let num_str: String = self.text[start..self.pos].iter().collect();
        num_str
            .parse()
            .map(Json::Num)
            .map_err(|_| self.error("invalid number"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut s = String::new();
        loop {
            match self.peek() {
                Some('"') => {
                    self.advance();
                    return Ok(s);
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('"') => {
                            s.push('"');
                            self.advance();
                        }
                        Some('\\') => {
                            s.push('\\');
                            self.advance();
                        }
                        Some('n') => {
                            s.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            s.push('\t');
                            self.advance();
                        }
                        Some('u') => {
                            self.advance();
                            let mut hex = String::new();
                            for _ in 0..4 {
                                if let Some(c) = self.peek() {
                                    hex.push(c);
                                    self.advance();
                                } else {
                                    return Err(self.error("incomplete \\u escape"));
                                }
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| self.error("invalid \\u escape"))?;
                            if let Some(c) = char::from_u32(code) {
                                s.push(c);
                            } else {
                                return Err(self.error("invalid unicode codepoint"));
                            }
                        }
                        _ => return Err(self.error("invalid escape")),
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.advance();
                }
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        self.skip_ws();
        let mut arr = Vec::new();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(Json::Arr(arr));
        }
        loop {
            arr.push(self.value()?);
            self.skip_ws();
            if self.peek() == Some(',') {
                self.advance();
                self.skip_ws();
            } else {
                break;
            }
        }
        self.expect(']')?;
        Ok(Json::Arr(arr))
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        self.skip_ws();
        let mut obj = Vec::new();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(Json::Obj(obj));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(':')?;
            let val = self.value()?;
            obj.push((key, val));
            self.skip_ws();
            if self.peek() == Some(',') {
                self.advance();
                self.skip_ws();
            } else {
                break;
            }
        }
        self.expect('}')?;
        Ok(Json::Obj(obj))
    }
}

// JsonTreeState

/// State for [`JsonTree`].
#[derive(Clone, Debug, Default)]
pub struct JsonTreeState {
    pub root: Option<Json>,
    pub expanded: HashSet<Vec<usize>>,
    pub cursor: usize,
    pub scroll: u16,
    pub hits: Vec<HitBox>,
}

impl JsonTreeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, val: Json) {
        self.root = Some(val);
        self.expanded.clear();
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn toggle_cursor(&mut self, rows: &[(Vec<usize>, bool)]) {
        if self.cursor < rows.len() {
            let path = rows[self.cursor].0.clone();
            if self.expanded.contains(&path) {
                self.expanded.remove(&path);
            } else {
                self.expanded.insert(path);
            }
        }
    }

    pub fn expand_all(&mut self) {
        let root = match &self.root {
            Some(r) => r.clone(),
            None => return,
        };
        Self::collect_all_paths(&root, &Vec::new(), &mut self.expanded);
    }

    fn collect_all_paths(val: &Json, path: &[usize], set: &mut HashSet<Vec<usize>>) {
        match val {
            Json::Arr(arr) => {
                set.insert(path.to_vec());
                for (i, v) in arr.iter().enumerate() {
                    let mut new_path = path.to_vec();
                    new_path.push(i);
                    Self::collect_all_paths(v, &new_path, set);
                }
            }
            Json::Obj(obj) => {
                set.insert(path.to_vec());
                for (i, (_, v)) in obj.iter().enumerate() {
                    let mut new_path = path.to_vec();
                    new_path.push(i);
                    Self::collect_all_paths(v, &new_path, set);
                }
            }
            _ => {}
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }
}

impl Interactive for JsonTreeState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        // We need rows to handle keys properly; defer to the widget render
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if let Some(delta) = wheel_delta(&m) {
            self.scroll = self.scroll.saturating_add_signed(-delta as i16);
            return Outcome::Consumed;
        }
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    return Outcome::Changed;
                }
                Hit::HoverChanged => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }
        Outcome::Ignored
    }
}

// JsonTree

/// JSON tree viewer.
#[derive(Clone, Debug)]
pub struct JsonTree {
    theme: Option<Theme>,
    focused: bool,
}

impl JsonTree {
    pub fn new() -> Self {
        Self {
            theme: None,
            focused: false,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    fn render_rows(
        &self,
        val: &Json,
        path: &[usize],
        depth: usize,
        state: &JsonTreeState,
    ) -> Vec<(Vec<usize>, String, bool)> {
        let mut rows = Vec::new();
        match val {
            Json::Null => rows.push((path.to_vec(), format!("{}null", "  ".repeat(depth)), false)),
            Json::Bool(b) => {
                rows.push((path.to_vec(), format!("{}{}", "  ".repeat(depth), b), false))
            }
            Json::Num(n) => {
                rows.push((path.to_vec(), format!("{}{}", "  ".repeat(depth), n), false))
            }
            Json::Str(s) => {
                let truncated = truncate(s, 60);
                rows.push((
                    path.to_vec(),
                    format!("{}\"{}\"", "  ".repeat(depth), truncated),
                    false,
                ));
            }
            Json::Arr(arr) => {
                let is_expanded = state.expanded.contains(path);
                let toggle = if is_expanded { "▾" } else { "▸" };
                let summary = if is_expanded {
                    format!("{}{}[", "  ".repeat(depth), toggle)
                } else {
                    format!("{}{} [{} items]", "  ".repeat(depth), toggle, arr.len())
                };
                rows.push((path.to_vec(), summary, true));
                if is_expanded {
                    for (i, item) in arr.iter().enumerate() {
                        let mut new_path = path.to_vec();
                        new_path.push(i);
                        rows.extend(self.render_rows(item, &new_path, depth + 1, state));
                    }
                    // No closing bracket row
                }
            }
            Json::Obj(obj) => {
                let is_expanded = state.expanded.contains(path);
                let toggle = if is_expanded { "▾" } else { "▸" };
                let summary = if is_expanded {
                    format!("{}{}{}", "  ".repeat(depth), toggle, "{")
                } else {
                    format!("{}{} {{{} keys}}", "  ".repeat(depth), toggle, obj.len())
                };
                rows.push((path.to_vec(), summary, true));
                if is_expanded {
                    for (i, (k, v)) in obj.iter().enumerate() {
                        let mut new_path = path.to_vec();
                        new_path.push(i);
                        let key_row = format!("\"{}\": ", k);
                        let mut val_rows = self.render_rows(v, &new_path, depth + 1, state);
                        if let Some((_p, first, _toggle)) = val_rows.first_mut() {
                            // Prepend key, preserving the value's indentation
                            *first = format!(
                                "{}{}{}",
                                "  ".repeat(depth + 1),
                                key_row,
                                first.trim_start()
                            );
                        }
                        rows.extend(val_rows);
                    }
                    // No closing bracket row
                }
            }
        }
        rows
    }
}

impl Default for JsonTree {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for JsonTree {
    type State = JsonTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);

        state.hits.clear();

        if area.height == 0 {
            return;
        }

        let rows = if let Some(ref root) = state.root {
            self.render_rows(root, &Vec::new(), 0, state)
        } else {
            vec![]
        };

        state.hits.resize(rows.len(), HitBox::default());

        let mut y = area.y;
        for (i, (_path, text, _is_toggle)) in rows.iter().enumerate() {
            if i < state.scroll as usize {
                continue;
            }
            if y >= area.bottom() {
                break;
            }

            let is_cursor = i == state.cursor;
            let bg = if is_cursor && self.focused {
                th.cursor_bg
            } else {
                th.background
            };

            let row_rect = Rect::new(area.x, y, area.width, 1);
            state.hits[i].set_area(row_rect);
            fill(buf, row_rect, bg);

            let truncated = truncate(text, area.width as usize);
            put(buf, area.x, y, &truncated, area.width, st(th.text, bg));

            y += 1;
        }
    }
}

// RetryNotice

/// Retry countdown notice.
#[derive(Clone, Debug)]
pub struct RetryNotice<'a> {
    theme: Option<Theme>,
    reason: &'a str,
    attempt: (u32, u32),
    deadline: Instant,
    now: Option<Instant>,
    compact: bool,
    retrying_text: Option<String>,
    retry_text: Option<String>,
}

impl<'a> RetryNotice<'a> {
    pub fn new() -> Self {
        Self {
            theme: None,
            reason: "",
            attempt: (1, 1),
            deadline: Instant::now(),
            now: None,
            compact: false,
            retrying_text: None,
            retry_text: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn reason(mut self, r: &'a str) -> Self {
        self.reason = r;
        self
    }

    pub fn attempt(mut self, current: u32, total: u32) -> Self {
        self.attempt = (current, total);
        self
    }

    pub fn deadline(mut self, d: Instant) -> Self {
        self.deadline = d;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn compact(mut self, c: bool) -> Self {
        self.compact = c;
        self
    }

    /// Retrying status text override. Default is `retrying…`.
    pub fn retrying_text(mut self, s: impl Into<String>) -> Self {
        self.retrying_text = Some(s.into());
        self
    }

    /// Retry countdown text override. Receives formatted string.
    /// Default formats as `retry {current}/{total} in {secs}s`.
    pub fn retry_text(mut self, s: impl Into<String>) -> Self {
        self.retry_text = Some(s.into());
        self
    }
}

impl MinSize for RetryNotice<'_> {
    /// Compact mode needs 1 row; card mode needs 3 rows.
    fn min_size(&self) -> (u16, u16) {
        if self.compact { (10, 1) } else { (10, 3) }
    }
}

impl Default for RetryNotice<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for RetryNotice<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);

        let remaining = self
            .deadline
            .saturating_duration_since(now)
            .as_secs_f32()
            .max(0.0);
        let is_retrying = remaining <= 0.0;

        if refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        if self.compact {
            // One row: ⊛ reason · retry N/M in Ns ▁▁▁
            if area.height == 0 {
                return;
            }
            let bg = th.background;
            fill(buf, Rect::new(area.x, area.y, area.width, 1), bg);

            let mut x = area.x;
            put(buf, x, area.y, "⊛", 2, st(th.warning, bg));
            x += 2;

            let status_text = if is_retrying {
                let retrying = self.retrying_text.as_deref().unwrap_or("retrying…");
                format!("{} · {}", self.reason, retrying)
            } else {
                if let Some(retry) = &self.retry_text {
                    format!("{} · {}", self.reason, retry)
                } else {
                    format!(
                        "{} · retry {}/{} in {}s",
                        self.reason,
                        self.attempt.0,
                        self.attempt.1,
                        remaining.ceil() as u32
                    )
                }
            };

            let bar_w = 8;
            let avail = area.width.saturating_sub((x - area.x) + bar_w + 1);
            let text = truncate(&status_text, avail as usize);
            put(buf, x, area.y, &text, avail, st(th.text, bg));

            // Bar
            let bar_x = area.right().saturating_sub(bar_w);
            if !is_retrying {
                let frac = (remaining / 6.0).min(1.0);
                hbar(buf, bar_x, area.y, bar_w, frac, th.warning, bg);
            }
        } else {
            // 3-row card (already validated by refuse)
            let card_h = 3;
            let card_area = Rect::new(area.x, area.y, area.width, card_h);
            Border::Round.draw(buf, card_area, th.warning, th.background);
            let inner = Border::Round.inner(card_area);

            if inner.height == 0 {
                return;
            }

            let mut y = inner.y;

            // Row 1: reason
            fill(buf, Rect::new(inner.x, y, inner.width, 1), th.background);
            let text = truncate(self.reason, inner.width as usize);
            put(
                buf,
                inner.x,
                y,
                &text,
                inner.width,
                bold(st(th.text, th.background)),
            );
            y += 1;

            if y < inner.bottom() {
                // Row 2: retry N/M in Ns or retrying…
                fill(buf, Rect::new(inner.x, y, inner.width, 1), th.background);
                let status = if is_retrying {
                    let retrying = self.retrying_text.as_deref().unwrap_or("retrying…");
                    let now_val = self.now.unwrap_or_else(Instant::now);
                    let frame = spinners::DOTS.frame(since(now_val));
                    format!("{}{}", frame, retrying)
                } else {
                    if let Some(retry) = &self.retry_text {
                        retry.clone()
                    } else {
                        format!(
                            "retry {}/{} in {}s",
                            self.attempt.0,
                            self.attempt.1,
                            remaining.ceil() as u32
                        )
                    }
                };
                put(
                    buf,
                    inner.x,
                    y,
                    &status,
                    inner.width,
                    st(th.text_muted, th.background),
                );
                y += 1;
            }

            if y < inner.bottom() && !is_retrying {
                // Row 3: bar
                fill(buf, Rect::new(inner.x, y, inner.width, 1), th.background);
                let frac = (remaining / 6.0).min(1.0);
                hbar(
                    buf,
                    inner.x,
                    y,
                    inner.width,
                    frac,
                    th.warning,
                    th.background,
                );
            }
        }
    }
}

fn fmt_ms(secs: f32) -> String {
    if secs < 1.0 {
        format!("{}ms", (secs * 1000.0) as u32)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = (secs / 60.0) as u32;
        let s = (secs % 60.0) as u32;
        format!("{}m {:02}s", mins, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Rgb;

    #[test]
    fn tool_step_builder() {
        let step = ToolStep::new("test")
            .summary("summary")
            .status(ToolStatus::Done)
            .depth(2)
            .expanded(true);
        assert_eq!(step.name, "test");
        assert_eq!(step.summary, "summary");
        assert_eq!(step.status, ToolStatus::Done);
        assert_eq!(step.depth, 2);
        assert!(step.expanded);
    }

    #[test]
    fn timeline_state_toggle() {
        let mut state = ToolTimelineState::default();
        state.steps.push(ToolStep::new("a"));
        state.steps.push(ToolStep::new("b"));
        state.toggle(0);
        assert!(state.steps[0].expanded);
        state.toggle(0);
        assert!(!state.steps[0].expanded);
    }

    #[test]
    fn timeline_expand_collapse_all() {
        let mut state = ToolTimelineState::default();
        state.steps.push(ToolStep::new("a"));
        state.steps.push(ToolStep::new("b"));
        state.expand_all();
        assert!(state.steps[0].expanded);
        assert!(state.steps[1].expanded);
        state.collapse_all();
        assert!(!state.steps[0].expanded);
        assert!(!state.steps[1].expanded);
    }

    #[test]
    fn json_parse_roundtrip() {
        let cases = vec![
            r#"null"#,
            r#"true"#,
            r#"false"#,
            r#"42"#,
            r#"-3.14"#,
            r#""hello""#,
            r#""with \"quotes\" and \\backslash""#,
            r#"[]"#,
            r#"[1, 2, 3]"#,
            r#"{}"#,
            r#"{"a": 1, "b": [2, 3]}"#,
            r#"{"nested": {"obj": {"with": [1, 2, {"deep": true}]}}}"#,
        ];
        for case in cases {
            let parsed = Json::parse(case);
            assert!(parsed.is_ok(), "failed to parse: {}", case);
        }
    }

    #[test]
    fn json_parse_errors() {
        let cases = vec![
            ("", "expected value"),
            ("{", "expected"),
            (r#"{"a":}"#, "expected value"),
            (r#"{"a": 1"#, "expected '}'"),
            ("[1, 2", "expected ']'"),
            (r#""unclosed"#, "unterminated string"),
            (r#""\x""#, "invalid escape"),
        ];
        for (input, msg_part) in cases {
            let result = Json::parse(input);
            assert!(result.is_err(), "should fail: {}", input);
            let err = result.unwrap_err();
            assert!(
                err.contains(msg_part),
                "error {:?} should contain {:?}",
                err,
                msg_part
            );
        }
    }

    #[test]
    fn json_parse_escapes() {
        let parsed = Json::parse(r#""a\nb\tc\"d\\e\u0041""#).unwrap();
        if let Json::Str(s) = parsed {
            assert_eq!(s, "a\nb\tc\"d\\eA");
        } else {
            panic!("expected string");
        }
    }

    #[test]
    fn edit_preview_decision() {
        let mut state = EditPreviewState::default();
        assert_eq!(state.take_decision(), None);
        state.decision = Some(EditDecision::Accept);
        assert_eq!(state.take_decision(), Some(EditDecision::Accept));
        assert_eq!(state.take_decision(), None);
    }

    #[test]
    fn changeset_activate() {
        let mut state = ChangeSetState::default();
        state.files.push(FileChange::new("a.rs", ChangeKind::Added));
        state
            .files
            .push(FileChange::new("b.rs", ChangeKind::Modified));
        state.cursor = 1;
        assert_eq!(state.take_activated(), None);
        state.activated = Some(1);
        assert_eq!(state.take_activated(), Some(1));
        assert_eq!(state.take_activated(), None);
    }

    #[test]
    fn size_sweep_no_panic() {
        let sizes = [(1, 1), (3, 2), (10, 3), (60, 16), (130, 42), (250, 70)];
        let th = Theme::default();

        for (w, h) in sizes {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);

            let mut timeline_state = ToolTimelineState::default();
            timeline_state
                .steps
                .push(ToolStep::new("test").status(ToolStatus::Done));
            ToolTimeline::new()
                .theme(&th)
                .render(area, &mut buf, &mut timeline_state);

            ShellBlock::new()
                .theme(&th)
                .command("ls")
                .output(&[(false, "file1".into()), (true, "error".into())])
                .render(area, &mut buf);

            CodeBlock::new()
                .theme(&th)
                .lang("rust")
                .text("fn main() {}")
                .render(area, &mut buf);

            let mut preview_state = EditPreviewState::default();
            let lines = vec![DiffLine {
                kind: DiffKind::Add,
                text: "new line".into(),
            }];
            EditPreview::new()
                .theme(&th)
                .path("test.rs")
                .lines(&lines)
                .render(area, &mut buf, &mut preview_state);

            let mut changeset_state = ChangeSetState::default();
            changeset_state
                .files
                .push(FileChange::new("a.rs", ChangeKind::Added));
            ChangeSet::new()
                .theme(&th)
                .render(area, &mut buf, &mut changeset_state);

            let mut json_state = JsonTreeState::default();
            json_state.set(Json::Num(42.0));
            JsonTree::new()
                .theme(&th)
                .render(area, &mut buf, &mut json_state);

            RetryNotice::new()
                .theme(&th)
                .reason("test")
                .render(area, &mut buf);
        }
    }

    #[test]
    fn code_block_highlighter_applies_styles() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::style::Style;

        let area = Rect::new(0, 0, 30, 5);
        let mut buf = Buffer::empty(area);
        let hl_style = st(Rgb(0, 255, 0), Rgb(0, 0, 0));

        fn simple_highlighter(line: &str) -> Vec<(usize, usize, Style)> {
            let hl = st(Rgb(0, 255, 0), Rgb(0, 0, 0));
            if line.contains("keyword") {
                vec![(0, 7, hl)] // highlight "keyword"
            } else {
                vec![]
            }
        }

        CodeBlock::new()
            .text("keyword rest")
            .line_numbers(false)
            .highlighter(simple_highlighter)
            .render(area, &mut buf);

        // The code starts at y=1 (after header)
        let code_y = 1;
        // First 7 chars should be highlighted (grapheme offsets 0-6 inclusive)
        assert_eq!(
            buf[(0, code_y)].fg,
            hl_style.fg.unwrap(),
            "char 0 (k) should be highlighted"
        );
        assert_eq!(
            buf[(6, code_y)].fg,
            hl_style.fg.unwrap(),
            "char 6 (d) should be highlighted"
        );
        // Char 7 (space) should not be highlighted
        assert_ne!(
            buf[(7, code_y)].fg,
            hl_style.fg.unwrap(),
            "char 7 (space) should use base style"
        );
    }

    #[test]
    fn code_block_no_panic_at_tiny_size() {
        use ratatui_core::buffer::Buffer;
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        CodeBlock::new()
            .text("hello")
            .highlighter(|_| vec![(0, 5, st(Rgb(0, 255, 0), Rgb(0, 0, 0)))])
            .render(area, &mut buf);
        // Should not panic
    }

    fn painted(buf: &Buffer) -> bool {
        buf.content().iter().any(|c| c.symbol() != " ")
    }

    #[test]
    fn edit_preview_custom_button_text() {
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        let mut state = EditPreviewState::default();
        let lines = vec![DiffLine {
            kind: DiffKind::Add,
            text: "new".into(),
        }];

        EditPreview::new()
            .lines(&lines)
            .accept_text("✓ OK")
            .render(area, &mut buf, &mut state);

        let footer_row: String = (0..40).map(|x| buf[(x, 3)].symbol()).collect();
        assert!(
            footer_row.contains("✓ OK"),
            "custom accept text should appear in buffer"
        );
    }

    #[test]
    fn edit_preview_default_button_text() {
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        let mut state = EditPreviewState::default();
        let lines = vec![DiffLine {
            kind: DiffKind::Add,
            text: "new".into(),
        }];

        EditPreview::new()
            .lines(&lines)
            .render(area, &mut buf, &mut state);

        let footer_row: String = (0..40).map(|x| buf[(x, 3)].symbol()).collect();
        assert!(
            footer_row.contains("[a] Accept"),
            "default accept text should appear"
        );
    }

    #[test]
    fn retry_notice_custom_retry_text() {
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        let deadline = Instant::now() + Duration::from_secs(5);

        RetryNotice::new()
            .reason("test")
            .compact(true)
            .retry_text("réessayer 1/3 dans 5s")
            .deadline(deadline)
            .render(area, &mut buf);

        let row: String = (0..40).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(row.contains("réessayer"), "custom retry text should appear");
    }

    #[test]
    fn changeset_activation_returns_submitted() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut state = ChangeSetState::default();
        state.files.push(FileChange::new("a.rs", ChangeKind::Added));
        state.cursor = 0;

        let k = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let outcome = state.handle_key(k);
        assert_eq!(outcome, Outcome::Submitted, "Enter should return Submitted");
        assert_eq!(state.activated, Some(0));
    }

    #[test]
    fn widgets_draw_at_minimum_and_refuse_below() {
        let th = Theme::default();

        // EditPreview: min is (8, 2)
        let (w, h) = (8u16, 2u16);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let mut state = EditPreviewState::default();
        let lines = vec![DiffLine {
            kind: DiffKind::Add,
            text: "new".into(),
        }];
        EditPreview::new()
            .theme(&th)
            .lines(&lines)
            .render(buf.area, &mut buf, &mut state);
        assert!(painted(&buf), "EditPreview should draw at its minimum");

        let mut buf = Buffer::empty(Rect::new(0, 0, w, h.saturating_sub(1)));
        EditPreview::new()
            .theme(&th)
            .lines(&lines)
            .render(buf.area, &mut buf, &mut state);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one row short must refuse visibly"
        );

        // RetryNotice compact: min is (10, 1)
        let (w, h) = (10u16, 1u16);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        RetryNotice::new()
            .theme(&th)
            .compact(true)
            .reason("test")
            .render(buf.area, &mut buf);
        assert!(painted(&buf), "RetryNotice compact should draw at minimum");

        let mut buf = Buffer::empty(Rect::new(0, 0, w.saturating_sub(1), h));
        RetryNotice::new()
            .theme(&th)
            .compact(true)
            .reason("test")
            .render(buf.area, &mut buf);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one cell short must refuse visibly"
        );

        // RetryNotice card: min is (10, 3)
        let (w, h) = (10u16, 3u16);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        RetryNotice::new()
            .theme(&th)
            .compact(false)
            .reason("test")
            .render(buf.area, &mut buf);
        assert!(painted(&buf), "RetryNotice card should draw at minimum");

        let mut buf = Buffer::empty(Rect::new(0, 0, w, h.saturating_sub(1)));
        RetryNotice::new()
            .theme(&th)
            .compact(false)
            .reason("test")
            .render(buf.area, &mut buf);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one row short must refuse visibly"
        );
    }
}
