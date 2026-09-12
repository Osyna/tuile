//! Buffer-level drawing primitives shared by every widget: clipped text, fills, the full set
//! of Textual border styles (with titles), colour blending, block glyph tables and text
//! wrapping/truncation. All functions clip to the buffer, so callers never index out of range.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Position, Rect};
use ratatui_core::style::{Modifier, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::Rgb;

// styles

pub fn st(fg: Rgb, bg: Rgb) -> Style {
    Style::new().fg(fg.color()).bg(bg.color())
}

pub fn bold(s: Style) -> Style {
    s.add_modifier(Modifier::BOLD)
}

// fills & text

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

pub fn put_aligned(
    buf: &mut Buffer,
    area: Rect,
    text: &str,
    align: Alignment,
    style: Style,
) -> u16 {
    match align {
        Alignment::Left => put(buf, area.x, area.y, text, area.width, style),
        Alignment::Center => put_centered(buf, area, text, style),
        Alignment::Right => put_right(buf, area, text, style),
    }
}
/// Draw `text` at (x, y) applying highlight `ranges`, clipped to `max_width` and the buffer.
/// Ranges are `(start, end, Style)` where `start..end` are half-open grapheme-cluster offsets.
/// Text outside all ranges uses `base_style`. Returns columns drawn.
///
/// Malformed ranges (out-of-bounds, overlapping, descending) are ignored; never panics.
pub fn put_highlighted(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    text: &str,
    max_width: u16,
    base_style: Style,
    ranges: &[(usize, usize, Style)],
) -> u16 {
    if y >= buf.area.bottom() || x >= buf.area.right() || max_width == 0 {
        return 0;
    }
    let clip_width = max_width.min(buf.area.right().saturating_sub(x));
    let mut drawn = 0u16;

    for (grapheme_idx, grapheme) in text.graphemes(true).enumerate() {
        if drawn >= clip_width {
            break;
        }
        // Find the range covering this grapheme (ranges are half-open: [start..end))
        let style = ranges
            .iter()
            .find(|(start, end, _)| *start <= grapheme_idx && grapheme_idx < *end && start < end)
            .map(|(_, _, s)| *s)
            .unwrap_or(base_style);

        let glyph_width = grapheme.width() as u16;
        if drawn + glyph_width <= clip_width {
            put(buf, x + drawn, y, grapheme, glyph_width, style);
            drawn += glyph_width;
        } else {
            break;
        }
    }
    drawn
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
            let to = Position {
                x: at.x.saturating_add(dx),
                y: at.y.saturating_add(dy),
            };
            if !dst.area.contains(to) {
                continue;
            }
            let from = &src[(src_area.x + dx, src_area.y + dy)];
            dst[(to.x, to.y)] = from.clone();
        }
    }
}

// block glyphs

/// `" ▏▎▍▌▋▊▉█"`: left-anchored eighths, index 0..=8.
pub const LEFT_BLOCKS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
/// `" ▁▂▃▄▅▆▇█"`: bottom-anchored eighths, index 0..=8.
pub const LOWER_BLOCKS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Horizontal bar of `width` cells filled to fraction `f` with eighth-block precision.
pub fn hbar(buf: &mut Buffer, x: u16, y: u16, width: u16, f: f32, fg: Rgb, bg: Rgb) {
    let cells = f.clamp(0.0, 1.0) * width as f32;
    for i in 0..width {
        let part = (cells - i as f32).clamp(0.0, 1.0);
        let idx = (part * 8.0).round() as usize;
        // full cells are painted as background: block glyphs leave seams in many fonts
        if idx == 8 {
            put_cell(buf, x + i, y, " ", st(fg, fg))
        } else {
            put_cell(buf, x + i, y, LEFT_BLOCKS[idx], st(fg, bg))
        }
    }
}

/// Vertical bar (bottom-up) of `height` cells filled to fraction `f`.
pub fn vbar(buf: &mut Buffer, x: u16, y: u16, height: u16, f: f32, fg: Rgb, bg: Rgb) {
    let cells = f.clamp(0.0, 1.0) * height as f32;
    for i in 0..height {
        let part = (cells - i as f32).clamp(0.0, 1.0);
        let idx = (part * 8.0).round() as usize;
        if idx == 8 {
            put_cell(buf, x, y + height - 1 - i, " ", st(fg, fg))
        } else {
            put_cell(buf, x, y + height - 1 - i, LOWER_BLOCKS[idx], st(fg, bg))
        }
    }
}

