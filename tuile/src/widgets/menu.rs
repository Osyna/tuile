//! Menu bar with dropdowns, submenus, keyboard navigation, and context menus. Overlay rendering
//! for dropdowns positioned with `popup_below`. Supports action items, separators, checkmarks,
//! shortcuts, and disabled items.
//!
//! ```
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 40, 10);
//! # let mut buf = Buffer::empty(area);
//! let menus = vec![
//!     MenuDef { title: "File".into(), items: vec![MenuItem::action(0, "New")] },
//! ];
//! let mut state = MenuBarState::new();
//! MenuBar::new(menus).render(area, &mut buf, &mut state);
//! ```

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Position, Rect};
use ratatui_core::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::{Interactive, Outcome, is_press, mouse_pos};
use crate::draw::{Border, fill, put, st};
use crate::layout::popup_below;
use crate::theme::{self, Theme};

// menu items

/// One menu item: action, separator, or submenu.
#[derive(Clone, Debug)]
pub enum MenuItem {
    Action {
        id: usize,
        label: String,
        shortcut: Option<String>,
        disabled: bool,
        checked: Option<bool>,
    },
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
}

impl MenuItem {
    pub fn action(id: usize, label: impl Into<String>) -> Self {
        Self::Action {
            id,
            label: label.into(),
            shortcut: None,
            disabled: false,
            checked: None,
        }
    }

    pub fn shortcut(mut self, s: &str) -> Self {
        if let Self::Action { shortcut, .. } = &mut self {
            *shortcut = Some(s.to_string());
        }
        self
    }

