//! Toast notifications: slide-in, auto-dismiss, stackable alerts.
//!
//! ```no_run
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let mut toaster = Toaster::default();
//! toaster.push(Toast::new("Saved", "File written successfully").variant(Variant::Success));
//! toaster.info("Quick message");
//! ToastStack::new().render(area, &mut buf, &mut toaster);
//! ```

use crate::anim::Easing;
use crate::core::{Hit, HitBox, Interactive, Outcome, is_press};
use crate::draw::{Border, Edge, blend_area, bold, fill, hbar, put, put_centered, st, wrap};
use crate::theme::{self, Theme, Variant};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::StatefulWidget;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastCorner {
    TopRight,
    BottomRight,
    TopLeft,
    BottomLeft,
}

impl From<ToastCorner> for ToastPosition {
    fn from(c: ToastCorner) -> Self {
        match c {
            ToastCorner::TopRight => ToastPosition::TopRight,
            ToastCorner::BottomRight => ToastPosition::BottomRight,
            ToastCorner::TopLeft => ToastPosition::TopLeft,
            ToastCorner::BottomLeft => ToastPosition::BottomLeft,
        }
    }
}

impl From<ToastPosition> for ToastCorner {
    fn from(p: ToastPosition) -> Self {
        match p {
            ToastPosition::TopRight => ToastCorner::TopRight,
            ToastPosition::BottomRight => ToastCorner::BottomRight,
            ToastPosition::TopLeft => ToastCorner::TopLeft,
            ToastPosition::BottomLeft => ToastCorner::BottomLeft,
            ToastPosition::TopCenter => ToastCorner::TopRight,
            ToastPosition::BottomCenter => ToastCorner::BottomRight,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastStyle {
    Card,
    Flat,
    Minimal,
    Pill,
    Outline,
    Banner,
    Glass,
    Progress,
    Action,
    Grouped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastAnim {
    Slide,
    Fade,
    Pop,
    None,
}

#[derive(Clone, Debug)]
pub struct Toast {
    id: u64,
    title: String,
    message: String,
    variant: Variant,
    timeout: Option<Duration>,
    dismissible: bool,
    show_progress: bool,
    created: Instant,
    closing: Option<Instant>,
    hit: HitBox,
    icon: Option<String>,
    style: Option<ToastStyle>,
    progress_value: f32,
    actions: Vec<String>,
    action_hits: Vec<HitBox>,
    focused_action: usize,
}

impl Toast {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            title: title.into(),
            message: message.into(),
            variant: Variant::Default,
            timeout: Some(Duration::from_secs(5)),
            dismissible: true,
            show_progress: false,
            created: Instant::now(),
            closing: None,
            hit: HitBox::default(),
            icon: None,
            style: None,
            progress_value: 0.0,
            actions: Vec::new(),
            action_hits: Vec::new(),
            focused_action: 0,
        }
    }
    pub fn id(&self) -> u64 {
        self.id
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
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn style(mut self, s: ToastStyle) -> Self {
        self.style = Some(s);
        self
    }
    pub fn progress_value(mut self, v: f32) -> Self {
        self.progress_value = v.clamp(0.0, 1.0);
        self
    }
    pub fn actions(mut self, labels: &[&str]) -> Self {
        self.actions = labels.iter().map(|&s| s.to_string()).collect();
        self.action_hits = vec![HitBox::default(); self.actions.len()];
        self
    }
    fn is_expired(&self, now: Instant) -> bool {
        if matches!(self.style, Some(ToastStyle::Progress)) && self.progress_value < 1.0 {
            return false;
        }
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

#[derive(Clone, Debug)]
pub struct Toaster {
    pub toasts: Vec<Toast>,
    max_visible: usize,
    pub corner: ToastCorner,
    position: ToastPosition,
    style: ToastStyle,
    anim: ToastAnim,
    pub bar: Edge,
    reduce_motion: bool,
    fade_duration: Duration,
    now: Instant,
    pause_on_hover: bool,
}

impl Toaster {
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            max_visible: 3,
            corner: ToastCorner::BottomRight,
            position: ToastPosition::BottomRight,
            style: ToastStyle::Card,
            anim: ToastAnim::Slide,
            bar: Edge::Thin,
            reduce_motion: false,
            fade_duration: Duration::from_millis(150),
            now: Instant::now(),
            pause_on_hover: false,
        }
    }
    pub fn bar(mut self, e: Edge) -> Self {
        self.bar = e;
        self
    }
    pub fn style(mut self, s: ToastStyle) -> Self {
        self.style = s;
        self
    }
    pub fn anim(mut self, a: ToastAnim) -> Self {
        self.anim = a;
        self
    }
    pub fn position(mut self, p: ToastPosition) -> Self {
        self.position = p;
        self.corner = p.into();
        self
    }
    pub fn corner(mut self, c: ToastCorner) -> Self {
        self.corner = c;
        self.position = c.into();
        self
    }
    pub fn pause_on_hover(mut self, v: bool) -> Self {
        self.pause_on_hover = v;
        self
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
    pub fn reduce_motion(mut self, v: bool) -> Self {
        self.reduce_motion = v;
        self
    }
    pub fn dismiss(&mut self, id: u64, now: Instant) {
        if let Some(toast) = self.toasts.iter_mut().find(|t| t.id == id) {
            toast.close(now);
        }
    }
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
        self.position = self.corner.into();
        for toast in &mut self.toasts {
            let hovered = self.pause_on_hover && toast.hit.hover;
            if !hovered && toast.is_expired(now) && toast.closing.is_none() {
                toast.close(now);
            }
        }
        self.toasts
            .retain(|t| !t.is_closed(now, self.fade_duration));
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
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        for toast in &mut self.toasts {
            if !toast.actions.is_empty() {
                match k.code {
                    KeyCode::Tab | KeyCode::Right => {
                        toast.focused_action = (toast.focused_action + 1) % toast.actions.len();
                        return Outcome::Consumed;
                    }
                    KeyCode::Left => {
                        if toast.focused_action > 0 {
                            toast.focused_action -= 1;
                        }
                        return Outcome::Consumed;
                    }
                    KeyCode::Enter => return Outcome::Changed,
                    _ => {}
                }
            }
        }
        Outcome::Ignored
    }
    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for toast in &mut self.toasts {
            for (i, hit) in toast.action_hits.iter_mut().enumerate() {
                let h = hit.mouse(&m);
                if matches!(h, Hit::Press) {
                    toast.focused_action = i;
                    return Outcome::Changed;
                }
            }
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

pub struct ToastStack {
    corner: Option<ToastCorner>,
    theme: Option<Theme>,
    now: Option<Instant>,
}
impl ToastStack {
    pub fn new() -> Self {
        Self {
            corner: None,
            theme: None,
            now: None,
        }
    }
    pub fn corner(mut self, c: ToastCorner) -> Self {
        self.corner = Some(c);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
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

fn default_icon(variant: Variant) -> &'static str {
    match variant {
        Variant::Default | Variant::Primary | Variant::Secondary | Variant::Accent => "•",
        Variant::Success => "✓",
        Variant::Warning => "▲",
        Variant::Error => "✗",
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
        if let Some(c) = self.corner {
            state.corner = c;
            state.position = c.into();
        }
        state.tick(now);
        let bar = state.bar;
        let position = state.position;
        let reduce_motion = state.reduce_motion;
        let anim = state.anim;
        let max_visible = state.max_visible;
        let overflow_count = state.toasts.len().saturating_sub(max_visible);
        let visible_toasts: Vec<_> = state.toasts.iter_mut().rev().take(max_visible).collect();
        let w = (area.width / 2).clamp(24, 60);
        let mut y_offset = match position {
            ToastPosition::TopLeft | ToastPosition::TopRight | ToastPosition::TopCenter => area.y,
            ToastPosition::BottomLeft
            | ToastPosition::BottomRight
            | ToastPosition::BottomCenter => area.bottom(),
        };
        for (idx, toast) in visible_toasts.into_iter().enumerate() {
            let style = toast.style.unwrap_or(state.style);
            if idx == 0 && overflow_count > 0 && matches!(style, ToastStyle::Grouped) {
                let h = 1;
                let x = match position {
                    ToastPosition::TopRight | ToastPosition::BottomRight => {
                        area.right().saturating_sub(w + 1)
                    }
                    ToastPosition::TopLeft | ToastPosition::BottomLeft => area.x + 1,
                    ToastPosition::TopCenter | ToastPosition::BottomCenter => {
                        area.x + (area.width.saturating_sub(w)) / 2
                    }
                };
                let y = match position {
                    ToastPosition::TopLeft | ToastPosition::TopRight | ToastPosition::TopCenter => {
                        let ret = y_offset;
                        y_offset += h + 1;
                        ret
                    }
                    ToastPosition::BottomLeft
                    | ToastPosition::BottomRight
                    | ToastPosition::BottomCenter => {
                        y_offset = y_offset.saturating_sub(h + 1);
                        y_offset
                    }
                };
                let r = Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                }
                .intersection(area);
                if !r.is_empty() {
                    let bg = th.panel;
                    fill(buf, r, bg);
                    let text = format!("+{} more", overflow_count);
                    put_centered(buf, r, &text, st(th.text_muted, bg));
                }
                continue;
            }
            let lines = wrap(&toast.message, w.saturating_sub(6) as usize);
            let h = match style {
                ToastStyle::Minimal | ToastStyle::Pill | ToastStyle::Banner => 1,
                ToastStyle::Progress => lines.len() as u16 + 4,
                ToastStyle::Action if !toast.actions.is_empty() => lines.len() as u16 + 5,
                _ => lines.len() as u16 + 3 + if toast.show_progress { 1 } else { 0 },
            };
            let x_base = match position {
                ToastPosition::TopRight | ToastPosition::BottomRight => {
                    area.right().saturating_sub(w + 1)
                }
                ToastPosition::TopLeft | ToastPosition::BottomLeft => area.x + 1,
                ToastPosition::TopCenter | ToastPosition::BottomCenter => {
                    area.x + (area.width.saturating_sub(w)) / 2
                }
            };
            let y = match position {
                ToastPosition::TopLeft | ToastPosition::TopRight | ToastPosition::TopCenter => {
                    let ret = y_offset;
                    y_offset += h + 1;
                    ret
                }
                ToastPosition::BottomLeft
                | ToastPosition::BottomRight
                | ToastPosition::BottomCenter => {
                    y_offset = y_offset.saturating_sub(h + 1);
                    y_offset
                }
            };
            if y >= area.bottom() || y + h > area.bottom() {
                continue;
            }
            let mut x = x_base;
            let mut alpha = 1.0;
            if !reduce_motion {
                let age = now.saturating_duration_since(toast.created);
                match anim {
                    ToastAnim::Slide => {
                        let slide_p = (age.as_secs_f32() / 0.2).min(1.0);
                        let ease_p = Easing::OutCubic.apply(slide_p);
                        let slide_offset = ((1.0 - ease_p) * (w + 1) as f32) as u16;
                        x = match position {
                            ToastPosition::TopRight | ToastPosition::BottomRight => {
                                x + slide_offset
                            }
                            ToastPosition::TopLeft | ToastPosition::BottomLeft => {
                                x.saturating_sub(slide_offset)
                            }
                            ToastPosition::TopCenter | ToastPosition::BottomCenter => x,
                        };
                    }
                    ToastAnim::Fade => {
                        let fade_p = (age.as_secs_f32() / 0.15).min(1.0);
                        alpha = fade_p;
                    }
                    ToastAnim::Pop => {
                        let pop_p = (age.as_secs_f32() / 0.2).min(1.0);
                        let ease_p = Easing::OutBack.apply(pop_p);
                        let scale = 0.6 + ease_p * 0.4;
                        let target_w = (w as f32 * scale) as u16;
                        let offset = (w - target_w) / 2;
                        x += offset;
                    }
                    ToastAnim::None => {}
                }
            }
            let r = Rect {
                x,
                y,
                width: w,
                height: h,
            }
            .intersection(area);
            if r.is_empty() {
                continue;
            }
            match style {
                ToastStyle::Card => {
                    let bg = th.toast_bg;
                    fill(buf, r, bg);
                    let bar_color = th.variant(toast.variant);
                    bar.draw(buf, r.x, r.y, r.height, false, bar_color, bg);
                    let text_x = r.x + 2;
                    let text_w = r.width.saturating_sub(3);
                    put(
                        buf,
                        text_x,
                        r.y + 1,
                        &toast.title,
                        text_w,
                        bold(st(th.text_variant(toast.variant), bg)),
                    );
                    let lines2 = wrap(&toast.message, text_w as usize);
                    for (i, line) in lines2.iter().enumerate() {
                        let ly = r.y + 2 + i as u16;
                        if ly
                            >= r.bottom()
                                .saturating_sub(if toast.show_progress { 1 } else { 0 })
                        {
                            break;
                        }
                        put(buf, text_x, ly, line, text_w, st(th.foreground, bg));
                    }
                    if toast.show_progress
                        && let Some(timeout) = toast.timeout
                    {
                        let elapsed = now.saturating_duration_since(toast.created);
                        let p = 1.0 - (elapsed.as_secs_f32() / timeout.as_secs_f32()).min(1.0);
                        hbar(buf, r.x, r.bottom() - 1, r.width, p, bar_color, bg);
                    }
                }
                ToastStyle::Flat => {
                    let bg = th.variant(toast.variant);
                    let fg = bg.text_on(0.87);
                    fill(buf, r, bg);
                    let text_x = r.x + 2;
                    let text_w = r.width.saturating_sub(4);
                    put(buf, text_x, r.y + 1, &toast.title, text_w, bold(st(fg, bg)));
                    for (i, line) in lines.iter().enumerate() {
                        let ly = r.y + 2 + i as u16;
                        if ly >= r.bottom() {
                            break;
                        }
                        put(buf, text_x, ly, line, text_w, st(fg, bg));
                    }
                }
                ToastStyle::Minimal => {
                    let bg = th.panel;
                    fill(buf, r, bg);
                    let icon = toast.icon.as_deref().unwrap_or(default_icon(toast.variant));
                    let text = format!("{} {}", icon, toast.message);
                    put(
                        buf,
                        r.x + 1,
                        r.y,
                        &text,
                        r.width.saturating_sub(2),
                        st(th.text_muted, bg),
                    );
                }
                ToastStyle::Pill => {
                    let bg = th.panel;
                    fill(buf, r, bg);
                    let icon = toast.icon.as_deref().unwrap_or(default_icon(toast.variant));
                    let icon_color = th.variant(toast.variant);
                    put(buf, r.x + 2, r.y, icon, 1, st(icon_color, bg));
                    put(
                        buf,
                        r.x + 4,
                        r.y,
                        &toast.message,
                        r.width.saturating_sub(6),
                        st(th.text, bg),
                    );
                }
                ToastStyle::Outline => {
                    let bg = th.background;
                    fill(buf, r, bg);
                    let border_color = th.variant(toast.variant);
                    Border::Round.draw(buf, r, border_color, bg);
                    let text_x = r.x + 3;
                    let text_w = r.width.saturating_sub(6);
                    put(
                        buf,
                        text_x,
                        r.y + 1,
                        &toast.title,
                        text_w,
                        bold(st(border_color, bg)),
                    );
                    for (i, line) in lines.iter().enumerate() {
                        let ly = r.y + 2 + i as u16;
                        if ly >= r.bottom().saturating_sub(1) {
                            break;
                        }
                        put(buf, text_x, ly, line, text_w, st(th.text, bg));
                    }
                }
                ToastStyle::Banner => {
                    let bg = th.variant(toast.variant);
                    let fg = bg.text_on(0.87);
                    fill(buf, r, bg);
                    let icon = toast.icon.as_deref().unwrap_or(default_icon(toast.variant));
                    put(buf, r.x + 2, r.y, icon, 1, st(fg, bg));
                    put(
                        buf,
                        r.x + 4,
                        r.y,
                        &toast.message,
                        r.width.saturating_sub(10),
                        st(fg, bg),
                    );
                    put(buf, r.right().saturating_sub(3), r.y, "✕", 1, st(fg, bg));
                }
                ToastStyle::Glass => {
                    blend_area(buf, r, th.surface, 0.3);
                    let border_color = th.border;
                    Border::Round.draw(buf, r, border_color, th.surface);
                    let text_x = r.x + 3;
                    let text_w = r.width.saturating_sub(6);
                    put(
                        buf,
                        text_x,
                        r.y + 1,
                        &toast.title,
                        text_w,
                        bold(st(th.text, th.surface)),
                    );
                    for (i, line) in lines.iter().enumerate() {
                        let ly = r.y + 2 + i as u16;
                        if ly >= r.bottom().saturating_sub(1) {
                            break;
                        }
                        put(buf, text_x, ly, line, text_w, st(th.text_muted, th.surface));
                    }
                }
                ToastStyle::Progress => {
                    let bg = th.toast_bg;
                    fill(buf, r, bg);
                    let text_x = r.x + 2;
                    let text_w = r.width.saturating_sub(4);
                    put(
                        buf,
                        text_x,
                        r.y + 1,
                        &toast.title,
                        text_w,
                        bold(st(th.text, bg)),
                    );
                    for (i, line) in lines.iter().enumerate() {
                        let ly = r.y + 2 + i as u16;
                        if ly >= r.bottom().saturating_sub(2) {
                            break;
                        }
                        put(buf, text_x, ly, line, text_w, st(th.text_muted, bg));
                    }
                    let bar_y = r.bottom().saturating_sub(1);
                    let bar_color = th.variant(toast.variant);
                    hbar(
                        buf,
                        r.x,
                        bar_y,
                        r.width,
                        toast.progress_value,
                        bar_color,
                        bg,
                    );
                }
                ToastStyle::Action => {
                    let bg = th.toast_bg;
                    fill(buf, r, bg);
                    let text_x = r.x + 2;
                    let text_w = r.width.saturating_sub(4);
                    put(
                        buf,
                        text_x,
                        r.y + 1,
                        &toast.title,
                        text_w,
                        bold(st(th.text, bg)),
                    );
                    for (i, line) in lines.iter().enumerate() {
                        let ly = r.y + 2 + i as u16;
                        if ly >= r.bottom().saturating_sub(3) {
                            break;
                        }
                        put(buf, text_x, ly, line, text_w, st(th.text_muted, bg));
                    }
                    if !toast.actions.is_empty() {
                        let button_y = r.bottom().saturating_sub(2);
                        let mut btn_x = text_x;
                        for (i, action) in toast.actions.iter().enumerate() {
                            let btn_w = action.len() as u16 + 4;
                            if btn_x + btn_w > r.right().saturating_sub(2) {
                                break;
                            }
                            let is_focused = i == toast.focused_action;
                            let btn_bg = if is_focused { th.primary } else { th.panel };
                            let btn_fg = if is_focused {
                                btn_bg.text_on(0.87)
                            } else {
                                th.text
                            };
                            let btn_r = Rect {
                                x: btn_x,
                                y: button_y,
                                width: btn_w,
                                height: 1,
                            };
                            fill(buf, btn_r, btn_bg);
                            put_centered(buf, btn_r, action, st(btn_fg, btn_bg));
                            toast.action_hits[i].set_area(btn_r);
                            btn_x += btn_w + 1;
                        }
                    }
                }
                _ => {}
            }
            toast.hit.set_area(r);
            if let Some(closing) = toast.closing {
                let fade_p = now.saturating_duration_since(closing).as_secs_f32()
                    / state.fade_duration.as_secs_f32();
                blend_area(buf, r, th.background, fade_p.min(1.0));
            } else if alpha < 1.0 {
                blend_area(buf, r, th.background, 1.0 - alpha);
            }
        }
    }
}

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
    Bar(Edge),
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
        self.theme = Some(*th);
        self
    }
}

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
            CalloutBorder::LeftBar | CalloutBorder::Bar(_) => {
                fill(buf, area, bg);
                let edge = if let CalloutBorder::Bar(e) = self.border_style {
                    e
                } else {
                    Edge::Thin
                };
                edge.draw(buf, area.x, area.y, area.height, false, var_color, bg);
            }
            CalloutBorder::Round => {
                fill(buf, area, bg);
                Border::Round.draw(buf, area, var_color, bg);
            }
        }
        let icon = default_icon(self.variant);
        let round = self.border_style == CalloutBorder::Round;
        let text_x = area.x + if round { 3 } else { 2 };
        let text_w = area.width.saturating_sub(if round { 6 } else { 3 });
        put(buf, text_x, area.y + 1, icon, 1, st(var_color, bg));
        put(
            buf,
            text_x + 2,
            area.y + 1,
            &self.title,
            text_w.saturating_sub(2),
            bold(st(th.text, bg)),
        );
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
        toaster.tick(created + Duration::from_secs(2));
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
    #[test]
    fn toast_progress_not_expired_until_complete() {
        let mut toaster = Toaster::new();
        let now = Instant::now();
        toaster.push(
            Toast::new("Loading", "Please wait")
                .style(ToastStyle::Progress)
                .progress_value(0.5)
                .timeout(Duration::from_millis(100)),
        );
        toaster.tick(now + Duration::from_millis(200));
        assert_eq!(toaster.toasts.len(), 1);
        toaster.toasts[0].progress_value = 1.0;
        toaster.tick(now + Duration::from_millis(300));

        toaster.tick(now + Duration::from_millis(500));
        assert_eq!(toaster.toasts.len(), 0);
    }

    #[test]
    fn corner_builder_writes_through() {
        let mut toaster = Toaster::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));

        ToastStack::new()
            .corner(ToastCorner::TopLeft)
            .render(buf.area, &mut buf, &mut toaster);

        assert_eq!(
            toaster.corner,
            ToastCorner::TopLeft,
            "corner builder should write through to state"
        );
    }

    #[test]
    fn corner_state_setter_still_works() {
        let mut toaster = Toaster::new().corner(ToastCorner::BottomLeft);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));

        ToastStack::new().render(buf.area, &mut buf, &mut toaster);

        assert_eq!(
            toaster.corner,
            ToastCorner::BottomLeft,
            "state corner setter should still work"
        );
    }
}