// borders

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
            Border::Tall => ["█", "▔", "█", "█", "█", "█", "▁", "█"],
            Border::Panel => ["█", "█", "█", "█", "█", "█", "▁", "█"],
            Border::Wide => ["█", "█", "█", "▏", "▕", "█", "█", "█"],
        }
    }

    /// Whether this style paints anything (`None` draws nothing but still reserves the row/col).
    pub fn visible(self) -> bool {
        !matches!(self, Border::None | Border::Blank)
    }

    /// Area inside the border.
    pub fn inner(self, area: Rect) -> Rect {
        if area.width < 2 || area.height < 2 {
            return Rect {
                width: 0,
                height: 0,
                ..area
            };
        }
        Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width - 2,
            height: area.height - 2,
        }
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
                // `█` means "solid": paint the cell background instead of drawing a glyph, so
                // fonts show no seams and the thin `▔▁▏▕` lines terminate cleanly against it
                if g[i] == "█" {
                    buf[(x, y)]
                        .set_symbol(" ")
                        .set_fg(fg.color())
                        .set_bg(fg.color());
                } else {
                    buf[(x, y)]
                        .set_symbol(g[i])
                        .set_fg(fg.color())
                        .set_bg(bg.color());
                }
            }
        }
    }

    /// Draw the border plus a title on the top edge (title in the border colour, bold).
    /// Returns the inner area.
    pub fn draw_titled(
        self,
        buf: &mut Buffer,
        area: Rect,
        fg: Rgb,
        bg: Rgb,
        title: &str,
        align: Alignment,
    ) -> Rect {
        let style = match self {
            Border::Panel => st(fg.text_on(0.9), fg).add_modifier(Modifier::BOLD),
            _ => st(fg, bg).add_modifier(Modifier::BOLD),
        };
        self.draw_titled_with(buf, area, fg, bg, title, align, style)
    }

    /// Like [`Border::draw_titled`] with an explicit title style (e.g. a dim border with a
    /// bright title, the usual "card" look).
    // The render path takes buffer, position, size and style separately: bundling them into a
    // struct would cost an allocation per call.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_titled_with(
        self,
        buf: &mut Buffer,
        area: Rect,
        fg: Rgb,
        bg: Rgb,
        title: &str,
        align: Alignment,
        title_style: Style,
    ) -> Rect {
        self.draw(buf, area, fg, bg);
        if !title.is_empty() && area.width > 4 {
            let text = if matches!(
                self,
                Border::Panel | Border::Thick | Border::Outer | Border::Inner
            ) {
                title.to_string()
            } else {
                format!(" {title} ")
            };
            let slot = Rect {
                x: area.x + 1,
                y: area.y,
                width: area.width - 2,
                height: 1,
            };
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

// edge bars & field shapes

/// Thickness of a vertical accent bar. `Full` paints the whole cell (Textual's `tall`
/// border); the others draw an eighth-block glyph, so `Hair` is one pixel column in most fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Edge {
    /// `▏` / `▕`, 1/8 cell.
    Hair,
    /// `▎` / `▕`, 2/8 cell (default for accent bars).
    #[default]
    Thin,
    /// `▌` / `▐`, half cell.
    Half,
    /// Whole cell, painted as background.
    Full,
}

impl Edge {
    /// Draw `height` rows of bar at `x`; `right` anchors the glyph to the cell's right side.
    // The render path takes buffer, position, size and style separately: bundling them into a
    // struct would cost an allocation per call.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        height: u16,
        right: bool,
        color: Rgb,
        bg: Rgb,
    ) {
        let (glyph, style) = match (self, right) {
            (Edge::Full, _) => (" ", st(color, color)),
            (Edge::Hair, false) => ("▏", st(color, bg)),
            (Edge::Thin, false) => ("▎", st(color, bg)),
            (Edge::Half, false) => ("▌", st(color, bg)),
            (Edge::Hair | Edge::Thin, true) => ("▕", st(color, bg)),
            (Edge::Half, true) => ("▐", st(color, bg)),
        };
        for dy in 0..height {
            put_cell(buf, x, y.saturating_add(dy), glyph, style);
        }
    }
}

/// Frame drawn around a text field; `Tall` is Textual's default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldShape {
    /// Side bars of the given thickness plus thin `▔`/`▁` lines above and below.
    Tall(Edge),
    /// Left and right bars only.
    Bars(Edge),
    /// Left bar only.
    Bar(Edge),
    /// A `─` rule above and below.
    Rule,
    /// Rounded box.
    Round,
    /// A `❯` prompt glyph before the first line, no frame.
    Prompt,
    /// Claude Code's composer: a full-width band in the field colour with one padding row
    /// above and below and a muted `›` prompt.
    Band,
    /// Nothing.
    None,
}

impl Default for FieldShape {
    fn default() -> Self {
        FieldShape::Tall(Edge::Full)
    }
}