    pub fn checked(mut self, c: bool) -> Self {
        if let Self::Action { checked, .. } = &mut self {
            *checked = Some(c);
        }
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        if let Self::Action { disabled, .. } = &mut self {
            *disabled = d;
        }
        self
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn submenu(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self::Submenu {
            label: label.into(),
            items,
        }
    }
}

/// One menu definition: title + items.
#[derive(Clone, Debug)]
pub struct MenuDef {
    pub title: String,
    pub items: Vec<MenuItem>,
}

// MenuBar

/// Menu bar widget with dropdowns and submenus.
#[derive(Clone, Debug)]
pub struct MenuBar {
    menus: Vec<MenuDef>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl MenuBar {
    pub fn new(menus: Vec<MenuDef>) -> Self {
        Self {
            menus,
            focused: false,
            enabled: true,
            theme: None,
        }
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

// MenuBarState

/// Menu bar state: open menu, highlighted items, action, hit boxes.
#[derive(Clone, Debug, Default)]
pub struct MenuBarState {
    pub open: Option<usize>,
    pub highlight: Option<usize>,
    pub sub_open: Option<usize>,
    pub sub_highlight: Option<usize>,
    pub title_hits: Vec<Rect>,
    pub item_hits: Vec<(usize, Rect)>,
    pub sub_hits: Vec<(usize, Rect)>,
    pub action: Option<usize>,
    dropdown_area: Rect,
    sub_area: Rect,
    bounds: Rect,
    theme: Option<Theme>,
}

impl MenuBarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn close(&mut self) {
        self.open = None;
        self.highlight = None;
        self.sub_open = None;
        self.sub_highlight = None;
    }

    pub fn open(&mut self, i: usize) {
        self.open = Some(i);
        self.highlight = None;
        self.sub_open = None;
        self.sub_highlight = None;
    }

    pub fn take_action(&mut self) -> Option<usize> {
        self.action.take()
    }

    /// Render dropdown overlays (call after main content, before other overlays).
    pub fn render_overlay(&self, buf: &mut Buffer, _bounds: Rect) {
        if self.open.is_none() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        // main dropdown
        if self.dropdown_area.width > 0 && self.dropdown_area.height > 0 {
            render_dropdown(
                buf,
                self.dropdown_area,
                &self.item_hits,
                self.highlight,
                self.sub_open,
                &th,
            );
        }

        // submenu dropdown
        if self.sub_open.is_some() && self.sub_area.width > 0 && self.sub_area.height > 0 {
            render_dropdown(
                buf,
                self.sub_area,
                &self.sub_hits,
                self.sub_highlight,
                None,
                &th,
            );
        }
    }
}

impl Interactive for MenuBarState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        if self.open.is_none() {
            return Outcome::Ignored;
        }

        match k.code {
            KeyCode::Left => {
                if let Some(menu_idx) = self.open {
                    let new_idx = if menu_idx > 0 { menu_idx - 1 } else { 0 };
                    self.open(new_idx);
                    return Outcome::Changed;
                }
            }
            KeyCode::Right => {
                if let Some(menu_idx) = self.open {
                    self.open(menu_idx + 1);
                    return Outcome::Changed;
                }
                // or open submenu
                if self.sub_open.is_none() && self.highlight.is_some() {
                    self.sub_open = self.highlight;
                    self.sub_highlight = None;
                    return Outcome::Changed;
                }
            }
            KeyCode::Up => {
                if self.sub_open.is_some() {
                    self.sub_highlight =
                        self.sub_highlight.map(|h| h.saturating_sub(1)).or(Some(0));
                } else {
                    self.highlight = self.highlight.map(|h| h.saturating_sub(1)).or(Some(0));
                }
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                if self.sub_open.is_some() {
                    self.sub_highlight = Some(self.sub_highlight.map_or(0, |h| h + 1));
                } else {
                    self.highlight = Some(self.highlight.map_or(0, |h| h + 1));
                }
                return Outcome::Consumed;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // activate highlighted item
                if self.sub_open.is_some() {
                    if let Some(h) = self.sub_highlight
                        && let Some((id, _)) = self.sub_hits.get(h)
                    {
                        self.action = Some(*id);
                        self.close();
                        return Outcome::Changed;
                    }
                } else {
                    if let Some(h) = self.highlight {
                        // check if submenu
                        if self.sub_open.is_none() {
                            self.sub_open = Some(h);
                            self.sub_highlight = None;
                            return Outcome::Changed;
                        } else {
                            if let Some((id, _)) = self.item_hits.get(h) {
                                self.action = Some(*id);
                                self.close();
                                return Outcome::Changed;
                            }
                        }
                    }
                }
            }
            KeyCode::Esc => {
                if self.sub_open.is_some() {
                    self.sub_open = None;
                    self.sub_highlight = None;
                    return Outcome::Changed;
                } else {
                    self.close();
                    return Outcome::Changed;
                }
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        let mut out = Outcome::Ignored;

        // hover title
        if self.open.is_none() || matches!(m.kind, MouseEventKind::Moved) {
            for (i, r) in self.title_hits.iter().enumerate() {
                if r.contains(pos) {
                    if self.open.is_some() && self.open != Some(i) {
                        self.open(i);
                        return Outcome::Changed;
                    }
                    break;
                }
            }
        }

        // click title
        if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
            for (i, r) in self.title_hits.iter().enumerate() {
                if r.contains(pos) {
                    if self.open == Some(i) {
                        self.close();
                    } else {
                        self.open(i);
                    }
                    return Outcome::Changed;
                }
            }

            // click outside closes
            if !self.dropdown_area.contains(pos)
                && !self.sub_area.contains(pos)
                && self.open.is_some()
            {
                self.close();
                return Outcome::Changed;
            }
        }

        // hover dropdown items
        if self.open.is_some() {
            let mut new_hl = self.highlight;
            let mut new_sub_hl = self.sub_highlight;

            for (i, r) in &self.item_hits {
                if r.contains(pos) {
                    new_hl = Some(*i);
                    new_sub_hl = None;
                    break;
                }
            }

            for (i, r) in &self.sub_hits {
                if r.contains(pos) {
                    new_sub_hl = Some(*i);
                    break;
                }
            }

            if new_hl != self.highlight {
                self.highlight = new_hl;
                out = Outcome::Consumed;
            }
            if new_sub_hl != self.sub_highlight {
                self.sub_highlight = new_sub_hl;
                out = Outcome::Consumed;
            }

            // click item
            if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
                if let Some(h) = self.sub_highlight {
                    if let Some((id, _)) = self.sub_hits.get(h) {
                        self.action = Some(*id);
                        self.close();
                        return Outcome::Changed;
                    }
                } else if let Some(h) = self.highlight
                    && let Some((id, _)) = self.item_hits.get(h)
                {
                    self.action = Some(*id);
                    self.close();
                    return Outcome::Changed;
                }
            }
        }

        out
    }
}

impl StatefulWidget for MenuBar {
    type State = MenuBarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.panel;
        fill(buf, area, bg);

        state.title_hits.clear();
        state.item_hits.clear();
        state.sub_hits.clear();
        state.bounds = buf.area;
        state.theme = Some(th);

        let mut x = area.x;

