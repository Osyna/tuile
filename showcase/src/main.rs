//! tuiforge showcase: a gallery app where every page exercises one widget family.
//!
//! Run `showcase --page charts` to open directly on a page.

mod pages;

use std::time::Instant;

use tuiforge::draw::{fill, put, put_centered, put_right, st};
use tuiforge::prelude::*;
use tuiforge::runtime::local_hms;

use pages::{Ctx, Page};

const SIDEBAR_W: u16 = 22;

struct Shell {
    pages: Vec<Box<dyn Page>>,
    current: usize,
    sidebar: bool,
    sidebar_hover: Option<usize>,
    theme_idx: usize,
    ctx: Ctx,
    /// Latest notice shown in the footer until `notice_until`.
    notice: Option<(String, Variant, Instant)>,
    /// Sidebar rect from the last draw (zero-width when hidden), for mouse routing.
    side_area: Rect,
}

impl Shell {
    fn new(start: Option<&str>) -> Self {
        let pages = pages::all();
        let current = start
            .and_then(|s| pages.iter().position(|p| p.title().to_lowercase().contains(&s.to_lowercase())))
            .unwrap_or(0);
        let now = Instant::now();
        let ctx = Ctx { theme: theme::current(), now, started: now, reduce_motion: false, notices: Vec::new(), area: Rect::default() };
        Shell { pages, current, sidebar: true, sidebar_hover: None, theme_idx: 0, ctx, notice: None, side_area: Rect::default() }
    }

    fn set_theme(&mut self, idx: usize) {
        let specs = theme::BUILTIN;
        self.theme_idx = idx % specs.len();
        let th = Theme::resolve(&specs[self.theme_idx], None);
        theme::set(th.clone());
        self.ctx.theme = th;
        let name = specs[self.theme_idx].name.to_string();
        self.ctx.notify(format!("Theme: {name}"), Variant::Primary);
    }

    fn goto(&mut self, idx: usize) {
        if !self.pages.is_empty() {
            self.current = idx % self.pages.len();
        }
    }

    fn layout(&self, area: Rect) -> (Rect, Rect, Rect, Rect) {
        let [header, body, footer] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let (side, content) = if self.sidebar && area.width >= 70 {
            let [s, c] = Layout::horizontal([Constraint::Length(SIDEBAR_W), Constraint::Fill(1)]).areas(body);
            (s, c)
        } else {
            (Rect { width: 0, ..body }, body)
        };
        (header, side, content, footer)
    }

    fn draw_header(&self, area: Rect, buf: &mut Buffer) {
        let th = &self.ctx.theme;
        fill(buf, area, th.panel);
        let page = &self.pages[self.current];
        let title = format!(" ⚒ tuiforge showcase  ›  {}", page.title());
        put(buf, area.x, area.y, &title, area.width, st(th.text, th.panel).add_modifier(Modifier::BOLD));
        let sub = page.subtitle();
        if !sub.is_empty() && area.width > 80 {
            let x = area.x + title.chars().count() as u16 + 2;
            put(buf, x, area.y, sub, area.width.saturating_sub(x + 12), st(th.text_muted, th.panel));
        }
        let (h, m, s) = local_hms();
        put_right(buf, area, &format!("{h:02}:{m:02}:{s:02} "), st(th.text_muted, th.panel));
    }

    fn draw_sidebar(&self, area: Rect, buf: &mut Buffer) {
        let th = &self.ctx.theme;
        fill(buf, area, th.surface);
        put(buf, area.x + 2, area.y + 1, "COMPONENTS", area.width, st(th.text_muted, th.surface).add_modifier(Modifier::BOLD));
        for (i, page) in self.pages.iter().enumerate() {
            let y = area.y + 3 + i as u16;
            if y >= area.bottom().saturating_sub(3) {
                break;
            }
            let row = Rect { x: area.x, y, width: area.width, height: 1 };
            let active = i == self.current;
            let hover = self.sidebar_hover == Some(i);
            let (fg, bg) = if active {
                (th.cursor_fg, th.cursor_bg)
            } else if hover {
                (th.text, th.hover_bg)
            } else {
                (th.text, th.surface)
            };
            fill(buf, row, bg);
            let label = format!("  {:<2} {}", page.icon(), page.title());
            let style = if active { st(fg, bg).add_modifier(Modifier::BOLD) } else { st(fg, bg) };
            put(buf, row.x, y, &label, row.width, style);
            if active {
                put(buf, row.x, y, "▌", 1, st(th.accent, bg));
            }
        }
        if area.height > 16 {
            let y = area.bottom() - 2;
            put(buf, area.x + 2, y, &format!("v{}", env!("CARGO_PKG_VERSION")), area.width - 3, st(th.text_disabled, th.surface));
            put(buf, area.x + 2, y + 1, th.name, area.width - 3, st(th.text_disabled, th.surface));
        }
    }

