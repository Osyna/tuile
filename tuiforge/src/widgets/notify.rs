//! Notification center, banners, and inline alerts for in-app messaging.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! use tuiforge::widgets::notify::*;
//! # let area = Rect::new(0, 0, 40, 20);
//! # let mut buf = Buffer::empty(area);
//! # let mut center = NotificationCenterState::new();
//! NotificationCenter::new().render(area, &mut buf, &mut center);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::widgets::{StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::core::{Hit, HitBox, Interactive, Outcome, is_press, mouse_in, wheel_delta};
use crate::draw::{Border, Edge, bold, fill, put, put_right, st, truncate, wrap};
use crate::theme::{self, Theme, Variant};
use crate::widgets::{Scrollbar, ScrollbarState};

/// A single notification entry.
#[derive(Clone, Debug)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub variant: Variant,
    pub time_label: String,
    pub read: bool,
    pub source: String,
    pub group: String,
}

impl Notification {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        group: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            variant: Variant::Default,
            time_label: String::new(),
            read: false,
            source: String::new(),
            group: group.into(),
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn time(mut self, label: impl Into<String>) -> Self {
        self.time_label = label.into();
        self
    }

    pub fn source(mut self, s: impl Into<String>) -> Self {
        self.source = s.into();
        self
    }

    pub fn read(mut self, r: bool) -> Self {
        self.read = r;
        self
    }
}

/// Builder for notification center panel.
pub struct NotificationCenter {
    theme: Option<Theme>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self { theme: None }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

/// State for interactive notification center.
#[derive(Clone, Debug)]
pub struct NotificationCenterState {
    pub notifications: Vec<Notification>,
    pub filter: Option<Variant>,
    scroll_offset: usize,
    selected: usize,
    hit: HitBox,
    clear_all_hit: HitBox,
    item_hits: Vec<HitBox>,
}

impl NotificationCenterState {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            filter: None,
            scroll_offset: 0,
            selected: 0,
            hit: HitBox::default(),
            clear_all_hit: HitBox::default(),
            item_hits: Vec::new(),
        }
    }

    pub fn push(&mut self, notif: Notification) {
        self.notifications.push(notif);
    }

    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    pub fn mark_read(&mut self, index: usize) {
        if let Some(n) = self.notifications.get_mut(index) {
            n.read = true;
        }
    }

    pub fn dismiss(&mut self, index: usize) {
        if index < self.notifications.len() {
            self.notifications.remove(index);
            if self.selected >= self.notifications.len() && self.selected > 0 {
                self.selected = self.selected.saturating_sub(1);
            }
            if self.scroll_offset > self.selected {
                self.scroll_offset = self.selected;
            }
        }
    }

    pub fn dismiss_all(&mut self) {
        self.notifications.clear();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn filtered_notifications(&self) -> Vec<&Notification> {
        self.notifications
            .iter()
            .filter(|n| self.filter.is_none_or(|f| n.variant == f))
            .collect()
    }

    fn clamp_scroll(&mut self, visible_rows: usize) {
        let count = self.filtered_notifications().len();
        if count == 0 {
            self.scroll_offset = 0;
            self.selected = 0;
            return;
        }
        self.selected = self.selected.min(count.saturating_sub(1));
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        if self.selected >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selected.saturating_sub(visible_rows.saturating_sub(1));
        }
    }
}

