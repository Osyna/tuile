//! Welcome page: hero banner, what the library is, how to navigate the gallery.

use tuiforge::draw::{Border, fill, put, put_centered, st};
use tuiforge::prelude::*;
use tuiforge::theme::gradient;

use super::{Ctx, Page};

const TITLE: [&str; 5] = [
    "▄▄▄▄▄ ▄   ▄ ▄▄▄ ▄▄▄▄▄ ▄▄▄▄  ▄▄▄▄   ▄▄▄▄ ▄▄▄▄▄",
    "  █   █   █  █  █     █   █ █   █ █     █    ",
    "  █   █   █  █  █▄▄▄  █   █ █▄▄▄▀ █  ▄▄ █▄▄▄ ",
    "  █   █   █  █  █     █   █ █  █  █   █ █    ",
    "  █   ▀▄▄▄▀ ▄█▄ █     ▀▄▄▄▀ █   █ ▀▄▄▄▀ █▄▄▄▄",
];

const FEATURES: [(&str, &str); 8] = [
    (
        "Design system",
        "Textual's colour math: one palette → 30 derived roles, 12 built-in themes",
    ),
    (
        "40+ widgets",
        "buttons, switches, inputs, selects, tabs, trees, tables, charts, modals, toasts…",
    ),
    (
        "Mouse & keyboard",
        "every widget handles clicks, hover, drag and wheel plus Textual key bindings",
    ),
    (
        "Animation",
        "tweened switches, sliders, tabs, progress and toasts at 60 fps only while moving",
    ),
    (
        "Overlays",
        "dropdowns, menus, tooltips, command palette, dialogs with dimmed backdrops",
    ),
    (
        "Layout",
        "split panes, scroll views, panels, collapsibles, headers and footers",
    ),
    (
        "Tiny API",
        "builder + State + Outcome: `Switch::new().render(area, buf, &mut state)`",
    ),
    (
        "No bloat",
        "ratatui + unicode-width only; the runtime restores your terminal on panic",
    ),
];

#[derive(Default)]
pub struct WelcomePage {
    hover: Option<usize>,
    hits: Vec<Rect>,
}

