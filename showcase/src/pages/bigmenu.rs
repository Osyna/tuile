//! Big menus: a splash title, the four `BigFont`s side by side, a full-width `Horizontal`
//! menu, and every vertical `BigMenuStyle` in a grid. One menu is focused; `s` moves the
//! focus to the next menu, `f` cycles its font, `g` toggles the selection gradient.

use std::time::Instant;

use tuile::draw::{put, st};
use tuile::prelude::*;

use super::{Ctx, Page, card};

/// Grid cells: (style, caption). The last cell repeats `Plain` in a pixel font.
const GRID: [(BigMenuStyle, &str); 10] = [
    (BigMenuStyle::Plain, "Plain"),
    (BigMenuStyle::Arrows, "Arrows"),
    (BigMenuStyle::Boxed, "Boxed"),
    (BigMenuStyle::Underline, "Underline"),
    (BigMenuStyle::Glow, "Glow"),
    (BigMenuStyle::Shadow, "Shadow"),
    (BigMenuStyle::Bracket, "Bracket"),
    (BigMenuStyle::Cards, "Cards"),
    (BigMenuStyle::Retro, "Retro"),
    (BigMenuStyle::Plain, "Plain · Half3"),
];
const STRIP: usize = GRID.len();
const FONTS: [BigFont; 4] = [
    BigFont::Box3,
    BigFont::Block5,
    BigFont::Half3,
    BigFont::Thin3,
];
const FONT_NAMES: [&str; 4] = ["Box3", "Block5", "Half3", "Thin3"];
const ITEMS: [&str; 2] = ["PLAY", "EXIT"];
/// Pixel fonts are 6 cells per letter: three-letter items keep them inside a grid cell.
const PIXEL_ITEMS: [&str; 2] = ["NEW", "END"];
const STRIP_ITEMS: [&str; 4] = ["START", "OPTIONS", "HELP", "QUIT"];
pub struct BigMenuPage {
    /// One state per grid cell, plus the horizontal strip at `STRIP`.
    states: Vec<BigMenuState>,
    focused: usize,
    /// Font override of the focused cell (index into `FONTS`), `None` = the cell's default.
    font: Option<usize>,
    gradient: bool,
}

impl Default for BigMenuPage {
    fn default() -> Self {
        Self {
            states: (0..=STRIP).map(|_| BigMenuState::default()).collect(),
            focused: 0,
            font: None,
            gradient: true,
        }
    }
}

impl BigMenuPage {
    fn labels(&self, idx: usize) -> &'static [&'static str] {
        if idx == STRIP {
            &STRIP_ITEMS
        } else if GRID[idx].0 != BigMenuStyle::Cards
            && matches!(self.cell_font(idx), BigFont::Block5 | BigFont::Half3)
        {
            &PIXEL_ITEMS
        } else {
            &ITEMS
        }
    }

    fn cell_font(&self, idx: usize) -> BigFont {
        if idx == self.focused
            && let Some(f) = self.font
        {
            return FONTS[f];
        }
        if idx == GRID.len() - 1 {
            BigFont::Half3
        } else {
            BigFont::Box3
        }
    }
}

