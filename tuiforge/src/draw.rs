//! Buffer-level drawing primitives shared by every widget: clipped text, fills, the full set
//! of Textual border styles (with titles), colour blending, block glyph tables and text
//! wrapping/truncation. All functions clip to the buffer, so callers never index out of range.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::Rgb;

// ───────────────────────────── styles ─────────────────────────────

pub fn st(fg: Rgb, bg: Rgb) -> Style {
    Style::new().fg(fg.color()).bg(bg.color())
}

pub fn bold(s: Style) -> Style {
    s.add_modifier(Modifier::BOLD)
}

// ───────────────────────────── fills & text ─────────────────────────────

/// Paint `bg` over an area (symbols reset to spaces).
pub fn fill(buf: &mut Buffer, area: Rect, bg: Rgb) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let c = &mut buf[(x, y)];
            c.set_symbol(" ").set_bg(bg.color()).set_fg(bg.color());
            c.modifier = Modifier::empty();
        }
    }
}

/// Paint only the background colour, keeping symbols and foregrounds (tint a region).
pub fn fill_bg(buf: &mut Buffer, area: Rect, bg: Rgb) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].set_bg(bg.color());
        }
    }
}

/// Draw `text` at (x, y) clipped to `max_width` and the buffer; returns the width drawn.
pub fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, max_width: u16, style: Style) -> u16 {
    if !buf.area.contains(Position { x, y }) || max_width == 0 {
        return 0;
    }
    let max = (max_width as u32).min((buf.area.right() - x) as u32) as usize;
    let (nx, _) = buf.set_stringn(x, y, text, max, style);
    nx - x
}

pub fn put_centered(buf: &mut Buffer, area: Rect, text: &str, style: Style) -> u16 {
    let w = (text.width() as u16).min(area.width);
    let x = area.x + (area.width - w) / 2;
    put(buf, x, area.y, text, w, style)
}

pub fn put_right(buf: &mut Buffer, area: Rect, text: &str, style: Style) -> u16 {
    let w = (text.width() as u16).min(area.width);
    put(buf, area.right() - w, area.y, text, w, style)
}

pub fn put_aligned(buf: &mut Buffer, area: Rect, text: &str, align: Alignment, style: Style) -> u16 {
    match align {
        Alignment::Left => put(buf, area.x, area.y, text, area.width, style),
        Alignment::Center => put_centered(buf, area, text, style),
        Alignment::Right => put_right(buf, area, text, style),
    }
}

/// Draw a single cell symbol.
pub fn put_cell(buf: &mut Buffer, x: u16, y: u16, sym: &str, style: Style) {
    if buf.area.contains(Position { x, y }) {
        buf[(x, y)].set_symbol(sym).set_style(style);
    }
}

pub fn hline(buf: &mut Buffer, x: u16, y: u16, width: u16, sym: &str, style: Style) {
    for i in 0..width {
        put_cell(buf, x.saturating_add(i), y, sym, style);
    }
}

pub fn vline(buf: &mut Buffer, x: u16, y: u16, height: u16, sym: &str, style: Style) {
    for i in 0..height {
        put_cell(buf, x, y.saturating_add(i), sym, style);
    }
}

/// Blend every cell's colours towards `toward` (Textual's dimmed modal backdrop / opacity).
pub fn blend_area(buf: &mut Buffer, area: Rect, toward: Rgb, f: f32) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            if let Some(fg) = Rgb::from_color(cell.fg) {
                cell.fg = fg.blend(toward, f).color();
            }
            if let Some(bg) = Rgb::from_color(cell.bg) {
                cell.bg = bg.blend(toward, f).color();
            }
        }
    }
}

/// Copy a rectangle from `src` onto `dst` at `at` (viewport blitting for scroll views).
pub fn blit(dst: &mut Buffer, at: Position, src: &Buffer, src_area: Rect) {
    let src_area = src_area.intersection(src.area);
    for dy in 0..src_area.height {
        for dx in 0..src_area.width {
            let to = Position { x: at.x.saturating_add(dx), y: at.y.saturating_add(dy) };
            if !dst.area.contains(to) {
                continue;
            }
            let from = &src[(src_area.x + dx, src_area.y + dy)];
            dst[(to.x, to.y)] = from.clone();
        }
    }
}

// ───────────────────────────── block glyphs ─────────────────────────────