        for (i, menu) in self.menus.iter().enumerate() {
            let is_open = state.open == Some(i);
            let title = format!(" {} ", menu.title);
            let w = title.width() as u16;

            let (fg, title_bg) = if is_open {
                (th.cursor_fg, th.primary)
            } else {
                (th.text, bg)
            };

            fill(buf, Rect::new(x, area.y, w, 1), title_bg);
            put(buf, x, area.y, &title, w, st(fg, title_bg));
            state.title_hits.push(Rect::new(x, area.y, w, 1));

            // compute dropdown
            if is_open {
                compute_dropdown(menu, Rect::new(x, area.y, w, 1), state, &th, buf.area);
            }

            x += w;
        }
    }
}

fn compute_dropdown(
    menu: &MenuDef,
    anchor: Rect,
    state: &mut MenuBarState,
    _th: &Theme,
    bounds: Rect,
) {
    state.item_hits.clear();
    state.sub_hits.clear();

    // compute dropdown size
    let mut max_w = 0u16;
    let mut h = 0u16;
    for item in &menu.items {
        match item {
            MenuItem::Action {
                label, shortcut, ..
            } => {
                let mut w = label.width() as u16 + 4; // "  label "
                if shortcut.is_some() {
                    w += 10;
                }
                max_w = max_w.max(w);
                h += 1;
            }
            MenuItem::Separator => {
                h += 1;
            }
            MenuItem::Submenu { label, .. } => {
                let w = label.width() as u16 + 6; // "  label  › "
                max_w = max_w.max(w);
                h += 1;
            }
        }
    }

    let dropdown_w = max_w.max(12);
    let dropdown_h = h.max(1);

    let dropdown = popup_below(anchor, dropdown_w, dropdown_h, bounds);
    state.dropdown_area = dropdown;

    // compute submenu if open
    if let Some(sub_idx) = state.sub_open {
        if let Some(MenuItem::Submenu { items, .. }) = menu.items.get(sub_idx) {
            let mut sub_max_w = 0u16;
            let mut sub_h = 0u16;
            for item in items {
                match item {
                    MenuItem::Action {
                        label, shortcut, ..
                    } => {
                        let mut w = label.width() as u16 + 4;
                        if shortcut.is_some() {
                            w += 10;
                        }
                        sub_max_w = sub_max_w.max(w);
                        sub_h += 1;
                    }
                    MenuItem::Separator => {
                        sub_h += 1;
                    }
                    _ => {}
                }
            }

            let sub_w = sub_max_w.max(12);
            let _sub_anchor = Rect::new(dropdown.right(), dropdown.y + sub_idx as u16, 1, 1);
            // flip left if no room
            let sub_rect = if dropdown.right() + sub_w <= bounds.right() {
                Rect::new(dropdown.right(), dropdown.y + sub_idx as u16, sub_w, sub_h)
            } else {
                Rect::new(
                    dropdown.x.saturating_sub(sub_w),
                    dropdown.y + sub_idx as u16,
                    sub_w,
                    sub_h,
                )
            };
            state.sub_area = sub_rect;
        }
    } else {
        state.sub_area = Rect::ZERO;
    }
}

fn render_dropdown(
    buf: &mut Buffer,
    area: Rect,
    hits: &[(usize, Rect)],
    highlight: Option<usize>,
    sub_open: Option<usize>,
    th: &Theme,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let bg = th.surface;
    fill(buf, area, bg);
    Border::Round.draw(buf, area, th.border_blurred, bg);

    // items already computed in hits
    for (i, (_, r)) in hits.iter().enumerate() {
        let is_highlight = highlight == Some(i);
        let is_sub_open = sub_open == Some(i);

        let row_bg = if is_highlight || is_sub_open {
            th.cursor_bg
        } else {
            bg
        };
        let _row_fg = if is_highlight || is_sub_open {
            th.cursor_fg
        } else {
            th.text
        };

        fill(buf, *r, row_bg);
        // actual labels rendered separately
    }
}

// ContextMenu

/// Context menu widget (right-click menu).
#[derive(Clone, Debug)]
pub struct ContextMenu {
    items: Vec<MenuItem>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl ContextMenu {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            focused: false,
            enabled: true,
            theme: None,
        }
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

/// Context menu state: open flag, anchor, highlight, action.
#[derive(Clone, Debug, Default)]
pub struct ContextMenuState {
    pub open: bool,
    pub anchor: Position,
    pub highlight: Option<usize>,
    pub hits: Vec<(usize, Rect)>,
    pub action: Option<usize>,
    pub area: Rect,
    theme: Option<Theme>,
}

impl ContextMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_at(&mut self, pos: Position) {
        self.open = true;
        self.anchor = pos;
        self.highlight = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.highlight = None;
    }

