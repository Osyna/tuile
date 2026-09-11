//! Application header with icon, title, subtitle, clock, and action buttons.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 80, 3);
//! # let mut buf = Buffer::empty(area);
//! let mut state = AppHeaderState::default();
//! AppHeader::new().title("Settings").icon("⊛").clock(true).render(area, &mut buf, &mut state);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::core::{Hit, HitBox, Outcome};
use crate::draw::{fill, put, put_centered, put_right, st};
use crate::runtime::local_hms;
use crate::theme::{self, Theme};

/// State for the app header: hit tracking for icon and actions.
#[derive(Clone, Debug, Default)]
pub struct AppHeaderState {
    /// HitBoxes for actions (right-aligned clickable labels).
    pub hits: Vec<HitBox>,
    /// Index of the hovered action.
    pub hover: Option<usize>,
    /// Icon hitbox.
    icon_hit: HitBox,
    /// Index of the action that was clicked (take with `take_action`).
    pressed: Option<usize>,
    /// True if icon was clicked (take with `take_icon_click`).
    icon_clicked: bool,
}

impl AppHeaderState {
    pub fn new() -> Self {
        Default::default()
    }

    /// Take the pressed action index (consumes it).
    pub fn take_action(&mut self) -> Option<usize> {
        self.pressed.take()
    }

    /// Take icon click (consumes it).
    pub fn take_icon_click(&mut self) -> bool {
        std::mem::take(&mut self.icon_clicked)
    }
}

impl crate::core::Interactive for AppHeaderState {
    fn handle_key(&mut self, _k: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;

        // Icon
        let ih = self.icon_hit.mouse(&m);
        if ih == Hit::Press {
            self.icon_clicked = true;
            out = Outcome::Changed;
        } else if ih == Hit::HoverChanged {
            out = Outcome::Consumed;
        }

        // Actions
        self.hover = None;
        for (i, hit) in self.hits.iter_mut().enumerate() {
            let h = hit.mouse(&m);
            if h == Hit::Press {
                self.pressed = Some(i);
                out = Outcome::Changed;
            } else if h == Hit::HoverChanged && hit.hover {
                self.hover = Some(i);
                out = Outcome::Consumed;
            }
        }

        out
    }
}

/// Fixed-height app header that renders a left icon+title+subtitle and right-aligned action labels plus an optional clock.
#[derive(Clone, Debug)]
pub struct AppHeader {
    title: String,
    subtitle: Option<String>,
    icon: Option<String>,
    clock: bool,
    clock_seconds: bool,
    tall: bool,
    right: Option<String>,
    actions: Vec<String>,
    theme: Option<Theme>,
}

