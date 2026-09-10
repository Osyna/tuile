//! Textual's 3-row big digits for displaying numbers, time, and other character sequences
//! with visual emphasis. Each digit occupies 3 cells wide by 3 rows tall.
//!
//! ```
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 20, 3);
//! # let mut buf = Buffer::empty(area);
//! let theme = theme::current();
//! Digits::new("12:34:56").color(theme.primary).render(area, &mut buf);
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::Widget;

use crate::draw::{put, put_centered, st};
use crate::theme::{self, Rgb, Theme, Variant};

// Textual's DIGITS3X3: " 0123456789+-^x:ABCDEF$£€()"
// Each glyph is 3 rows of 3 cells (some are narrower like ":" at 1 cell)
const GLYPHS: &str = " 0123456789+-^x:ABCDEF$£€()";

const GLYPH_DATA: &[&str] = &[
    // space (3x3)
    "   ",
    "   ",
    "   ",
    // 0
    "╭─╮",
    "│ │",
    "╰─╯",
    // 1
    "╶╮ ",
    " │ ",
    "╶┴╴",
    // 2
    "╶─╮",
    "┌─┘",
    "╰─╴",
    // 3
    "╶─╮",
    " ─┤",
    "╶─╯",
    // 4
    "╷ ╷",
    "╰─┤",
    "  ╵",
    // 5
    "╭─╴",
    "╰─╮",
    "╶─╯",
    // 6
    "╭─╴",
    "├─╮",
    "╰─╯",
    // 7
    "╶─┐",
    "  │",
    "  ╵",
    // 8
    "╭─╮",
    "├─┤",
    "╰─╯",
    // 9
    "╭─╮",
    "╰─┤",
    "╶─╯",
    // +
    "   ",
    "╶┼╴",
    "   ",
    // -
    "   ",
    "╶─╴",
    "   ",
    // ^
    " ^ ",
    "   ",
    "   ",
    // x
    "   ",
    " × ",
    "   ",
    // : (1 cell wide)
    " ",
    ":",
    " ",
    // A
    "╭─╮",
    "├─┤",
    "╵ ╵",
    // B
    "┌─╮",
    "├─┤",
    "└─╯",
    // C
    "╭─╮",
    "│  ",
    "╰─╯",
    // D
    "┌─╮",
    "│ │",
    "└─╯",
    // E
    "╭─╴",
    "├─ ",
    "╰─╴",
    // F
    "╭─╴",
    "├─ ",
    "╵  ",
    // $
    "╭╫╮",
    "╰╫╮",
    "╰╫╯",
    // £
    "╭─╮",
    "╪═ ",
    "┷━╸",
    // €
    "╭─╮",
    "╪═ ",
    "╰─╯",
    // (
    "╭╴ ",
    "│  ",
    "╰╴ ",
    // )
    " ╶╮",
    "  │",
    " ╶╯",
];

/// Big 3-row digits widget (Textual `Digits` renderable).
pub struct Digits {
    text: String,
    color: Option<Rgb>,
    variant: Option<Variant>,
    align: Alignment,
    bold: bool,
    bg: Option<Rgb>,
    theme: Option<Theme>,
}

impl Digits {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            variant: None,
            align: Alignment::Left,
            bold: false,
            bg: None,
            theme: None,
        }
    }

    /// Display width in cells (3 per digit, 1 for colon).
    pub fn width(text: &str) -> u16 {
        text.chars().map(|c| if c == ':' { 1 } else { 3 }).sum()
    }

    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = Some(v);
        self
    }

    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }

    pub fn bold(mut self, b: bool) -> Self {
        self.bold = b;
        self
    }

    pub fn bg(mut self, b: Rgb) -> Self {
        self.bg = Some(b);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

impl Widget for Digits {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        if area.height < 3 {
            return;
        }

        let fg = if let Some(c) = self.color {
            c
        } else if let Some(v) = self.variant {
            th.variant(v)
        } else {
            th.primary
        };
        let bg = self.bg.unwrap_or(th.surface);

        let mut style = st(fg, bg);
        if self.bold {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }

        let w = Self::width(&self.text);
        let start_x = match self.align {
            Alignment::Left => area.x,
            Alignment::Center => area.x + (area.width.saturating_sub(w)) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(w),
        };

        let mut x = start_x;
        for c in self.text.chars() {
            if x >= area.x + area.width {
                break;
            }

            let idx = GLYPHS.chars().position(|g| g == c);
            let char_w = if c == ':' { 1 } else { 3 };

            if let Some(i) = idx {
                let base = i * 3;
                let lines = [GLYPH_DATA[base], GLYPH_DATA[base + 1], GLYPH_DATA[base + 2]];
                for (row, line) in lines.iter().enumerate() {
                    let y = area.y + row as u16;
                    if y >= area.y + area.height {
                        break;
                    }
                    put(buf, x, y, line, char_w, style);
                }
            } else {
                // Fallback: center the char in a 3x3 box
                let char_rect = Rect {
                    x,
                    y: area.y + 1,
                    width: 3,
                    height: 1,
                };
                put_centered(buf, char_rect, &c.to_string(), style);
            }
            x += char_w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_width() {
        assert_eq!(Digits::width("123"), 9);
        assert_eq!(Digits::width("12:34"), 13); // 3+3+1+3+3
        assert_eq!(Digits::width(":"), 1);
    }

    #[test]
    fn glyph_lookup() {
        assert_eq!(GLYPH_DATA[3], "╭─╮"); // 0 first line
        assert_eq!(GLYPH_DATA[4], "│ │"); // 0 second line
        assert_eq!(GLYPH_DATA[5], "╰─╯"); // 0 third line
    }
}
