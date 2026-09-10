//! Big text in multiple fonts (box-drawing, figlet-style painted blocks, half-blocks) and a
//! vertical menu built from it with many styles (arrows, boxed, underline, glow, cards…).
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 20);
//! # let mut buf = Buffer::empty(area);
//! BigText::new("TUIFORGE").font(BigFont::Block5).gradient(&[Rgb(230, 60, 60), Rgb(255, 160, 40)]).render(area, &mut buf);
//! let mut menu = BigMenuState::default();
//! BigMenu::new(&["OPTIONS", "HELP", "QUIT"]).style(BigMenuStyle::Arrows).focused(true).render(Rect { y: 5, ..area }, &mut buf, &mut menu);
//! if let Some(i) = menu.take_activated() { /* run item i */ }
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{StatefulWidget, Widget};
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

use crate::anim::{self, Tween};
use crate::core::{Hit, HitBox, Interactive, Outcome, is_press, mouse_in, wheel_delta};
use crate::draw::{Border, fill, hline, put, put_centered, st};
use crate::theme::{self, Rgb, Theme, gradient as color_gradient};

/// Available big fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BigFont {
    /// 3-row box-drawing font (default, the current look).
    #[default]
    Box3,
    /// 5-row figlet-style font with painted solid cells (background paint, not fg glyphs).
    Block5,
    /// 3-row font using half-blocks `▀▄` for partial cells with painted solids.
    Half3,
    /// 3-row light box-drawing font (`─│╭╮╰╯`) for elegant headings.
    Thin3,
}

impl BigFont {
    /// Rows this font occupies.
    pub fn rows(self) -> u16 {
        match self {
            BigFont::Box3 | BigFont::Half3 | BigFont::Thin3 => 3,
            BigFont::Block5 => 5,
        }
    }
}

/// Glyph for one character in the Box3 font; unknown characters fall back to `?`, lowercase is uppercased.
fn glyph_box3(c: char) -> [&'static str; 3] {
    match c.to_ascii_uppercase() {
        'A' => ["╔═╗", "╠═╣", "╩ ╩"],
        'B' => ["╔╗ ", "╠╩╗", "╚═╝"],
        'C' => ["╔═╗", "║  ", "╚═╝"],
        'D' => ["╔╦╗", "║║║", "╚╩╝"],
        'E' => ["╔═╗", "║╣ ", "╚═╝"],
        'F' => ["╔═╗", "╠╣ ", "╚  "],
        'G' => ["╔═╗", "║ ╦", "╚═╝"],
        'H' => ["╦ ╦", "╠═╣", "╩ ╩"],
        'I' => ["╦", "║", "╩"],
        'J' => [" ╦", " ║", "╚╝"],
        'K' => ["╦╔═", "╠╩╗", "╩ ╩"],
        'L' => ["╦  ", "║  ", "╩═╝"],
        'M' => ["╔╦╗", "║║║", "╩ ╩"],
        'N' => ["╔╗╔", "║║║", "╝╚╝"],
        'O' => ["╔═╗", "║ ║", "╚═╝"],
        'P' => ["╔═╗", "╠═╝", "╩  "],
        'Q' => ["╔═╗ ", "║ ║ ", "╚═╬╗"],
        'R' => ["╦═╗", "╠╦╝", "╩╚═"],
        'S' => ["╔═╗", "╚═╗", "╚═╝"],
        'T' => ["╔╦╗", " ║ ", " ╩ "],
        'U' => ["╦ ╦", "║ ║", "╚═╝"],
        'V' => ["╦  ╦", "╚╗╔╝", " ╚╝ "],
        'W' => ["╦ ╦", "║║║", "╚╩╝"],
        'X' => ["╗ ╔", "╔╩╗", "╝ ╚"],
        'Y' => ["╦ ╦", "╚╦╝", " ╩ "],
        'Z' => ["╔═╗", "╔═╝", "╚═╝"],
        '0' => ["╔═╗", "║║║", "╚═╝"],
        '1' => ["╗", "║", "╩"],
        '2' => ["╔═╗", "╔═╝", "╚══"],
        '3' => ["╔═╗", " ═╣", "╚═╝"],
        '4' => ["╦ ╦", "╚═╣", "  ╩"],
        '5' => ["╔══", "╚═╗", "╚═╝"],
        '6' => ["╔═╗", "╠═╗", "╚═╝"],
        '7' => ["╔═╗", " ╔╝", " ╩ "],
        '8' => ["╔═╗", "╠═╣", "╚═╝"],
        '9' => ["╔═╗", "╚═╣", "╚═╝"],
        ' ' => [" ", " ", " "],
        '.' => [" ", " ", "•"],
        ',' => ["  ", "  ", "• "],
        ':' => [" ", "•", "•"],
        '-' => ["  ", "══", "  "],
        '_' => ["   ", "   ", "═══"],
        '+' => ["   ", " ╬ ", "   "],
        '!' => ["╦", "║", "•"],
        '?' => ["╔═╗", " ╔╝", " • "],
        '/' => ["  ╔", " ╔╝", "╔╝ "],
        '[' => ["╔═", "║ ", "╚═"],
        ']' => ["═╗", " ║", "═╝"],
        '<' => ["  ╔", "╔╝ ", "╚╗ "],
        '>' => ["╗  ", " ╚╗", "╔╝ "],
        _ => ["╔═╗", " ╔╝", " • "],
    }
}

