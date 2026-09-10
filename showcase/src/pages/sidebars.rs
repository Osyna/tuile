//! Showcase for sidebar navigation widget with 10 distinct styles.

use super::{Ctx, Page};
use std::time::Instant;
use tuiforge::draw::{put, st};
use tuiforge::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Sidebar,
}

pub struct SidebarsPage {
    focus: Focus<Id>,
    sidebar_states: Vec<SidebarState>,
    current_style: usize,
    collapsed: bool,
    side: Side,
    show_badges: bool,
}

impl SidebarsPage {
    fn styles() -> [SidebarStyle; 10] {
        [
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
        ]
    }

    fn style_name(style: SidebarStyle) -> &'static str {
        match style {
            SidebarStyle::Flat => "Flat",
            SidebarStyle::Accent => "Accent",
            SidebarStyle::Pill => "Pill",
            SidebarStyle::Boxed => "Boxed",
            SidebarStyle::Tabs => "Tabs",
            SidebarStyle::Minimal => "Minimal",
            SidebarStyle::Numbered => "Numbered",
            SidebarStyle::Rail => "Rail",
            SidebarStyle::Docked => "Docked",
            SidebarStyle::Tree => "Tree",
        }
    }

    fn make_groups(show_badges: bool) -> Vec<SidebarGroup> {
        vec![
            SidebarGroup::new("Workspace").items(vec![
                SidebarItem::new("Dashboard")
                    .icon("⌂")
                    .badge(if show_badges { "12" } else { "" }),
                SidebarItem::new("Projects").icon("◆"),
                SidebarItem::new("Tasks")
                    .icon("✓")
                    .badge(if show_badges { "3" } else { "" }),
                SidebarItem::new("Calendar").icon("≡"),
            ]),
            SidebarGroup::new("Tools").items(vec![
                SidebarItem::new("Analytics").icon("◉"),
                SidebarItem::new("Reports").icon("¶"),
                SidebarItem::new("Archive").icon("⊗").disabled(true),
            ]),
            SidebarGroup::new("Account").items(vec![
                SidebarItem::new("Profile").icon("◈"),
                SidebarItem::new("Billing")
                    .icon("⊙")
                    .badge(if show_badges { "•" } else { "" }),
            ]),
        ]
    }

    fn make_footer(show_badges: bool) -> Vec<SidebarItem> {
        vec![
            SidebarItem::new("Settings").icon("⌘"),
            SidebarItem::new("User")
                .icon("●")
                .badge(if show_badges { "•" } else { "" }),
        ]
    }
}

impl Default for SidebarsPage {
    fn default() -> Self {
        let styles = Self::styles();
        let sidebar_states: Vec<_> = (0..styles.len()).map(|_| SidebarState::new(1)).collect();
        Self {
            focus: Focus::new([Id::Sidebar]),
            sidebar_states,
            current_style: 0,
            collapsed: false,
            side: Side::Left,
            show_badges: true,
        }
    }
}

