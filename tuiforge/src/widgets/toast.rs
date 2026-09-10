//! Toast notifications: slide-in, auto-dismiss, stackable alerts.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let mut toaster = Toaster::default();
//! toaster.push(Toast::new("Saved", "File written successfully").variant(Variant::Success));
//! toaster.info("Quick message");
//! ToastStack::new().render(area, &mut buf, &mut toaster);
//! ```

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

use crate::anim::Easing;
use crate::core::{HitBox, Hit, Outcome, Interactive};
use crate::draw::{fill, put, wrap, st, blend_area, bold, Border};
use crate::theme::{self, Theme, Variant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastCorner {
    TopRight,
    BottomRight,
    TopLeft,
    BottomLeft,
}

/// A single toast notification.
#[derive(Clone, Debug)]
pub struct Toast {
    title: String,
    message: String,
    variant: Variant,
    timeout: Option<Duration>,
    dismissible: bool,
    show_progress: bool,
    created: Instant,
    closing: Option<Instant>,
    hit: HitBox,
}

impl Toast {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            variant: Variant::Default,
            timeout: Some(Duration::from_secs(5)),
            dismissible: true,
            show_progress: false,
            created: Instant::now(),
            closing: None,
            hit: HitBox::default(),
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    pub fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    pub fn dismissible(mut self, v: bool) -> Self {
        self.dismissible = v;
        self
    }

    pub fn progress(mut self, v: bool) -> Self {
        self.show_progress = v;
        self
    }

    fn is_expired(&self, now: Instant) -> bool {
        if let Some(timeout) = self.timeout {
            now.saturating_duration_since(self.created) >= timeout
        } else {
            false
        }
    }

    fn close(&mut self, now: Instant) {
        if self.closing.is_none() {
            self.closing = Some(now);
        }
    }

    fn is_closed(&self, now: Instant, fade_duration: Duration) -> bool {
        if let Some(closing) = self.closing {
            now.saturating_duration_since(closing) >= fade_duration
        } else {
            false
        }
    }
}

/// Manages a stack of toasts with auto-dismiss and animations.
#[derive(Clone, Debug)]
pub struct Toaster {
    toasts: Vec<Toast>,
    max_visible: usize,
    pub corner: ToastCorner,
    reduce_motion: bool,
    fade_duration: Duration,
    now: Instant,
}

impl Toaster {
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            max_visible: 3,
            corner: ToastCorner::BottomRight,
            reduce_motion: false,
            fade_duration: Duration::from_millis(150),
            now: Instant::now(),
        }
    }

    pub fn push(&mut self, mut toast: Toast) {
        toast.created = Instant::now();
        self.toasts.push(toast);
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.push(Toast::new("Info", msg).variant(Variant::Default));
    }

    pub fn success(&mut self, msg: impl Into<String>) {
        self.push(Toast::new("Success", msg).variant(Variant::Success));
    }

    pub fn warning(&mut self, msg: impl Into<String>) {
        self.push(Toast::new("Warning", msg).variant(Variant::Warning));
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.push(Toast::new("Error", msg).variant(Variant::Error));
    }

    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n;
        self
    }

    pub fn corner(mut self, c: ToastCorner) -> Self {
        self.corner = c;
        self
    }

    pub fn reduce_motion(mut self, v: bool) -> Self {
        self.reduce_motion = v;
        self
    }

    pub fn tick(&mut self, now: Instant) {
        self.now = now;
        for toast in &mut self.toasts {
            if toast.is_expired(now) && toast.closing.is_none() {
                toast.close(now);
            }
        }
        self.toasts.retain(|t| !t.is_closed(now, self.fade_duration));
    }

    pub fn dismiss_all(&mut self, now: Instant) {
        for toast in &mut self.toasts {
            toast.close(now);
        }
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.toasts.iter().any(|t| {
            let age = now.saturating_duration_since(t.created);
            age < Duration::from_millis(200) || t.closing.is_some()
        })
    }
}

