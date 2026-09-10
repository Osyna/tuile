//! App-level navigation sidebars with 10 distinct styles.

use crate::anim::Tween;
use crate::core::{
    Hit, HitBox, Interactive, Outcome, is_press, mouse_pos, plain_char, wheel_delta,
};
use crate::draw::{self, Border, bold, fill, put, st, truncate};
use crate::layout::pad;
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub struct SidebarItem {
    pub label: String,
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub shortcut: Option<String>,
    pub disabled: bool,
}

impl SidebarItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            badge: None,
            shortcut: None,
            disabled: false,
        }
    }
    pub fn icon(mut self, i: &str) -> Self {
        self.icon = Some(i.to_string());
        self
    }
    pub fn badge(mut self, b: impl Into<String>) -> Self {
        self.badge = Some(b.into());
        self
    }
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = Some(s.to_string());
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

impl From<&str> for SidebarItem {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

#[derive(Clone, Debug)]
pub struct SidebarGroup {
    pub title: Option<String>,
    pub items: Vec<SidebarItem>,
    pub collapsible: bool,
}

impl SidebarGroup {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            items: Vec::new(),
            collapsible: true,
        }
    }
    pub fn items(mut self, items: Vec<SidebarItem>) -> Self {
        self.items = items;
        self
    }
    pub fn collapsible(mut self, c: bool) -> Self {
        self.collapsible = c;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarStyle {
    #[default]
    Flat,
    Accent,
    Pill,
    Boxed,
    Tabs,
    Minimal,
    Numbered,
    Rail,
    Docked,
    Tree,
}

#[derive(Clone, Debug)]
pub struct Sidebar {
    groups: Vec<SidebarGroup>,
    footer: Vec<SidebarItem>,
    header_title: Option<String>,
    header_subtitle: Option<String>,
    side: Side,
    width: u16,
    collapsed: bool,
    show_badges: bool,
    style: SidebarStyle,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
    now: Option<Instant>,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            footer: Vec::new(),
            header_title: None,
            header_subtitle: None,
            side: Side::Left,
            width: 24,
            collapsed: false,
            show_badges: true,
            style: SidebarStyle::default(),
            focused: false,
            enabled: true,
            theme: None,
            now: None,
        }
    }
    pub fn groups(mut self, groups: Vec<SidebarGroup>) -> Self {
        self.groups = groups;
        self
    }
    pub fn items(mut self, items: Vec<SidebarItem>) -> Self {
        self.groups = vec![SidebarGroup {
            title: None,
            items,
            collapsible: false,
        }];
        self
    }
    pub fn footer(mut self, footer: Vec<SidebarItem>) -> Self {
        self.footer = footer;
        self
    }
    pub fn header(mut self, title: &str, subtitle: &str) -> Self {
        self.header_title = Some(title.to_string());
        self.header_subtitle = Some(subtitle.to_string());
        self
    }
    pub fn side(mut self, s: Side) -> Self {
        self.side = s;
        self
    }
    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self
    }
    pub fn collapsed(mut self, c: bool) -> Self {
        self.collapsed = c;
        self
    }
    pub fn show_badges(mut self, b: bool) -> Self {
        self.show_badges = b;
        self
    }
    pub fn style(mut self, s: SidebarStyle) -> Self {
        self.style = s;
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
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct SidebarState {
    pub active: usize,
    pub cursor: usize,
    pub hit: HitBox,
    hits: Vec<Rect>,
    collapsed_groups: HashSet<usize>,
    scroll: ScrollbarState,
    width_tween: Tween,
    activated: Option<usize>,
}

impl SidebarState {
    pub fn new(active: usize) -> Self {
        Self {
            active,
            cursor: active,
            hit: HitBox::default(),
            hits: Vec::new(),
            collapsed_groups: HashSet::new(),
            scroll: ScrollbarState::new(),
            width_tween: Tween::new(1.0),
            activated: None,
        }
    }
    pub fn animating(&self, now: Instant) -> bool {
        self.width_tween.active(now)
    }
    pub fn set_collapsed(&mut self, collapsed: bool, now: Instant, dur: Duration) {
        self.width_tween
            .go(if collapsed { 0.0 } else { 1.0 }, now, dur);
    }
    pub fn current_width(&self, now: Instant, full: u16, rail: u16) -> u16 {
        let rail = rail.min(full);
        rail + ((full - rail) as f32 * self.width_tween.value(now)) as u16
    }
    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }
    pub fn toggle_group(&mut self, gi: usize) {
        if self.collapsed_groups.contains(&gi) {
            self.collapsed_groups.remove(&gi);
        } else {
            self.collapsed_groups.insert(gi);
        }
    }
}

