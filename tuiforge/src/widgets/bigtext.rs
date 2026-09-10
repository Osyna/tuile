//! Big text in a 3-row box-drawing font (the look of btop's menu / figlet "Calvin S") and a
//! vertical menu built from it.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 20);
//! # let mut buf = Buffer::empty(area);
//! BigText::new("TUIFORGE").gradient(&[Rgb(230, 60, 60), Rgb(255, 160, 40)]).render(area, &mut buf);
//! let mut menu = BigMenuState::default();
//! BigMenu::new(&["OPTIONS", "HELP", "QUIT"]).focused(true).render(Rect { y: 5, ..area }, &mut buf, &mut menu);
//! if let Some(i) = menu.take_activated() { /* run item i */ }
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::core::{Hit, HitBox, Interactive, Outcome, is_press};
use crate::draw::{put, st};
use crate::theme::{self, Rgb, Theme, gradient as color_gradient};

/// Rows of the font.
pub const BIG_ROWS: u16 = 3;

/// Glyph for one character; unknown characters fall back to `?`, lowercase is uppercased.
fn glyph(c: char) -> [&'static str; 3] {
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
        'Q' => ["╔═╗", "║ ║", "╚═╩"],
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
        '.' => [" ", " ", "▪"],
        ':' => [" ", "▪", "▪"],
        '-' => ["  ", "══", "  "],
        '_' => ["   ", "   ", "═══"],
        '+' => ["   ", " ╬ ", "   "],
        '!' => ["╦", "║", "▪"],
        '/' => ["  ╔", " ╔╝", "╔╝ "],
        _ => ["╔═╗", " ╔╝", " ▪ "],
    }
}

/// Text rendered in the 3-row font.
#[derive(Clone, Debug)]
pub struct BigText<'a> {
    text: &'a str,
    spacing: u16,
    align: Alignment,
    color: Option<Rgb>,
    gradient: Option<&'a [Rgb]>,
    bold: bool,
    theme: Option<Theme>,
}

impl<'a> BigText<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, spacing: 0, align: Alignment::Left, color: None, gradient: None, bold: false, theme: None }
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
    pub fn width_of(text: &str, spacing: u16) -> u16 {
        let n = text.chars().count() as u16;
        text.chars().map(|c| glyph(c)[0].width() as u16).sum::<u16>() + spacing * n.saturating_sub(1)
    }

    pub fn width(&self) -> u16 {
        Self::width_of(self.text, self.spacing)
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
        let rows = BIG_ROWS.min(area.height);
        let mut x = x0;
        for c in self.text.chars() {
            let g = glyph(c);
            let gw = g[0].width() as u16;
            if x >= area.right() {
                break;
            }
            let frac = if w > 1 { (x - x0) as f32 / (w - 1) as f32 } else { 0.0 };
            let color = self.gradient.map_or(base, |stops| color_gradient(stops, frac));
            let mut style = st(color, th.background);
            if self.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            for r in 0..rows {
                put(buf, x, area.y + r, g[r as usize], area.right() - x, style);
            }
            x += gw + self.spacing;
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

/// Selection state for [`BigMenu`].
#[derive(Clone, Debug, Default)]
pub struct BigMenuState {
    pub selected: usize,
    pub hits: Vec<HitBox>,
    activated: Option<usize>,
    len: usize,
}

impl BigMenuState {
    /// Item chosen with Enter/Space/click since the last call.
    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
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
        out
    }
}

/// Vertical menu of big-font items (btop's `OPTIONS / HELP / QUIT`). Selected item in the
/// accent colour, the rest muted; hover selects, click or Enter activates.
#[derive(Clone, Debug)]
pub struct BigMenu<'a> {
    items: &'a [&'a str],
    gap: u16,
    focused: bool,
    align: Alignment,
    color: Option<Rgb>,
    selected_gradient: Option<&'a [Rgb]>,
    theme: Option<Theme>,
}

impl<'a> BigMenu<'a> {
    pub fn new(items: &'a [&'a str]) -> Self {
        Self { items, gap: 1, focused: false, align: Alignment::Center, color: None, selected_gradient: None, theme: None }
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

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Rows the menu needs.
    pub fn height(&self) -> u16 {
        let n = self.items.len() as u16;
        n * BIG_ROWS + self.gap * n.saturating_sub(1)
    }

    /// Widest item in cells.
    pub fn width(&self) -> u16 {
        self.items.iter().map(|s| BigText::width_of(s, 0)).max().unwrap_or(0)
    }
}

impl StatefulWidget for BigMenu<'_> {
    type State = BigMenuState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.len = self.items.len();
        state.hits.resize_with(self.items.len(), HitBox::default);
        if area.width < 3 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let accent = self.color.unwrap_or(th.accent);
        let mut y = area.y;
        for (i, item) in self.items.iter().enumerate() {
            if y + BIG_ROWS > area.bottom() {
                state.hits[i].set_area(Rect::default());
                break;
            }
            let w = BigText::width_of(item, 0);
            let x = match self.align {
                Alignment::Left => area.x,
                Alignment::Center => area.x + area.width.saturating_sub(w) / 2,
                Alignment::Right => area.x + area.width.saturating_sub(w),
            };
            let item_area = Rect { x, y, width: w.min(area.width), height: BIG_ROWS };
            state.hits[i].set_area(item_area);
            let selected = i == state.selected;
            let text = BigText::new(item).align(Alignment::Left).bold(selected && self.focused);
            let text = match self.selected_gradient {
                Some(g) if selected => text.gradient(g),
                _ => text,
            };
            let color = if selected {
                if self.focused { accent } else { accent.blend(th.text_muted, 0.4) }
            } else {
                th.text_muted
            };
            text.draw(item_area, buf, &th, color);
            y += BIG_ROWS + self.gap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_matches_rendered_cells() {
        // "HI" = H(3) + I(1); with spacing 1 → 5
        assert_eq!(BigText::width_of("HI", 0), 4);
        assert_eq!(BigText::width_of("HI", 1), 5);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        BigText::new("HI").render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "╦");
        assert_eq!(buf[(3, 1)].symbol(), "║"); // the I's stem
        assert_eq!(buf[(4, 1)].symbol(), " ");
    }

    #[test]
    fn menu_wraps_and_activates() {
        let items = ["OPTIONS", "HELP", "QUIT"];
        let mut state = BigMenuState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        BigMenu::new(&items).render(buf.area, &mut buf, &mut state);
        state.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(state.selected, 2);
        state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(state.selected, 0);
        assert!(state.handle_key(KeyEvent::from(KeyCode::Enter)).is_changed());
        assert_eq!(state.take_activated(), Some(0));
        assert_eq!(state.take_activated(), None);
        // third item sits at rows 8..11 (3 rows + gap 1, twice)
        assert_eq!(state.hits[2].area.y, 8);
    }
}