/// `" ▏▎▍▌▋▊▉█"` — left-anchored eighths, index 0..=8.
pub const LEFT_BLOCKS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
/// `" ▁▂▃▄▅▆▇█"` — bottom-anchored eighths, index 0..=8.
pub const LOWER_BLOCKS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Horizontal bar of `width` cells filled to fraction `f` with eighth-block precision.
pub fn hbar(buf: &mut Buffer, x: u16, y: u16, width: u16, f: f32, fg: Rgb, bg: Rgb) {
    let cells = f.clamp(0.0, 1.0) * width as f32;
    for i in 0..width {
        let part = (cells - i as f32).clamp(0.0, 1.0);
        let idx = (part * 8.0).round() as usize;
        // full cells are painted as background: block glyphs leave seams in many fonts
        if idx == 8 { put_cell(buf, x + i, y, " ", st(fg, fg)) } else { put_cell(buf, x + i, y, LEFT_BLOCKS[idx], st(fg, bg)) }
    }
}

/// Vertical bar (bottom-up) of `height` cells filled to fraction `f`.
pub fn vbar(buf: &mut Buffer, x: u16, y: u16, height: u16, f: f32, fg: Rgb, bg: Rgb) {
    let cells = f.clamp(0.0, 1.0) * height as f32;
    for i in 0..height {
        let part = (cells - i as f32).clamp(0.0, 1.0);
        let idx = (part * 8.0).round() as usize;
        if idx == 8 { put_cell(buf, x, y + height - 1 - i, " ", st(fg, fg)) } else { put_cell(buf, x, y + height - 1 - i, LOWER_BLOCKS[idx], st(fg, bg)) }
    }
}

// ───────────────────────────── borders ─────────────────────────────

/// Every Textual border style. Glyph tables are `[top-left, top, top-right, left, right,
/// bottom-left, bottom, bottom-right]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Border {
    #[default]
    None,
    Ascii,
    Round,
    Solid,
    Double,
    Dashed,
    Heavy,
    Inner,
    Outer,
    Thick,
    Hkey,
    Vkey,
    /// Textual's signature: `▊` left, `▎` right, `▔` top, `▁` bottom.
    Tall,
    /// Like `Tall` with a solid top row (title bar).
    Panel,
    Wide,
    Blank,
}