impl Page for WelcomePage {
    fn title(&self) -> &'static str {
        "Welcome"
    }
    fn subtitle(&self) -> &'static str {
        "Textual-grade components for ratatui"
    }
    fn icon(&self) -> &'static str {
        "◈"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("alt+1-9", "Jump to page"), ("F3", "Reduce motion")]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let t = ctx.elapsed();
        let inner = pad(area, 2, 1);
        if inner.width < 20 || inner.height < 8 {
            return;
        }
        // banner with a slowly moving gradient
        let banner_w = TITLE[0].chars().count() as u16;
        let show_banner = inner.width >= banner_w + 2 && inner.height >= 18;
        let mut y = inner.y;
        if show_banner {
            let x0 = inner.x + (inner.width - banner_w) / 2;
            let stops = [th.primary, th.accent, th.secondary, th.primary];
            for (row, line) in TITLE.iter().enumerate() {
                for (i, ch) in line.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    let phase = (i as f32 / banner_w as f32 + t * 0.08 + row as f32 * 0.02).fract();
                    let c = gradient(&stops, phase);
                    // `█` as background paint: no seams between cells in any font
                    if ch == '█' {
                        put(buf, x0 + i as u16, y + row as u16, " ", 1, st(c, c));
                    } else {
                        put(
                            buf,
                            x0 + i as u16,
                            y + row as u16,
                            &ch.to_string(),
                            1,
                            st(c, th.background),
                        );
                    }
                }
            }
            y += TITLE.len() as u16 + 1;
        } else {
            put_centered(
                buf,
                Rect {
                    y,
                    height: 1,
                    ..inner
                },
                "tuiforge",
                st(th.primary, th.background).add_modifier(Modifier::BOLD),
            );
            y += 2;
        }
        put_centered(
            buf,
            Rect {
                y,
                height: 1,
                ..inner
            },
            "A reusable component library that makes ratatui feel like Textual.",
            st(th.text, th.background),
        );
        y += 1;
        put_centered(
            buf,
            Rect {
                y,
                height: 1,
                ..inner
            },
            "Pick a page in the sidebar, press ] / [ to move, ^t to switch theme, ^b to hide the sidebar.",
            st(th.text_muted, th.background),
        );
        y += 2;

        // feature cards in a responsive grid
        let card_w: u16 = 44;
        let cols_n = ((inner.width + 2) / (card_w + 2)).clamp(1, 4) as usize;
        let rows_n = FEATURES.len().div_ceil(cols_n);
        // title row + tallest wrapped description + border rows
        let desc_lines = FEATURES
            .iter()
            .map(|(_, d)| wrap(d, (card_w - 6) as usize).len())
            .max()
            .unwrap_or(1)
            .min(3) as u16;
        let card_h = desc_lines + 3;
        let avail = inner.bottom().saturating_sub(y + 3);
        // gap row between card rows only when it fits
        let gap = if rows_n as u16 * (card_h + 1) - 1 <= avail {
            1
        } else {
            0
        };
        let grid_h = rows_n as u16 * (card_h + gap);
        let grid_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: grid_h.min(avail),
        };
        let total_w = cols_n as u16 * card_w + (cols_n as u16 - 1) * 2;
        let gx = grid_area.x + grid_area.width.saturating_sub(total_w) / 2;
        self.hits.clear();
        for (i, (name, desc)) in FEATURES.iter().enumerate() {
            let (r, c) = (i / cols_n, i % cols_n);
            let rect = Rect {
                x: gx + c as u16 * (card_w + 2),
                y: grid_area.y + r as u16 * (card_h + gap),
                width: card_w,
                height: card_h,
            };
            if rect.bottom() > grid_area.bottom() {
                break;
            }
            self.hits.push(rect);
            let hovered = self.hover == Some(i);
            let bg = if hovered { th.boost } else { th.surface };
            fill(buf, rect, bg);
            let border = if hovered {
                th.primary
            } else {
                th.border_blurred
            };
            Border::Round.draw(buf, rect, border, bg);
            let accent = gradient(
                &[th.primary, th.accent],
                i as f32 / (FEATURES.len() - 1) as f32,
            );
            put(buf, rect.x + 2, rect.y + 1, "●", 1, st(accent, bg));
            put(
                buf,
                rect.x + 4,
                rect.y + 1,
                name,
                rect.width - 6,
                st(th.text, bg).add_modifier(Modifier::BOLD),
            );
            for (k, line) in wrap(desc, (rect.width - 6) as usize)
                .iter()
                .take(desc_lines as usize)
                .enumerate()
            {
                put(
                    buf,
                    rect.x + 4,
                    rect.y + 2 + k as u16,
                    line,
                    rect.width - 6,
                    st(th.text_muted, bg),
                );
            }
        }

        // footer line: code sample
        let sample = "let mut sw = SwitchState::new(true);   Switch::new().focused(true).now(now).render(area, buf, &mut sw);   if sw.handle(&ev).is_changed() { … }";
        let sy = inner.bottom() - 1;
        if inner.height > 20 {
            put_centered(
                buf,
                Rect {
                    y: sy,
                    height: 1,
                    ..inner
                },
                &truncate(sample, inner.width as usize),
                st(th.text_disabled, th.background),
            );
        }
    }

    fn event(&mut self, ev: &Event, _ctx: &mut Ctx) -> Outcome {
        if let Event::Mouse(m) = ev {
            let pos = mouse_pos(m);
            let over = self.hits.iter().position(|r| r.contains(pos));
            if over != self.hover {
                self.hover = over;
                return Outcome::Consumed;
            }
        }
        Outcome::Ignored
    }

    fn animating(&self, _now: Instant) -> bool {
        true
    }
}