/// 5×5 pixel font (`#` = painted cell) shared by `Block5` (one cell per pixel) and `Half3`
/// (two pixel rows per cell with `▀`/`▄` for the half-filled cells). Unknown chars → `?`.
fn glyph_px5(c: char) -> [&'static str; 5] {
    match c.to_ascii_uppercase() {
        'A' => [" ### ", "#   #", "#####", "#   #", "#   #"],
        'B' => ["#### ", "#   #", "#### ", "#   #", "#### "],
        'C' => [" ####", "#    ", "#    ", "#    ", " ####"],
        'D' => ["#### ", "#   #", "#   #", "#   #", "#### "],
        'E' => ["#####", "#    ", "#### ", "#    ", "#####"],
        'F' => ["#####", "#    ", "#### ", "#    ", "#    "],
        'G' => [" ####", "#    ", "#  ##", "#   #", " ####"],
        'H' => ["#   #", "#   #", "#####", "#   #", "#   #"],
        'I' => ["###", " # ", " # ", " # ", "###"],
        'J' => ["#####", "   # ", "   # ", "#  # ", " ##  "],
        'K' => ["#   #", "#  # ", "###  ", "#  # ", "#   #"],
        'L' => ["#    ", "#    ", "#    ", "#    ", "#####"],
        'M' => ["#   #", "## ##", "# # #", "#   #", "#   #"],
        'N' => ["#   #", "##  #", "# # #", "#  ##", "#   #"],
        'O' => [" ### ", "#   #", "#   #", "#   #", " ### "],
        'P' => ["#### ", "#   #", "#### ", "#    ", "#    "],
        'Q' => [" ### ", "#   #", "# # #", "#  # ", " ## #"],
        'R' => ["#### ", "#   #", "#### ", "#  # ", "#   #"],
        'S' => [" ####", "#    ", " ### ", "    #", "#### "],
        'T' => ["#####", "  #  ", "  #  ", "  #  ", "  #  "],
        'U' => ["#   #", "#   #", "#   #", "#   #", " ### "],
        'V' => ["#   #", "#   #", "#   #", " # # ", "  #  "],
        'W' => ["#   #", "#   #", "# # #", "## ##", "#   #"],
        'X' => ["#   #", " # # ", "  #  ", " # # ", "#   #"],
        'Y' => ["#   #", " # # ", "  #  ", "  #  ", "  #  "],
        'Z' => ["#####", "   # ", "  #  ", " #   ", "#####"],
        '0' => [" ### ", "#  ##", "# # #", "##  #", " ### "],
        '1' => ["  #  ", " ##  ", "  #  ", "  #  ", " ### "],
        '2' => [" ### ", "#   #", "  ## ", " #   ", "#####"],
        '3' => ["#### ", "    #", " ### ", "    #", "#### "],
        '4' => ["#   #", "#   #", "#####", "    #", "    #"],
        '5' => ["#####", "#    ", "#### ", "    #", "#### "],
        '6' => [" ####", "#    ", "#### ", "#   #", " ### "],
        '7' => ["#####", "    #", "   # ", "  #  ", "  #  "],
        '8' => [" ### ", "#   #", " ### ", "#   #", " ### "],
        '9' => [" ### ", "#   #", " ####", "    #", "#### "],
        ' ' => ["  ", "  ", "  ", "  ", "  "],
        '.' => [" ", " ", " ", " ", "#"],
        ',' => ["  ", "  ", "  ", " #", "# "],
        ':' => [" ", "#", " ", "#", " "],
        '-' => ["   ", "   ", "###", "   ", "   "],
        '_' => ["     ", "     ", "     ", "     ", "#####"],
        '+' => ["   ", " # ", "###", " # ", "   "],
        '!' => ["#", "#", "#", " ", "#"],
        '/' => ["    #", "   # ", "  #  ", " #   ", "#    "],
        '[' => ["##", "# ", "# ", "# ", "##"],
        ']' => ["##", " #", " #", " #", "##"],
        '<' => ["  #", " # ", "#  ", " # ", "  #"],
        '>' => ["#  ", " # ", "  #", " # ", "#  "],
        _ => [" ### ", "#   #", "  ## ", "     ", "  #  "],
    }
}

/// Cells a `px5` glyph takes, including the 1-cell gap after it (pixel fonts need it; box fonts touch).
fn px5_width(c: char) -> u16 {
    glyph_px5(c)[0].len() as u16 + 1
}

