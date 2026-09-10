//! Spinner catalog: every spinner in `tuiforge::widgets::spinners`, filterable, with a live
//! preview and the one-liner to use it.

use std::time::Instant;

use tuiforge::draw::{fill, put, put_right, st, truncate};
use tuiforge::prelude::*;
use tuiforge::widgets::spinners;

use super::{Ctx, Page, card};

/// `bouncingBar` → `BOUNCING_BAR`: the `const` name in `spinners`.
pub fn const_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

const CELL_W: u16 = 26;

enum Row {
    Header(&'static str),
    Cells(Vec<&'static SpinnerDef>),
}

pub struct SpinnersPage {
    filter: InputState,
    scroll: usize,
    selected: &'static SpinnerDef,
    hover: Option<&'static SpinnerDef>,
    hits: Vec<(Rect, &'static SpinnerDef)>,
    scrollbar: ScrollbarState,
    rows_total: usize,
    rows_visible: usize,
    /// Column count from the last draw, so key navigation can keep the selection in view.
    cols_last: usize,
}

impl Default for SpinnersPage {
    fn default() -> Self {
        Self {
            filter: InputState::new(),
            scroll: 0,
            selected: &spinners::DOTS,
            hover: None,
            hits: Vec::new(),
            scrollbar: ScrollbarState::default(),
            rows_total: 0,
            rows_visible: 0,
            cols_last: 1,
        }
    }
}

impl SpinnersPage {
    fn groups(&self) -> [(&'static str, Vec<&'static SpinnerDef>); 2] {
        let q = self.filter.value().trim().to_ascii_lowercase();
        let pick = |set: &[&'static SpinnerDef]| -> Vec<&'static SpinnerDef> {
            set.iter()
                .copied()
                .filter(|d| q.is_empty() || d.name.to_ascii_lowercase().contains(&q))
                .collect()
        };
        [
            ("tuiforge originals", pick(spinners::ORIGINALS)),
            ("yaspin / cli-spinners", pick(spinners::YASPIN)),
        ]
    }

    fn rows(&self, cols: usize) -> Vec<Row> {
        let mut rows = Vec::new();
        for (title, defs) in self.groups() {
            if defs.is_empty() {
                continue;
            }
            if !rows.is_empty() {
                rows.push(Row::Cells(Vec::new()));
            }
            rows.push(Row::Header(title));
            for chunk in defs.chunks(cols) {
                rows.push(Row::Cells(chunk.to_vec()));
            }
        }
        rows
    }

    /// Flat, in display order, for arrow-key navigation.
    fn order(&self) -> Vec<&'static SpinnerDef> {
        self.groups().into_iter().flat_map(|(_, d)| d).collect()
    }

    fn step(&mut self, delta: i32) {
        let order = self.order();
        if order.is_empty() {
            return;
        }
        let cur = order
            .iter()
            .position(|d| std::ptr::eq(*d, self.selected))
            .unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, order.len() as i32 - 1) as usize;
        self.selected = order[next];
        self.scroll_to_selected();
    }

    fn scroll_to_selected(&mut self) {
        // rows are recomputed on draw; find the selected row with the last layout's column count
        let cols = self.cols_last.max(1);
        let rows = self.rows(cols);
        let row = rows
            .iter()
            .position(
                |r| matches!(r, Row::Cells(c) if c.iter().any(|d| std::ptr::eq(*d, self.selected))),
            )
            .unwrap_or(0);
        self.scroll = keep_visible(self.scroll, row, self.rows_visible.max(1));
    }

    fn snippet(&self) -> String {
        format!(
            "Spinner::new(&spinners::{}).label(\"Working…\").now(now)",
            const_name(self.selected.name)
        )
    }

    fn cols_for(width: u16) -> usize {
        (width / CELL_W).max(1) as usize
    }
}

impl Page for SpinnersPage {
    fn title(&self) -> &'static str {
        "Spinners"
    }
    fn subtitle(&self) -> &'static str {
        "102 spinners: the full yaspin / cli-spinners set plus tuiforge originals"
    }
    fn icon(&self) -> &'static str {
        "◌"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        let inner = pad(area, 1, 0);
        if inner.width < 20 || inner.height < 6 {
            return;
        }
        let preview_h = if inner.height >= 18 { 6 } else { 0 };
        let [head, grid_a, preview_a] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(preview_h),
        ])
        .areas(inner);

        // filter row
        let count = self.order().len();
        let counter = format!("{count} / {}", spinners::ALL.len());
        put_right(buf, head, &counter, st(th.text_muted, th.background));
        let filter_area = Rect {
            width: head.width.saturating_sub(counter.len() as u16 + 2).min(40),
            ..head
        };
        Input::new()
            .placeholder("Type to filter…")
            .prefix("/ ")
            .compact(true)
            .focused(true)
            .theme(&th)
            .render(filter_area, buf, &mut self.filter);

        // grid
        let g = card(
            buf,
            grid_a,
            &th,
            "Catalog (↑↓←→ select · Enter for the snippet · hover to preview)",
        );
        let g = pad(g, 1, 0);
        let cols = Self::cols_for(g.width);
        self.cols_last = cols;
        let cw = g.width / cols as u16;
        let rows = self.rows(cols);
        self.rows_total = rows.len();
        self.rows_visible = g.height as usize;
        if self.scroll + g.height as usize > rows.len() {
            self.scroll = rows.len().saturating_sub(g.height as usize);
        }
        self.hits.clear();
        for (i, row) in rows
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(g.height as usize)
        {
            let y = g.y + (i - self.scroll) as u16;
            match row {
                Row::Header(t) => {
                    put(
                        buf,
                        g.x,
                        y,
                        t,
                        g.width,
                        st(th.text_muted, th.background).add_modifier(Modifier::BOLD),
                    );
                }
                Row::Cells(defs) => {
                    for (c, def) in defs.iter().enumerate() {
                        let rect = Rect {
                            x: g.x + c as u16 * cw,
                            y,
                            width: cw.saturating_sub(1),
                            height: 1,
                        };
                        let is_sel = std::ptr::eq(*def, self.selected);
                        let is_hover = self.hover.is_some_and(|h| std::ptr::eq(h, *def));
                        let bg = if is_sel {
                            th.cursor_bg
                        } else if is_hover {
                            th.hover_bg
                        } else {
                            th.background
                        };
                        fill(buf, rect, bg);
                        let slot = def.width().min(rect.width);
                        let (glyph_fg, name_fg) = if is_sel {
                            (th.cursor_fg, th.cursor_fg)
                        } else {
                            (th.primary, th.text)
                        };
                        put(
                            buf,
                            rect.x,
                            y,
                            def.frame(since(now)),
                            slot,
                            st(glyph_fg, bg),
                        );
                        let nx = rect.x + slot + 1;
                        if nx < rect.right() {
                            put(
                                buf,
                                nx,
                                y,
                                &truncate(def.name, (rect.right() - nx) as usize),
                                rect.right() - nx,
                                st(name_fg, bg),
                            );
                        }
                        self.hits.push((rect, def));
                    }
                }
            }
        }
        if rows.len() > g.height as usize {
            let sb = Rect {
                x: g.right(),
                y: g.y,
                width: 1,
                height: g.height,
            };
            Scrollbar::vertical(rows.len(), g.height as usize)
                .offset(self.scroll)
                .theme(&th)
                .render(sb, buf, &mut self.scrollbar);
        }

        // preview
        if preview_h > 0 {
            let def = self.hover.unwrap_or(self.selected);
            let p = pad(
                card(
                    buf,
                    preview_a,
                    &th,
                    &format!("Preview · spinners::{}", const_name(def.name)),
                ),
                1,
                0,
            );
            let meta = format!(
                "{} frames · {} ms · {} cell{} wide",
                def.frames.len(),
                def.interval_ms,
                def.width(),
                if def.width() == 1 { "" } else { "s" }
            );
            put(
                buf,
                p.x,
                p.y,
                &meta,
                p.width,
                st(th.text_muted, th.background),
            );
            Spinner::new(def)
                .label("Working…")
                .now(now)
                .theme(&th)
                .render(
                    Rect {
                        y: p.y + 1,
                        height: 1,
                        ..p
                    },
                    buf,
                );
            let frames = def.frames.join(" ");
            put(
                buf,
                p.x,
                p.y + 2,
                &truncate(&frames, p.width as usize),
                p.width,
                st(th.text_muted, th.background),
            );
            put(
                buf,
                p.x,
                p.y + 3,
                &truncate(&self.snippet(), p.width as usize),
                p.width,
                st(th.text_accent, th.background),
            );
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => match k.code {
                KeyCode::Up => {
                    self.step(-(self.cols_last as i32));
                    Outcome::Consumed
                }
                KeyCode::Down => {
                    self.step(self.cols_last as i32);
                    Outcome::Consumed
                }
                KeyCode::Right
                    if k.modifiers.contains(KeyModifiers::ALT)
                        || self.filter.value().is_empty() =>
                {
                    self.step(1);
                    Outcome::Consumed
                }
                KeyCode::Left
                    if k.modifiers.contains(KeyModifiers::ALT)
                        || self.filter.value().is_empty() =>
                {
                    self.step(-1);
                    Outcome::Consumed
                }
                KeyCode::PageDown => {
                    self.step((self.cols_last * self.rows_visible.max(1)) as i32);
                    Outcome::Consumed
                }
                KeyCode::PageUp => {
                    self.step(-((self.cols_last * self.rows_visible.max(1)) as i32));
                    Outcome::Consumed
                }
                KeyCode::Enter => {
                    ctx.notify(self.snippet(), Variant::Primary);
                    Outcome::Consumed
                }
                _ => {
                    let out = self.filter.handle_key(*k);
                    if out.is_changed() {
                        self.scroll = 0;
                        if let Some(first) = self.order().first() {
                            self.selected = first;
                        }
                    }
                    out
                }
            },
            Event::Mouse(m) => {
                if let Some(delta) = wheel_delta(m) {
                    let max = self.rows_total.saturating_sub(self.rows_visible);
                    self.scroll = (self.scroll as i64 + delta as i64).clamp(0, max as i64) as usize;
                    return Outcome::Consumed;
                }
                let pos = mouse_pos(m);
                self.hover = self
                    .hits
                    .iter()
                    .find(|(r, _)| r.contains(pos))
                    .map(|(_, d)| *d);
                if is_left_down(m)
                    && let Some(def) = self.hover
                {
                    self.selected = def;
                    ctx.notify(self.snippet(), Variant::Primary);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        true
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("type", "Filter"), ("↑↓", "Select"), ("Enter", "Snippet")]
    }
}