impl Page for SidebarsPage {
    fn title(&self) -> &'static str {
        "Sidebars"
    }

    fn subtitle(&self) -> &'static str {
        "Navigation sidebars: 10 distinct styles, collapsible groups, badges, footer"
    }

    fn icon(&self) -> &'static str {
        "◧"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = &ctx.theme;
        let styles = Self::styles();
        let cols = if area.width >= 100 {
            5
        } else if area.width >= 60 {
            3
        } else {
            2
        };
        let rows = styles.len().div_ceil(cols);

        use tuiforge::layout::columns;
        let col_rects = columns(area, cols, 1);
        let row_height = (area.height / rows.max(1) as u16).saturating_sub(1);

        for (i, style) in styles.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            if col >= col_rects.len() {
                break;
            }

            let col_area = col_rects[col];
            let y = col_area.y + row as u16 * (row_height + 1);
            if y >= area.bottom() {
                break;
            }

            let card_area = Rect {
                x: col_area.x,
                y,
                width: col_area.width,
                height: row_height.min(area.bottom().saturating_sub(y)),
            };
            if card_area.width < 4 || card_area.height < 4 {
                continue;
            }

            let caption = Self::style_name(*style);
            if card_area.y < buf.area.bottom() {
                put(
                    buf,
                    card_area.x,
                    card_area.y,
                    caption,
                    card_area.width,
                    st(th.text_muted, th.background),
                );
            }

            let sidebar_area = Rect {
                x: card_area.x,
                y: card_area.y.saturating_add(1),
                width: card_area.width,
                height: card_area.height.saturating_sub(1),
            };
            if sidebar_area.height == 0 {
                continue;
            }

            let is_active = i == self.current_style;
            let state = &mut self.sidebar_states[i];
            let groups = Self::make_groups(self.show_badges);
            let footer = Self::make_footer(self.show_badges);

            Sidebar::new()
                .groups(groups)
                .style(*style)
                .header("tuiforge", "v0.1")
                .footer(footer)
                .side(if is_active { self.side } else { Side::Left })
                .collapsed(if is_active { self.collapsed } else { false })
                .focused(is_active && self.focus.is(Id::Sidebar))
                .theme(th)
                .now(ctx.now)
                .render(sidebar_area, buf, state);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => {
                match k.code {
                    KeyCode::Char('s') => {
                        self.current_style = (self.current_style + 1) % Self::styles().len();
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('c') => {
                        self.collapsed = !self.collapsed;
                        let state = &mut self.sidebar_states[self.current_style];
                        state.set_collapsed(self.collapsed, ctx.now, ctx.dur(200));
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('r') => {
                        self.side = match self.side {
                            Side::Left => Side::Right,
                            Side::Right => Side::Left,
                        };
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('b') => {
                        self.show_badges = !self.show_badges;
                        return Outcome::Consumed;
                    }
                    _ => {}
                }

                if self.focus.is(Id::Sidebar) {
                    let state = &mut self.sidebar_states[self.current_style];
                    let out = state.handle_key(*k);
                    if let Some(idx) = state.take_activated() {
                        let label = self.get_label_at_index(idx);
                        ctx.notify(format!("Sidebar: {}", label), Variant::Primary);
                        return Outcome::Changed;
                    }
                    return out;
                }
            }
            Event::Mouse(m) => {
                let mut out = Outcome::Ignored;
                for (i, state) in self.sidebar_states.iter_mut().enumerate() {
                    let o = state.handle_mouse(*m);
                    if o.is_changed()
                        && i == self.current_style
                        && let Some(idx) = state.take_activated()
                    {
                        let label = self.get_label_at_index(idx);
                        ctx.notify(format!("Sidebar: {}", label), Variant::Primary);
                        return Outcome::Changed;
                    }
                    out |= o;
                }
                return out;
            }
            _ => {}
        }
        Outcome::Ignored
    }

    fn animating(&self, now: Instant) -> bool {
        self.sidebar_states.iter().any(|s| s.animating(now))
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("s", "cycle style"),
            ("c", "toggle collapse"),
            ("r", "toggle side"),
            ("b", "toggle badges"),
            ("↑/↓", "navigate"),
            ("Enter", "activate"),
        ]
    }
}

impl SidebarsPage {
    fn get_label_at_index(&self, idx: usize) -> String {
        let groups = Self::make_groups(self.show_badges);
        let footer = Self::make_footer(self.show_badges);
        let mut flat_index = 0;

        for group in &groups {
            if group.title.is_some() {
                if flat_index == idx {
                    return group.title.clone().unwrap_or_default();
                }
                flat_index += 1;
            }
            for item in &group.items {
                if flat_index == idx {
                    return item.label.clone();
                }
                flat_index += 1;
            }
        }
        for item in &footer {
            if flat_index == idx {
                return item.label.clone();
            }
            flat_index += 1;
        }
        String::from("Unknown")
    }
}