impl FieldShape {
    /// Rows the shape adds around the content.
    pub fn vertical_chrome(self) -> u16 {
        match self {
            FieldShape::Tall(_) | FieldShape::Rule | FieldShape::Round | FieldShape::Band => 2,
            _ => 0,
        }
    }

    /// Horizontal padding between the shape and the text (prompt shapes carry their own gap).
    pub fn padding(self) -> u16 {
        match self {
            FieldShape::Prompt | FieldShape::Band => 0,
            _ => 1,
        }
    }

    /// Draw the shape in `color` over `bg`; returns the content rect.
    pub fn draw(self, buf: &mut Buffer, area: Rect, color: Rgb, bg: Rgb) -> Rect {
        if area.is_empty() {
            return area;
        }
        match self {
            FieldShape::Tall(Edge::Full) => {
                Border::Tall.draw(buf, area, color, bg);
                Border::Tall.inner(area)
            }
            FieldShape::Tall(edge) => {
                if area.height < 2 || area.width < 2 {
                    return area;
                }
                hline(buf, area.x, area.y, area.width, "▔", st(color, bg));
                hline(
                    buf,
                    area.x,
                    area.bottom() - 1,
                    area.width,
                    "▁",
                    st(color, bg),
                );
                edge.draw(buf, area.x, area.y + 1, area.height - 2, false, color, bg);
                edge.draw(
                    buf,
                    area.right() - 1,
                    area.y + 1,
                    area.height - 2,
                    true,
                    color,
                    bg,
                );
                crate::layout::pad(area, 1, 1)
            }
            FieldShape::Bars(edge) => {
                if area.width < 2 {
                    return area;
                }
                edge.draw(buf, area.x, area.y, area.height, false, color, bg);
                edge.draw(buf, area.right() - 1, area.y, area.height, true, color, bg);
                crate::layout::pad(area, 1, 0)
            }
            FieldShape::Bar(edge) => {
                edge.draw(buf, area.x, area.y, area.height, false, color, bg);
                Rect {
                    x: area.x + 1,
                    width: area.width - 1,
                    ..area
                }
            }
            FieldShape::Rule => {
                if area.height < 2 {
                    return area;
                }
                hline(buf, area.x, area.y, area.width, "─", st(color, bg));
                hline(
                    buf,
                    area.x,
                    area.bottom() - 1,
                    area.width,
                    "─",
                    st(color, bg),
                );
                crate::layout::pad(area, 0, 1)
            }
            FieldShape::Round => {
                Border::Round.draw(buf, area, color, bg);
                Border::Round.inner(area)
            }
            FieldShape::Prompt => {
                put(
                    buf,
                    area.x,
                    area.y,
                    "❯",
                    1,
                    st(color, bg).add_modifier(Modifier::BOLD),
                );
                Rect {
                    x: area.x + 2,
                    width: area.width.saturating_sub(2),
                    ..area
                }
            }
            FieldShape::Band => {
                if area.height < 3 || area.width < 4 {
                    return area;
                }
                // the caller already filled `bg` across the area; the band is that fill plus padding
                put(buf, area.x + 1, area.y + 1, "›", 1, st(color, bg));
                Rect {
                    x: area.x + 3,
                    y: area.y + 1,
                    width: area.width - 3,
                    height: area.height - 2,
                }
            }
            FieldShape::None => area,
        }
    }
}

/// Simple drop shadow (one cell right and below), Textual `.-shadow` look.
pub fn shadow(buf: &mut Buffer, area: Rect, toward: Rgb, f: f32) {
    let right = Rect {
        x: area.right(),
        y: area.y + 1,
        width: 1,
        height: area.height,
    };
    let bottom = Rect {
        x: area.x + 1,
        y: area.bottom(),
        width: area.width,
        height: 1,
    };
    blend_area(buf, right, toward, f);
    blend_area(buf, bottom, toward, f);
}