impl Interactive for SidebarState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                Outcome::Consumed
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.hits.len() {
                    self.cursor += 1;
                }
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.cursor = 0;
                Outcome::Consumed
            }
            KeyCode::End => {
                self.cursor = self.hits.len().saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.activated = Some(self.cursor);
                self.active = self.cursor;
                Outcome::Changed
            }
            _ => {
                if let Some(c) = plain_char(&k)
                    && ('1'..='9').contains(&c)
                {
                    let idx = (c as usize - '1' as usize).min(self.hits.len().saturating_sub(1));
                    self.cursor = idx;
                    self.activated = Some(idx);
                    self.active = idx;
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
        }
    }
    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let scroll_out = self.scroll.handle_mouse(m);
        if scroll_out.is_consumed() || scroll_out.is_changed() {
            return scroll_out;
        }
        if let Some(delta) = wheel_delta(&m) {
            self.scroll.scroll_by(delta as i64);
            return Outcome::Consumed;
        }
        match self.hit.mouse(&m) {
            Hit::Press | Hit::Click => {
                let pos = mouse_pos(&m);
                for (i, r) in self.hits.iter().enumerate() {
                    if r.contains(pos) {
                        self.cursor = i;
                        self.activated = Some(i);
                        self.active = i;
                        return Outcome::Changed;
                    }
                }
                Outcome::Consumed
            }
            Hit::HoverChanged => Outcome::Consumed,
            _ => Outcome::Ignored,
        }
    }
}

#[derive(Clone, Debug)]
struct RowData {
    label: String,
    icon: Option<String>,
    badge: Option<String>,
    is_header: bool,
    disabled: bool,
    number: Option<usize>,
}