impl Default for NotificationCenterState {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactive for NotificationCenterState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }

        let count = self.filtered_notifications().len();
        if count == 0 {
            return Outcome::Ignored;
        }

        match k.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    return Outcome::Consumed;
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < count {
                    self.selected += 1;
                    return Outcome::Consumed;
                }
            }
            KeyCode::Enter => {
                self.mark_read(self.selected);
                return Outcome::Changed;
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                self.dismiss(self.selected);
                return Outcome::Changed;
            }
            _ => {}
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        // Clear all button
        let clear_hit = self.clear_all_hit.mouse(&m);
        if matches!(clear_hit, Hit::Press) {
            self.dismiss_all();
            return Outcome::Changed;
        }

        // Scroll wheel
        if let Some(delta) = wheel_delta(&m)
            && mouse_in(self.hit.area, &m)
        {
            let new_offset = self.scroll_offset.saturating_add_signed(delta as isize);
            self.scroll_offset =
                new_offset.min(self.filtered_notifications().len().saturating_sub(1));
            return Outcome::Consumed;
        }

        // Item clicks
        for (i, hit) in self.item_hits.iter_mut().enumerate() {
            let h = hit.mouse(&m);
            if matches!(h, Hit::Press) || (matches!(h, Hit::HoverChanged) && hit.hover) {
                let idx = self.scroll_offset + i;
                if idx < self.notifications.len() {
                    self.selected = idx;
                    if matches!(h, Hit::Press) {
                        self.mark_read(idx);
                        return Outcome::Changed;
                    }
                    return Outcome::Consumed;
                }
            }
        }

        Outcome::Ignored
    }
}

impl StatefulWidget for NotificationCenter {
    type State = NotificationCenterState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        fill(buf, area, bg);

        state.hit.set_area(area);

        // Header with unread count
        let unread = state.unread_count();
        let header = if unread > 0 {
            format!("Inbox ({unread})")
        } else {
            "Inbox".to_string()
        };

        let header_style = bold(st(th.text, bg));
        let clear_text = "Clear all";
        let clear_w = if state.notifications.is_empty() {
            0
        } else {
            clear_text.len() as u16 + 1
        };
        put(
            buf,
            area.x + 1,
            area.y,
            &header,
            area.width.saturating_sub(2 + clear_w),
            header_style,
        );

        // Clear all button
        if clear_w > 0 {
            let clear_x = area.right().saturating_sub(clear_w);
            put(
                buf,
                clear_x,
                area.y,
                clear_text,
                clear_text.len() as u16,
                st(th.text_muted, bg),
            );
            state.clear_all_hit.set_area(Rect {
                x: clear_x,
                y: area.y,
                width: clear_text.len() as u16,
                height: 1,
            });
        }

        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        let filtered_count = state
            .notifications
            .iter()
            .filter(|n| state.filter.is_none_or(|f| n.variant == f))
            .count();

        if filtered_count == 0 {
            // Empty state
            let empty_y = content_area.y + content_area.height / 2;
            put(
                buf,
                content_area.x + content_area.width / 2 - 8,
                empty_y,
                "No notifications",
                17,
                st(th.text_muted, bg),
            );
            return;
        }

        // Render notifications with groups
        let mut last_group = String::new();
        let mut y = content_area.y;

        let row_height = 3_u16;
        let visible_rows = (content_area.height / row_height) as usize;
        state.clamp_scroll(visible_rows);

        state.item_hits.clear();
        state.item_hits.resize(visible_rows, HitBox::default());

