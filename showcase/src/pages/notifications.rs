//! Notifications: toast styles, notification center, banners, inline alerts.

use super::{Ctx, Page, card};
use std::time::Instant;
use tuile::draw::{Edge, fill, put, st};
use tuile::prelude::*;
use tuile::widgets::notify::{
    Banner, BannerState, BannerStyle, InlineAlert, Notification, NotificationCenter,
    NotificationCenterState,
};
use tuile::widgets::{
    Callout, CalloutBorder, CalloutState, Toast, ToastAnim, ToastPosition, ToastStack, ToastStyle,
    Toaster,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    ToastArea,
    Center,
}

pub struct NotificationsPage {
    focus: Focus<Id>,
    toaster: Toaster,
    center: NotificationCenterState,
    banners: [BannerState; 3],
    position_idx: usize,
    anim_idx: usize,
    progress_toast_id: Option<u64>,
    progress_start: Option<Instant>,
}

const POSITIONS: [ToastPosition; 6] = [
    ToastPosition::BottomRight,
    ToastPosition::TopRight,
    ToastPosition::BottomLeft,
    ToastPosition::TopLeft,
    ToastPosition::BottomCenter,
    ToastPosition::TopCenter,
];
const ANIMS: [ToastAnim; 4] = [
    ToastAnim::Slide,
    ToastAnim::Fade,
    ToastAnim::Pop,
    ToastAnim::None,
];

impl Default for NotificationsPage {
    fn default() -> Self {
        let mut center = NotificationCenterState::new();
        center.push(
            Notification::new(
                "Deploy successful",
                "Production build deployed to 3 regions",
                "Today",
            )
            .variant(Variant::Success)
            .time("2m ago")
            .read(false),
        );
        center.push(
            Notification::new(
                "High memory usage",
                "Container prod-api-3 using 92% memory",
                "Today",
            )
            .variant(Variant::Warning)
            .time("15m ago")
            .read(false),
        );
        center.push(
            Notification::new(
                "New team member",
                "Alice joined the Engineering team",
                "Today",
            )
            .variant(Variant::Default)
            .time("1h ago")
            .read(true),
        );
        center.push(
            Notification::new("Security update", "Critical patch for OpenSSL 3.0", "Today")
                .variant(Variant::Error)
                .time("3h ago")
                .read(false),
        );
        center.push(
            Notification::new(
                "Backup completed",
                "Daily backup finished successfully",
                "Yesterday",
            )
            .variant(Variant::Success)
            .time("18h ago")
            .read(true),
        );
        center.push(
            Notification::new(
                "Disk space low",
                "Database volume at 85% capacity",
                "Yesterday",
            )
            .variant(Variant::Warning)
            .time("1d ago")
            .read(true),
        );
        center.push(
            Notification::new(
                "Build failed",
                "Feature branch build #423 failed",
                "Yesterday",
            )
            .variant(Variant::Error)
            .time("1d ago")
            .read(true),
        );
        center.push(
            Notification::new(
                "PR review",
                "Review needed for feat/notifications",
                "Yesterday",
            )
            .variant(Variant::Default)
            .time("2d ago")
            .read(true),
        );

        let mut toaster = Toaster::new().max_visible(4);
        toaster.push(
            Toast::new("Card", "Example Card toast")
                .variant(Variant::Default)
                .style(ToastStyle::Card),
        );
        toaster.push(
            Toast::new("Flat", "Example Flat toast")
                .variant(Variant::Primary)
                .style(ToastStyle::Flat),
        );
        toaster.push(
            Toast::new("Minimal", "Quick update")
                .variant(Variant::Success)
                .style(ToastStyle::Minimal),
        );
        toaster.push(
            Toast::new("Pill", "Compact notification")
                .variant(Variant::Warning)
                .style(ToastStyle::Pill),
        );

        NotificationsPage {
            focus: Focus::new([Id::ToastArea, Id::Center]),
            toaster,
            center,
            banners: Default::default(),
            position_idx: 0,
            anim_idx: 0,
            progress_toast_id: None,
            progress_start: None,
        }
    }
}

