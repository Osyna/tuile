//! Page contract for the showcase. Each widget group owns one file here and exposes a
//! `Default`-constructible `XxxPage` registered in [`all`].

use std::time::Instant;

use tuiforge::prelude::*;

pub mod charts;
pub mod content;
pub mod controls;
pub mod dashboard;
pub mod data;
pub mod feedback;
pub mod inputs;
pub mod layout;
pub mod navigation;
pub mod settings;
pub mod themes;
pub mod welcome;

/// Per-frame/per-event context handed to pages by the shell.
pub struct Ctx {
    pub theme: Theme,
    pub now: Instant,
    /// Epoch of the app, for looping animations (`elapsed(ctx.started, ctx.now)`).
    pub started: Instant,
    /// Disable slide/fade animations (the shell exposes a toggle).
    pub reduce_motion: bool,
    /// Notifications requested by the page; the shell drains them into toasts.
    pub notices: Vec<(String, Variant)>,
    /// Content rect of the last draw; pages use it for overlays and popups.
    pub area: Rect,
}

impl Ctx {
    pub fn notify(&mut self, msg: impl Into<String>, v: Variant) {
        self.notices.push((msg.into(), v));
    }
    pub fn elapsed(&self) -> f32 {
        elapsed(self.started, self.now)
    }
    /// Standard tween duration honouring reduce-motion.
    pub fn dur(&self, ms: u64) -> Duration {
        if self.reduce_motion { Duration::ZERO } else { Duration::from_millis(ms) }
    }
}

pub trait Page {
    fn title(&self) -> &'static str;
    /// One-line blurb shown in the header.
    fn subtitle(&self) -> &'static str {
        ""
    }
    /// Sidebar glyph.
    fn icon(&self) -> &'static str {
        "•"
    }
    /// Draw into `area` (the content region; the shell already painted `theme.background`).
    /// Overlays (dropdowns, tooltips) must be drawn last, inside this call, via `Overlay`.
    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx);
    /// Key presses (not consumed by the shell) and *every* mouse event reach the page.
    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome;
    /// `true` while the page has a running tween/spinner so the shell redraws at 60 fps.
    fn animating(&self, _now: Instant) -> bool {
        false
    }
    /// Page-specific key bindings for the footer: `(key, description)`.
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
}

pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(welcome::WelcomePage::default()),
        Box::new(dashboard::DashboardPage::default()),
        Box::new(controls::ControlsPage::default()),
        Box::new(inputs::InputsPage::default()),
        Box::new(navigation::NavigationPage::default()),
        Box::new(data::DataPage::default()),
        Box::new(charts::ChartsPage::default()),
        Box::new(feedback::FeedbackPage::default()),
        Box::new(layout::LayoutPage::default()),
        Box::new(content::ContentPage::default()),
        Box::new(settings::SettingsPage::default()),
        Box::new(themes::ThemesPage::default()),
    ]
}