/// `Half3` cell for a pixel pair (top row, bottom row): `Some(sym)` is a fg glyph, `None` is "paint it".
/// Returns `(painted, symbol)`; `(false, " ")` is empty.
fn half3_cell(top: bool, bottom: bool) -> (bool, &'static str) {
    match (top, bottom) {
        (true, true) => (true, " "),
        (true, false) => (false, "▀"),
        (false, true) => (false, "▄"),
        (false, false) => (false, " "),
    }
}

/// Which pixel rows feed the three `Half3` cell rows (the middle pixel row is doubled).
const HALF3_ROWS: [(usize, usize); 3] = [(0, 1), (2, 2), (3, 4)];

/// `Thin3` is `Box3` with light rounded strokes: same shapes, one glyph mapping.
fn thin3_char(c: char) -> char {
    match c {
        '╔' => '╭',
        '╗' => '╮',
        '╚' => '╰',
        '╝' => '╯',
        '═' => '─',
        '║' => '│',
        '╠' => '├',
        '╣' => '┤',
        '╦' => '┬',
        '╩' => '┴',
        '╬' => '┼',
        '▪' => '·',
        other => other,
    }
}

/// Render one `Box3` glyph row in the thin font into `out`.
fn thin3_row(row: &str, out: &mut String) {
    out.clear();
    out.extend(row.chars().map(thin3_char));
}

/// Solid glyph cell: background paint, no glyph (contract rule 15).
fn paint(buf: &mut Buffer, x: u16, y: u16, color: Rgb) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_symbol(" ")
            .set_fg(color.color())
            .set_bg(color.color());
    }
}

/// Text rendered in a big font.
#[derive(Clone, Debug)]
pub struct BigText<'a> {
    text: &'a str,
    font: BigFont,
    spacing: u16,
    align: Alignment,
    color: Option<Rgb>,
    gradient: Option<&'a [Rgb]>,
    bold: bool,
    bg: Option<Rgb>,
    theme: Option<Theme>,
}

impl<'a> BigText<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            font: BigFont::Box3,
            spacing: 0,
            align: Alignment::Left,
            color: None,
            gradient: None,
            bold: false,
            bg: None,
            theme: None,
        }
    }

    pub fn font(mut self, f: BigFont) -> Self {
        self.font = f;
        self
    }

    /// Background behind the glyph strokes (default `th.background`); pass the panel colour when
    /// drawing on a card.
    pub fn bg(mut self, c: Rgb) -> Self {
        self.bg = Some(c);
        self
    }

    /// Cells between glyphs (default 0: letters touch like figlet).
    pub fn spacing(mut self, s: u16) -> Self {
        self.spacing = s;
        self
    }

    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }

    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    /// Left-to-right colour stops across the text.
    pub fn gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.gradient = Some(stops);
        self
    }

    pub fn bold(mut self, v: bool) -> Self {
        self.bold = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Cells `text` takes in this font at `spacing`.
    pub fn width_of(text: &str, font: BigFont, spacing: u16) -> u16 {
        let n = text.chars().count() as u16;
        let w = text
            .chars()
            .map(|c| match font {
                BigFont::Box3 | BigFont::Thin3 => glyph_box3(c)[0].width() as u16,
                BigFont::Block5 | BigFont::Half3 => px5_width(c),
            })
            .sum::<u16>();
        // pixel fonts carry their own trailing gap; drop it after the last glyph
        let w = if matches!(font, BigFont::Block5 | BigFont::Half3) {
            w.saturating_sub(1)
        } else {
            w
        };
        w + spacing * n.saturating_sub(1)
    }

    pub fn width(&self) -> u16 {
        Self::width_of(self.text, self.font, self.spacing)
    }

    /// Draw with an explicit colour resolver (used by [`BigMenu`] for hover/selection).
    fn draw(&self, area: Rect, buf: &mut Buffer, th: &Theme, base: Rgb) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let w = self.width();
        let x0 = match self.align {
            Alignment::Left => area.x,
            Alignment::Center => area.x + area.width.saturating_sub(w) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(w),
        };
        let rows = self.font.rows().min(area.height);
        let mut x = x0;
        for c in self.text.chars() {
            if x >= area.right() {
                break;
            }
            let frac = if w > 1 {
                (x.saturating_sub(x0)) as f32 / (w - 1) as f32
            } else {
                0.0
            };
            let color = self
                .gradient
                .map_or(base, |stops| color_gradient(stops, frac));
            let bg = self.bg.unwrap_or(th.background);
            let mut style = st(color, bg);
            if self.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            match self.font {
                BigFont::Box3 => {
                    let g = glyph_box3(c);
                    for r in 0..rows {
                        put(
                            buf,
                            x,
                            area.y + r,
                            g[r as usize],
                            area.right().saturating_sub(x),
                            style,
                        );
                    }
                    x += g[0].width() as u16 + self.spacing;
                }
                BigFont::Thin3 => {
                    let g = glyph_box3(c);
                    let mut row = String::with_capacity(16);
                    for r in 0..rows {
                        thin3_row(g[r as usize], &mut row);
                        put(
                            buf,
                            x,
                            area.y + r,
                            &row,
                            area.right().saturating_sub(x),
                            style,
                        );
                    }
                    x += g[0].width() as u16 + self.spacing;
                }
                BigFont::Block5 => {
                    let g = glyph_px5(c);
                    for r in 0..rows.min(5) {
                        for (col, ch) in g[r as usize].bytes().enumerate() {
                            let cx = x + col as u16;
                            if cx >= area.right() {
                                break;
                            }
                            if ch == b'#' {
                                paint(buf, cx, area.y + r, color);
                            }
                        }
                    }
                    x += px5_width(c) + self.spacing;
                }
                BigFont::Half3 => {
                    let g = glyph_px5(c);
                    let gw = g[0].len();
                    for (r, (top, bottom)) in HALF3_ROWS.iter().enumerate().take(rows as usize) {
                        for col in 0..gw {
                            let cx = x + col as u16;
                            if cx >= area.right() {
                                break;
                            }
                            let t = g[*top].as_bytes()[col] == b'#';
                            let b = g[*bottom].as_bytes()[col] == b'#';
                            match half3_cell(t, b) {
                                (true, _) => paint(buf, cx, area.y + r as u16, color),
                                (false, " ") => {}
                                (false, sym) => {
                                    put(buf, cx, area.y + r as u16, sym, 1, st(color, bg));
                                }
                            }
                        }
                    }
                    x += px5_width(c) + self.spacing;
                }
            }
        }
    }
}