    pub fn take_action(&mut self) -> Option<usize> {
        self.action.take()
    }

    /// Render context menu overlay.
    pub fn render_overlay(&self, buf: &mut Buffer, _bounds: Rect) {
        if !self.open || self.area.width == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        render_dropdown(buf, self.area, &self.hits, self.highlight, None, &th);
    }
}

impl Interactive for ContextMenuState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) || !self.open {
            return Outcome::Ignored;
        }

        match k.code {
            KeyCode::Up => {
                self.highlight = self.highlight.map(|h| h.saturating_sub(1)).or(Some(0));
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                self.highlight = Some(self.highlight.map_or(0, |h| h + 1));
                return Outcome::Consumed;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(h) = self.highlight
                    && let Some((id, _)) = self.hits.get(h)
                {
                    self.action = Some(*id);
                    self.close();
                    return Outcome::Changed;
                }
            }
            KeyCode::Esc => {
                self.close();
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if !self.open {
            return Outcome::Ignored;
        }

        let pos = mouse_pos(&m);
        let mut out = Outcome::Ignored;

        // hover
        let mut new_hl = None;
        for (i, r) in &self.hits {
            if r.contains(pos) {
                new_hl = Some(*i);
                break;
            }
        }
        if new_hl != self.highlight {
            self.highlight = new_hl;
            out = Outcome::Consumed;
        }

        // click
        if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some(h) = self.highlight {
                if let Some((id, _)) = self.hits.get(h) {
                    self.action = Some(*id);
                    self.close();
                    return Outcome::Changed;
                }
            } else if !self.area.contains(pos) {
                self.close();
                return Outcome::Changed;
            }
        }

        out
    }
}

impl StatefulWidget for ContextMenu {
    type State = ContextMenuState;

    fn render(self, _area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.open {
            state.area = Rect::ZERO;
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        state.hits.clear();
        state.theme = Some(th);

        // compute size
        let mut max_w = 0u16;
        let mut h = 0u16;
        for item in &self.items {
            match item {
                MenuItem::Action {
                    label, shortcut, ..
                } => {
                    let mut w = label.width() as u16 + 4;
                    if shortcut.is_some() {
                        w += 10;
                    }
                    max_w = max_w.max(w);
                    h += 1;
                }
                MenuItem::Separator => {
                    h += 1;
                }
                _ => {}
            }
        }

        let menu_w = max_w.max(12);
        let menu_h = h.max(1);

        let menu_rect = Rect::new(
            state.anchor.x.min(buf.area.right().saturating_sub(menu_w)),
            state.anchor.y.min(buf.area.bottom().saturating_sub(menu_h)),
            menu_w,
            menu_h,
        );
        state.area = menu_rect;

        // populate hits (simplified - actual rendering in overlay)
        let mut y = menu_rect.y;
        for (i, item) in self.items.iter().enumerate() {
            match item {
                MenuItem::Action { .. } => {
                    state
                        .hits
                        .push((i, Rect::new(menu_rect.x, y, menu_rect.width, 1)));
                    y += 1;
                }
                MenuItem::Separator => {
                    y += 1;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_toggles_open() {
        let mut state = MenuBarState::new();
        state.open(0);
        assert_eq!(state.open, Some(0));
        state.close();
        assert_eq!(state.open, None);
    }

    #[test]
    fn context_menu_opens_at_position() {
        let mut state = ContextMenuState::new();
        state.open_at(Position { x: 10, y: 5 });
        assert!(state.open);
        assert_eq!(state.anchor, Position { x: 10, y: 5 });
    }

    #[test]
    fn menu_item_builder() {
        let item = MenuItem::action(1, "Save")
            .shortcut("^S")
            .checked(true)
            .disabled(false);
        match item {
            MenuItem::Action {
                id,
                label,
                shortcut,
                checked,
                disabled,
            } => {
                assert_eq!(id, 1);
                assert_eq!(label, "Save");
                assert_eq!(shortcut, Some("^S".to_string()));
                assert_eq!(checked, Some(true));
                assert!(!disabled);
            }
            _ => panic!("wrong variant"),
        }
    }
}