// text

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
    if tw < w {
        format!("{t}{}", " ".repeat(w - tw))
    } else {
        t
    }
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
    fn field_shapes_return_the_content_rect_and_draw_their_edges() {
        let area = Rect::new(0, 0, 10, 3);
        let c = Rgb(0, 0, 255);
        let bg = Rgb(0, 0, 0);
        let mut buf = Buffer::empty(area);
        assert_eq!(
            FieldShape::Bars(Edge::Hair).draw(&mut buf, area, c, bg),
            Rect::new(1, 0, 8, 3)
        );
        assert_eq!(buf[(0, 1)].symbol(), "▏");
        assert_eq!(buf[(9, 1)].symbol(), "▕");
        let mut buf = Buffer::empty(area);
        assert_eq!(
            FieldShape::Bar(Edge::Full).draw(&mut buf, area, c, bg),
            Rect::new(1, 0, 9, 3)
        );
        assert_eq!(buf[(0, 2)].bg, c.color(), "full edge is painted background");
        let mut buf = Buffer::empty(area);
        assert_eq!(
            FieldShape::Rule.draw(&mut buf, area, c, bg),
            Rect::new(0, 1, 10, 1)
        );
        assert_eq!(buf[(5, 0)].symbol(), "─");
        assert_eq!(buf[(5, 2)].symbol(), "─");
        let mut buf = Buffer::empty(area);
        assert_eq!(
            FieldShape::Prompt.draw(&mut buf, area, c, bg),
            Rect::new(2, 0, 8, 3)
        );
        assert_eq!(buf[(0, 0)].symbol(), "❯");
        let mut buf = Buffer::empty(area);
        assert_eq!(
            FieldShape::Band.draw(&mut buf, area, c, bg),
            Rect::new(3, 1, 7, 1)
        );
        assert_eq!(buf[(1, 1)].symbol(), "›");
        assert_eq!(FieldShape::Band.padding(), 0);
        assert_eq!(FieldShape::Tall(Edge::Thin).vertical_chrome(), 2);
        assert_eq!(FieldShape::Bars(Edge::Thin).vertical_chrome(), 0);
    }

    #[test]
    fn put_highlighted_applies_ranges_at_grapheme_offsets() {
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let base = st(Rgb(255, 255, 255), Rgb(0, 0, 0));
        let hl = st(Rgb(0, 255, 0), Rgb(0, 0, 0));
        let text = "hello world";
        let ranges = vec![(0, 5, hl)]; // highlight "hello"
        put_highlighted(&mut buf, 0, 0, text, 20, base, &ranges);
        // First 5 graphemes should be highlighted
        assert_eq!(buf[(0, 0)].fg, hl.fg.unwrap());
        assert_eq!(buf[(4, 0)].fg, hl.fg.unwrap());
        // Space and rest should use base
        assert_eq!(buf[(5, 0)].fg, base.fg.unwrap());
        assert_eq!(buf[(6, 0)].fg, base.fg.unwrap());
    }

    #[test]
    fn put_highlighted_handles_wide_graphemes() {
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let base = st(Rgb(255, 255, 255), Rgb(0, 0, 0));
        let hl = st(Rgb(0, 255, 0), Rgb(0, 0, 0));
        let text = "你好 world"; // "你好" are 2 graphemes, each 2 cells wide
        let ranges = vec![(0, 2, hl)]; // highlight the two CJK chars
        put_highlighted(&mut buf, 0, 0, text, 20, base, &ranges);
        // Check first cell of each wide grapheme (cells 0 and 2)
        assert_eq!(buf[(0, 0)].fg, hl.fg.unwrap(), "first wide char first cell");
        assert_eq!(
            buf[(2, 0)].fg,
            hl.fg.unwrap(),
            "second wide char first cell"
        );
        // Space after (cell 4) should use base
        assert_eq!(buf[(4, 0)].fg, base.fg.unwrap());
    }

    #[test]
    fn put_highlighted_ignores_malformed_ranges() {
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let base = st(Rgb(255, 255, 255), Rgb(0, 0, 0));
        let hl = st(Rgb(0, 255, 0), Rgb(0, 0, 0));
        let text = "hello";
        // Out of bounds, overlapping, and descending ranges
        let ranges = vec![
            (100, 200, hl), // out of bounds
            (3, 2, hl),     // descending (start > end)
        ];
        // Should not panic
        put_highlighted(&mut buf, 0, 0, text, 20, base, &ranges);
        // All cells should use base style
        for x in 0..5 {
            assert_eq!(buf[(x, 0)].fg, base.fg.unwrap());
        }
    }

    #[test]
    fn put_highlighted_clips_to_width() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer::empty(area);
        let base = st(Rgb(255, 255, 255), Rgb(0, 0, 0));
        let hl = st(Rgb(0, 255, 0), Rgb(0, 0, 0));
        let text = "hello world";
        let ranges = vec![(0, 11, hl)];
        // Only 3 cells available
        let drawn = put_highlighted(&mut buf, 0, 0, text, 3, base, &ranges);
        assert_eq!(drawn, 3);
        assert_eq!(buf[(0, 0)].symbol(), "h");
        assert_eq!(buf[(1, 0)].symbol(), "e");
        assert_eq!(buf[(2, 0)].symbol(), "l");
    }

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
            b.draw_titled(
                &mut buf,
                Rect::new(2, 1, 20, 10),
                Rgb(1, 2, 3),
                Rgb(0, 0, 0),
                "Title",
                Alignment::Left,
            );
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
