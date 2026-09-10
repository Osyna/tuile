//! Markdown renderer for CommonMark subset with scrolling.
//!
//! ```
//! # use tuiforge::prelude::*;
//! # use tuiforge::widgets::markdown::*;
//! # let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
//! # let mut state = MarkdownState::default();
//! let src = "# Hello\n\nThis is **bold** and *italic*.";
//! Markdown::new(src).render(Rect::new(0, 0, 60, 20), &mut buf, &mut state);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;

use crate::core::{Interactive, Outcome, is_press, wheel_delta};
use crate::draw::{put, st, wrap as text_wrap};
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState};

// ─────────────────────────────────────────────────────────────────────────────
// Markdown state
// ─────────────────────────────────────────────────────────────────────────────

/// Markdown view state with scrolling.
#[derive(Clone, Debug, Default)]
pub struct MarkdownState {
    pub scroll: usize,
    pub vbar: ScrollbarState,
}

impl MarkdownState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Interactive for MarkdownState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(1);
                Outcome::Consumed
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                Outcome::Consumed
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.scroll = 0;
                Outcome::Consumed
            }
            KeyCode::End => {
                self.scroll = usize::MAX;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let out = self.vbar.handle_mouse(m);
        if out.is_consumed() {
            self.scroll = self.vbar.offset;
            return out;
        }

        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                self.scroll = self.scroll.saturating_add(3);
            } else {
                self.scroll = self.scroll.saturating_sub(3);
            }
            return Outcome::Consumed;
        }

        Outcome::Ignored
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Markdown widget
// ─────────────────────────────────────────────────────────────────────────────

/// Markdown renderer widget.
#[derive(Clone, Debug)]
pub struct Markdown {
    src: String,
    theme: Option<Theme>,
}

impl Markdown {
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            src: src.into(),
            theme: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Render markdown to styled lines at a given width.
    pub fn lines(&self, width: usize, th: &Theme) -> Vec<Line<'static>> {
        let mut parser = MdParser::new(&self.src, width, th);
        parser.parse();
        parser.lines
    }

    /// Compute height when rendered at width.
    pub fn height(&self, width: usize, th: &Theme) -> usize {
        self.lines(width, th).len()
    }
}

impl StatefulWidget for Markdown {
    type State = MarkdownState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 5 || area.height < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        // Don't fill - let the parent background show through
        let content_w = area.width.saturating_sub(1);
        let lines = self.lines(content_w as usize, &th);

        let total = lines.len();
        let viewport = area.height as usize;
        state.scroll = state.scroll.min(total.saturating_sub(viewport));

        // Render visible lines
        for (i, line) in lines.iter().enumerate().skip(state.scroll).take(viewport) {
            let y = area.y + (i - state.scroll) as u16;
            let mut x = area.x;
            for span in &line.spans {
                let w = span.content.len() as u16;
                if x + w > area.x + content_w {
                    break;
                }
                put(buf, x, y, &span.content, w, span.style);
                x += w;
            }
        }

        // Scrollbar
        let sb_area = Rect {
            x: area.x + content_w,
            y: area.y,
            width: 1,
            height: area.height,
        };
        Scrollbar::vertical(total, viewport)
            .offset(state.scroll)
            .theme(&th)
            .render(sb_area, buf, &mut state.vbar);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Markdown parser
// ─────────────────────────────────────────────────────────────────────────────

struct MdParser<'a> {
    src: &'a str,
    width: usize,
    th: &'a Theme,
    lines: Vec<Line<'static>>,
    in_code_block: bool,
    code_lang: String,
}

impl<'a> MdParser<'a> {
    fn new(src: &'a str, width: usize, th: &'a Theme) -> Self {
        Self {
            src,
            width,
            th,
            lines: Vec::new(),
            in_code_block: false,
            code_lang: String::new(),
        }
    }