    fn draw_footer(&self, area: Rect, buf: &mut Buffer, now: Instant) {
        let th = &self.ctx.theme;
        fill(buf, area, th.footer_bg);
        if let Some((msg, v, until)) = &self.notice
            && now < *until
        {
            let color = th.text_variant(*v);
            put(buf, area.x + 1, area.y, &format!("● {msg}"), area.width.saturating_sub(2), st(color, th.footer_bg).add_modifier(Modifier::BOLD));
            return;
        }
        let mut x = area.x + 1;
        let mut bindings: Vec<(&str, &str)> = vec![("^c", "Quit"), ("[ ]", "Page"), ("^t", "Theme"), ("^b", "Sidebar")];
        bindings.extend(self.pages[self.current].bindings().iter().copied());
        for (key, desc) in bindings {
            let need = key.chars().count() as u16 + desc.chars().count() as u16 + 4;
            if x + need > area.right() {
                break;
            }
            x += put(buf, x, area.y, &format!(" {key} "), need, st(th.footer_key, th.footer_bg).add_modifier(Modifier::BOLD));
            x += put(buf, x, area.y, &format!("{desc} "), need, st(th.footer_desc, th.footer_bg));
        }
    }

    fn drain_notices(&mut self, now: Instant) {
        if let Some((msg, v)) = self.ctx.notices.drain(..).last() {
            self.notice = Some((msg, v, now + Duration::from_millis(2500)));
        }
    }

    fn sidebar_item_at(&self, side: Rect, pos: Position) -> Option<usize> {
        if side.width == 0 || !side.contains(pos) {
            return None;
        }
        let i = pos.y.checked_sub(side.y + 3)? as usize;
        (i < self.pages.len()).then_some(i)
    }
}

impl App for Shell {
    fn draw(&mut self, frame: &mut Frame, now: Instant) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let th = self.ctx.theme.clone();
        fill(buf, area, th.background);
        if area.width < 60 || area.height < 16 {
            put_centered(buf, Rect { y: area.y + area.height / 2, height: 1, ..area }, "Terminal too small (need 60×16)", st(th.text, th.background));
            return;
        }
        let (header, side, content, footer) = self.layout(area);
        self.ctx.now = now;
        self.ctx.area = content;
        self.draw_header(header, buf);
        self.side_area = side;
        if side.width > 0 {
            self.draw_sidebar(side, buf);
        }
        let cur = self.current;
        self.pages[cur].draw(content, buf, &mut self.ctx);
        self.drain_notices(now);
        self.draw_footer(footer, buf, now);
    }

    fn event(&mut self, ev: Event, now: Instant) -> Flow {
        self.ctx.now = now;
        match &ev {
            Event::Key(k) => {
                // shell-global chords first
                if ctrl(k, 'c') || ctrl(k, 'q') {
                    return Flow::Quit;
                }
                if ctrl(k, 't') {
                    self.set_theme(self.theme_idx + 1);
                } else if ctrl(k, 'b') {
                    self.sidebar = !self.sidebar;
                } else if k.code == KeyCode::F(3) {
                    self.ctx.reduce_motion = !self.ctx.reduce_motion;
                    let msg = if self.ctx.reduce_motion { "Reduce motion: on" } else { "Reduce motion: off" };
                    self.ctx.notify(msg, Variant::Accent);
                } else if k.modifiers.contains(KeyModifiers::ALT)
                    && let KeyCode::Char(c) = k.code
                    && let Some(d) = c.to_digit(10)
                    && d >= 1
                {
                    self.goto(d as usize - 1);
                } else {
                    let cur = self.current;
                    let out = self.pages[cur].event(&ev, &mut self.ctx);
                    if out.is_ignored() {
                        match k.code {
                            KeyCode::Char(']') => self.goto(self.current + 1),
                            KeyCode::Char('[') => self.goto(self.current + self.pages.len() - 1),
                            KeyCode::Char('q') => return Flow::Quit,
                            _ => {}
                        }
                    }
                }
            }
            Event::Mouse(m) => {
                let side = self.side_area;
                let pos = mouse_pos(m);
                let over = self.sidebar_item_at(side, pos);
                if is_left_down(m) && let Some(i) = over {
                    self.goto(i);
                } else if side.contains(pos) || over != self.sidebar_hover {
                    self.sidebar_hover = over;
                }
                if !side.contains(pos) {
                    let cur = self.current;
                    self.pages[cur].event(&ev, &mut self.ctx);
                }
            }
            _ => {}
        }
        self.drain_notices(now);
        Flow::Continue
    }

    fn animating(&self, now: Instant) -> bool {
        self.pages[self.current].animating(now) || self.notice.as_ref().is_some_and(|(_, _, until)| now < *until)
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut start = None;
    let mut theme_name = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--page" | "-p" => start = it.next().map(String::as_str),
            "--theme" | "-t" => theme_name = it.next().map(String::as_str),
            "--help" | "-h" => {
                println!("showcase [--page NAME] [--theme NAME]\n\nThemes: {}", theme::theme_names().join(", "));
                return Ok(());
            }
            _ => {}
        }
    }
    let mut shell = Shell::new(start);
    if let Some(name) = theme_name
        && let Some(i) = theme::BUILTIN.iter().position(|t| t.name == name)
    {
        shell.set_theme(i);
        shell.ctx.notices.clear();
    }
    run(&mut shell)
}