impl Border {
    pub const ALL: [Border; 16] = [
        Border::None,
        Border::Ascii,
        Border::Round,
        Border::Solid,
        Border::Double,
        Border::Dashed,
        Border::Heavy,
        Border::Inner,
        Border::Outer,
        Border::Thick,
        Border::Hkey,
        Border::Vkey,
        Border::Tall,
        Border::Panel,
        Border::Wide,
        Border::Blank,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Border::None => "none",
            Border::Ascii => "ascii",
            Border::Round => "round",
            Border::Solid => "solid",
            Border::Double => "double",
            Border::Dashed => "dashed",
            Border::Heavy => "heavy",
            Border::Inner => "inner",
            Border::Outer => "outer",
            Border::Thick => "thick",
            Border::Hkey => "hkey",
            Border::Vkey => "vkey",
            Border::Tall => "tall",
            Border::Panel => "panel",
            Border::Wide => "wide",
            Border::Blank => "blank",
        }
    }

    pub fn glyphs(self) -> [&'static str; 8] {
        match self {
            Border::None => [" "; 8],
            Border::Blank => [" "; 8],
            Border::Ascii => ["+", "-", "+", "|", "|", "+", "-", "+"],
            Border::Round => ["╭", "─", "╮", "│", "│", "╰", "─", "╯"],
            Border::Solid => ["┌", "─", "┐", "│", "│", "└", "─", "┘"],
            Border::Double => ["╔", "═", "╗", "║", "║", "╚", "═", "╝"],
            Border::Dashed => ["┏", "╍", "┓", "╏", "╏", "┗", "╍", "┛"],
            Border::Heavy => ["┏", "━", "┓", "┃", "┃", "┗", "━", "┛"],
            Border::Inner => ["▗", "▄", "▖", "▐", "▌", "▝", "▀", "▘"],
            Border::Outer => ["▛", "▀", "▜", "▌", "▐", "▙", "▄", "▟"],
            Border::Thick => ["█", "▀", "█", "█", "█", "█", "▄", "█"],
            Border::Hkey => ["▔", "▔", "▔", " ", " ", "▁", "▁", "▁"],
            Border::Vkey => ["▏", " ", "▕", "▏", "▕", "▏", " ", "▕"],
            Border::Tall => ["▏", "▔", "▕", "▏", "▕", "▏", "▁", "▕"],
            Border::Panel => ["█", "█", "█", "▏", "▕", "▏", "▁", "▕"],
            Border::Wide => ["▁", "▁", "▁", "▏", "▕", "▔", "▔", "▔"],
        }
    }

    /// Whether this style paints anything (`None` draws nothing but still reserves the row/col).
    pub fn visible(self) -> bool {
        !matches!(self, Border::None | Border::Blank)
    }

    /// Area inside the border.
    pub fn inner(self, area: Rect) -> Rect {
        if area.width < 2 || area.height < 2 {
            return Rect { width: 0, height: 0, ..area };
        }
        Rect { x: area.x + 1, y: area.y + 1, width: area.width - 2, height: area.height - 2 }
    }

    /// Draw the border. `fg` is the line colour, `bg` the fill behind the glyphs.
    pub fn draw(self, buf: &mut Buffer, area: Rect, fg: Rgb, bg: Rgb) {
        self.draw_ex(buf, area, fg, fg, fg, bg);
    }

    /// Draw with separate colours for top, bottom and sides (Textual `border-top:`…).
    pub fn draw_ex(self, buf: &mut Buffer, area: Rect, top: Rgb, bottom: Rgb, sides: Rgb, bg: Rgb) {
        if area.width < 2 || area.height < 2 || self == Border::None {
            return;
        }
        let g = self.glyphs();
        let a = area.intersection(buf.area);
        for y in a.top()..a.bottom() {
            for x in a.left()..a.right() {
                let (i, fg) = if y == area.top() {
                    if x == area.left() {
                        (0, top)
                    } else if x == area.right() - 1 {
                        (2, top)
                    } else {
                        (1, top)
                    }
                } else if y == area.bottom() - 1 {
                    if x == area.left() {
                        (5, bottom)
                    } else if x == area.right() - 1 {
                        (7, bottom)
                    } else {
                        (6, bottom)
                    }
                } else if x == area.left() {
                    (3, sides)
                } else if x == area.right() - 1 {
                    (4, sides)
                } else {
                    continue;
                };
                // Panel's top row is a solid bar in the line colour
                let (fg, bg) = if self == Border::Panel && y == area.top() { (fg, fg) } else { (fg, bg) };
                buf[(x, y)].set_symbol(g[i]).set_fg(fg.color()).set_bg(bg.color());
            }
        }
    }

    /// Draw the border plus a title on the top edge (title in the border colour, bold).
    /// Returns the inner area.
    pub fn draw_titled(self, buf: &mut Buffer, area: Rect, fg: Rgb, bg: Rgb, title: &str, align: Alignment) -> Rect {
        let style = match self {
            Border::Panel => st(fg.text_on(0.9), fg).add_modifier(Modifier::BOLD),
            _ => st(fg, bg).add_modifier(Modifier::BOLD),
        };
        self.draw_titled_with(buf, area, fg, bg, title, align, style)
    }

    /// Like [`Border::draw_titled`] with an explicit title style (e.g. a dim border with a
    /// bright title, the usual "card" look).
    pub fn draw_titled_with(self, buf: &mut Buffer, area: Rect, fg: Rgb, bg: Rgb, title: &str, align: Alignment, title_style: Style) -> Rect {
        self.draw(buf, area, fg, bg);
        if !title.is_empty() && area.width > 4 {
            let text = if matches!(self, Border::Panel | Border::Tall | Border::Thick | Border::Outer | Border::Inner) {
                title.to_string()
            } else {
                format!(" {title} ")
            };
            let slot = Rect { x: area.x + 1, y: area.y, width: area.width - 2, height: 1 };
            let text = truncate(&text, slot.width as usize);
            put_aligned(buf, slot, &text, align, title_style);
        }
        self.inner(area)
    }
}

/// Textual's `tall` border with per-edge colours (kept for widgets that colour edges separately).
pub fn tall_border(buf: &mut Buffer, area: Rect, top: Rgb, bottom: Rgb, sides: Rgb, bg: Rgb) {
    Border::Tall.draw_ex(buf, area, top, bottom, sides, bg);
}

/// Textual's `thick` border: `█` sides and corners, `▀` top, `▄` bottom.
pub fn thick_border(buf: &mut Buffer, area: Rect, color: Rgb, bg: Rgb) {
    Border::Thick.draw(buf, area, color, bg);
}