impl Interactive for Toaster {
    fn handle_key(&mut self, _k: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for toast in &mut self.toasts {
            if toast.dismissible {
                let hit = toast.hit.mouse(&m);
                if matches!(hit, Hit::Press) {
                    toast.close(self.now);
                    return Outcome::Changed;
                }
            }
        }
        Outcome::Ignored
    }
}

impl Default for Toaster {
    fn default() -> Self {
        Self::new()
    }
}

/// StatefulWidget that renders the toast stack.
pub struct ToastStack {
    theme: Option<Theme>,
    now: Option<Instant>,
}

impl ToastStack {
    pub fn new() -> Self {
        Self { theme: None, now: None }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
}

impl Default for ToastStack {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ToastStack {
    type State = Toaster;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);

        state.tick(now);

        let w = (area.width / 2).clamp(24, 60);
        let visible: Vec<_> = state.toasts.iter_mut().rev().take(state.max_visible).collect();

        let mut y_offset = match state.corner {
            ToastCorner::TopRight | ToastCorner::TopLeft => area.y,
            ToastCorner::BottomRight | ToastCorner::BottomLeft => area.bottom(),
        };

        for toast in visible {
            let lines = wrap(&toast.message, w.saturating_sub(4) as usize);
            let h = lines.len() as u16 + 3 + if toast.show_progress { 1 } else { 0 };

            let x_base = match state.corner {
                ToastCorner::TopRight | ToastCorner::BottomRight => area.right().saturating_sub(w + 1),
                ToastCorner::TopLeft | ToastCorner::BottomLeft => area.x + 1,
            };

            let y = match state.corner {
                ToastCorner::TopRight | ToastCorner::TopLeft => {
                    let ret = y_offset;
                    y_offset += h + 1;
                    ret
                }
                ToastCorner::BottomRight | ToastCorner::BottomLeft => {
                    y_offset = y_offset.saturating_sub(h + 1);
                    y_offset
                }
            };

            if y >= area.bottom() || y + h > area.bottom() {
                continue;
            }

            let mut x = x_base;
            if !state.reduce_motion {
                let age = now.saturating_duration_since(toast.created);
                let slide_p = (age.as_secs_f32() / 0.2).min(1.0);
                let ease_p = Easing::OutCubic.apply(slide_p);
                let slide_offset = ((1.0 - ease_p) * (w + 1) as f32) as u16;
                x = match state.corner {
                    ToastCorner::TopRight | ToastCorner::BottomRight => x + slide_offset,
                    ToastCorner::TopLeft | ToastCorner::BottomLeft => x.saturating_sub(slide_offset),
                };
            }

            let r = Rect { x, y, width: w, height: h }.intersection(area);
            if r.is_empty() {
                continue;
            }

            let bg = th.toast_bg;
            fill(buf, r, bg);
            
            // Left bar in variant color
            let bar_color = th.variant(toast.variant);
            for dy in 0..r.height {
                put(buf, r.x, r.y + dy, "┃", 1, st(bar_color, bg));
            }

            let text_x = r.x + 2;
            let text_w = r.width.saturating_sub(3);

            put(buf, text_x, r.y + 1, &toast.title, text_w, bold(st(th.text_variant(toast.variant), bg)));

            for (i, line) in lines.iter().enumerate() {
                let ly = r.y + 2 + i as u16;
                if ly >= r.bottom().saturating_sub(if toast.show_progress { 1 } else { 0 }) {
                    break;
                }
                put(buf, text_x, ly, line, text_w, st(th.foreground, bg));
            }

            // Progress bar if enabled
            if toast.show_progress
                && let Some(timeout) = toast.timeout {
                    let elapsed = now.saturating_duration_since(toast.created);
                    let p = 1.0 - (elapsed.as_secs_f32() / timeout.as_secs_f32()).min(1.0);
                    let bar_y = r.bottom().saturating_sub(1);
                    let bar_w = (r.width as f32 * p) as u16;
                    for dx in 0..bar_w {
                        put(buf, r.x + dx, bar_y, "▔", 1, st(bar_color, bg));
                    }
                }

            toast.hit.set_area(r);

            if let Some(closing) = toast.closing {
                let fade_p = now.saturating_duration_since(closing).as_secs_f32() / state.fade_duration.as_secs_f32();
                blend_area(buf, r, th.background, fade_p.min(1.0));
            }
        }
    }
}