impl NotificationsPage {
    fn fire_toast(&mut self, style: ToastStyle, ctx: &Ctx) {
        let variants = [
            Variant::Default,
            Variant::Primary,
            Variant::Success,
            Variant::Warning,
            Variant::Error,
        ];
        let v = variants[self.toaster.toasts.len() % variants.len()];
        let (title, message) = match style {
            ToastStyle::Card => ("Card", "Example Card toast"),
            ToastStyle::Flat => ("Flat", "Example Flat toast"),
            ToastStyle::Minimal => ("Minimal", "Quick update"),
            ToastStyle::Pill => ("Pill", "Compact notification"),
            ToastStyle::Outline => ("Outline", "Outlined notification"),
            ToastStyle::Banner => ("Banner", "Full-width announcement"),
            ToastStyle::Glass => ("Glass", "Translucent notification"),
            ToastStyle::Progress => ("Loading", "Please wait..."),
            ToastStyle::Action => ("File deleted", "Item moved to trash"),
            ToastStyle::Grouped => ("Grouped", "Summary"),
        };
        let mut toast = Toast::new(title, message).variant(v).style(style);
        match style {
            ToastStyle::Action => {
                toast = toast.actions(&["Undo", "Dismiss"]);
            }
            ToastStyle::Progress => {
                toast = toast.progress_value(0.0).no_timeout();
                self.progress_toast_id = Some(toast.id());
                self.progress_start = Some(ctx.now);
            }
            _ => {}
        }
        self.toaster.push(toast);
    }
}

impl Page for NotificationsPage {
    fn title(&self) -> &'static str {
        "Notifications"
    }
    fn subtitle(&self) -> &'static str {
        "Toast styles, notification center, banners, inline alerts"
    }
    fn icon(&self) -> &'static str {
        "◔"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("1-9,0", "Toast styles"),
            ("p", "Position"),
            ("a", "Animation"),
            ("R", "Show banners"),
        ]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        if let (Some(id), Some(start)) = (self.progress_toast_id, self.progress_start) {
            let elapsed = ctx.now.saturating_duration_since(start).as_secs_f32();
            let progress = (elapsed / 4.0).min(1.0);
            if let Some(toast) = self.toaster.toasts.iter_mut().find(|t| t.id() == id) {
                *toast = toast.clone().progress_value(progress);
            }
            if progress >= 1.0 {
                self.progress_toast_id = None;
                self.progress_start = None;
            }
        }
        let is_small = area.width < 90 || area.height < 20;
        if is_small {
            let [toaster_area, center_area] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(area);
            self.render_toaster_demo(toaster_area, buf, ctx);
            self.render_center(center_area, buf, ctx);
        } else {
            let [toaster_area, center_area, right] = Layout::horizontal([
                Constraint::Percentage(40),
                Constraint::Percentage(28),
                Constraint::Fill(1),
            ])
            .areas(area);
            self.render_toaster_demo(toaster_area, buf, ctx);
            self.render_center(center_area, buf, ctx);
            self.render_other_widgets(right, buf, ctx);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        use tuile::crossterm::event::{Event::*, KeyCode::*};
        match ev {
            Key(k) if is_press(k) => match k.code {
                Char('1') => {
                    self.fire_toast(ToastStyle::Card, ctx);
                    return Outcome::Changed;
                }
                Char('2') => {
                    self.fire_toast(ToastStyle::Flat, ctx);
                    return Outcome::Changed;
                }
                Char('3') => {
                    self.fire_toast(ToastStyle::Minimal, ctx);
                    return Outcome::Changed;
                }
                Char('4') => {
                    self.fire_toast(ToastStyle::Pill, ctx);
                    return Outcome::Changed;
                }
                Char('5') => {
                    self.fire_toast(ToastStyle::Outline, ctx);
                    return Outcome::Changed;
                }
                Char('6') => {
                    self.fire_toast(ToastStyle::Banner, ctx);
                    return Outcome::Changed;
                }
                Char('7') => {
                    self.fire_toast(ToastStyle::Glass, ctx);
                    return Outcome::Changed;
                }
                Char('8') => {
                    self.fire_toast(ToastStyle::Progress, ctx);
                    return Outcome::Changed;
                }
                Char('9') => {
                    self.fire_toast(ToastStyle::Action, ctx);
                    return Outcome::Changed;
                }
                Char('0') => {
                    for _ in 0..6 {
                        self.fire_toast(ToastStyle::Card, ctx);
                    }
                    return Outcome::Changed;
                }
                Char('p') => {
                    self.position_idx = (self.position_idx + 1) % POSITIONS.len();
                    self.toaster = self.toaster.clone().position(POSITIONS[self.position_idx]);
                    return Outcome::Changed;
                }
                Char('a') => {
                    self.anim_idx = (self.anim_idx + 1) % ANIMS.len();
                    self.toaster = self.toaster.clone().anim(ANIMS[self.anim_idx]);
                    return Outcome::Changed;
                }
                Char('R') => {
                    for banner in &mut self.banners {
                        banner.show();
                    }
                    return Outcome::Changed;
                }
                _ => {}
            },
            Mouse(m) => {
                let outcome = self.toaster.handle_mouse(*m) | self.center.handle_mouse(*m);
                for banner in &mut self.banners {
                    let _ = banner.handle_mouse(*m);
                }
                return outcome;
            }
            _ => {}
        }
        if let Some(Id::Center) = self.focus.current()
            && let Key(k) = ev
        {
            return self.center.handle_key(*k);
        }
        Outcome::Ignored
    }

    fn animating(&self, now: Instant) -> bool {
        self.toaster.animating(now) || self.progress_toast_id.is_some()
    }
}