/// Simple drop shadow (one cell right and below), Textual `.-shadow` look.
pub fn shadow(buf: &mut Buffer, area: Rect, toward: Rgb, f: f32) {
    let right = Rect { x: area.right(), y: area.y + 1, width: 1, height: area.height };
    let bottom = Rect { x: area.x + 1, y: area.bottom(), width: area.width, height: 1 };
    blend_area(buf, right, toward, f);
    blend_area(buf, bottom, toward, f);
}

// ───────────────────────────── text ─────────────────────────────

pub fn width(s: &str) -> usize {
    s.width()
}

/// Truncate to `max` cells, appending `…` when cut.
pub fn truncate(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for g in s.graphemes(true) {
        let gw = g.width();
        if w + gw > max - 1 {
            break;
        }
        out.push_str(g);
        w += gw;
    }
    out.push('…');
    out
}

/// Truncate keeping the *end* of the string (`…tail`), handy for paths.
pub fn truncate_start(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut parts: Vec<&str> = Vec::new();
    let mut w = 0;
    for g in s.graphemes(true).rev() {
        let gw = g.width();
        if w + gw > max - 1 {
            break;
        }
        parts.push(g);
        w += gw;
    }
    let mut out = String::from("…");
    out.extend(parts.iter().rev().copied());
    out
}

/// Pad or truncate to exactly `w` cells.
pub fn fit(s: &str, w: usize) -> String {
    let t = truncate(s, w);
    let tw = t.width();
    if tw < w { format!("{t}{}", " ".repeat(w - tw)) } else { t }
}

/// Greedy word wrap on display width; explicit newlines respected; long words are split.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut lw = 0usize;
        for word in para.split(' ') {
            let ww = word.width();
            if ww > width {
                // hard-split an over-long word
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                let mut chunk = String::new();
                let mut cw = 0;
                for ch in word.chars() {
                    let c = ch.width().unwrap_or(0);
                    if cw + c > width {
                        out.push(std::mem::take(&mut chunk));
                        cw = 0;
                    }
                    chunk.push(ch);
                    cw += c;
                }
                line = chunk;
                lw = cw;
                continue;
            }
            if lw == 0 {
                line.push_str(word);
                lw = ww;
            } else if lw + 1 + ww <= width {
                line.push(' ');
                line.push_str(word);
                lw += 1 + ww;
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
                lw = ww;
            }
        }
        out.push(line);
    }
    out
}

/// Number of grapheme clusters (what a cursor steps over).
pub fn grapheme_len(s: &str) -> usize {
    s.graphemes(true).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_respects_width_and_splits_long_words() {
        let lines = wrap("the quick brown fox jumps", 10);
        assert_eq!(lines, vec!["the quick", "brown fox", "jumps"]);
        assert!(wrap("abcdefghijklmnop", 5).iter().all(|l| l.width() <= 5));
        assert_eq!(wrap("a\n\nb", 10), vec!["a", "", "b"]);
    }

    #[test]
    fn truncate_uses_ellipsis() {
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("hi", 5), "hi");
        assert_eq!(truncate_start("/usr/local/bin", 8), "…cal/bin");
        assert_eq!(fit("ab", 4), "ab  ");
    }

    #[test]
    fn borders_draw_inside_buffer_only() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
        for b in Border::ALL {
            b.draw_titled(&mut buf, Rect::new(2, 1, 20, 10), Rgb(1, 2, 3), Rgb(0, 0, 0), "Title", Alignment::Left);
        }
        assert_eq!(buf[(2, 1)].symbol(), Border::Blank.glyphs()[0]);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
        Border::Round.draw(&mut buf, Rect::new(0, 0, 10, 4), Rgb(1, 2, 3), Rgb(0, 0, 0));
        assert_eq!(buf[(0, 0)].symbol(), "╭");
        assert_eq!(buf[(9, 3)].symbol(), "╯");
        hbar(&mut buf, 1, 1, 4, 0.5, Rgb(9, 9, 9), Rgb(0, 0, 0));
        assert_eq!(buf[(2, 1)].bg, Rgb(9, 9, 9).color());
        assert_eq!(buf[(3, 1)].symbol(), " ");
        assert_eq!(buf[(3, 1)].bg, Rgb(0, 0, 0).color());
    }
}