impl StatefulWidget for Sidebar {
    type State = SidebarState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 3 || area.height < 3 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);
        let rail_width = 5u16;
        let current_width = if self.collapsed || self.style == SidebarStyle::Rail {
            rail_width.min(area.width)
        } else {
            let full_width = area.width;
            state
                .current_width(now, full_width, rail_width)
                .min(area.width)
        };
        let sidebar_area = Rect {
            x: area.x,
            y: area.y,
            width: current_width,
            height: area.height,
        };
        state.hit.set_area(sidebar_area);
        let bg = match self.style {
            SidebarStyle::Minimal => th.background,
            SidebarStyle::Accent => th.panel.darken(0.1),
            _ => th.surface,
        };
        fill(buf, sidebar_area, bg);
        if self.style == SidebarStyle::Boxed {
            if let Some(title) = &self.header_title {
                let combined = format!(
                    "{} {}",
                    title,
                    self.header_subtitle.as_deref().unwrap_or("")
                );
                let _ = Border::Round.draw_titled(
                    buf,
                    sidebar_area,
                    th.border,
                    bg,
                    &combined,
                    ratatui::layout::Alignment::Left,
                );
            } else {
                Border::Round.draw(buf, sidebar_area, th.border, bg);
            }
        }
        let content_area = if self.style == SidebarStyle::Boxed {
            pad(sidebar_area, 1, 1)
        } else {
            pad(sidebar_area, 1, 0)
        };
        if content_area.height == 0 {
            return;
        }
        let mut rows = Vec::new();
        let mut num_counter = 1usize;
        for (gi, group) in self.groups.iter().enumerate() {
            if let Some(title) = &group.title {
                rows.push(RowData {
                    label: title.clone(),
                    icon: None,
                    badge: None,
                    is_header: true,
                    disabled: false,
                    number: None,
                });
            }
            if !state.collapsed_groups.contains(&gi) || !group.collapsible {
                for item in &group.items {
                    rows.push(RowData {
                        label: item.label.clone(),
                        icon: item.icon.clone(),
                        badge: if self.show_badges {
                            item.badge.clone()
                        } else {
                            None
                        },
                        is_header: false,
                        disabled: item.disabled,
                        number: if num_counter <= 9 {
                            Some(num_counter)
                        } else {
                            None
                        },
                    });
                    num_counter += 1;
                }
            }
        }
        let header_height = if self.header_title.is_some() && self.style != SidebarStyle::Boxed {
            2
        } else {
            0
        };
        let footer_height = self.footer.len() as u16;
        let content_height = content_area
            .height
            .saturating_sub(header_height + footer_height);
        let visible_area = Rect {
            x: content_area.x,
            y: content_area.y + header_height,
            width: content_area.width,
            height: content_height,
        };
        if let Some(title) = &self.header_title
            && content_area.y < buf.area.bottom()
            && self.style != SidebarStyle::Boxed
            && current_width > rail_width
        {
            put(
                buf,
                content_area.x,
                content_area.y,
                title,
                content_area.width,
                bold(st(th.text, bg)),
            );
            if let Some(sub) = &self.header_subtitle {
                put(
                    buf,
                    content_area.x,
                    content_area.y + 1,
                    sub,
                    content_area.width,
                    st(th.text_muted, bg),
                );
            }
        }
        let total_rows = rows.len();
        let viewport_rows = visible_area.height as usize;
        state.scroll.offset = keep_visible(state.scroll.offset, state.cursor, viewport_rows);
        state.hits.clear();
        state
            .hits
            .resize(rows.len() + self.footer.len(), Rect::default());
        let start_row = state.scroll.offset.min(total_rows.saturating_sub(1));
        let end_row = (start_row + viewport_rows).min(total_rows);
        let expanded = current_width > rail_width;
        for (i, row_idx) in (start_row..end_row).enumerate() {
            let y = visible_area.y + i as u16;
            if y >= visible_area.bottom() {
                break;
            }
            let row_rect = Rect {
                x: visible_area.x,
                y,
                width: visible_area.width,
                height: 1,
            };
            state.hits[row_idx] = row_rect;
            let row = &rows[row_idx];
            let is_active = row_idx == state.active;
            let is_cursor = row_idx == state.cursor;
            draw_row(
                buf,
                row_rect,
                row,
                is_active,
                is_cursor && self.focused,
                self.style,
                self.side,
                expanded,
                &th,
            );
        }
        let footer_y = content_area.bottom().saturating_sub(footer_height);
        for (fi, item) in self.footer.iter().enumerate() {
            let y = footer_y + fi as u16;
            if y < buf.area.bottom() && y >= visible_area.bottom() {
                let footer_rect = Rect {
                    x: visible_area.x,
                    y,
                    width: visible_area.width,
                    height: 1,
                };
                let footer_idx = rows.len() + fi;
                state.hits[footer_idx] = footer_rect;
                let row_data = RowData {
                    label: item.label.clone(),
                    icon: item.icon.clone(),
                    badge: if self.show_badges {
                        item.badge.clone()
                    } else {
                        None
                    },
                    is_header: false,
                    disabled: item.disabled,
                    number: None,
                };
                let is_active = footer_idx == state.active;
                let is_cursor = footer_idx == state.cursor;
                draw_row(
                    buf,
                    footer_rect,
                    &row_data,
                    is_active,
                    is_cursor && self.focused,
                    self.style,
                    self.side,
                    expanded,
                    &th,
                );
            }
        }
        if total_rows > viewport_rows && visible_area.width > 2 {
            let scrollbar_x = if self.side == Side::Right {
                visible_area.x
            } else {
                visible_area.right().saturating_sub(1)
            };
            let scrollbar_area = Rect {
                x: scrollbar_x,
                y: visible_area.y,
                width: 1,
                height: visible_area.height,
            };
            Scrollbar::vertical(total_rows, viewport_rows)
                .offset(state.scroll.offset)
                .theme(&th)
                .render(scrollbar_area, buf, &mut state.scroll);
        }
    }
}