impl Page for BigMenuPage {
    fn title(&self) -> &'static str {
        "Big menus"
    }
    fn subtitle(&self) -> &'static str {
        "BigText fonts, BigTitle, and every BigMenuStyle - animated, keyboard + mouse"
    }
    fn icon(&self) -> &'static str {
        "▸"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("↑↓/←→", "Select"),
            ("Enter", "Activate"),
            ("s", "Focus next menu"),
            ("f", "Font"),
            ("g", "Gradient"),
        ]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        if area.width < 24 || area.height < 8 {
            return;
        }
        let stops = [th.primary, th.accent, th.secondary];
        let grad: Option<&[Rgb]> = if self.gradient { Some(&stops) } else { None };

        // small terminals: title + the focused menu only
        if area.width < 90 || area.height < 30 {
            let [top, rest] =
                Layout::vertical([Constraint::Length(4), Constraint::Fill(1)]).areas(area);
            let mut title = BigTitle::new("TUILE")
                .font(BigFont::Half3)
                .subtitle("big menus");
            if let Some(g) = grad {
                title = title.gradient(g);
            }
            title.theme(&th).render(top, buf);
            let idx = self.focused.min(GRID.len() - 1);
            let mut menu = BigMenu::new(self.labels(idx))
                .style(GRID[idx].0)
                .font(self.cell_font(idx))
                .gap(0)
                .focused(true)
                .now(ctx.now);
            if let Some(g) = grad {
                menu = menu.selected_gradient(g);
            }
            menu.theme(&th).render(rest, buf, &mut self.states[idx]);
            return;
        }

        let [top, fonts, strip, grid] = Layout::vertical([
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(area);

        // -- title --
        let mut title = BigTitle::new("TUILE")
            .font(BigFont::Block5)
            .subtitle("big text · big menus · four fonts");
        if let Some(g) = grad {
            title = title.gradient(g);
        }
        title.theme(&th).render(top, buf);

        // -- fonts side by side (names in the card title so Block5 keeps its 5 rows) --
        let inner = pad(
            card(buf, fonts, &th, "Fonts  Box3 · Block5 · Half3 · Thin3"),
            1,
            0,
        );
        let cols = Layout::horizontal([Constraint::Fill(1); 4]).split(inner);
        for (font, col) in FONTS.iter().zip(cols.iter()) {
            let rows = font.rows();
            let y = col.y + col.height.saturating_sub(rows) / 2;
            let mut sample = BigText::new("MENU")
                .font(*font)
                .align(Alignment::Center)
                .color(th.accent);
            if let Some(g) = grad {
                sample = sample.gradient(g);
            }
            sample.theme(&th).render(
                Rect {
                    y,
                    height: rows.min(col.height),
                    ..*col
                },
                buf,
            );
        }

        // -- horizontal strip --
        let focused = self.focused == STRIP;
        let cap = if focused {
            st(th.accent, th.background).add_modifier(Modifier::BOLD)
        } else {
            st(th.text_muted, th.background)
        };
        put(buf, strip.x + 1, strip.y + 1, "Horizontal", 12, cap);
        let menu_area = Rect {
            x: strip.x + 13,
            width: strip.width.saturating_sub(14),
            ..strip
        };
        let mut menu = BigMenu::new(&STRIP_ITEMS)
            .style(BigMenuStyle::Horizontal)
            .font(self.cell_font(STRIP))
            .gap(3)
            .focused(focused)
            .now(ctx.now);
        if let Some(g) = grad {
            menu = menu.selected_gradient(g);
        }
        menu.theme(&th)
            .render(menu_area, buf, &mut self.states[STRIP]);

        // -- vertical styles grid --
        let inner = pad(card(buf, grid, &th, "Styles"), 1, 0);
        let rows = Layout::vertical([Constraint::Fill(1); 2]).split(inner);
        let cards = [
            BigMenuItem::new("PLAY").description("new game"),
            BigMenuItem::new("EXIT").description("to desktop"),
        ];
        for (r, row) in rows.iter().enumerate() {
            let cols = Layout::horizontal([Constraint::Fill(1); 5]).split(*row);
            for (c, col) in cols.iter().enumerate() {
                let idx = r * 5 + c;
                let (style, caption) = GRID[idx];
                let focused = idx == self.focused;
                let cap_style = if focused {
                    st(th.accent, th.background).add_modifier(Modifier::BOLD)
                } else {
                    st(th.text_muted, th.background)
                };
                put(
                    buf,
                    col.x,
                    col.y,
                    caption,
                    col.width.saturating_sub(1),
                    cap_style,
                );
                let body = Rect {
                    x: col.x,
                    y: col.y + 1,
                    width: col.width.saturating_sub(1),
                    height: col.height.saturating_sub(1),
                };
                let font = self.cell_font(idx);
                let mut menu = match (style, font) {
                    (BigMenuStyle::Cards, _) => BigMenu::items(&cards),
                    (_, BigFont::Block5 | BigFont::Half3) => BigMenu::new(&PIXEL_ITEMS),
                    _ => BigMenu::new(&ITEMS),
                };
                menu = menu
                    .style(style)
                    .font(font)
                    .gap(0)
                    .focused(focused)
                    .now(ctx.now);
                if let Some(g) = grad {
                    menu = menu.selected_gradient(g);
                }
                menu.theme(&th).render(body, buf, &mut self.states[idx]);
            }
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Mouse(m) => {
                for i in 0..self.states.len() {
                    let out = self.states[i].handle_mouse(*m);
                    if out.is_changed() || out.is_consumed() {
                        if out.is_changed() {
                            self.focused = i;
                            self.font = None;
                        }
                        if let Some(sel) = self.states[i].take_activated() {
                            ctx.notify(format!("Menu: {}", self.labels(i)[sel]), Variant::Primary);
                        }
                        return out;
                    }
                }
                Outcome::Ignored
            }
            Event::Key(k) if is_press(k) => match k.code {
                KeyCode::Char('s') => {
                    self.focused = (self.focused + 1) % self.states.len();
                    self.font = None;
                    Outcome::Consumed
                }
                KeyCode::Char('f') => {
                    self.font = Some(self.font.map_or(1, |f| (f + 1) % FONTS.len()));
                    ctx.notify(
                        format!("Font: {}", FONT_NAMES[self.font.unwrap_or(0)]),
                        Variant::Primary,
                    );
                    Outcome::Consumed
                }
                KeyCode::Char('g') => {
                    self.gradient = !self.gradient;
                    Outcome::Consumed
                }
                _ => {
                    let out = self.states[self.focused].handle_key(*k);
                    if let Some(sel) = self.states[self.focused].take_activated() {
                        ctx.notify(
                            format!("Menu: {}", self.labels(self.focused)[sel]),
                            Variant::Primary,
                        );
                    }
                    out
                }
            },
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, now: Instant) -> bool {
        // arrows / glow / retro / underline all move
        let _ = now;
        true
    }
}