        for (display_idx, (real_idx, notif)) in state
            .notifications
            .iter()
            .enumerate()
            .filter(|(_, n)| state.filter.is_none_or(|f| n.variant == f))
            .skip(state.scroll_offset)
            .take(visible_rows)
            .enumerate()
        {
            // Group label
            if notif.group != last_group {
                if y + 1 > content_area.bottom() {
                    break;
                }
                put(
                    buf,
                    content_area.x + 1,
                    y,
                    &notif.group,
                    content_area.width.saturating_sub(2),
                    st(th.text_disabled, bg),
                );
                y += 1;
                last_group = notif.group.clone();
            }

            if y + row_height > content_area.bottom() {
                break;
            }

            let item_area = Rect {
                x: content_area.x,
                y,
                width: content_area.width,
                height: row_height.min(content_area.bottom().saturating_sub(y)),
            };

            let is_selected = real_idx == state.selected;
            let item_bg = if is_selected { th.hover_bg } else { bg };
            fill(buf, item_area, item_bg);

            // Unread indicator
            if !notif.read {
                put(
                    buf,
                    item_area.x + 1,
                    item_area.y,
                    "•",
                    1,
                    st(th.primary, item_bg),
                );
            }

            // Title, truncated before the time column
            let time_w = if notif.time_label.is_empty() {
                0
            } else {
                notif.time_label.width() as u16 + 1
            };
            let title_w = item_area.width.saturating_sub(4 + time_w);
            let title_style = if notif.read {
                st(th.text_muted, item_bg)
            } else {
                bold(st(th.text, item_bg))
            };
            put(
                buf,
                item_area.x + 3,
                item_area.y,
                &truncate(&notif.title, title_w as usize),
                title_w,
                title_style,
            );

            // Time label
            if time_w > 0 {
                put_right(
                    buf,
                    Rect {
                        x: item_area.x,
                        y: item_area.y,
                        width: item_area.width.saturating_sub(1),
                        height: 1,
                    },
                    &notif.time_label,
                    st(th.text_disabled, item_bg),
                );
            }

            // Message: one line, ellipsis when cut
            if item_area.height > 1 {
                let msg_w = item_area.width.saturating_sub(4);
                put(
                    buf,
                    item_area.x + 3,
                    item_area.y + 1,
                    &truncate(&notif.message, msg_w as usize),
                    msg_w,
                    st(th.text_muted, item_bg),
                );
            }

            if display_idx < state.item_hits.len() {
                state.item_hits[display_idx].set_area(item_area);
            }
            y += row_height;
        }

        // Scrollbar if needed
        if filtered_count > visible_rows {
            let scroll_area = Rect {
                x: content_area.right().saturating_sub(1),
                y: content_area.y,
                width: 1,
                height: content_area.height,
            };
            let mut scrollbar_state = ScrollbarState::new();
            Scrollbar::vertical(filtered_count, visible_rows)
                .offset(state.scroll_offset)
                .theme(&th)
                .render(scroll_area, buf, &mut scrollbar_state);
        }
    }
}

/// Style for banner notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerStyle {
    /// Solid variant background.
    Solid,
    /// Variant tinted 15% into background with left rail.
    Tinted,
    /// Outline border.
    Outline,
}

/// Full-width announcement banner.
pub struct Banner {
    message: String,
    variant: Variant,
    style: BannerStyle,
    action_label: Option<String>,
    closeable: bool,
    theme: Option<Theme>,
}

impl Banner {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            variant: Variant::Default,
            style: BannerStyle::Solid,
            action_label: None,
            closeable: true,
            theme: None,
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn style(mut self, s: BannerStyle) -> Self {
        self.style = s;
        self
    }

    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    pub fn closeable(mut self, v: bool) -> Self {
        self.closeable = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

/// State for dismissible banners.
#[derive(Clone, Debug, Default)]
pub struct BannerState {
    pub dismissed: bool,
    close_hit: HitBox,
    action_hit: HitBox,
}

impl BannerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&mut self) {
        self.dismissed = false;
    }

    pub fn action_clicked(&self) -> bool {
        self.action_hit.pressed
    }
}

impl Interactive for BannerState {
    fn handle_key(&mut self, _k: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if self.dismissed {
            return Outcome::Ignored;
        }

        let close = self.close_hit.mouse(&m);
        if matches!(close, Hit::Press) {
            self.dismissed = true;
            return Outcome::Changed;
        }

        let action = self.action_hit.mouse(&m);
        if matches!(action, Hit::Press) {
            return Outcome::Changed;
        }

        Outcome::Ignored
    }
}