impl NotificationsPage {
    fn render_toaster_demo(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let demo_area = card(buf, area, &th, "Toast demo (bounds)");
        let legend_y = demo_area.y;
        let legends = [
            "1:Card  2:Flat  3:Minimal",
            "4:Pill  5:Outline  6:Banner",
            "7:Glass  8:Progress  9:Action",
            "0:Many  p:Pos  a:Anim  R:Bann",
        ];
        for (i, legend) in legends.iter().enumerate() {
            if (legend_y + i as u16) < demo_area.bottom() {
                put(
                    buf,
                    demo_area.x + 1,
                    legend_y + i as u16,
                    legend,
                    demo_area.width.saturating_sub(2),
                    st(th.text_muted, th.background),
                );
            }
        }
        let toast_bounds = Rect {
            x: demo_area.x + 1,
            y: demo_area.y + 5,
            width: demo_area.width.saturating_sub(2),
            height: demo_area.height.saturating_sub(6),
        };
        fill(buf, toast_bounds, th.panel);
        ToastStack::new()
            .theme(&th)
            .now(ctx.now)
            .render(toast_bounds, buf, &mut self.toaster);
    }

    fn render_center(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let center_area = card(buf, area, &th, "Notification Center");
        NotificationCenter::new()
            .theme(&th)
            .render(center_area, buf, &mut self.center);
    }

    fn render_other_widgets(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let [banners_area, alerts_area, callouts_area] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(16),
            Constraint::Fill(1),
        ])
        .areas(area);
        let banner_card = card(buf, banners_area, &th, "Banners (R to restore)");
        let heights = vec![2_u16, 2, 2];
        let banner_rows = tuile::layout::stack(banner_card, &heights, 1);
        if banner_rows.len() >= 3 {
            Banner::new("System maintenance tonight 23:00 UTC")
                .variant(Variant::Warning)
                .style(BannerStyle::Solid)
                .action("Schedule")
                .theme(&th)
                .render(banner_rows[0], buf, &mut self.banners[0]);
            Banner::new("Dark mode now available")
                .variant(Variant::Success)
                .style(BannerStyle::Tinted)
                .closeable(true)
                .theme(&th)
                .render(banner_rows[1], buf, &mut self.banners[1]);
            Banner::new("Update required - refresh now")
                .variant(Variant::Primary)
                .style(BannerStyle::Outline)
                .action("Refresh")
                .theme(&th)
                .render(banner_rows[2], buf, &mut self.banners[2]);
        }
        let alert_card = card(buf, alerts_area, &th, "Inline Alerts");
        let alert_heights = vec![5_u16, 5, 1, 5];
        let alert_rows = tuile::layout::stack(alert_card, &alert_heights, 1);
        if alert_rows.len() >= 4 {
            InlineAlert::new("Success", "Your changes saved")
                .variant(Variant::Success)
                .theme(&th)
                .render(alert_rows[0], buf);
            InlineAlert::new("Warning", "Cannot be undone")
                .variant(Variant::Warning)
                .theme(&th)
                .render(alert_rows[1], buf);
            InlineAlert::new("Compact info", "")
                .variant(Variant::Default)
                .compact(true)
                .theme(&th)
                .render(alert_rows[2], buf);
            InlineAlert::new("Error", "Connection failed")
                .variant(Variant::Error)
                .theme(&th)
                .render(alert_rows[3], buf);
        }
        if callouts_area.height >= 12 {
            let callout_card = card(buf, callouts_area, &th, "Callouts");
            let callout_heights = vec![4_u16, 4, 4];
            let callout_rows = tuile::layout::stack(callout_card, &callout_heights, 0);
            let mut callout_states = [
                CalloutState::new(),
                CalloutState::new(),
                CalloutState::new(),
            ];
            if callout_rows.len() >= 3 {
                Callout::new("LeftBar", "Standard callout")
                    .variant(Variant::Primary)
                    .border_style(CalloutBorder::LeftBar)
                    .theme(&th)
                    .render(callout_rows[0], buf, &mut callout_states[0]);
                Callout::new("Thick Bar", "Half-width edge")
                    .variant(Variant::Success)
                    .border_style(CalloutBorder::Bar(Edge::Half))
                    .theme(&th)
                    .render(callout_rows[1], buf, &mut callout_states[1]);
                Callout::new("Round", "Framed")
                    .variant(Variant::Warning)
                    .border_style(CalloutBorder::Round)
                    .theme(&th)
                    .render(callout_rows[2], buf, &mut callout_states[2]);
            }
        }
    }
}