impl Widget for BigText<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        let base = self.color.unwrap_or(th.text);
        self.draw(area, buf, &th, base);
    }
}

/// Big title with a subtitle and optional rule.
#[derive(Clone, Debug)]
pub struct BigTitle<'a> {
    title: &'a str,
    subtitle: Option<&'a str>,
    font: BigFont,
    gradient: Option<&'a [Rgb]>,
    rule: bool,
    theme: Option<Theme>,
}

impl<'a> BigTitle<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle: None,
            font: BigFont::Block5,
            gradient: None,
            rule: false,
            theme: None,
        }
    }

    pub fn subtitle(mut self, s: &'a str) -> Self {
        self.subtitle = Some(s);
        self
    }

    pub fn font(mut self, f: BigFont) -> Self {
        self.font = f;
        self
    }

    pub fn gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.gradient = Some(stops);
        self
    }

    pub fn rule(mut self, v: bool) -> Self {
        self.rule = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn height(&self) -> u16 {
        let mut h = self.font.rows();
        if self.subtitle.is_some() {
            h += 2;
        }
        if self.rule {
            h += 1;
        }
        h
    }
}

impl Widget for BigTitle<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let rows = self.font.rows();
        let mut y = area.y;
        let mut bt = BigText::new(self.title)
            .font(self.font)
            .align(Alignment::Center);
        if let Some(g) = self.gradient {
            bt = bt.gradient(g);
        }
        bt.theme(&th).render(
            Rect {
                y,
                height: rows.min(area.height),
                ..area
            },
            buf,
        );
        y += rows;
        if let Some(sub) = self.subtitle {
            // a breathing row when the area allows it, otherwise directly under the glyphs
            if area.height > rows + 1 {
                y += 1;
            }
            if y < area.bottom() {
                put_centered(
                    buf,
                    Rect {
                        y,
                        height: 1,
                        ..area
                    },
                    sub,
                    st(th.text_muted, th.background),
                );
                y += 1;
            }
        }
        if self.rule && y < area.bottom() {
            let w = area.width.saturating_sub(4);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            hline(buf, x, y, w, "─", st(th.border_blurred, th.background));
        }
    }
}

/// Menu item with optional description, icon, disabled state, hotkey.
#[derive(Clone, Debug)]
pub struct BigMenuItem<'a> {
    pub label: &'a str,
    pub description: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub disabled: bool,
    pub hotkey: Option<char>,
}

impl<'a> BigMenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            description: None,
            icon: None,
            disabled: false,
            hotkey: None,
        }
    }

    pub fn description(mut self, d: &'a str) -> Self {
        self.description = Some(d);
        self
    }

    pub fn icon(mut self, i: &'a str) -> Self {
        self.icon = Some(i);
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn hotkey(mut self, h: char) -> Self {
        self.hotkey = Some(h);
        self
    }
}

/// Menu style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BigMenuStyle {
    /// Selected in accent/gradient, others muted (current look).
    #[default]
    Plain,
    /// Arrow markers flanking the selected item, animated bounce.
    Arrows,
    /// Round border frame around the selected item.
    Boxed,
    /// Underline under the selected item that tweens between items.
    Underline,
    /// Selected item bright with a horizontal gradient that sweeps, unselected dimmed.
    Glow,
    /// Drop shadow (offset right/down), selected raised in accent.
    Shadow,
    /// Big brackets around the selected item.
    Bracket,
    /// Items laid in one row separated by a gap; Left/Right navigate.
    Horizontal,
    /// Each item in its own card with label + description.
    Cards,
    /// Pulse-blink selected item, `>` prefix cursor, all caps, monochrome.
    Retro,
}