fn draw_row(
    buf: &mut Buffer,
    area: Rect,
    row: &RowData,
    active: bool,
    _cursor: bool,
    style: SidebarStyle,
    side: Side,
    expanded: bool,
    th: &Theme,
) {
    if area.width == 0 {
        return;
    }
    let fg = if row.disabled {
        th.text_disabled
    } else if active && style == SidebarStyle::Minimal {
        th.primary
    } else if row.is_header {
        th.text_muted
    } else {
        th.text
    };
    let bg = match style {
        SidebarStyle::Flat | SidebarStyle::Numbered => {
            if active {
                th.cursor_bg
            } else {
                th.surface
            }
        }
        SidebarStyle::Accent => th.panel.darken(0.1),
        SidebarStyle::Pill => th.panel.darken(0.1),
        SidebarStyle::Minimal => th.background,
        SidebarStyle::Boxed | SidebarStyle::Tree => th.surface,
        SidebarStyle::Tabs => th.panel.darken(0.05),
        SidebarStyle::Rail => th.panel.darken(0.15),
        SidebarStyle::Docked => {
            if active {
                th.cursor_bg
            } else {
                th.surface
            }
        }
    };
    fill(buf, area, bg);
    if style == SidebarStyle::Pill && active && expanded && area.width > 4 {
        let pill_bg = th.primary.blend(bg, 0.6);
        let pill_fg = pill_bg.text_on(0.9);
        let pill_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        fill(buf, pill_area, pill_bg);
        if let Some(icon) = &row.icon {
            put(buf, pill_area.x, pill_area.y, icon, 2, st(pill_fg, pill_bg));
        }
        let label_x = pill_area.x + if row.icon.is_some() { 3 } else { 1 };
        let label_w = pill_area
            .width
            .saturating_sub(if row.icon.is_some() { 3 } else { 1 });
        let label_text = truncate(&row.label, label_w as usize);
        put(
            buf,
            label_x,
            pill_area.y,
            &label_text,
            label_w,
            st(pill_fg, pill_bg),
        );
        return;
    }
    if style == SidebarStyle::Tabs && active {
        fill(buf, area, th.background);
    }
    if !expanded && style != SidebarStyle::Rail {
        return;
    }
    let mut x = area.x;
    if style == SidebarStyle::Numbered
        && !row.is_header
        && let Some(num) = row.number
    {
        let num_fg = if active { th.primary } else { th.text_muted };
        let num_str = format!("{} ", num);
        put(
            buf,
            x,
            area.y,
            &num_str,
            2,
            st(
                num_fg,
                if style == SidebarStyle::Tabs && active {
                    th.background
                } else {
                    bg
                },
            ),
        );
        x += 2;
    }
    if let Some(icon) = &row.icon {
        if style == SidebarStyle::Rail && !expanded {
            let icon_x = area.x + (area.width / 2).saturating_sub(1);
            put(buf, icon_x, area.y, icon, 2, st(fg, bg));
        } else if style == SidebarStyle::Docked && expanded {
        } else if expanded {
            put(
                buf,
                x,
                area.y,
                icon,
                2,
                st(
                    fg,
                    if style == SidebarStyle::Tabs && active {
                        th.background
                    } else {
                        bg
                    },
                ),
            );
            x += 3;
        }
    } else if expanded && style != SidebarStyle::Rail && style != SidebarStyle::Docked {
        x += 3;
    }
    if expanded {
        let label_text = if row.is_header && style == SidebarStyle::Minimal {
            row.label.to_uppercase()
        } else {
            row.label.clone()
        };
        let text_style = if active
            && (style == SidebarStyle::Accent
                || style == SidebarStyle::Minimal
                || style == SidebarStyle::Tabs)
        {
            bold(st(
                fg,
                if style == SidebarStyle::Tabs && active {
                    th.background
                } else {
                    bg
                },
            ))
        } else {
            st(
                fg,
                if style == SidebarStyle::Tabs && active {
                    th.background
                } else {
                    bg
                },
            )
        };
        let badge_reserve = if row.badge.is_some() { 5 } else { 0 };
        let scrollbar_reserve = 1;
        let label_max_w = area
            .width
            .saturating_sub(x - area.x)
            .saturating_sub(badge_reserve + scrollbar_reserve);
        if style == SidebarStyle::Docked {
            let display_text = truncate(&label_text, label_max_w as usize);
            let label_start_x = area
                .right()
                .saturating_sub(display_text.len() as u16 + scrollbar_reserve + badge_reserve + 3);
            put(
                buf,
                label_start_x,
                area.y,
                &display_text,
                label_max_w,
                text_style,
            );
            if let Some(icon) = &row.icon {
                let icon_x = area
                    .right()
                    .saturating_sub(scrollbar_reserve + badge_reserve + 1);
                put(buf, icon_x, area.y, icon, 2, st(fg, bg));
            }
        } else {
            let display_text = truncate(&label_text, label_max_w as usize);
            put(buf, x, area.y, &display_text, label_max_w, text_style);
        }
        if let Some(badge) = &row.badge
            && !badge.is_empty()
            && badge != "0"
        {
            let badge_x = area.right().saturating_sub(scrollbar_reserve + 4);
            let badge_bg = th.error;
            let badge_fg = badge_bg.text_on(0.9);
            let badge_text = if badge == "•" {
                badge.clone()
            } else {
                let num: usize = badge.parse().unwrap_or(0);
                if num > 99 {
                    "99+".to_string()
                } else {
                    format!("{:>2}", num)
                }
            };
            fill(
                buf,
                Rect {
                    x: badge_x,
                    y: area.y,
                    width: 3,
                    height: 1,
                },
                badge_bg,
            );
            put(buf, badge_x, area.y, &badge_text, 3, st(badge_fg, badge_bg));
        }
    }
    if style == SidebarStyle::Accent && active && expanded {
        let rail_x = if side == Side::Right {
            area.right().saturating_sub(1)
        } else {
            area.x
        };
        draw::put_cell(buf, rail_x, area.y, "┃", st(th.primary, bg));
    }
    if style == SidebarStyle::Docked && active && side == Side::Right {
        let rail_x = area.right().saturating_sub(1);
        draw::put_cell(buf, rail_x, area.y, "┃", st(th.primary, bg));
    }
    if style == SidebarStyle::Tabs {
        let edge_x = if side == Side::Right {
            area.x
        } else {
            area.right().saturating_sub(1)
        };
        if active {
            draw::put_cell(buf, edge_x, area.y, " ", st(fg, th.background));
        } else {
            draw::put_cell(buf, edge_x, area.y, "│", st(th.border, bg));
        }
    }
    if style == SidebarStyle::Tree && row.is_header && expanded {
        draw::put_cell(buf, area.x, area.y, "▾", st(th.text_muted, bg));
    }
    if style == SidebarStyle::Minimal && active && !row.is_header && expanded {
        draw::put_cell(buf, area.x, area.y, "▸", st(th.primary, bg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn sidebar_renders_without_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 20));
        let mut state = SidebarState::new(0);
        let groups = vec![SidebarGroup::new("Test").items(vec![SidebarItem::new("Item 1")])];
        Sidebar::new()
            .groups(groups)
            .render(buf.area, &mut buf, &mut state);
    }

    #[test]
    fn sidebar_nav_moves_cursor() {
        let mut state = SidebarState::new(0);
        state.hits = vec![
            Rect::new(0, 0, 20, 1),
            Rect::new(0, 1, 20, 1),
            Rect::new(0, 2, 20, 1),
        ];
        state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(state.cursor, 1);
        state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(state.cursor, 2);
        state.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn sidebar_activation_sets_flag() {
        let mut state = SidebarState::new(0);
        state.hits = vec![Rect::new(0, 0, 20, 1)];
        state.cursor = 0;
        state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(state.take_activated(), Some(0));
        assert_eq!(state.take_activated(), None);
    }

    #[test]
    fn sidebar_collapse_tween_animates() {
        let mut state = SidebarState::new(0);
        let now = Instant::now();
        state.set_collapsed(true, now, Duration::from_millis(200));
        assert!(state.animating(now));
        let later = now + Duration::from_millis(300);
        assert!(!state.animating(later));
        let w = state.current_width(later, 24, 5);
        assert_eq!(w, 5);
    }

    #[test]
    fn sidebar_number_keys_jump() {
        let mut state = SidebarState::new(0);
        state.hits = vec![
            Rect::new(0, 0, 20, 1),
            Rect::new(0, 1, 20, 1),
            Rect::new(0, 2, 20, 1),
        ];
        state.handle_key(KeyEvent::from(KeyCode::Char('3')));
        assert_eq!(state.cursor, 2);
        assert_eq!(state.take_activated(), Some(2));
    }

    #[test]
    fn sidebar_mouse_click_activates() {
        let mut state = SidebarState::new(0);
        let area = Rect::new(0, 0, 20, 10);
        state.hit.set_area(area);
        state.hits = vec![Rect::new(0, 0, 20, 1), Rect::new(0, 1, 20, 1)];
        let m = MouseEvent {
            kind: ratatui::crossterm::event::MouseEventKind::Down(
                ratatui::crossterm::event::MouseButton::Left,
            ),
            column: 5,
            row: 1,
            modifiers: ratatui::crossterm::event::KeyModifiers::empty(),
        };
        state.handle_mouse(m);
        assert_eq!(state.cursor, 1);
        assert_eq!(state.take_activated(), Some(1));
    }

    #[test]
    fn sidebar_all_styles_render_without_panic() {
        let styles = [
            SidebarStyle::Flat,
            SidebarStyle::Accent,
            SidebarStyle::Pill,
            SidebarStyle::Boxed,
            SidebarStyle::Tabs,
            SidebarStyle::Minimal,
            SidebarStyle::Numbered,
            SidebarStyle::Rail,
            SidebarStyle::Docked,
            SidebarStyle::Tree,
        ];
        for style in styles {
            let mut buf = Buffer::empty(Rect::new(0, 0, 30, 20));
            let mut state = SidebarState::new(0);
            let groups = vec![SidebarGroup::new("Test").items(vec![
                SidebarItem::new("Item 1").icon("⌂"),
                SidebarItem::new("Item 2").icon("⌘"),
            ])];
            Sidebar::new()
                .groups(groups)
                .style(style)
                .render(buf.area, &mut buf, &mut state);
        }
    }

    #[test]
    fn sidebar_both_sides_render_without_panic() {
        for side in [Side::Left, Side::Right] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 30, 20));
            let mut state = SidebarState::new(0);
            let groups =
                vec![SidebarGroup::new("Test").items(vec![SidebarItem::new("Item").icon("⌂")])];
            Sidebar::new()
                .groups(groups)
                .side(side)
                .render(buf.area, &mut buf, &mut state);
        }
    }

    #[test]
    fn sidebar_tiny_sizes_no_panic() {
        for w in 1..=10 {
            for h in 1..=10 {
                let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
                let mut state = SidebarState::new(0);
                Sidebar::new()
                    .groups(vec![
                        SidebarGroup::new("T").items(vec![SidebarItem::new("I").icon("⌂")]),
                    ])
                    .render(buf.area, &mut buf, &mut state);
            }
        }
    }
}