impl StatefulWidget for Banner {
    type State = BannerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || state.dismissed {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let icon = match self.variant {
            Variant::Default => "•",
            Variant::Primary => "•",
            Variant::Success => "✓",
            Variant::Warning => "▲",
            Variant::Error => "✗",
            Variant::Secondary | Variant::Accent => "•",
        };

        let (bg, fg, rail_color) = match self.style {
            BannerStyle::Solid => {
                let bg = th.variant(self.variant);
                let fg = bg.text_on(0.87);
                (bg, fg, bg)
            }
            BannerStyle::Tinted => {
                let variant_color = th.variant(self.variant);
                let bg = variant_color.blend(th.background, 0.15);
                (bg, th.text, variant_color)
            }
            BannerStyle::Outline => (th.surface, th.text, th.variant(self.variant)),
        };

        fill(buf, area, bg);

        if matches!(self.style, BannerStyle::Tinted) {
            Edge::Half.draw(buf, area.x, area.y, area.height, false, rail_color, bg);
        } else if matches!(self.style, BannerStyle::Outline) {
            Border::Round.draw(buf, area, rail_color, bg);
        }

        let text_x = area.x
            + if matches!(self.style, BannerStyle::Outline) {
                3
            } else {
                2
            };
        let mut text_w = area
            .width
            .saturating_sub(if matches!(self.style, BannerStyle::Outline) {
                6
            } else {
                4
            });

        put(buf, text_x, area.y, icon, 1, st(fg, bg));

        // Reserve space for close button
        if self.closeable {
            text_w = text_w.saturating_sub(4);
        }

        // Reserve space for action
        if let Some(ref action) = self.action_label {
            let action_w = action.len() as u16 + 4;
            text_w = text_w.saturating_sub(action_w + 2);

            let action_x = area.x + text_x + text_w + 2;
            let action_rect = Rect {
                x: action_x,
                y: area.y,
                width: action_w,
                height: 1,
            };
            put(
                buf,
                action_x,
                area.y,
                &format!("[{}]", action),
                action_w,
                bold(st(fg, bg)),
            );
            state.action_hit.set_area(action_rect);
        }

        let lines = wrap(&self.message, text_w as usize);
        for (i, line) in lines.iter().take(area.height as usize).enumerate() {
            put(buf, text_x + 2, area.y + i as u16, line, text_w, st(fg, bg));
        }

        if self.closeable {
            let close_x = area.right().saturating_sub(2);
            put(buf, close_x, area.y, "✕", 1, st(fg, bg));
            state.close_hit.set_area(Rect {
                x: close_x,
                y: area.y,
                width: 1,
                height: 1,
            });
        }
    }
}

/// Inline alert widget (stateless).
pub struct InlineAlert {
    title: String,
    message: String,
    variant: Variant,
    dismissible: bool,
    compact: bool,
    theme: Option<Theme>,
}

impl InlineAlert {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            variant: Variant::Default,
            dismissible: false,
            compact: false,
            theme: None,
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn dismissible(mut self, v: bool) -> Self {
        self.dismissible = v;
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for InlineAlert {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let var_color = if self.variant == Variant::Default {
            th.primary
        } else {
            th.variant(self.variant)
        };
        // tinted surface so the variant reads even without the frame
        let bg = th.surface.blend(var_color, 0.12);
        let icon = match self.variant {
            Variant::Success => "✓",
            Variant::Warning => "▲",
            Variant::Error => "✗",
            _ => "•",
        };

        fill(buf, area, bg);
        if self.compact {
            // one row: `┃ ✓ Title  message`
            put(buf, area.x, area.y, "┃", 1, st(var_color, bg));
            put(buf, area.x + 2, area.y, icon, 1, st(var_color, bg));
            let used = put(
                buf,
                area.x + 4,
                area.y,
                &self.title,
                area.width.saturating_sub(5),
                bold(st(th.text, bg)),
            );
            let mx = area.x + 4 + used + 2;
            if !self.message.is_empty() && mx < area.right() {
                put(
                    buf,
                    mx,
                    area.y,
                    &truncate(&self.message, (area.right() - mx) as usize),
                    area.right() - mx,
                    st(th.text_muted, bg),
                );
            }
        } else {
            Border::Round.draw(buf, area, var_color, bg);
            let icon_x = area.x + 2;
            let text_x = icon_x + 2;
            let text_w = area.width.saturating_sub(6);
            put(buf, icon_x, area.y + 1, icon, 1, st(var_color, bg));
            put(
                buf,
                text_x,
                area.y + 1,
                &self.title,
                text_w,
                bold(st(th.text, bg)),
            );
            for (i, line) in wrap(&self.message, text_w as usize).iter().enumerate() {
                let y = area.y + 2 + i as u16;
                if y + 1 >= area.bottom() {
                    break;
                }
                put(buf, text_x, y, line, text_w, st(th.text_muted, bg));
            }
        }
    }
}

/// Draw a small count badge (e.g., unread counter).
pub fn count_badge(buf: &mut Buffer, x: u16, y: u16, count: usize, th: &Theme) {
    if count == 0 {
        return;
    }

    let text = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };

    let w = text.len() as u16 + 2;
    let badge_area = Rect {
        x,
        y,
        width: w,
        height: 1,
    };

    let bg = th.error;
    let fg = bg.text_on(0.87);

    fill(buf, badge_area, bg);
    put(buf, x + 1, y, &text, text.len() as u16, st(fg, bg));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_center_unread_count() {
        let mut state = NotificationCenterState::new();
        state.push(Notification::new("A", "msg", "Today").read(false));
        state.push(Notification::new("B", "msg", "Today").read(true));
        state.push(Notification::new("C", "msg", "Today").read(false));
        assert_eq!(state.unread_count(), 2);
    }

    #[test]
    fn notification_center_mark_read() {
        let mut state = NotificationCenterState::new();
        state.push(Notification::new("A", "msg", "Today").read(false));
        state.mark_read(0);
        assert!(state.notifications[0].read);
    }

    #[test]
    fn notification_center_dismiss() {
        let mut state = NotificationCenterState::new();
        state.push(Notification::new("A", "msg", "Today"));
        state.push(Notification::new("B", "msg", "Today"));
        assert_eq!(state.notifications.len(), 2);
        state.dismiss(0);
        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.notifications[0].title, "B");
    }

    #[test]
    fn notification_center_scroll_clamping() {
        let mut state = NotificationCenterState::new();
        for i in 0..10 {
            state.push(Notification::new(format!("N{}", i), "msg", "Today"));
        }
        state.selected = 8;
        state.scroll_offset = 5;
        state.clamp_scroll(3);
        assert!(state.scroll_offset <= state.selected);
        assert!(state.selected < state.scroll_offset + 3 || state.selected == 9);
    }

    #[test]
    fn banner_dismiss() {
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        let mut state = BannerState::new();

        Banner::new("Test message").render(area, &mut buf, &mut state);
        assert!(!state.dismissed);

        state.dismissed = true;
        Banner::new("Test message").render(area, &mut buf, &mut state);
        // Should not render when dismissed
    }

    #[test]
    fn banner_styles_render() {
        let area = Rect::new(0, 0, 80, 2);
        let mut buf = Buffer::empty(area);
        let mut state = BannerState::new();

        let styles = [
            BannerStyle::Solid,
            BannerStyle::Tinted,
            BannerStyle::Outline,
        ];
        for style in styles {
            Banner::new("Test")
                .style(style)
                .render(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn inline_alert_compact_and_full() {
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);

        InlineAlert::new("Title", "Message")
            .compact(false)
            .render(area, &mut buf);
        InlineAlert::new("Title", "Message")
            .compact(true)
            .render(area, &mut buf);
    }

    #[test]
    fn count_badge_renders() {
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        let th = theme::current();

        count_badge(&mut buf, 0, 0, 0, &th); // Should not draw
        count_badge(&mut buf, 0, 1, 5, &th);
        count_badge(&mut buf, 0, 2, 150, &th); // Should show 99+
    }
}
