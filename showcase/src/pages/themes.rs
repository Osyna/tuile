//! Themes page: browse the built-in palettes, see every derived role, and override the
//! primary hue live to watch the whole design system re-derive itself.

use tuiforge::draw::{Border, fill, hbar, put, put_right, st};
use tuiforge::prelude::*;
use tuiforge::theme::BUILTIN;

use super::{Ctx, Page, card};

pub struct ThemesPage {
    cursor: usize,
    hover: Option<usize>,
    list_hits: Vec<Rect>,
    /// Hue shift applied to the primary colour, in degrees.
    hue_shift: f32,
    slider: Rect,
    dragging: bool,
}

impl Default for ThemesPage {
    fn default() -> Self {
        Self {
            cursor: 0,
            hover: None,
            list_hits: Vec::new(),
            hue_shift: 0.0,
            slider: Rect::default(),
            dragging: false,
        }
    }
}

impl ThemesPage {
    fn apply(&self, ctx: &mut Ctx) {
        let spec = &BUILTIN[self.cursor % BUILTIN.len()];
        let primary = if self.hue_shift.abs() < 0.5 {
            None
        } else {
            let (h, s, l) = spec.primary.to_hsl();
            Some(Rgb::from_hsl(h + self.hue_shift, s, l))
        };
        let th = Theme::resolve(spec, primary);
        theme::set(th);
        ctx.theme = th;
    }

    fn roles(th: &Theme) -> Vec<(&'static str, Rgb)> {
        vec![
            ("primary", th.primary),
            ("secondary", th.secondary),
            ("accent", th.accent),
            ("success", th.success),
            ("warning", th.warning),
            ("error", th.error),
            ("background", th.background),
            ("surface", th.surface),
            ("panel", th.panel),
            ("boost", th.boost),
            ("foreground", th.foreground),
            ("text", th.text),
            ("text-muted", th.text_muted),
            ("text-disabled", th.text_disabled),
            ("text-primary", th.text_primary),
            ("text-accent", th.text_accent),
            ("text-success", th.text_success),
            ("text-warning", th.text_warning),
            ("text-error", th.text_error),
            ("border", th.border),
            ("border-blurred", th.border_blurred),
            ("cursor", th.cursor_bg),
            ("cursor-blurred", th.cursor_blurred_bg),
            ("hover", th.hover_bg),
            ("selection", th.selection_bg),
            ("scrollbar", th.scrollbar),
            ("footer-key", th.footer_key),
            ("link", th.link),
        ]
    }
}