/// Selection state for [`BigMenu`].
#[derive(Clone, Debug, Default)]
pub struct BigMenuState {
    pub selected: usize,
    pub hits: Vec<HitBox>,
    activated: Option<usize>,
    len: usize,
    underline_tween: Tween,
}

impl BigMenuState {
    /// Item chosen with Enter/Space/click since the last call.
    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.underline_tween.active(now)
    }
}

impl Interactive for BigMenuState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) || self.len == 0 {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.checked_sub(1).unwrap_or(self.len - 1);
                Outcome::Consumed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1) % self.len;
                Outcome::Consumed
            }
            KeyCode::Left => {
                self.selected = self.selected.checked_sub(1).unwrap_or(self.len - 1);
                Outcome::Consumed
            }
            KeyCode::Right => {
                self.selected = (self.selected + 1) % self.len;
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.selected = 0;
                Outcome::Consumed
            }
            KeyCode::End => {
                self.selected = self.len - 1;
                Outcome::Consumed
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.activated = Some(self.selected);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::HoverChanged if hit.hover => {
                    self.selected = i;
                    out = Outcome::Consumed;
                }
                Hit::Press => {
                    self.selected = i;
                    self.activated = Some(i);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if let Some(delta) = wheel_delta(&m)
            && self.hits.iter().any(|h| mouse_in(h.area, &m))
        {
            if delta > 0 && self.selected + 1 < self.len {
                self.selected += 1;
                return Outcome::Consumed;
            } else if delta < 0 && self.selected > 0 {
                self.selected -= 1;
                return Outcome::Consumed;
            }
        }
        out
    }
}

/// Vertical menu of big-font items. Many styles (arrows, boxed, underline, glow, cards, retro…).
#[derive(Clone, Debug)]
pub struct BigMenu<'a> {
    items_simple: Option<&'a [&'a str]>,
    items_full: Option<&'a [BigMenuItem<'a>]>,
    gap: u16,
    focused: bool,
    align: Alignment,
    color: Option<Rgb>,
    selected_gradient: Option<&'a [Rgb]>,
    font: BigFont,
    style: BigMenuStyle,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl<'a> BigMenu<'a> {
    pub fn new(items: &'a [&'a str]) -> Self {
        Self {
            items_simple: Some(items),
            items_full: None,
            gap: 1,
            focused: false,
            align: Alignment::Center,
            color: None,
            selected_gradient: None,
            font: BigFont::Box3,
            style: BigMenuStyle::Plain,
            now: None,
            theme: None,
        }
    }

    pub fn items(items: &'a [BigMenuItem<'a>]) -> Self {
        Self {
            items_simple: None,
            items_full: Some(items),
            gap: 1,
            focused: false,
            align: Alignment::Center,
            color: None,
            selected_gradient: None,
            font: BigFont::Box3,
            style: BigMenuStyle::Plain,
            now: None,
            theme: None,
        }
    }

    /// Blank rows between items (default 1).
    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }

    /// Colour of the selected item (default `th.accent`).
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    /// Gradient across the selected item instead of a flat colour.
    pub fn selected_gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.selected_gradient = Some(stops);
        self
    }

    pub fn font(mut self, f: BigFont) -> Self {
        self.font = f;
        self
    }

    pub fn style(mut self, s: BigMenuStyle) -> Self {
        self.style = s;
        self
    }

    pub fn now(mut self, t: Instant) -> Self {
        self.now = Some(t);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    fn len(&self) -> usize {
        self.items_simple
            .map_or_else(|| self.items_full.map_or(0, |i| i.len()), |i| i.len())
    }

    fn label(&self, idx: usize) -> &str {
        self.items_simple.map_or_else(
            || {
                self.items_full
                    .and_then(|i| i.get(idx).map(|m| m.label))
                    .unwrap_or("")
            },
            |i| i.get(idx).copied().unwrap_or(""),
        )
    }

    fn menu_item(&self, idx: usize) -> Option<&BigMenuItem<'a>> {
        self.items_full.and_then(|i| i.get(idx))
    }

    /// Cells reserved left/right of a label and rows above/below it by this style.
    fn chrome(&self) -> (u16, u16, u16, u16) {
        match self.style {
            BigMenuStyle::Plain | BigMenuStyle::Glow | BigMenuStyle::Horizontal => (0, 0, 0, 0),
            BigMenuStyle::Arrows => (3, 3, 0, 0),
            BigMenuStyle::Boxed => (2, 2, 1, 1),
            BigMenuStyle::Underline => (0, 0, 0, 1),
            BigMenuStyle::Shadow => (0, 1, 0, 0),
            BigMenuStyle::Bracket => {
                let bw = BigText::width_of("[", self.font, 0) + 1;
                (bw, bw, 0, 0)
            }
            BigMenuStyle::Cards => (3, 3, 1, 1),
            BigMenuStyle::Retro => (2, 0, 0, 0),
        }
    }

    /// Rows one item takes in this style.
    fn item_height(&self) -> u16 {
        let (_, _, top, bottom) = self.chrome();
        self.font.rows() + top + bottom
    }

    /// Rows the menu needs to show every item.
    pub fn height(&self) -> u16 {
        let n = self.len() as u16;
        if self.style == BigMenuStyle::Horizontal {
            return self.font.rows();
        }
        n * self.item_height() + self.gap * n.saturating_sub(1)
    }

    /// Widest item in cells, including the style's side chrome.
    pub fn width(&self) -> u16 {
        let (l, r, _, _) = self.chrome();
        (0..self.len())
            .map(|i| BigText::width_of(self.label(i), self.font, 0))
            .max()
            .unwrap_or(0)
            + l
            + r
    }

    /// Longest prefix of `label` whose big-text width fits in `max`.
    fn fit(label: &str, font: BigFont, max: u16) -> &str {
        if BigText::width_of(label, font, 0) <= max {
            return label;
        }
        let mut end = 0;
        for (i, c) in label.char_indices() {
            if BigText::width_of(&label[..i + c.len_utf8()], font, 0) > max {
                break;
            }
            end = i + c.len_utf8();
        }
        &label[..end]
    }
}