    fn parse(&mut self) {
        let raw_lines: Vec<&str> = self.src.lines().collect();
        let mut i = 0;
        while i < raw_lines.len() {
            let line = raw_lines[i];

            // code fence
            if line.starts_with("```") {
                if self.in_code_block {
                    // close
                    self.in_code_block = false;
                    self.lines.push(Line::raw(""));
                } else {
                    // open
                    self.in_code_block = true;
                    self.code_lang = line.strip_prefix("```").unwrap().trim().to_string();
                    if !self.code_lang.is_empty() {
                        // lang label at top-right
                        let label = format!(" {} ", self.code_lang);
                        self.lines.push(Line::from(vec![
                            Span::raw(" ".repeat(self.width.saturating_sub(label.len()))),
                            Span::styled(
                                label,
                                st(self.th.text_muted, self.th.markdown_code_bg)
                                    .add_modifier(Modifier::DIM),
                            ),
                        ]));
                    }
                }
                i += 1;
                continue;
            }

            if self.in_code_block {
                // render code line with bg
                let padded = format!(" {} ", line);
                self.lines.push(Line::styled(
                    padded,
                    st(self.th.text, self.th.markdown_code_bg),
                ));
                i += 1;
                continue;
            }

            // headings
            if let Some(level) = heading_level(line) {
                let text = line[level..].trim();
                let (fg, underline) = match level {
                    1 => (self.th.text, true),
                    2 => (self.th.text, true),
                    3 => (self.th.text, false),
                    4 => (self.th.text_muted, false),
                    _ => (self.th.text, false),
                };
                // Use Span like paragraphs do, with both fg and bg
                let span = Span::styled(
                    text.to_string(),
                    st(fg, self.th.background).add_modifier(Modifier::BOLD),
                );
                self.lines.push(Line::from(vec![span]));
                if underline && level == 1 {
                    let rule_len = text.len().min(self.width);
                    let rule_span = Span::styled(
                        "▔".repeat(rule_len),
                        st(self.th.secondary, self.th.background),
                    );
                    self.lines.push(Line::from(vec![rule_span]));
                } else if underline && level == 2 {
                    let rule_len = text.len().min(self.width);
                    let rule_span = Span::styled(
                        "─".repeat(rule_len),
                        st(self.th.secondary, self.th.background),
                    );
                    self.lines.push(Line::from(vec![rule_span]));
                }
                self.lines.push(Line::raw(""));
                i += 1;
                continue;
            }

            // horizontal rule
            if line.trim() == "---" || line.trim() == "***" || line.trim() == "___" {
                self.lines.push(Line::styled(
                    "─".repeat(self.width),
                    st(self.th.border, self.th.surface),
                ));
                self.lines.push(Line::raw(""));
                i += 1;
                continue;
            }

            // blockquote
            if line.starts_with('>') {
                let text = line.strip_prefix('>').unwrap().trim();
                let spans = vec![
                    Span::styled("▌ ", st(self.th.secondary, self.th.surface)),
                    Span::styled(
                        text.to_string(),
                        st(self.th.text_muted, self.th.surface).add_modifier(Modifier::ITALIC),
                    ),
                ];
                self.lines.push(Line::from(spans));
                i += 1;
                continue;
            }

            // unordered list
            if line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ") {
                let indent = line.len() - line.trim_start().len();
                let depth = indent / 2;
                let text = line.trim_start()[1..].trim();
                let bullet = match depth % 3 {
                    0 => "•",
                    1 => "◦",
                    _ => "•",
                };
                let prefix = format!("{}{} ", " ".repeat(indent), bullet);
                let parsed = self.parse_inline(text);
                let mut spans = vec![Span::styled(prefix, st(self.th.text, self.th.surface))];
                spans.extend(parsed);
                self.lines.push(Line::from(spans));
                i += 1;
                continue;
            }

            // ordered list
            if let Some(num_end) = line.trim_start().find('.')
                && line.trim_start()[..num_end]
                    .chars()
                    .all(|c| c.is_ascii_digit())
            {
                let indent = line.len() - line.trim_start().len();
                let num = &line.trim_start()[..num_end];
                let text = line.trim_start()[num_end + 1..].trim();
                let prefix = format!("{}{}. ", " ".repeat(indent), num);
                let parsed = self.parse_inline(text);
                let mut spans = vec![Span::styled(prefix, st(self.th.text, self.th.surface))];
                spans.extend(parsed);
                self.lines.push(Line::from(spans));
                i += 1;
                continue;
            }

            // task list
            if line.trim_start().starts_with("- [") {
                let indent = line.len() - line.trim_start().len();
                let checkbox_end = line.find(']').unwrap_or(0);
                let checked = line.contains("[x]") || line.contains("[X]");
                let text = if checkbox_end > 0 {
                    line[checkbox_end + 1..].trim()
                } else {
                    ""
                };
                let checkbox = if checked { "[✓]" } else { "[ ]" };
                let prefix = format!("{}{} ", " ".repeat(indent), checkbox);
                let parsed = self.parse_inline(text);
                let mut spans = vec![Span::styled(
                    prefix,
                    st(self.th.text_muted, self.th.surface),
                )];
                spans.extend(parsed);
                self.lines.push(Line::from(spans));
                i += 1;
                continue;
            }

            // table
            if line.contains('|') && i + 1 < raw_lines.len() && raw_lines[i + 1].contains("---") {
                i = self.parse_table(&raw_lines, i);
                continue;
            }

            // blank line
            if line.trim().is_empty() {
                self.lines.push(Line::raw(""));
                i += 1;
                continue;
            }

            // paragraph
            let wrapped = text_wrap(line, self.width);
            for wline in wrapped {
                let spans = self.parse_inline(&wline);
                self.lines.push(Line::from(spans));
            }
            i += 1;
        }
    }

    fn parse_inline(&self, text: &str) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = text.chars().collect();
        let mut current = String::new();
        let current_style = st(self.th.text, self.th.surface);

        while i < chars.len() {
            // **bold**
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), current_style));
                    current.clear();
                }
                i += 2;
                let mut bold_text = String::new();
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '*') {
                    bold_text.push(chars[i]);
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                spans.push(Span::styled(
                    bold_text,
                    current_style.add_modifier(Modifier::BOLD),
                ));
                continue;
            }

            // *italic*
            if chars[i] == '*' {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), current_style));
                    current.clear();
                }
                i += 1;
                let mut italic_text = String::new();
                while i < chars.len() && chars[i] != '*' {
                    italic_text.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
                spans.push(Span::styled(
                    italic_text,
                    current_style.add_modifier(Modifier::ITALIC),
                ));
                continue;
            }

            // `code`
            if chars[i] == '`' {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), current_style));
                    current.clear();
                }
                i += 1;
                let mut code_text = String::new();
                while i < chars.len() && chars[i] != '`' {
                    code_text.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
                let code_padded = format!(" {} ", code_text);
                spans.push(Span::styled(
                    code_padded,
                    st(self.th.text, self.th.markdown_code_bg),
                ));
                continue;
            }

            // ~~strike~~
            if i + 1 < chars.len() && chars[i] == '~' && chars[i + 1] == '~' {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), current_style));
                    current.clear();
                }
                i += 2;
                let mut strike_text = String::new();
                while i + 1 < chars.len() && !(chars[i] == '~' && chars[i + 1] == '~') {
                    strike_text.push(chars[i]);
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                spans.push(Span::styled(
                    strike_text,
                    current_style.add_modifier(Modifier::CROSSED_OUT),
                ));
                continue;
            }

            // [text](url)
            if chars[i] == '[' {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), current_style));
                    current.clear();
                }
                i += 1;
                let mut link_text = String::new();
                while i < chars.len() && chars[i] != ']' {
                    link_text.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
                // skip (url)
                if i < chars.len() && chars[i] == '(' {
                    while i < chars.len() && chars[i] != ')' {
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1;
                    }
                }
                spans.push(Span::styled(
                    link_text,
                    st(self.th.link, self.th.surface).add_modifier(Modifier::UNDERLINED),
                ));
                continue;
            }

            current.push(chars[i]);
            i += 1;
        }

        if !current.is_empty() {
            spans.push(Span::styled(current, current_style));
        }

        spans
    }

    fn parse_table(&mut self, raw_lines: &[&str], start: usize) -> usize {
        let header = raw_lines[start];
        let _sep = raw_lines[start + 1];
        let mut i = start + 2;

        // parse header
        let headers: Vec<&str> = header
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if headers.is_empty() {
            return i;
        }

        let col_w = (self.width / headers.len()).max(5);

        // header row
        let mut header_spans = Vec::new();
        for (idx, h) in headers.iter().enumerate() {
            let display = crate::draw::fit(h, col_w);
            header_spans.push(Span::styled(
                display,
                st(self.th.text, self.th.surface).add_modifier(Modifier::BOLD),
            ));
            if idx < headers.len() - 1 {
                header_spans.push(Span::raw(" │ "));
            }
        }
        self.lines.push(Line::from(header_spans));

        // separator
        let sep_line = (0..headers.len())
            .map(|_| "─".repeat(col_w))
            .collect::<Vec<_>>()
            .join("─┼─");
        self.lines
            .push(Line::styled(sep_line, st(self.th.border, self.th.surface)));

        // data rows
        while i < raw_lines.len() {
            let row = raw_lines[i];
            if !row.contains('|') {
                break;
            }
            let cells: Vec<&str> = row
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if cells.is_empty() {
                break;
            }

            let mut row_spans = Vec::new();
            for (idx, cell) in cells.iter().enumerate().take(headers.len()) {
                let display = crate::draw::fit(cell, col_w);
                row_spans.push(Span::raw(display));
                if idx < headers.len() - 1 {
                    row_spans.push(Span::raw(" │ "));
                }
            }
            self.lines.push(Line::from(row_spans));
            i += 1;
        }

        self.lines.push(Line::raw(""));
        i
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes > 0 && hashes <= 6 && trimmed.chars().nth(hashes) == Some(' ') {
        Some(hashes)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_detected() {
        assert_eq!(heading_level("# Title"), Some(1));
        assert_eq!(heading_level("## Sub"), Some(2));
        assert_eq!(heading_level("### Third"), Some(3));
        assert_eq!(heading_level("#NoSpace"), None);
    }

    #[test]
    fn markdown_renders_bold() {
        let th = Theme::default();
        let md = Markdown::new("**bold**");
        let lines = md.lines(40, &th);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn markdown_wraps_paragraphs() {
        let th = Theme::default();
        let md = Markdown::new("hello world test long line");
        let lines = md.lines(10, &th);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn markdown_renders_heading() {
        let th = Theme::default();
        let md = Markdown::new("# Title\n\ntext");
        let lines = md.lines(40, &th);
        // Debug: print what we got
        eprintln!("Lines generated: {}", lines.len());
        for (i, line) in lines.iter().enumerate() {
            eprintln!(
                "Line {}: spans={}, first_span={:?}",
                i,
                line.spans.len(),
                line.spans.first().map(|s| &s.content)
            );
        }
        assert!(
            lines.len() >= 3,
            "Expected heading + underline + blank + text"
        );
        assert!(
            lines[0].spans[0].content.contains("Title"),
            "First line should contain 'Title'"
        );
    }
}
