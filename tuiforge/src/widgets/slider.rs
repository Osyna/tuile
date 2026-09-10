//! Sliders, range sliders, steppers, and rating controls.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 3);
//! # let mut buf = Buffer::empty(area);
//! # let key = KeyEvent::from(KeyCode::Right);
//! let mut state = SliderState::new(0.0, 0.0, 100.0, 1.0);
//! Slider::new().label("Volume").show_value(true).render(area, &mut buf, &mut state);
//! if state.handle_key(key).is_changed() { /* value changed */ }
//! ```

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::anim::{Easing, Tween};
use crate::core::{Hit, HitBox, Interactive, Look, Outcome, is_left_down, is_press, mouse_in, mouse_pos, wheel_delta};
use crate::draw::{Border, fill, put, st};
use crate::theme::{self, Theme, Variant};

// ───────────────────────────── slider ─────────────────────────────

/// Single-value slider with track, thumb, and optional ticks/value display.
#[derive(Clone, Debug)]
pub struct Slider {
    label: Option<String>,
    show_value: bool,
    format: Option<fn(f32) -> String>,
    ticks: bool,
    variant: Variant,
    duration: Option<Duration>,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl Slider {
    pub fn new() -> Self {
        Self {
            label: None,
            show_value: false,
            format: None,
            ticks: false,
            variant: Variant::Primary,
            duration: None,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    pub fn show_value(mut self, v: bool) -> Self {
        self.show_value = v;
        self
    }

    pub fn format(mut self, f: fn(f32) -> String) -> Self {
        self.format = Some(f);
        self
    }

    pub fn ticks(mut self, v: bool) -> Self {
        self.ticks = v;
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = Some(d);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for Slider {
    type State = SliderState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.duration = self.duration.unwrap_or(Duration::from_millis(150));
        if area.width < 10 || area.height < 3 {
            state.hit.set_area(Rect::default());
            state.track = Rect::default();
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };
        let bg = if look.focused { th.surface.blend(th.foreground, 0.05) } else { th.surface };
        fill(buf, area, bg);

        let border = if look.focused { th.border } else { th.border_blurred };
        Border::Tall.draw(buf, area, border, bg);

        let value_text = if self.show_value {
            if let Some(fmt) = self.format {
                fmt(state.value)
            } else {
                format!("{:.0}", state.value)
            }
        } else {
            String::new()
        };
        let label_text = self.label.as_deref().unwrap_or("");
        let right_w = value_text.width() as u16;

        state.track = Rect {
            x: area.x + 3,
            y: area.y + 1,
            width: area.width.saturating_sub(6 + right_w + if right_w > 0 { 1 } else { 0 }),
            height: 1,
        };
        state.hit.set_area(area);

        if state.track.width < 2 {
            return;
        }

        let t = state.anim.value(self.now.unwrap_or_else(Instant::now));
        let p = (t - state.min) / (state.max - state.min).max(0.001);
        let thumb_x = state.track.x + (p * (state.track.width - 1) as f32).round() as u16;

        let base = th.variant(self.variant);
        let mut filled = base;
        let mut rest = bg.blend(th.foreground, 0.2);
        if !look.enabled {
            filled = filled.blend(bg, 0.5);
            rest = rest.blend(bg, 0.5);
        }

        // ticks: 11 evenly spaced marks on the track itself, so no extra row and no collision
        // with the label row
        let tick_at = |x: u16| self.ticks && (0..=10u32).any(|k| x == state.track.x + ((k * (state.track.width as u32 - 1) + 5) / 10) as u16);
        for x in state.track.left()..state.track.right() {
            let (sym, color) = if x < thumb_x {
                (if tick_at(x) { "┿" } else { "━" }, filled)
            } else if x > thumb_x {
                (if tick_at(x) { "┼" } else { "─" }, rest)
            } else {
                ("●", if look.focused || look.hover { th.accent } else { filled })
            };
            if let Some(c) = buf.cell_mut((x, state.track.y)) {
                c.set_symbol(sym).set_fg(color.color()).set_bg(bg.color());
            }
        }

        if !value_text.is_empty() {
            let fg = if look.enabled { th.foreground } else { th.text_disabled };
            put(buf, state.track.right() + 1, state.track.y, &value_text, right_w, st(fg, bg).add_modifier(Modifier::BOLD));
        }

        if !label_text.is_empty() {
            let fg = if look.enabled { th.text } else { th.text_disabled };
            put(buf, area.x + 1, area.bottom() - 1, label_text, area.width.saturating_sub(2), st(fg, bg));
        }
    }
}

/// Slider state.
#[derive(Clone, Debug)]
pub struct SliderState {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub anim: Tween,
    pub dragging: bool,
    pub hit: HitBox,
    pub track: Rect,
    pub duration: Duration,
}

impl SliderState {
    pub fn new(value: f32, min: f32, max: f32, step: f32) -> Self {
        Self {
            value,
            min,
            max,
            step,
            anim: Tween::new(value),
            dragging: false,
            hit: HitBox::default(),
            track: Rect::default(),
            duration: Duration::from_millis(150),
        }
    }

    pub fn set(&mut self, v: f32, now: Instant, dur: Duration) {
        let clamped = v.clamp(self.min, self.max);
        if (self.value - clamped).abs() > 0.001 {
            self.value = clamped;
            if !self.dragging {
                self.anim.go_with(clamped, now, dur, Easing::OutCubic);
            } else {
                self.anim.set(clamped);
            }
        }
    }

    pub fn adjust(&mut self, delta: f32, now: Instant, dur: Duration) {
        self.set(self.value + delta, now, dur);
    }

    pub fn value_at(&self, x: u16) -> f32 {
        if self.track.width <= 1 {
            return self.min;
        }
        let p = (x.saturating_sub(self.track.x) as f32 / (self.track.width - 1) as f32).clamp(0.0, 1.0);
        let raw = self.min + p * (self.max - self.min);
        let stepped = ((raw - self.min) / self.step).round() * self.step + self.min;
        stepped.clamp(self.min, self.max)
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.anim.active(now)
    }
}

impl Interactive for SliderState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let now = Instant::now();
        let dur = self.duration;
        let multiplier = if key.modifiers.contains(KeyModifiers::SHIFT) { 10.0 } else { 1.0 };
        match key.code {
            KeyCode::Left => {
                self.adjust(-self.step * multiplier, now, dur);
                Outcome::Changed
            }
            KeyCode::Right => {
                self.adjust(self.step * multiplier, now, dur);
                Outcome::Changed
            }
            KeyCode::PageDown => {
                self.adjust(-self.step * 10.0, now, dur);
                Outcome::Changed
            }
            KeyCode::PageUp => {
                self.adjust(self.step * 10.0, now, dur);
                Outcome::Changed
            }
            KeyCode::Home => {
                self.set(self.min, now, dur);
                Outcome::Changed
            }
            KeyCode::End => {
                self.set(self.max, now, dur);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let hit = self.hit.mouse(&m);
        
        if let Some(delta) = wheel_delta(&m)
            && mouse_in(self.track, &m) {
                let now = Instant::now();
                self.adjust(self.step * delta as f32, now, self.duration);
                return Outcome::Changed;
            }

        match hit {
            Hit::Press if mouse_in(self.track, &m) => {
                self.dragging = true;
                let v = self.value_at(m.column);
                self.set(v, Instant::now(), Duration::ZERO);
                Outcome::Changed
            }
            Hit::Drag if self.dragging => {
                let v = self.value_at(m.column);
                self.set(v, Instant::now(), Duration::ZERO);
                Outcome::Changed
            }
            Hit::Click | Hit::Cancel => {
                self.dragging = false;
                Outcome::Consumed
            }
            Hit::HoverChanged => Outcome::Consumed,
            _ => Outcome::Ignored,
        }
    }
}

// ───────────────────────────── range slider ─────────────────────────────

/// Two-thumb range slider.
#[derive(Clone, Debug)]
pub struct RangeSlider {
    label: Option<String>,
    variant: Variant,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl RangeSlider {
    pub fn new() -> Self {
        Self {
            label: None,
            variant: Variant::Primary,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for RangeSlider {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for RangeSlider {
    type State = RangeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 10 || area.height < 3 {
            state.hit.set_area(Rect::default());
            state.track = Rect::default();
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };
        let bg = if look.focused { th.surface.blend(th.foreground, 0.05) } else { th.surface };
        fill(buf, area, bg);

        let border = if look.focused { th.border } else { th.border_blurred };
        Border::Tall.draw(buf, area, border, bg);

        let label_text = format!("{:.0} – {:.0}", state.lo, state.hi);
        let right_w = label_text.width() as u16;

        state.track = Rect {
            x: area.x + 3,
            y: area.y + 1,
            width: area.width.saturating_sub(6 + right_w + 1),
            height: 1,
        };
        state.hit.set_area(area);

        if state.track.width < 4 {
            return;
        }

        let t_lo = state.anim_lo.value(self.now.unwrap_or_else(Instant::now));
        let t_hi = state.anim_hi.value(self.now.unwrap_or_else(Instant::now));
        let p_lo = (t_lo - state.min) / (state.max - state.min).max(0.001);
        let p_hi = (t_hi - state.min) / (state.max - state.min).max(0.001);
        let x_lo = state.track.x + (p_lo * (state.track.width - 1) as f32).round() as u16;
        let x_hi = state.track.x + (p_hi * (state.track.width - 1) as f32).round() as u16;

        let base = th.variant(self.variant);
        let mut filled = base;
        let mut rest = bg.blend(th.foreground, 0.2);
        if !look.enabled {
            filled = filled.blend(bg, 0.5);
            rest = rest.blend(bg, 0.5);
        }

        for x in state.track.left()..state.track.right() {
            let (sym, color) = if x == x_lo || x == x_hi {
                ("●", if look.focused || look.hover { th.accent } else { filled })
            } else if x > x_lo && x < x_hi {
                ("━", filled)
            } else {
                ("─", rest)
            };
            if let Some(c) = buf.cell_mut((x, state.track.y)) {
                c.set_symbol(sym).set_fg(color.color()).set_bg(bg.color());
            }
        }

        let fg = if look.enabled { th.foreground } else { th.text_disabled };
        put(buf, state.track.right() + 1, state.track.y, &label_text, right_w, st(fg, bg).add_modifier(Modifier::BOLD));

        if let Some(label) = &self.label {
            let fg = if look.enabled { th.text } else { th.text_disabled };
            put(buf, area.x + 1, area.bottom() - 1, label, area.width.saturating_sub(2), st(fg, bg));
        }
    }
}

/// Range slider state.
#[derive(Clone, Debug)]
pub struct RangeState {
    pub lo: f32,
    pub hi: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub active_thumb: bool, // false = lo, true = hi
    pub anim_lo: Tween,
    pub anim_hi: Tween,
    pub dragging: Option<bool>,
    pub hit: HitBox,
    pub track: Rect,
}

impl RangeState {
    pub fn new(lo: f32, hi: f32, min: f32, max: f32, step: f32) -> Self {
        Self {
            lo,
            hi,
            min,
            max,
            step,
            active_thumb: false,
            anim_lo: Tween::new(lo),
            anim_hi: Tween::new(hi),
            dragging: None,
            hit: HitBox::default(),
            track: Rect::default(),
        }
    }

    pub fn value_at(&self, x: u16) -> f32 {
        if self.track.width <= 1 {
            return self.min;
        }
        let p = (x.saturating_sub(self.track.x) as f32 / (self.track.width - 1) as f32).clamp(0.0, 1.0);
        let raw = self.min + p * (self.max - self.min);
        let stepped = ((raw - self.min) / self.step).round() * self.step + self.min;
        stepped.clamp(self.min, self.max)
    }

    pub fn set_lo(&mut self, v: f32, now: Instant, dur: Duration) {
        let clamped = v.clamp(self.min, self.hi);
        if (self.lo - clamped).abs() > 0.001 {
            self.lo = clamped;
            if self.dragging.is_none() {
                self.anim_lo.go_with(clamped, now, dur, Easing::OutCubic);
            } else {
                self.anim_lo.set(clamped);
            }
        }
    }

    pub fn set_hi(&mut self, v: f32, now: Instant, dur: Duration) {
        let clamped = v.clamp(self.lo, self.max);
        if (self.hi - clamped).abs() > 0.001 {
            self.hi = clamped;
            if self.dragging.is_none() {
                self.anim_hi.go_with(clamped, now, dur, Easing::OutCubic);
            } else {
                self.anim_hi.set(clamped);
            }
        }
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.anim_lo.active(now) || self.anim_hi.active(now)
    }
}

impl Interactive for RangeState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let now = Instant::now();
        let multiplier = if key.modifiers.contains(KeyModifiers::SHIFT) { 10.0 } else { 1.0 };
        match key.code {
            KeyCode::Enter => {
                self.active_thumb = !self.active_thumb;
                Outcome::Consumed
            }
            KeyCode::Left => {
                if self.active_thumb {
                    self.set_hi(self.hi - self.step * multiplier, now, Duration::from_millis(150));
                } else {
                    self.set_lo(self.lo - self.step * multiplier, now, Duration::from_millis(150));
                }
                Outcome::Changed
            }
            KeyCode::Right => {
                if self.active_thumb {
                    self.set_hi(self.hi + self.step * multiplier, now, Duration::from_millis(150));
                } else {
                    self.set_lo(self.lo + self.step * multiplier, now, Duration::from_millis(150));
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let hit = self.hit.mouse(&m);

        if let Some(delta) = wheel_delta(&m)
            && mouse_in(self.track, &m) {
                let now = Instant::now();
                if self.active_thumb {
                    self.set_hi(self.hi + self.step * delta as f32, now, Duration::from_millis(150));
                } else {
                    self.set_lo(self.lo + self.step * delta as f32, now, Duration::from_millis(150));
                }
                return Outcome::Changed;
            }

        match hit {
            Hit::Press if mouse_in(self.track, &m) => {
                let v = self.value_at(m.column);
                let dist_lo = (v - self.lo).abs();
                let dist_hi = (v - self.hi).abs();
                let thumb = dist_lo < dist_hi;
                self.active_thumb = !thumb;
                self.dragging = Some(thumb);
                if thumb {
                    self.set_lo(v, Instant::now(), Duration::ZERO);
                } else {
                    self.set_hi(v, Instant::now(), Duration::ZERO);
                }
                Outcome::Changed
            }
            Hit::Drag => {
                if let Some(thumb) = self.dragging {
                    let v = self.value_at(m.column);
                    if thumb {
                        self.set_lo(v, Instant::now(), Duration::ZERO);
                    } else {
                        self.set_hi(v, Instant::now(), Duration::ZERO);
                    }
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            Hit::Click | Hit::Cancel => {
                self.dragging = None;
                Outcome::Consumed
            }
            Hit::HoverChanged => Outcome::Consumed,
            _ => Outcome::Ignored,
        }
    }
}

// ───────────────────────────── stepper ─────────────────────────────

/// Numeric stepper with +/- buttons.
#[derive(Clone, Debug)]
pub struct Stepper {
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Stepper {
    pub fn new() -> Self {
        Self {
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Stepper {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for Stepper {
    type State = StepperState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit_minus = Rect::default();
        state.hit_plus = Rect::default();
        if area.height < 1 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: false, enabled: self.enabled };
        let bg = th.background;

        // `[ - ]  42  [ + ]` — value column sized for the widest possible number
        let value_text = format!("{}", state.value);
        let max_w = format!("{}", state.max).width().max(format!("{}", state.min).width()) as u16;
        let btn_w = 5u16;
        let total_w = btn_w + 1 + max_w + 2 + 1 + btn_w;
        if total_w > area.width {
            return;
        }
        let x = area.x;
        state.hit_minus = Rect { x, y: area.y, width: btn_w, height: 1 };
        state.hit_plus = Rect { x: x + btn_w + 1 + max_w + 2 + 1, y: area.y, width: btn_w, height: 1 };

        let btn_fg = if look.enabled { th.text } else { th.text_disabled };
        let btn_bg = if look.enabled { th.panel } else { bg };
        let at_min = state.value <= state.min;
        let at_max = state.value >= state.max;
        let minus_fg = if at_min { th.text_disabled } else { btn_fg };
        let plus_fg = if at_max { th.text_disabled } else { btn_fg };
        put(buf, state.hit_minus.x, area.y, "[ - ]", btn_w, st(minus_fg, btn_bg).add_modifier(Modifier::BOLD));
        put(buf, state.hit_plus.x, area.y, "[ + ]", btn_w, st(plus_fg, btn_bg).add_modifier(Modifier::BOLD));

        let (val_fg, val_bg) = if look.focused {
            (th.cursor_fg, th.cursor_bg)
        } else if look.enabled {
            (th.text, th.surface)
        } else {
            (th.text_disabled, bg)
        };
        let val = format!(" {value_text:>width$} ", width = max_w as usize);
        put(buf, x + btn_w + 1, area.y, &val, max_w + 2, st(val_fg, val_bg).add_modifier(Modifier::BOLD));
    }
}

/// Stepper state.
#[derive(Clone, Debug, Default)]
pub struct StepperState {
    pub value: i64,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub hit_minus: Rect,
    pub hit_plus: Rect,
}

impl StepperState {
    pub fn new(value: i64, min: i64, max: i64, step: i64) -> Self {
        Self {
            value,
            min,
            max,
            step,
            hit_minus: Rect::default(),
            hit_plus: Rect::default(),
        }
    }

    pub fn increment(&mut self) -> bool {
        let new_val = (self.value + self.step).clamp(self.min, self.max);
        if new_val != self.value {
            self.value = new_val;
            true
        } else {
            false
        }
    }

    pub fn decrement(&mut self) -> bool {
        let new_val = (self.value - self.step).clamp(self.min, self.max);
        if new_val != self.value {
            self.value = new_val;
            true
        } else {
            false
        }
    }
}

impl Interactive for StepperState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('+') => {
                if self.increment() {
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            KeyCode::Down | KeyCode::Char('-') => {
                if self.decrement() {
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            // ponytail: digit editing omitted; add when needed
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        if is_left_down(&m) {
            if self.hit_minus.contains(pos) {
                return if self.decrement() { Outcome::Changed } else { Outcome::Consumed };
            }
            if self.hit_plus.contains(pos) {
                return if self.increment() { Outcome::Changed } else { Outcome::Consumed };
            }
        }
        Outcome::Ignored
    }
}

// ───────────────────────────── rating ─────────────────────────────

/// Star rating widget.
#[derive(Clone, Debug)]
pub struct Rating {
    max: u8,
    allow_clear: bool,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Rating {
    pub fn new() -> Self {
        Self {
            max: 5,
            allow_clear: true,
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn max(mut self, m: u8) -> Self {
        self.max = m;
        self
    }

    pub fn allow_clear(mut self, v: bool) -> Self {
        self.allow_clear = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Rating {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for Rating {
    type State = RatingState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        // ★ is an ambiguous-width glyph that overdraws its neighbour in most fonts: 2 cells each
        if area.width < self.max as u16 * 2 - 1 || area.height == 0 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: false, enabled: self.enabled };
        let bg = th.background;

        let display = state.hover_value.unwrap_or(state.value);

        for i in 0..self.max {
            let x = area.x + i as u16 * 2;
            let r = Rect { x, y: area.y, width: 2, height: 1 };
            state.hits.push(r);

            let filled = i < display;
            let sym = if filled { "★" } else { "☆" };
            let mut fg = if filled { th.warning } else { th.text_muted };
            if !look.enabled {
                fg = fg.blend(bg, 0.5);
            }
            let mut style = st(fg, bg);
            if look.focused && state.hover_value.is_none() && i == state.value.saturating_sub(1) {
                style = st(th.accent, bg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }
            put(buf, x, area.y, sym, 1, style);
            put(buf, x + 1, area.y, " ", 1, st(bg, bg));
        }
    }
}

/// Rating state.
#[derive(Clone, Debug, Default)]
pub struct RatingState {
    pub value: u8,
    pub hover_value: Option<u8>,
    pub hits: Vec<Rect>,
}

impl RatingState {
    pub fn new(value: u8) -> Self {
        Self { value, hover_value: None, hits: vec![] }
    }

    pub fn set(&mut self, v: u8, max: u8, allow_clear: bool) -> bool {
        let new_val = if allow_clear && v == self.value { 0 } else { v.min(max) };
        if new_val != self.value {
            self.value = new_val;
            true
        } else {
            false
        }
    }
}

impl Interactive for RatingState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let max = self.hits.len() as u8;
        if max == 0 {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Left => {
                if self.value > 0 {
                    self.value -= 1;
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            KeyCode::Right => {
                if self.value < max {
                    self.value += 1;
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        if let Some(idx) = self.hits.iter().position(|r| r.contains(pos)) {
            match m.kind {
                ratatui::crossterm::event::MouseEventKind::Moved => {
                    self.hover_value = Some((idx + 1) as u8);
                    Outcome::Consumed
                }
                ratatui::crossterm::event::MouseEventKind::Down(_) => {
                    let v = (idx + 1) as u8;
                    let max = self.hits.len() as u8;
                    self.hover_value = None;
                    if self.set(v, max, true) {
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    }
                }
                _ => Outcome::Consumed,
            }
        } else {
            if m.kind == ratatui::crossterm::event::MouseEventKind::Moved {
                self.hover_value = None;
            }
            Outcome::Ignored
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_value_at_maps_position() {
        let state = SliderState::new(50.0, 0.0, 100.0, 1.0);
        let track = Rect { x: 10, y: 5, width: 21, height: 1 };
        let mut s = state;
        s.track = track;
        assert!((s.value_at(10) - 0.0).abs() < 0.1);
        assert!((s.value_at(20) - 50.0).abs() < 1.0);
        assert!((s.value_at(30) - 100.0).abs() < 0.1);
    }

    #[test]
    fn range_slider_maintains_order() {
        let mut state = RangeState::new(20.0, 80.0, 0.0, 100.0, 1.0);
        let now = Instant::now();
        state.set_lo(90.0, now, Duration::ZERO);
        assert_eq!(state.lo, 80.0); // clamped to hi
        state.set_hi(10.0, now, Duration::ZERO);
        assert_eq!(state.hi, 80.0); // clamped to lo
    }

    #[test]
    fn stepper_increments() {
        let mut state = StepperState::new(5, 0, 10, 1);
        assert!(state.increment());
        assert_eq!(state.value, 6);
        assert!(state.decrement());
        assert_eq!(state.value, 5);
    }

    #[test]
    fn rating_handles_clicks() {
        let mut state = RatingState::new(0);
        state.hits = vec![Rect::default(); 5];
        assert!(state.set(3, 5, false));
        assert_eq!(state.value, 3);
        assert!(state.set(3, 5, true)); // allow_clear toggles
        assert_eq!(state.value, 0);
    }
}