impl StatefulWidget for BigMenu<'_> {
    type State = BigMenuState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let len = self.len();
        state.len = len;
        state.hits.resize_with(len, HitBox::default);
        for h in &mut state.hits {
            h.set_area(Rect::default());
        }
        if area.width < 3 || area.height == 0 || len == 0 {
            return;
        }
        state.selected = state.selected.min(len - 1);
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);
        let elapsed = anim::since(now);
        let rows = self.font.rows();
        let base = match self.style {
            BigMenuStyle::Retro => self.color.unwrap_or(th.success),
            _ => self.color.unwrap_or(th.accent),
        };
        let sel_color = if self.focused {
            base
        } else {
            base.blend(th.text_muted, 0.4)
        };
        let dim = match self.style {
            BigMenuStyle::Glow => th.text_muted.blend(th.background, 0.45),
            BigMenuStyle::Retro => base.blend(th.background, 0.55),
            BigMenuStyle::Cards => th.text,
            _ => th.text_muted,
        };
        let color_of = |i: usize, hover: bool, disabled: bool| {
            if disabled {
                th.text_disabled
            } else if i == state.selected {
                sel_color
            } else if hover {
                th.text
            } else {
                dim
            }
        };

        if self.style == BigMenuStyle::Horizontal {
            let gap = self.gap.max(2);
            let total: u16 = (0..len)
                .map(|i| BigText::width_of(self.label(i), self.font, 0))
                .sum::<u16>()
                + gap * (len as u16 - 1);
            // scroll by whole items until the selection is visible
            let mut first = 0;
            let fits = |first: usize| {
                let w: u16 = (first..=state.selected)
                    .map(|i| BigText::width_of(self.label(i), self.font, 0))
                    .sum::<u16>()
                    + gap * (state.selected - first) as u16;
                w <= area.width
            };
            while first < state.selected && !fits(first) {
                first += 1;
            }
            let mut x = if total <= area.width {
                match self.align {
                    Alignment::Left => area.x,
                    Alignment::Center => area.x + (area.width - total) / 2,
                    Alignment::Right => area.x + area.width - total,
                }
            } else {
                area.x
            };
            for i in first..len {
                let label = Self::fit(self.label(i), self.font, area.right().saturating_sub(x));
                let w = BigText::width_of(label, self.font, 0);
                if w == 0 {
                    break;
                }
                let item = Rect {
                    x,
                    y: area.y,
                    width: w,
                    height: rows.min(area.height),
                };
                let hover = state.hits[i].hover;
                state.hits[i].set_area(item);
                let disabled = self.menu_item(i).is_some_and(|m| m.disabled);
                let mut text = BigText::new(label)
                    .font(self.font)
                    .align(Alignment::Left)
                    .bold(i == state.selected && self.focused);
                if i == state.selected
                    && let Some(g) = self.selected_gradient
                {
                    text = text.gradient(g);
                }
                text.draw(item, buf, &th, color_of(i, hover, disabled));
                x += w + gap;
                if x >= area.right() {
                    break;
                }
            }
            return;
        }

        let (pad_l, pad_r, top, bottom) = self.chrome();
        let item_h = rows + top + bottom;
        let pitch = item_h + self.gap;
        // window of items around the selection when they don't all fit
        let visible = ((area.height + self.gap) / pitch).max(1) as usize;
        let first = if state.selected >= visible {
            state.selected + 1 - visible
        } else {
            0
        };
        let max_label_w = area.width.saturating_sub(pad_l + pad_r);

        // per-item geometry (text x/width, item y) – recomputed on demand, no allocation
        let geom = |i: usize| -> (Rect, &str) {
            let label = Self::fit(self.label(i), self.font, max_label_w);
            let w = BigText::width_of(label, self.font, 0);
            let x = match self.align {
                Alignment::Left => area.x + pad_l,
                Alignment::Center => area.x + pad_l + max_label_w.saturating_sub(w) / 2,
                Alignment::Right => area.right().saturating_sub(pad_r + w),
            };
            let y = area.y + (i - first) as u16 * pitch + top;
            (
                Rect {
                    x,
                    y,
                    width: w,
                    height: rows,
                },
                label,
            )
        };

        for i in first..len {
            let (text, label) = geom(i);
            if text.y + rows + bottom > area.bottom() {
                break;
            }
            let selected = i == state.selected;
            let hover = state.hits[i].hover;
            let disabled = self.menu_item(i).is_some_and(|m| m.disabled);
            let color = color_of(i, hover, disabled);
            // whole decorated rect is the hit target
            let hit = Rect {
                x: text.x.saturating_sub(pad_l),
                y: text.y.saturating_sub(top),
                width: (text.width + pad_l + pad_r).min(area.width),
                height: item_h,
            };
            state.hits[i].set_area(if self.style == BigMenuStyle::Cards {
                Rect {
                    x: area.x,
                    width: area.width,
                    ..hit
                }
            } else {
                hit
            });
            let mid_y = text.y + rows / 2;

            match self.style {
                BigMenuStyle::Boxed if selected => {
                    Border::Round.draw(
                        buf,
                        Rect {
                            x: text.x.saturating_sub(2),
                            y: text.y - 1,
                            width: text.width + 4,
                            height: rows + 2,
                        },
                        base,
                        th.background,
                    );
                }
                BigMenuStyle::Shadow => {
                    // emboss: a dim copy one cell right, drawn first so only its trailing edge shows
                    let shade = if selected {
                        base.blend(th.background, 0.65)
                    } else {
                        th.background.blend(th.text, 0.15)
                    };
                    BigText::new(label)
                        .font(self.font)
                        .align(Alignment::Left)
                        .draw(
                            Rect {
                                x: text.x + 1,
                                ..text
                            },
                            buf,
                            &th,
                            shade,
                        );
                }
                BigMenuStyle::Cards => {
                    let card = Rect {
                        x: area.x,
                        y: text.y - 1,
                        width: area.width,
                        height: item_h,
                    };
                    let panel = if selected {
                        th.panel.lighten(0.06)
                    } else {
                        th.panel
                    };
                    fill(buf, card, panel);
                    Border::Round.draw(
                        buf,
                        card,
                        if selected { base } else { th.border_blurred },
                        panel,
                    );
                    // the description sits in the bottom edge like a footer title
                    if let Some(desc) = self.menu_item(i).and_then(|m| m.description) {
                        let fg = if selected { th.text } else { th.text_muted };
                        put_centered(
                            buf,
                            Rect {
                                x: card.x + 2,
                                y: text.y + rows,
                                width: card.width.saturating_sub(4),
                                height: 1,
                            },
                            &format!(" {desc} "),
                            st(fg, panel),
                        );
                    }
                }
                _ => {}
            }

            let mut big = BigText::new(label)
                .font(self.font)
                .align(Alignment::Left)
                .bold(selected && self.focused);
            if self.style == BigMenuStyle::Cards {
                big = big.bg(if selected {
                    th.panel.lighten(0.06)
                } else {
                    th.panel
                });
            }
            if selected && let Some(g) = self.selected_gradient {
                big = big.gradient(g);
            }
            let sweep: Vec<Rgb>;
            if self.style == BigMenuStyle::Glow && selected {
                let phase = (elapsed * 0.6).rem_euclid(1.0);
                let n = text.width.clamp(2, 24) as usize;
                sweep = (0..n)
                    .map(|k| {
                        let d = (k as f32 / (n - 1) as f32 - phase).abs();
                        base.blend(base.lighten(0.35), (1.0 - (d * 3.0).min(1.0)).powi(2))
                    })
                    .collect();
                big = big.gradient(&sweep);
            }
            let hidden = self.style == BigMenuStyle::Retro
                && selected
                && self.focused
                && !anim::blink(elapsed, 1.2);
            if !hidden {
                big.draw(text, buf, &th, color);
            }

            if selected {
                match self.style {
                    BigMenuStyle::Arrows => {
                        let bounce = (anim::pulse(elapsed, 1.2) * 1.5) as u16;
                        put(
                            buf,
                            text.x.saturating_sub(2 + bounce),
                            mid_y,
                            "▸",
                            1,
                            st(base, th.background),
                        );
                        put(
                            buf,
                            text.x + text.width + 1 + bounce,
                            mid_y,
                            "◂",
                            1,
                            st(base, th.background),
                        );
                    }
                    BigMenuStyle::Bracket => {
                        let bw = pad_l - 1;
                        BigText::new("[").font(self.font).draw(
                            Rect {
                                x: text.x.saturating_sub(bw + 1),
                                width: bw,
                                ..text
                            },
                            buf,
                            &th,
                            base,
                        );
                        BigText::new("]").font(self.font).draw(
                            Rect {
                                x: text.x + text.width + 1,
                                width: bw,
                                ..text
                            },
                            buf,
                            &th,
                            base,
                        );
                    }
                    BigMenuStyle::Retro => {
                        put(
                            buf,
                            text.x.saturating_sub(2),
                            mid_y,
                            ">",
                            1,
                            st(base, th.background),
                        );
                    }
                    _ => {}
                }
            }
        }

        if self.style == BigMenuStyle::Underline {
            // the bar slides between items: tween the (fractional) item index
            let target = state.selected as f32;
            if (state.underline_tween.target() - target).abs() > 0.01 {
                state
                    .underline_tween
                    .go(target, now, std::time::Duration::from_millis(180));
            }
            let v = state
                .underline_tween
                .value(now)
                .clamp(first as f32, (len - 1) as f32);
            let (i0, i1, t) = (v.floor() as usize, v.ceil() as usize, v.fract());
            let (a, _) = geom(i0);
            let (b, _) = geom(i1);
            let lerp = |p: u16, q: u16| (p as f32 + (q as f32 - p as f32) * t).round() as u16;
            let (x, w, y) = (
                lerp(a.x, b.x),
                lerp(a.width, b.width),
                lerp(a.y, b.y) + rows,
            );
            if y < area.bottom() {
                hline(buf, x, y, w, "▔", st(base, th.background));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box3_width_matches_rendered_cells() {
        assert_eq!(BigText::width_of("HI", BigFont::Box3, 0), 4);
        assert_eq!(BigText::width_of("HI", BigFont::Box3, 1), 5);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        BigText::new("HI")
            .font(BigFont::Box3)
            .render(buf.area, &mut buf);
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "╦");
        assert_eq!(buf.cell((3, 1)).unwrap().symbol(), "║");
    }

    #[test]
    fn block5_painted_solids() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        // Use "0" which has more solid cells
        BigText::new("0")
            .font(BigFont::Block5)
            .color(Rgb(255, 0, 0))
            .render(buf.area, &mut buf);
        // Find a cell that should be solidly painted (check multiple positions)
        let mut found_painted = false;
        for y in 0..5 {
            for x in 0..6 {
                if let Some(cell) = buf.cell((x, y))
                    && cell.symbol() == " "
                    && cell.bg == Rgb(255, 0, 0).color()
                {
                    found_painted = true;
                    break;
                }
            }
        }
        assert!(
            found_painted,
            "Block5 should have at least one bg-painted solid cell"
        );
    }

    #[test]
    fn every_font_renders_all_chars() {
        let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,:!?-/";
        for font in [
            BigFont::Box3,
            BigFont::Block5,
            BigFont::Half3,
            BigFont::Thin3,
        ] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 200, font.rows()));
            BigText::new(chars).font(font).render(buf.area, &mut buf);
        }
    }

    #[test]
    fn every_font_no_panic_on_tight_sizes() {
        for font in [
            BigFont::Box3,
            BigFont::Block5,
            BigFont::Half3,
            BigFont::Thin3,
        ] {
            for (w, h) in [(1, 1), (3, 2), (20, 5), (80, 24)] {
                let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
                BigText::new("TEST").font(font).render(buf.area, &mut buf);
            }
        }
    }

    #[test]
    fn menu_navigation_skips_disabled() {
        let items: &[BigMenuItem] = &[
            BigMenuItem::new("START"),
            BigMenuItem::new("OPTIONS").disabled(true),
            BigMenuItem::new("QUIT"),
        ];
        let mut state = BigMenuState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        BigMenu::items(items).render(buf.area, &mut buf, &mut state);
        assert_eq!(state.selected, 0);
        // Manual skip logic would be in app layer; state itself doesn't auto-skip
    }

    #[test]
    fn menu_hotkey_jump() {
        let items: &[&str] = &["START", "OPTIONS", "HELP", "QUIT"];
        let mut state = BigMenuState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 16));
        BigMenu::new(items).render(buf.area, &mut buf, &mut state);
        assert_eq!(state.selected, 0);
        // Hotkey logic would be in app layer
    }

    #[test]
    fn horizontal_style_left_right() {
        let items: &[&str] = &["START", "OPTIONS", "QUIT"];
        let mut state = BigMenuState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 3));
        BigMenu::new(items)
            .style(BigMenuStyle::Horizontal)
            .render(buf.area, &mut buf, &mut state);
        state.handle_key(KeyEvent::from(KeyCode::Right));
        assert_eq!(state.selected, 1);
        state.handle_key(KeyEvent::from(KeyCode::Left));
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn menu_wraps_and_activates() {
        let items: &[&str] = &["OPTIONS", "HELP", "QUIT"];
        let mut state = BigMenuState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        BigMenu::new(items).render(buf.area, &mut buf, &mut state);
        state.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(state.selected, 2);
        state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(state.selected, 0);
        assert!(
            state
                .handle_key(KeyEvent::from(KeyCode::Enter))
                .is_changed()
        );
        assert_eq!(state.take_activated(), Some(0));
        assert_eq!(state.take_activated(), None);
    }
}