impl Page for ThemesPage {
    fn title(&self) -> &'static str {
        "Themes"
    }
    fn subtitle(&self) -> &'static str {
        "12 palettes, 30 derived roles, live primary-hue override"
    }
    fn icon(&self) -> &'static str {
        "●"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("↑↓", "Theme"), ("←→", "Primary hue"), ("r", "Reset hue")]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        if let Some(i) = BUILTIN.iter().position(|t| t.name == th.name) {
            self.cursor = i;
        }
        let inner = pad(area, 1, 1);
        let [list_a, right] =
            Layout::horizontal([Constraint::Length(26), Constraint::Fill(1)]).areas(inner);

        // theme list
        let list_in = card(buf, list_a, &th, "Themes");
        self.list_hits.clear();
        for (i, spec) in BUILTIN.iter().enumerate() {
            let y = list_in.y + i as u16;
            if y >= list_in.bottom() {
                break;
            }
            let row = Rect {
                x: list_in.x,
                y,
                width: list_in.width,
                height: 1,
            };
            self.list_hits.push(row);
            let active = i == self.cursor;
            let (fg, bg) = if active {
                (th.cursor_fg, th.cursor_bg)
            } else if self.hover == Some(i) {
                (th.text, th.hover_bg)
            } else {
                (th.text, th.background)
            };
            fill(buf, row, bg);
            let preview = Theme::resolve(spec, None);
            put(
                buf,
                row.x + 1,
                y,
                "  ",
                2,
                st(preview.primary, preview.primary),
            );
            put(
                buf,
                row.x + 3,
                y,
                " ",
                1,
                st(preview.accent, preview.accent),
            );
            put(
                buf,
                row.x + 5,
                y,
                spec.name,
                row.width.saturating_sub(6),
                st(fg, bg).add_modifier(if active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            );
            if !spec.dark {
                put_right(
                    buf,
                    Rect {
                        width: row.width - 1,
                        ..row
                    },
                    "☼",
                    st(if active { fg } else { th.text_muted }, bg),
                );
            }
        }

        // right side: hue slider + swatches + preview strip
        let [slider_a, swatch_a, preview_a] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(9),
        ])
        .areas(pad(right, 1, 0));

        let s_in = card(buf, slider_a, &th, "Primary hue override");
        if s_in.width > 12 {
            let track = Rect {
                x: s_in.x + 1,
                y: s_in.y,
                width: s_in.width.saturating_sub(12),
                height: 1,
            };
            self.slider = track;
            for i in 0..track.width {
                let hue = i as f32 / track.width.max(1) as f32 * 360.0;
                put(
                    buf,
                    track.x + i,
                    track.y,
                    "━",
                    1,
                    st(Rgb::from_hsl(hue, 0.7, 0.5), th.background),
                );
            }
            let (h0, _, _) = BUILTIN[self.cursor].primary.to_hsl();
            let hue = (h0 + self.hue_shift).rem_euclid(360.0);
            let tx = track.x + ((hue / 360.0) * (track.width - 1) as f32).round() as u16;
            put(
                buf,
                tx,
                track.y,
                "●",
                1,
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            let label = format!("{:+.0}°", self.hue_shift);
            put(
                buf,
                track.right() + 2,
                track.y,
                &label,
                9,
                st(th.text_muted, th.background),
            );
            put(
                buf,
                s_in.x + 1,
                track.y + 1,
                "every derived role below follows the primary",
                s_in.width - 2,
                st(th.text_disabled, th.background),
            );
        }

        let sw_in = card(buf, swatch_a, &th, "Roles");
        let roles = Self::roles(&th);
        let cell_w: u16 = 18;
        let cols_n = (sw_in.width / cell_w).max(1) as usize;
        for (i, (name, color)) in roles.iter().enumerate() {
            let (r, c) = (i / cols_n, i % cols_n);
            let x = sw_in.x + c as u16 * cell_w;
            let y = sw_in.y + r as u16 * 3;
            if y + 2 > sw_in.bottom() || x + cell_w > sw_in.right() + 1 {
                continue;
            }
            let block = Rect {
                x,
                y,
                width: cell_w - 1,
                height: 2,
            };
            fill(buf, block, *color);
            put(
                buf,
                x + 1,
                y,
                &color.to_hex_string(),
                cell_w - 2,
                st(color.text_on(0.8), *color),
            );
            put(
                buf,
                x,
                y + 2,
                name,
                cell_w - 1,
                st(th.text_muted, th.background),
            );
        }

        // preview strip: what real widgets look like with these roles
        let p_in = card(buf, preview_a, &th, "Preview");
        fill(buf, p_in, th.surface);
        let y = p_in.y + 1;
        let x = p_in.x + 2;
        // 3D button
        let btn = Rect {
            x,
            y,
            width: 16,
            height: 3,
        };
        fill(buf, btn, th.primary);
        for bx in btn.left()..btn.right() {
            put(
                buf,
                bx,
                btn.y,
                "▔",
                1,
                st(Rgb::shade(th.primary, 3), th.primary),
            );
            put(
                buf,
                bx,
                btn.bottom() - 1,
                "▁",
                1,
                st(Rgb::shade(th.primary, -3), th.primary),
            );
        }
        put(
            buf,
            btn.x + 4,
            btn.y + 1,
            "Primary",
            8,
            st(th.primary.text_on(0.9), th.primary).add_modifier(Modifier::BOLD),
        );
        // input
        let inp = Rect {
            x: x + 18,
            y,
            width: 24,
            height: 3,
        };
        fill(buf, inp, th.focus_bg());
        Border::Tall.draw(buf, inp, th.border, th.focus_bg());
        put(
            buf,
            inp.x + 2,
            inp.y + 1,
            "typed text",
            10,
            st(th.text, th.focus_bg()),
        );
        put(
            buf,
            inp.x + 12,
            inp.y + 1,
            " ",
            1,
            st(th.cursor_fg, th.cursor_bg),
        );
        // text roles + gauge
        let tx = x + 44;
        if tx + 24 < p_in.right() {
            put(buf, tx, y, "text  ", 6, st(th.text, th.surface));
            put(buf, tx + 6, y, "muted  ", 7, st(th.text_muted, th.surface));
            put(
                buf,
                tx + 13,
                y,
                "disabled  ",
                10,
                st(th.text_disabled, th.surface),
            );
            put(
                buf,
                tx + 23,
                y,
                "link",
                4,
                st(th.link, th.surface).add_modifier(Modifier::UNDERLINED),
            );
            put(
                buf,
                tx,
                y + 1,
                "primary ",
                8,
                st(th.text_primary, th.surface),
            );
            put(
                buf,
                tx + 8,
                y + 1,
                "success ",
                8,
                st(th.text_success, th.surface),
            );
            put(
                buf,
                tx + 16,
                y + 1,
                "warning ",
                8,
                st(th.text_warning, th.surface),
            );
            put(
                buf,
                tx + 24,
                y + 1,
                "error",
                5,
                st(th.text_error, th.surface),
            );
            hbar(
                buf,
                tx,
                y + 2,
                30.min(p_in.right().saturating_sub(tx)),
                0.62,
                th.primary,
                th.panel,
            );
        }
        // toast
        let ty = y + 4;
        if ty < p_in.bottom() {
            let toast = Rect {
                x,
                y: ty,
                width: 40.min(p_in.width.saturating_sub(4)),
                height: 2,
            };
            fill(buf, toast, th.toast_bg);
            put(buf, toast.x, ty, "┃", 1, st(th.success, th.toast_bg));
            put(buf, toast.x, ty + 1, "┃", 1, st(th.success, th.toast_bg));
            put(
                buf,
                toast.x + 2,
                ty,
                "Saved",
                20,
                st(th.text_success, th.toast_bg).add_modifier(Modifier::BOLD),
            );
            put(
                buf,
                toast.x + 2,
                ty + 1,
                "Settings written to disk.",
                toast.width - 3,
                st(th.text, th.toast_bg),
            );
            let sel = Rect {
                x: toast.right() + 2,
                y: ty,
                width: 22.min(p_in.right().saturating_sub(toast.right() + 2)),
                height: 1,
            };
            fill(buf, sel, th.selection_bg);
            put(
                buf,
                sel.x + 1,
                ty,
                "selected text",
                sel.width,
                st(th.text, th.selection_bg),
            );
            let hov = Rect { y: ty + 1, ..sel };
            fill(buf, hov, th.hover_bg);
            put(
                buf,
                hov.x + 1,
                ty + 1,
                "hovered row",
                hov.width,
                st(th.text, th.hover_bg),
            );
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) => {
                let n = BUILTIN.len();
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => self.cursor = (self.cursor + n - 1) % n,
                    KeyCode::Down | KeyCode::Char('j') => self.cursor = (self.cursor + 1) % n,
                    KeyCode::Left => {
                        self.hue_shift -= if k.modifiers.contains(KeyModifiers::SHIFT) {
                            1.0
                        } else {
                            10.0
                        }
                    }
                    KeyCode::Right => {
                        self.hue_shift += if k.modifiers.contains(KeyModifiers::SHIFT) {
                            1.0
                        } else {
                            10.0
                        }
                    }
                    KeyCode::Char('r') => self.hue_shift = 0.0,
                    KeyCode::Home => self.cursor = 0,
                    KeyCode::End => self.cursor = n - 1,
                    _ => return Outcome::Ignored,
                }
                self.apply(ctx);
                Outcome::Changed
            }
            Event::Mouse(m) => {
                let pos = mouse_pos(m);
                let over = self.list_hits.iter().position(|r| r.contains(pos));
                if is_left_down(m) {
                    if let Some(i) = over {
                        self.cursor = i;
                        self.apply(ctx);
                        return Outcome::Changed;
                    }
                    if self.slider.contains(pos) {
                        self.dragging = true;
                        return self.slide_to(pos.x, ctx);
                    }
                } else if is_left_drag(m) && self.dragging {
                    return self.slide_to(pos.x, ctx);
                } else if is_left_up(m) {
                    self.dragging = false;
                } else if let Some(d) = wheel_delta(m)
                    && over.is_some()
                {
                    let n = BUILTIN.len() as i32;
                    self.cursor = ((self.cursor as i32 + d).rem_euclid(n)) as usize;
                    self.apply(ctx);
                    return Outcome::Changed;
                }
                if over != self.hover {
                    self.hover = over;
                    return Outcome::Consumed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }
}

impl ThemesPage {
    fn slide_to(&mut self, x: u16, ctx: &mut Ctx) -> Outcome {
        let t = self.slider;
        if t.width < 2 {
            return Outcome::Ignored;
        }
        let f = (x.saturating_sub(t.x) as f32 / (t.width - 1) as f32).clamp(0.0, 1.0);
        let (h0, _, _) = BUILTIN[self.cursor].primary.to_hsl();
        let mut shift = f * 360.0 - h0;
        // keep the shift in -180..180 so the label stays readable
        while shift > 180.0 {
            shift -= 360.0;
        }
        while shift < -180.0 {
            shift += 360.0;
        }
        self.hue_shift = shift;
        self.apply(ctx);
        Outcome::Changed
    }
}
