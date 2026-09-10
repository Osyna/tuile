//! Loading states: every `LoaderStyle`, the multi-row scenes, painted skeletons, and a
//! `LoadingOverlay` over live content. Space pauses the clock, `+`/`-` change speed.

use std::time::Instant;

use tuiforge::draw::{put, put_right, st};
use tuiforge::prelude::*;

use super::{Ctx, Page, card};

const OVERLAY_STYLES: [LoaderStyle; 6] = [
    LoaderStyle::Sweep,
    LoaderStyle::Scanner,
    LoaderStyle::Snake,
    LoaderStyle::Ping,
    LoaderStyle::Radar,
    LoaderStyle::Rain,
];

pub struct LoadingPage {
    speed: f32,
    /// Phase frozen while paused.
    paused_at: Option<f32>,
    /// Subtracted from the app clock so resuming continues where it stopped.
    offset: f32,
    overlay: bool,
    overlay_style: usize,
}

impl Default for LoadingPage {
    fn default() -> Self {
        Self {
            speed: 1.0,
            paused_at: None,
            offset: 0.0,
            overlay: true,
            overlay_style: 0,
        }
    }
}

impl LoadingPage {
    /// Animation phase in seconds: `(app clock - offset) × speed`, frozen while paused.
    fn clock(&self, ctx: &Ctx) -> f32 {
        self.paused_at
            .unwrap_or((ctx.elapsed() - self.offset) * self.speed)
    }

    /// Change speed keeping the current phase, so nothing jumps.
    fn set_speed(&mut self, ctx: &Ctx, speed: f32) {
        let phase = self.clock(ctx);
        self.speed = speed;
        if self.paused_at.is_none() {
            self.offset = ctx.elapsed() - phase / self.speed;
        }
    }
}