impl AppHeader {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            subtitle: None,
            icon: None,
            clock: false,
            clock_seconds: false,
            tall: false,
            right: None,
            actions: Vec::new(),
            theme: None,
        }
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(i.into());
        self
    }

    pub fn clock(mut self, c: bool) -> Self {
        self.clock = c;
        self
    }

    pub fn clock_seconds(mut self, s: bool) -> Self {
        self.clock_seconds = s;
        self
    }

    pub fn tall(mut self, t: bool) -> Self {
        self.tall = t;
        self
    }

    pub fn right(mut self, r: impl Into<String>) -> Self {
        self.right = Some(r.into());
        self
    }

    pub fn actions(mut self, a: &[&str]) -> Self {
        self.actions = a.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut AppHeaderState) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.width == 0 || area.height == 0 {
            return;
        }

        fill(buf, area, th.panel);

        if self.tall {
            // Tall: 3 rows, subtitle centered on row 2
            if area.height < 3 {
                return;
            }

            // Icon
            if let Some(ref icon) = self.icon {
                let icon_area = Rect {
                    x: area.x + 1,
                    y: area.y,
                    width: icon.len() as u16 + 2,
                    height: 1,
                };
                state.icon_hit.set_area(icon_area);
                let icon_bg = if state.icon_hit.hover {
                    th.hover_bg
                } else {
                    th.panel
                };
                fill(buf, icon_area, icon_bg);
                put(
                    buf,
                    area.x + 1,
                    area.y,
                    icon,
                    icon.len() as u16,
                    st(th.foreground, icon_bg),
                );
            } else {
                state.icon_hit.set_area(Rect::default());
            }

            // Title row 1
            put_centered(
                buf,
                Rect { height: 1, ..area },
                &self.title,
                st(th.foreground, th.panel).add_modifier(Modifier::BOLD),
            );

            // Subtitle row 2
            if let Some(ref sub) = self.subtitle {
                put_centered(
                    buf,
                    Rect {
                        y: area.y + 1,
                        height: 1,
                        ..area
                    },
                    sub,
                    st(th.text_muted, th.panel),
                );
            }

            // Actions row 2 right
            let mut x = area.right();
            state.hits.clear();
            state.hits.resize(self.actions.len(), HitBox::default());
            for (i, act) in self.actions.iter().enumerate().rev() {
                let w = act.len() as u16 + 2;
                x = x.saturating_sub(w + 1);
                let r = Rect {
                    x,
                    y: area.y + 1,
                    width: w,
                    height: 1,
                };
                state.hits[i].set_area(r);
                let bg = if state.hover == Some(i) {
                    th.hover_bg
                } else {
                    th.panel
                };
                fill(buf, r, bg);
                put(
                    buf,
                    x,
                    area.y + 1,
                    &format!(" {} ", act),
                    w,
                    st(th.accent, bg),
                );
            }

            // Clock row 2 or row 0 right
            if self.clock && area.width > 40 {
                let (h, m, s) = local_hms();
                let text = if self.clock_seconds {
                    format!("{:02}:{:02}:{:02}", h, m, s)
                } else {
                    format!("{:02}:{:02}", h, m)
                };
                let clock_w = text.len() as u16 + 2;
                let clock_area = Rect {
                    x: area.right().saturating_sub(clock_w + 1),
                    y: area.y,
                    width: clock_w,
                    height: 1,
                };
                let clock_bg = th.panel.blend(Theme::shade(th.foreground, -1), 0.05);
                fill(buf, clock_area, clock_bg);
                put_centered(buf, clock_area, &text, st(th.foreground, clock_bg));
            }
        } else {
            // Normal: 1 row
            // Icon
            let mut x_offset = 0;
            if let Some(ref icon) = self.icon {
                let icon_area = Rect {
                    x: area.x + 1,
                    y: area.y,
                    width: icon.len() as u16 + 2,
                    height: 1,
                };
                state.icon_hit.set_area(icon_area);
                let icon_bg = if state.icon_hit.hover {
                    th.hover_bg
                } else {
                    th.panel
                };
                fill(buf, icon_area, icon_bg);
                put(
                    buf,
                    area.x + 1,
                    area.y,
                    icon,
                    icon.len() as u16,
                    st(th.foreground, icon_bg),
                );
                x_offset = icon_area.width + 1;
            } else {
                state.icon_hit.set_area(Rect::default());
            }

            let title_text = if let Some(sub) = &self.subtitle {
                format!("{} · {}", self.title, sub)
            } else {
                self.title.clone()
            };
            put_centered(
                buf,
                Rect {
                    x: area.x + x_offset,
                    width: area.width.saturating_sub(x_offset),
                    ..area
                },
                &title_text,
                st(th.foreground, th.panel).add_modifier(Modifier::BOLD),
            );

            // Right text
            if let Some(ref right_text) = self.right {
                put_right(buf, area, right_text, st(th.foreground, th.panel));
            }

            // Actions
            let mut x = area.right();
            state.hits.clear();
            state.hits.resize(self.actions.len(), HitBox::default());
            for (i, act) in self.actions.iter().enumerate().rev() {
                let w = act.len() as u16 + 2;
                x = x.saturating_sub(w + 1);
                let r = Rect {
                    x,
                    y: area.y,
                    width: w,
                    height: 1,
                };
                state.hits[i].set_area(r);
                let bg = if state.hover == Some(i) {
                    th.hover_bg
                } else {
                    th.panel
                };
                fill(buf, r, bg);
                put(buf, x, area.y, &format!(" {} ", act), w, st(th.accent, bg));
            }

            // Clock
            if self.clock && area.width > 40 {
                let (h, m, s) = local_hms();
                let text = if self.clock_seconds {
                    format!("{:02}:{:02}:{:02}", h, m, s)
                } else {
                    format!("{:02}:{:02}", h, m)
                };
                let clock_w = text.len() as u16 + 2;
                let clock_area = Rect {
                    x: area.right().saturating_sub(clock_w + 1),
                    y: area.y,
                    width: clock_w,
                    height: 1,
                };
                let clock_bg = th.panel.blend(Theme::shade(th.foreground, -1), 0.05);
                fill(buf, clock_area, clock_bg);
                put_centered(buf, clock_area, &text, st(th.foreground, clock_bg));
            }
        }
    }
}

impl Default for AppHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn header_renders_without_panic() {
        let mut state = AppHeaderState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        AppHeader::new()
            .title("Test")
            .render(buf.area, &mut buf, &mut state);
    }

    #[test]
    fn header_tall_mode() {
        let mut state = AppHeaderState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        AppHeader::new()
            .title("Test")
            .subtitle("Sub")
            .tall(true)
            .render(buf.area, &mut buf, &mut state);
    }

    #[test]
    fn header_actions_clickable() {
        let mut state = AppHeaderState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        AppHeader::new()
            .title("Test")
            .actions(&["Action1", "Action2"])
            .render(buf.area, &mut buf, &mut state);
        assert_eq!(state.hits.len(), 2);
    }
}
