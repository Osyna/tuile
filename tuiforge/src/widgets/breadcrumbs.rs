//! Breadcrumb navigation with collapsing middle segments, clickable links, hover highlights,
//! and a paginator widget for page-based navigation.
//!
//! ```
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 3);
//! # let mut buf = Buffer::empty(area);
//! let segments = vec!["Home".to_string(), "Projects".to_string(), "tuiforge".to_string()];
//! let mut state = BreadcrumbsState::new();
//! Breadcrumbs::new(segments).render(area, &mut buf, &mut state);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{MouseEvent, MouseEventKind, MouseButton};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::{mouse_pos, Interactive, Outcome};
use crate::draw::{fill, put, st};
use crate::theme::{self, Theme};

// ───────────────────────────── breadcrumbs ─────────────────────────────

/// Breadcrumb segments with collapsing and click handling.
#[derive(Clone, Debug)]
pub struct Breadcrumbs {
    segments: Vec<String>,
    separator: String,
    icons: Vec<Option<String>>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Breadcrumbs {
    pub fn new(segments: impl Into<Vec<String>>) -> Self {
        let segs = segments.into();
        Self {
            icons: vec![None; segs.len()],
            segments: segs,
            separator: "›".to_string(),
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn separator(mut self, s: &str) -> Self {
        self.separator = s.to_string();
        self
    }

    pub fn icons(mut self, icons: Vec<Option<String>>) -> Self {
        self.icons = icons;
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

impl<T: Into<String>> From<Vec<T>> for Breadcrumbs {
    fn from(v: Vec<T>) -> Self {
        Self::new(v.into_iter().map(Into::into).collect::<Vec<_>>())
    }
}

/// Breadcrumbs state: hit boxes, hover, clicked segment.
#[derive(Clone, Debug, Default)]
pub struct BreadcrumbsState {
    pub hits: Vec<(usize, Rect)>,
    pub hover: Option<usize>,
    pub clicked: Option<usize>,
}

impl BreadcrumbsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_clicked(&mut self) -> Option<usize> {
        self.clicked.take()
    }
}

impl Interactive for BreadcrumbsState {
    fn handle_key(&mut self, _: ratatui::crossterm::event::KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        let mut out = Outcome::Ignored;

        // hover
        let mut hover = None;
        for (i, r) in &self.hits {
            if r.contains(pos) {
                hover = Some(*i);
                break;
            }
        }
        if hover != self.hover {
            self.hover = hover;
            out = Outcome::Consumed;
        }

        // click
        if matches!(m.kind, MouseEventKind::Down(MouseButton::Left))
            && let Some(i) = hover {
                self.clicked = Some(i);
                return Outcome::Changed;
            }

        out
    }
}

impl StatefulWidget for Breadcrumbs {
    type State = BreadcrumbsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || self.segments.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        fill(buf, area, bg);

        state.hits.clear();
        state.hits.reserve(self.segments.len());

        // compute widths
        let sep_w = self.separator.width() as u16 + 2; // space around separator
        let seg_widths: Vec<u16> = self.segments.iter().enumerate().map(|(i, s)| {
            let mut w = s.width() as u16;
            if let Some(Some(icon)) = self.icons.get(i) {
                w += icon.width() as u16 + 1;
            }
            w
        }).collect();

        let total: u16 = seg_widths.iter().sum::<u16>() + sep_w * (self.segments.len().saturating_sub(1)) as u16;

        // collapse middle if too wide
        let mut visible: Vec<(usize, &str)> = self.segments.iter().enumerate().map(|(i, s)| (i, s.as_str())).collect();
        if total > area.width && self.segments.len() > 2 {
            // keep first and last, collapse middle to "…"
            let first_w = seg_widths[0];
            let last_w = seg_widths[self.segments.len() - 1];
            let needed = first_w + sep_w + 1 + sep_w + last_w; // "first › … › last"
            if needed <= area.width {
                visible = vec![(0, self.segments[0].as_str()), (usize::MAX, "…"), (self.segments.len() - 1, self.segments.last().unwrap().as_str())];
            }
        }

        let mut x = area.x;
        for (vi, (i, seg)) in visible.iter().enumerate() {
            let is_last = vi == visible.len() - 1;
            let is_hover = state.hover == Some(*i);
            let is_ellipsis = *i == usize::MAX;

            let (_fg, style) = if is_last {
                (th.text, st(th.text, bg).add_modifier(Modifier::BOLD))
            } else if is_hover && !is_ellipsis {
                (th.primary, st(th.primary, bg).add_modifier(Modifier::UNDERLINED))
            } else {
                (th.text_muted, st(th.text_muted, bg))
            };

            if is_ellipsis {
                let w = put(buf, x, area.y, seg, area.right().saturating_sub(x), st(th.text_muted, bg));
                x += w;
            } else {
                let start_x = x;
                if let Some(Some(icon)) = self.icons.get(*i) {
                    let iw = put(buf, x, area.y, icon, area.right().saturating_sub(x), style);
                    x += iw + 1;
                }
                let w = put(buf, x, area.y, seg, area.right().saturating_sub(x), style);
                x += w;
                state.hits.push((*i, Rect::new(start_x, area.y, x - start_x, 1)));
            }

            if !is_last && x < area.right() {
                x += 1;
                put(buf, x, area.y, &self.separator, area.right().saturating_sub(x), st(th.text_muted, bg));
                x += self.separator.width() as u16 + 1;
            }
        }
    }
}

// ───────────────────────────── paginator ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginatorHit {
    Prev,
    Next,
    Page(usize),
}

/// Paginator widget for page-based navigation.
#[derive(Clone, Debug)]
pub struct Paginator {
    total: usize,
    window: usize,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Paginator {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            window: 2,
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn window(mut self, w: usize) -> Self {
        self.window = w;
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

/// Paginator state: current page, hit boxes.
#[derive(Clone, Debug)]
pub struct PaginatorState {
    pub page: usize,
    pub hits: Vec<(PaginatorHit, Rect)>,
}

impl PaginatorState {
    pub fn new() -> Self {
        Self {
            page: 0,
            hits: Vec::new(),
        }
    }
}

impl Default for PaginatorState {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactive for PaginatorState {
    fn handle_key(&mut self, k: ratatui::crossterm::event::KeyEvent) -> Outcome {
        use ratatui::crossterm::event::{KeyCode, KeyEventKind};
        if k.kind != KeyEventKind::Press && k.kind != KeyEventKind::Repeat {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Left => {
                if self.page > 0 {
                    self.page -= 1;
                    return Outcome::Changed;
                }
            }
            KeyCode::Right => {
                self.page += 1;
                return Outcome::Changed;
            }
            KeyCode::Home => {
                if self.page != 0 {
                    self.page = 0;
                    return Outcome::Changed;
                }
            }
            KeyCode::End => {
                self.page = usize::MAX;
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        if !matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
            return Outcome::Ignored;
        }

        for (hit, r) in &self.hits {
            if r.contains(pos) {
                match hit {
                    PaginatorHit::Prev => {
                        if self.page > 0 {
                            self.page -= 1;
                            return Outcome::Changed;
                        }
                    }
                    PaginatorHit::Next => {
                        self.page += 1;
                        return Outcome::Changed;
                    }
                    PaginatorHit::Page(p) => {
                        if self.page != *p {
                            self.page = *p;
                            return Outcome::Changed;
                        }
                    }
                }
            }
        }

        Outcome::Ignored
    }
}

impl StatefulWidget for Paginator {
    type State = PaginatorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || self.total == 0 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        fill(buf, area, bg);

        state.page = state.page.min(self.total - 1);
        state.hits.clear();

        let pages = page_window(state.page, self.total, self.window);

        let mut x = area.x;

        // prev arrow
        if state.page > 0 {
            let w = put(buf, x, area.y, "‹", 1, st(th.accent, bg));
            state.hits.push((PaginatorHit::Prev, Rect::new(x, area.y, w, 1)));
            x += w + 1;
        }

        for p_opt in pages {
            if x >= area.right() {
                break;
            }

            match p_opt {
                Some(p) => {
                    let is_current = p == state.page;
                    let label = format!("{}", p + 1);
                    let (fg, page_bg) = if is_current {
                        (th.cursor_fg, th.primary)
                    } else {
                        (th.text_muted, bg)
                    };

                    let label_w = label.width() as u16;
                    if is_current {
                        fill(buf, Rect::new(x, area.y, label_w + 2, 1), page_bg);
                        put(buf, x + 1, area.y, &label, label_w, st(fg, page_bg).add_modifier(Modifier::BOLD));
                        state.hits.push((PaginatorHit::Page(p), Rect::new(x, area.y, label_w + 2, 1)));
                        x += label_w + 2;
                    } else {
                        let w = put(buf, x, area.y, &label, label_w, st(fg, page_bg));
                        state.hits.push((PaginatorHit::Page(p), Rect::new(x, area.y, w, 1)));
                        x += w;
                    }

                    x += 1;
                }
                None => {
                    put(buf, x, area.y, "…", 1, st(th.text_muted, bg));
                    x += 2;
                }
            }
        }

        // next arrow
        if state.page < self.total - 1
            && x < area.right() {
                let w = put(buf, x, area.y, "›", 1, st(th.accent, bg));
                state.hits.push((PaginatorHit::Next, Rect::new(x, area.y, w, 1)));
            }
    }
}

/// Compute visible page window: always show first, last, and `window` pages around current.
/// Returns `Some(page)` or `None` for ellipsis.
pub fn page_window(current: usize, total: usize, window: usize) -> Vec<Option<usize>> {
    if total == 0 {
        return vec![];
    }
    if total <= 2 * window + 5 {
        // show all
        return (0..total).map(Some).collect();
    }

    let mut pages = Vec::new();
    pages.push(Some(0));

    let start = current.saturating_sub(window);
    let end = (current + window + 1).min(total);

    if start > 1 {
        pages.push(None); // ellipsis
    }

    for p in start..end {
        if p > 0 && p < total - 1 {
            pages.push(Some(p));
        }
    }

    if end < total - 1 {
        pages.push(None); // ellipsis
    }

    pages.push(Some(total - 1));
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_window_shows_all_when_small() {
        let w = page_window(2, 7, 2);
        assert_eq!(w.len(), 7);
        assert!(w.iter().all(|x| x.is_some()));
    }

    #[test]
    fn page_window_collapses_middle() {
        let w = page_window(20, 42, 2);
        // should be [0, None, 18, 19, 20, 21, 22, None, 41]
        assert_eq!(w[0], Some(0));
        assert_eq!(w[1], None);
        assert!(w.contains(&Some(20)));
        assert_eq!(w.last(), Some(&Some(41)));
    }

    #[test]
    fn breadcrumbs_handles_click() {
        let mut state = BreadcrumbsState::new();
        state.hits.push((1, Rect::new(5, 0, 10, 1)));
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 0,
            modifiers: ratatui::crossterm::event::KeyModifiers::empty(),
        };
        assert_eq!(state.handle_mouse(m), Outcome::Changed);
        assert_eq!(state.take_clicked(), Some(1));
    }
}