impl Page for LoadingPage {
    fn title(&self) -> &'static str {
        "Loading"
    }
    fn subtitle(&self) -> &'static str {
        "Loader × 20 styles, Skeleton shapes, LoadingOverlay"
    }
    fn icon(&self) -> &'static str {
        "◌"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("space", "Pause"),
            ("+/-", "Speed"),
            ("o", "Overlay"),
            ("l", "Overlay style"),
        ]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let el = self.clock(ctx);
        let muted = st(th.text_muted, th.background);
        if area.height < 6 || area.width < 30 {
            return;
        }
        let [top, mid, bottom] = Layout::vertical([
            Constraint::Length(17),
            Constraint::Length(11),
            Constraint::Fill(1),
        ])
        .areas(area);

        // ── one-row loaders: 4 columns × 5 rows of (name / loader / gap) ──
        let title = format!(
            "Loader · {} styles · {:.1}×{}",
            LoaderStyle::ALL.len(),
            self.speed,
            if self.paused_at.is_some() {
                " · paused"
            } else {
                ""
            }
        );
        let g = pad(card(buf, top, &th, &title), 1, 0);
        let one_row: Vec<LoaderStyle> = LoaderStyle::ALL
            .iter()
            .copied()
            .filter(|s| !s.multi_row())
            .collect();
        let cols = if g.width >= 100 {
            4
        } else if g.width >= 60 {
            3
        } else {
            2
        };
        let col_w = g.width / cols;
        let palette = [
            th.primary,
            th.accent,
            th.success,
            th.warning,
            th.secondary,
            th.error,
        ];
        for (i, style) in one_row.iter().enumerate() {
            let (c, r) = ((i % cols as usize) as u16, (i / cols as usize) as u16);
            let y = g.y + r * 3;
            if y + 1 >= g.bottom() {
                break;
            }
            let x = g.x + c * col_w;
            let w = col_w.saturating_sub(3);
            put(buf, x, y, style.name(), w, muted);
            let color = palette[i % palette.len()];
            let label = match style {
                LoaderStyle::Ellipsis => "Thinking",
                LoaderStyle::Shimmer => "Generating a response",
                LoaderStyle::Typewriter => "Indexing 1,204 files",
                _ => "",
            };
            Loader::new(*style)
                .label(label)
                .color(color)
                .color2(palette[(i + 2) % palette.len()])
                .elapsed(el)
                .theme(&th)
                .render(
                    Rect {
                        x,
                        y: y + 1,
                        width: w,
                        height: 1,
                    },
                    buf,
                );
        }

        // ── scenes: the three multi-row styles side by side ──
        let s = pad(
            card(buf, mid, &th, "Scenes (use every row you give them)"),
            1,
            0,
        );
        let [e, r, d] = Layout::horizontal([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Fill(1),
        ])
        .areas(s);
        for (cell, style, c1, c2) in [
            (e, LoaderStyle::Equalizer, th.success, th.error),
            (r, LoaderStyle::Rain, th.success, th.success),
            (d, LoaderStyle::Radar, th.primary, th.warning),
        ] {
            put(buf, cell.x, cell.y, style.name(), cell.width, muted);
            let body = Rect {
                y: cell.y + 1,
                height: cell.height.saturating_sub(1),
                width: cell.width.saturating_sub(2),
                ..cell
            };
            Loader::new(style)
                .color(c1)
                .color2(c2)
                .elapsed(el)
                .theme(&th)
                .render(body, buf);
        }

        // ── skeletons | overlay ──
        let [sk, ov] =
            Layout::horizontal([Constraint::Percentage(72), Constraint::Fill(1)]).areas(bottom);
        let k = pad(card(buf, sk, &th, "Skeleton shapes"), 1, 0);
        let shapes = [
            (SkeletonShape::Text, "Text", 15u16),
            (SkeletonShape::Card, "Card", 13),
            (SkeletonShape::List, "List", 13),
            (SkeletonShape::Table, "Table", 19),
            (SkeletonShape::Chart, "Chart", 0),
        ];
        let mut x = k.x;
        for (shape, name, fixed) in shapes {
            let w = if fixed == 0 {
                k.right().saturating_sub(x)
            } else {
                fixed.min(k.right().saturating_sub(x))
            };
            if w < 4 {
                break;
            }
            let w = w.saturating_sub(2);
            put(buf, x, k.y, name, w, muted);
            let body = Rect {
                x,
                y: k.y + 1,
                width: w,
                height: k.height.saturating_sub(1),
            };
            Skeleton::new()
                .shape(shape)
                .lines(&[w, w * 9 / 10, w * 19 / 20, w * 3 / 5])
                .elapsed(el)
                .theme(&th)
                .render(body, buf);
            x += w + 2;
        }

        let o = pad(card(buf, ov, &th, "LoadingOverlay over content"), 1, 0);
        // the "content": a little table that the overlay dims
        let rows = [
            ("id", "name", "status"),
            ("1042", "deploy-api", "running"),
            ("1041", "build-web", "passed"),
            ("1040", "lint", "passed"),
            ("1039", "e2e-smoke", "failed"),
            ("1038", "migrate-db", "passed"),
            ("1037", "publish", "queued"),
        ];
        for (i, (a, b, c)) in rows.iter().enumerate() {
            let y = o.y + i as u16;
            if y >= o.bottom() {
                break;
            }
            let style = if i == 0 {
                st(th.text_muted, th.background).add_modifier(Modifier::BOLD)
            } else {
                st(th.text, th.background)
            };
            put(buf, o.x, y, a, 6, style);
            put(buf, o.x + 6, y, b, o.width.saturating_sub(16), style);
            put_right(buf, Rect { y, height: 1, ..o }, c, style);
        }
        if self.overlay {
            let style = OVERLAY_STYLES[self.overlay_style % OVERLAY_STYLES.len()];
            LoadingOverlay::new(Loader::new(style).color(th.primary).color2(th.accent))
                .message(format!("Fetching runs… ({})", style.name()))
                .elapsed(el)
                .theme(&th)
                .render(o, buf);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        let Event::Key(k) = ev else {
            return Outcome::Ignored;
        };
        if !is_press(k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Char(' ') => {
                match self.paused_at {
                    Some(at) => {
                        self.offset = ctx.elapsed() - at / self.speed;
                        self.paused_at = None;
                    }
                    None => self.paused_at = Some(self.clock(ctx)),
                }
                Outcome::Changed
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.set_speed(ctx, (self.speed + 0.25).min(4.0));
                Outcome::Changed
            }
            KeyCode::Char('-') => {
                self.set_speed(ctx, (self.speed - 0.25).max(0.25));
                Outcome::Changed
            }
            KeyCode::Char('o') => {
                self.overlay = !self.overlay;
                Outcome::Changed
            }
            KeyCode::Char('l') => {
                self.overlay_style = (self.overlay_style + 1) % OVERLAY_STYLES.len();
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        self.paused_at.is_none()
    }
}
