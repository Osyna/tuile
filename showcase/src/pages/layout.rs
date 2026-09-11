//! Layout: draggable split panes, a scroll view with both scrollbars, panels in every
//! border style, an accordion, header/footer chrome and placeholders.

use tuiforge::draw::{Border, fill, put, st};
use tuiforge::prelude::*;
use tuiforge::widgets::{
    CollapsibleHeader, PanelBg, PlaceholderVariant, ScrollBars, SplitDivider, SplitSize,
};

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Scroll,
    Accordion,
    Header,
    Footer,
}

const DOC_LINES: usize = 120;
const DOC_WIDTH: u16 = 140;

pub struct LayoutPage {
    focus: Focus<Id>,
    outer: SplitState,
    inner: SplitState,
    scroll: ScrollViewState,
    smooth: bool,
    accordion: AccordionState,
    exclusive: bool,
    header: AppHeaderState,
    header2: AppHeaderState,
    footer: KeyFooterState,
    footer2: KeyFooterState,
}

impl Default for LayoutPage {
    fn default() -> Self {
        let mut accordion = AccordionState::new(3, false);
        accordion.states[0] = CollapsibleState::new(true);
        LayoutPage {
            focus: Focus::new([Id::Scroll, Id::Accordion, Id::Header, Id::Footer]),
            outer: SplitState::new(SplitSize::Ratio(0.42), 30, 40),
            inner: SplitState::new(SplitSize::Ratio(0.55), 8, 10),
            scroll: ScrollViewState::new(),
            smooth: true,
            accordion,
            exclusive: false,
            header: AppHeaderState::new(),
            header2: AppHeaderState::new(),
            footer: KeyFooterState::new(),
            footer2: KeyFooterState::new(),
        }
    }
}