/// Inline alert box (callout).
pub struct Callout {
    title: String,
    message: String,
    variant: Variant,
    dismissible: bool,
    border_style: CalloutBorder,
    theme: Option<Theme>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalloutBorder {
    LeftBar,
    Round,
}

impl Callout {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            variant: Variant::Default,
            dismissible: false,
            border_style: CalloutBorder::LeftBar,
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

    pub fn border_style(mut self, s: CalloutBorder) -> Self {
        self.border_style = s;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

/// State for dismissible callouts.
#[derive(Clone, Debug, Default)]
pub struct CalloutState {
    pub dismissed: bool,
    hit: HitBox,
}

impl CalloutState {
    pub fn new() -> Self {
        Self::default()
    }

}

impl Interactive for CalloutState {
    fn handle_key(&mut self, _k: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let hit = self.hit.mouse(&m);
        if matches!(hit, Hit::Press) {
            self.dismissed = true;
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }
}

impl StatefulWidget for Callout {
    type State = CalloutState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || state.dismissed {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        let var_color = th.variant(self.variant);

        match self.border_style {
            CalloutBorder::LeftBar => {
                fill(buf, area, bg);
                for y in area.top()..area.bottom() {
                    put(buf, area.x, y, "┃", 1, st(var_color, bg));
                }
            }
            CalloutBorder::Round => {
                fill(buf, area, bg);
                Border::Round.draw(buf, area, var_color, bg);
            }
        }

        let icon = match self.variant {
            Variant::Default => "ⓘ",
            Variant::Primary => "ⓘ",
            Variant::Success => "✓",
            Variant::Warning => "⚠",
            Variant::Error => "✖",
            Variant::Secondary | Variant::Accent => "ⓘ",
        };

        let text_x = area.x + if self.border_style == CalloutBorder::LeftBar { 2 } else { 3 };
        let text_w = area.width.saturating_sub(if self.border_style == CalloutBorder::LeftBar { 3 } else { 6 });

        put(buf, text_x, area.y + 1, icon, 1, st(var_color, bg));
        put(buf, text_x + 2, area.y + 1, &self.title, text_w.saturating_sub(2), bold(st(th.text, bg)));

        let lines = wrap(&self.message, text_w as usize);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y + 2 + i as u16;
            if y >= area.bottom().saturating_sub(1) {
                break;
            }
            put(buf, text_x, y, line, text_w, st(th.text_muted, bg));
        }

        state.hit.set_area(area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_auto_expire() {
        let mut toaster = Toaster::new();
        toaster.push(Toast::new("Test", "Message").timeout(Duration::from_secs(1)));
        let created = toaster.toasts[0].created;
        assert_eq!(toaster.toasts.len(), 1);
        toaster.tick(created + Duration::from_millis(500));
        assert_eq!(toaster.toasts.len(), 1);
        // Tick at expiry to close it
        toaster.tick(created + Duration::from_secs(2));
        // Tick again after fade duration to remove it
        toaster.tick(created + Duration::from_millis(2200));
        assert_eq!(toaster.toasts.len(), 0);
    }

    #[test]
    fn toast_dismiss_all() {
        let mut toaster = Toaster::new();
        let now = Instant::now();
        toaster.push(Toast::new("A", "1").no_timeout());
        toaster.push(Toast::new("B", "2").no_timeout());
        assert_eq!(toaster.toasts.len(), 2);
        toaster.dismiss_all(now);
        toaster.tick(now + Duration::from_millis(200));
        assert_eq!(toaster.toasts.len(), 0);
    }
}