impl Page for LayoutPage {
    fn title(&self) -> &'static str {
        "Layout"
    }
    fn subtitle(&self) -> &'static str {
        "Split panes, scroll views, panels, accordion, header & footer"
    }
    fn icon(&self) -> &'static str {
        "□"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("drag", "Dividers"),
            ("^←→ ^↑↓", "Resize"),
            ("s", "Smooth"),
            ("x", "Exclusive"),
            ("r", "Reset"),
        ]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        let inner = pad(area, 1, 0);
        let [main, strip] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(4)]).areas(inner);

        // ── outer split: scroll view | right column ──
        let (left, right) = SplitPane::new()
            .direction(Direction::Horizontal)
            .divider(SplitDivider::Line)
            .focused(true)
            .theme(&th)
            .render_split(main, buf, &mut self.outer);

        let focused = self.focus.is(Id::Scroll);
        let title = format!(
            "Scroll view  ·  smooth {}  ·  {}×{}",
            if self.smooth { "on" } else { "off" },
            DOC_WIDTH,
            DOC_LINES
        );
        let sv_area = card(buf, left, &th, &title);
        if focused {
            Border::Round.draw(buf, left, th.border, th.background);
            put(
                buf,
                left.x + 2,
                left.y,
                &format!(" {title} "),
                left.width.saturating_sub(4),
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
        }
        ScrollView::new()
            .content_size(DOC_WIDTH, DOC_LINES as u16)
            .show_scrollbars(ScrollBars::Auto)
            .smooth(self.smooth)
            .theme(&th)
            .render_with(sv_area, buf, &mut self.scroll, |cbuf, carea| {
                fill(cbuf, carea, th.surface);
                for i in 0..carea.height as usize {
                    let y = carea.y + i as u16;
                    let (text, color) = match i % 10 {
                        0 => (format!("     Section {}", i / 10 + 1), th.text_primary),
                        1 => ("─".repeat(60), th.border_blurred),
                        5 => ("  │ id │ name         │ status  │ latency │ region     │ notes                          │".to_string(), th.text_muted),
                        6 => (format!("  │ {:>2} │ node-{:<7} │ healthy │ {:>4} ms │ {:<10} │ scrolled horizontally to see me │", i, i * 7, 40 + i * 3, ["us-east", "eu-west", "ap-south"][i % 3]), th.text),
                        _ => (format!("{:>3}  The quick brown fox jumps over the lazy dog on line {i}, which keeps scrolling smoothly with the wheel, arrows and PgUp/PgDn.", i + 1), th.text),
                    };
                    put(cbuf, carea.x, y, &text, carea.width, st(color, th.surface));
                    put(cbuf, carea.x, y, &format!("{:>3}", i + 1), 3, st(th.text_disabled, th.surface));
                }
            });

        // ── right column: accordion / chrome ──
        let (top, bottom) = SplitPane::new()
            .direction(Direction::Vertical)
            .divider(SplitDivider::Line)
            .focused(true)
            .theme(&th)
            .render_split(right, buf, &mut self.inner);

        let acc_area = card(
            buf,
            top,
            &th,
            &format!(
                "Accordion  ·  exclusive {}",
                if self.exclusive { "on" } else { "off" }
            ),
        );
        let acc_inner = pad(acc_area, 1, 0);
        self.accordion.exclusive = self.exclusive;
        let titles = ["Overview", "Panels & borders", "Placeholders"];
        let borders = [
            Border::Round,
            Border::Tall,
            Border::Double,
            Border::Heavy,
            Border::Dashed,
            Border::Outer,
        ];
        Accordion::new()
            .titles(&titles)
            .exclusive(self.exclusive)
            .header_style(CollapsibleHeader::Panel)
            .gap(1)
            .focused(self.focus.is(Id::Accordion))
            .theme(&th)
            .render_with(acc_inner, buf, &mut self.accordion, &[4, 6, 5], |i, r, b| match i {
                0 => {
                    let lines = [
                        "SplitPane divides an area with a draggable divider (hover it, then drag).",
                        "ScrollView renders content into an offscreen buffer and blits the viewport.",
                        "Panel/Collapsible/Accordion return the inner rect so you compose freely.",
                        "Press ↑↓ to move between headers, Enter to toggle, x for exclusive mode.",
                    ];
                    for (k, l) in lines.iter().enumerate() {
                        put(b, r.x + 1, r.y + k as u16, l, r.width.saturating_sub(1), st(th.text_muted, th.background));
                    }
                }
                1 => {
                    let cols = columns(r, borders.len().min((r.width / 12).max(1) as usize), 1);
                    for (k, (c, border)) in cols.iter().zip(borders.iter()).enumerate() {
                        let bg = [PanelBg::Surface, PanelBg::Panel, PanelBg::Boost][k % 3];
                        Panel::new().title(border.name()).border(*border).background(bg).theme(&th).render(*c, b);
                    }
                }
                _ => {
                    let cols = columns(r, 3, 1);
                    for (k, c) in cols.iter().enumerate() {
                        Placeholder::new().variant([PlaceholderVariant::Default, PlaceholderVariant::Size, PlaceholderVariant::Text][k]).index(k + 2).theme(&th).render(*c, b, &format!("placeholder {}", k + 1));
                    }
                }
            });

        let chrome = pad(card(buf, bottom, &th, "Header & footer chrome"), 1, 0);
        let rows = stack(chrome, &[3, 1, 1, 1, 1, 1, 0], 0);
        if rows.len() >= 6 {
            AppHeader::new()
                .title("Application")
                .subtitle("tall header with clock")
                .icon("⊛")
                .clock(true)
                .clock_seconds(true)
                .tall(true)
                .actions(&["Save", "Share", "Help"])
                .theme(&th)
                .render(rows[0], buf, &mut self.header);
            AppHeader::new()
                .title("Compact header")
                .icon("◈")
                .right("v0.1.0")
                .theme(&th)
                .render(rows[2], buf, &mut self.header2);
            KeyFooter::new()
                .bindings(&[
                    ("q", "Quit"),
                    ("^s", "Save"),
                    ("^p", "Palette"),
                    ("F1", "Help"),
                    ("tab", "Focus"),
                    ("esc", "Back"),
                    ("^b", "Sidebar"),
                ])
                .right(if self.focus.is(Id::Footer) {
                    "focused: click a key"
                } else {
                    "Ready"
                })
                .theme(&th)
                .render(rows[4], buf, &mut self.footer);
            KeyFooter::new()
                .bindings(&[("q", "Quit"), ("^s", "Save"), ("F1", "Help")])
                .compact(true)
                .message("Saved 3 files in 120 ms", Variant::Success)
                .theme(&th)
                .render(rows[5], buf, &mut self.footer2);
        }

        // ── placeholders strip ──
        let s_in = card(buf, strip, &th, "Placeholder");
        let cells = columns(s_in, 6, 1);
        for (i, c) in cells.iter().enumerate() {
            let v = [
                PlaceholderVariant::Default,
                PlaceholderVariant::Size,
                PlaceholderVariant::Text,
            ][i % 3];
            Placeholder::new().variant(v).index(i).theme(&th).render(
                *c,
                buf,
                ["hero", "nav", "aside", "main", "footer", "ad"][i],
            );
        }
        let _ = now;
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) => {
                if self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                if k.modifiers.contains(KeyModifiers::CONTROL) {
                    match k.code {
                        KeyCode::Left => {
                            self.outer.resize_by(-2);
                            return Outcome::Consumed;
                        }
                        KeyCode::Right => {
                            self.outer.resize_by(2);
                            return Outcome::Consumed;
                        }
                        KeyCode::Up => {
                            self.inner.resize_by(-1);
                            return Outcome::Consumed;
                        }
                        KeyCode::Down => {
                            self.inner.resize_by(1);
                            return Outcome::Consumed;
                        }
                        _ => {}
                    }
                }
                match k.code {
                    KeyCode::Char('s') => {
                        self.smooth = !self.smooth;
                        return Outcome::Changed;
                    }
                    KeyCode::Char('x') => {
                        self.exclusive = !self.exclusive;
                        return Outcome::Changed;
                    }
                    KeyCode::Char('r') => {
                        self.outer = SplitState::new(SplitSize::Ratio(0.42), 30, 40);
                        self.inner = SplitState::new(SplitSize::Ratio(0.55), 8, 10);
                        ctx.notify("Splits reset", Variant::Default);
                        return Outcome::Changed;
                    }
                    _ => {}
                }
                match self.focus.current() {
                    Some(Id::Scroll) => self.scroll.handle_key(*k),
                    Some(Id::Accordion) => self.accordion.handle_key(*k),
                    _ => Outcome::Ignored,
                }
            }
            Event::Mouse(m) => {
                let m = *m;
                let mut out = self.outer.handle_mouse(m) | self.inner.handle_mouse(m);
                let o = self.scroll.handle_mouse(m);
                if o.is_consumed() && is_left_down(&m) {
                    self.focus.set(Id::Scroll);
                }
                out |= o;
                let o = self.accordion.handle_mouse(m);
                if o.is_changed() {
                    self.focus.set(Id::Accordion);
                }
                out |= o;
                out |= self.header.handle_mouse(m);
                if let Some(a) = self.header.take_action() {
                    self.focus.set(Id::Header);
                    ctx.notify(
                        format!(
                            "Header action: {}",
                            ["Save", "Share", "Help"].get(a).unwrap_or(&"?")
                        ),
                        Variant::Primary,
                    );
                }
                if self.header.take_icon_click() {
                    ctx.notify(
                        "Header icon clicked (open your palette here)",
                        Variant::Accent,
                    );
                }
                out |= self.header2.handle_mouse(m);
                out |= self.footer.handle_mouse(m);
                if let Some(i) = self.footer.take_pressed() {
                    self.focus.set(Id::Footer);
                    ctx.notify(format!("Footer binding #{i} clicked"), Variant::Primary);
                }
                out |= self.footer2.handle_mouse(m);
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, now: Instant) -> bool {
        self.scroll.animating(now) || self.accordion.animating(now)
    }
}
