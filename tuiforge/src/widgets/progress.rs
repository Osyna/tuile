//! Progress bars, rings and step indicators for long-running tasks.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 3);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! # let mut state = ProgressState::default();
//! ProgressBar::new().show_percentage(true).show_eta(true).render(area, &mut buf, &mut state);
//! state.set(0.75, now, Duration::from_millis(300));
//! ```

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{StatefulWidget, Widget};

use crate::anim::{Tween, elapsed};
use crate::core::{Interactive, Outcome};
use crate::draw::{put, st};
use crate::theme::{self, Rgb, Theme, Variant};

/// State for a progress bar: current value, animation, and ETA calculation.
#[derive(Clone, Debug)]
pub struct ProgressState {
    pub target: f32,
    anim: Tween,
    started: Option<Instant>,
    samples: Vec<(Instant, f32)>,
    indeterminate_phase: f32,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self { target: 0.0, anim: Tween::default(), started: None, samples: Vec::new(), indeterminate_phase: 0.0 }
    }
}

impl ProgressState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, v: f32, now: Instant, duration: Duration) {
        let _current = self.anim.value(now);
        self.target = v.clamp(0.0, 1.0);
        self.anim.go(self.target, now, duration);
        if self.started.is_none() && v > 0.0 {
            self.started = Some(now);
        }
        self.samples.push((now, self.target));
        if self.samples.len() > 10 {
            self.samples.remove(0);
        }
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.anim.value(now).clamp(0.0, 1.0)
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.anim.active(now)
    }

    fn eta(&self, now: Instant, current: f32) -> Option<Duration> {
        if current >= 1.0 || current <= 0.0 {
            return None;
        }
        let started = self.started?;
        if self.samples.len() < 2 {
            return None;
        }
        let elapsed_s = now.saturating_duration_since(started).as_secs_f32();
        if elapsed_s < 1.0 {
            return None;
        }
        let rate = current / elapsed_s;
        if rate <= 0.0 {
            return None;
        }
        let remaining = (1.0 - current) / rate;
        Some(Duration::from_secs_f32(remaining))
    }

    pub fn tick_indeterminate(&mut self, now: Instant, speed: f32) {
        if let Some(s) = self.started {
            self.indeterminate_phase = elapsed(s, now) * speed;
        }
    }
}

impl Interactive for ProgressState {
    fn handle_key(&mut self, _key: ratatui::crossterm::event::KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, _m: ratatui::crossterm::event::MouseEvent) -> Outcome {
        Outcome::Ignored
    }
}

/// Textual ProgressBar: `━` bar with `╸` half-end, `NN%` and ETA columns.
pub struct ProgressBar {
    label: Option<String>,
    show_bar: bool,
    show_percentage: bool,
    show_eta: bool,
    variant: Variant,
    indeterminate: bool,
    compact: bool,
    width_hint: Option<u16>,
    background: Option<Rgb>,
    now: Option<Instant>,
    theme: Option<Theme>,
}
impl ProgressBar {
    pub fn new() -> Self {
        Self {
            label: None,
            show_bar: true,
            show_percentage: true,
            show_eta: true,
            variant: Variant::Primary,
            indeterminate: false,
            compact: false,
            width_hint: None,
            now: None,
            theme: None,
            background: None,
        }
    }
    pub fn label(mut self, s: &str) -> Self {
        self.label = Some(s.to_string());
        self
    }

    pub fn show_bar(mut self, v: bool) -> Self {
        self.show_bar = v;
        self
    }

    pub fn show_percentage(mut self, v: bool) -> Self {
        self.show_percentage = v;
        self
    }

    pub fn show_eta(mut self, v: bool) -> Self {
        self.show_eta = v;
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn indeterminate(mut self) -> Self {
        self.indeterminate = true;
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn width_hint(mut self, w: u16) -> Self {
        self.width_hint = Some(w);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn background(mut self, bg: Rgb) -> Self {
        self.background = Some(bg);
        self
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ProgressBar {
    type State = ProgressState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);
        let bg = self.background.unwrap_or(th.surface);

        let pct_w = if self.show_percentage && !self.compact { 5 } else { 0 };
        let eta_w = if self.show_eta && !self.compact { 10 } else { 0 };
        let bar_w = area.width.saturating_sub(pct_w + eta_w + if pct_w > 0 || eta_w > 0 { 1 } else { 0 });

        if bar_w < 4 {
            return;
        }

        let y = area.y;
        let x = area.x;

        if self.show_bar {
            let track = bg.blend(th.foreground, 0.15);
            for dx in 0..bar_w {
                put(buf, x + dx, y, "━", 1, st(track, bg));
            }

            if self.indeterminate {
                state.tick_indeterminate(now, 30.0);
                let width = bar_w as f32;
                let hl = 0.25 * width;
                let total = width + hl;
                let mut start = state.indeterminate_phase % (2.0 * total);
                if start > total {
                    start = 2.0 * total - start;
                }
                start -= hl;
                let end = start + hl;
                let color = th.variant(self.variant);
                for dx in 0..bar_w {
                    let fx = dx as f32;
                    if fx >= start && fx < end {
                        put(buf, x + dx, y, "━", 1, st(color, bg));
                    }
                }
            } else {
                let p = state.value(now);
                let color = if p >= 1.0 { th.success } else { th.variant(self.variant) };
                let cells = p * bar_w as f32;
                let full = cells.floor() as u16;
                for dx in 0..full {
                    put(buf, x + dx, y, "━", 1, st(color, bg));
                }
                if full < bar_w && cells - full as f32 >= 0.5 {
                    put(buf, x + full, y, "╸", 1, st(color, bg));
                }
            }
        }

        let mut text_x = x + bar_w;
        if pct_w > 0 {
            if self.indeterminate {
                put(buf, text_x, y, " --% ", pct_w, st(th.text_muted, bg));
            } else {
                let p = state.value(now);
                let pct = format!(" {:>3}%", (p * 100.0).round() as u32);
                put(buf, text_x, y, &pct, pct_w, st(th.foreground, bg));
            }
            text_x += pct_w + 1;
        }

        if eta_w > 0 {
            if self.indeterminate {
                put(buf, text_x, y, " --:--:--", eta_w, st(th.text_muted, bg));
            } else {
                let p = state.value(now);
                let eta_text = if let Some(d) = state.eta(now, p) {
                    let s = d.as_secs();
                    format!("{:>9}", format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60))
                } else {
                    format!("{:>9}", "--:--:--")
                };
                put(buf, text_x, y, &eta_text, eta_w, st(th.text_muted, bg));
            }
        }
    }
}

/// Step progress: `●●●○○` dots for N steps.
pub struct StepProgress {
    total: usize,
    current: usize,
    filled: &'static str,
    empty: &'static str,
    theme: Option<Theme>,
}

impl StepProgress {
    pub fn new(total: usize) -> Self {
        Self { total, current: 0, filled: "●", empty: "○", theme: None }
    }

    pub fn current(mut self, n: usize) -> Self {
        self.current = n;
        self
    }

    pub fn filled(mut self, g: &'static str) -> Self {
        self.filled = g;
        self
    }

    pub fn empty(mut self, g: &'static str) -> Self {
        self.empty = g;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for StepProgress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.total == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let fg = th.primary;
        let dim = th.text_muted;
        let bg = th.background;

        for (x, i) in (area.x..).zip(0..self.total.min(area.width as usize)) {
            if x >= area.right() {
                break;
            }
            let glyph = if i < self.current { self.filled } else { self.empty };
            let color = if i < self.current { fg } else { dim };
            put(buf, x, area.y, glyph, 1, st(color, bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_state_animates() {
        let mut state = ProgressState::new();
        let now = Instant::now();
        state.set(0.5, now, Duration::from_millis(100));
        assert!(state.animating(now));
        assert!(state.value(now + Duration::from_millis(200)) > 0.49);
    }

    #[test]
    fn eta_calculation() {
        let mut state = ProgressState::new();
        let now = Instant::now();
        state.started = Some(now - Duration::from_secs(5));
        state.samples.push((now - Duration::from_secs(5), 0.0));
        state.samples.push((now, 0.5));
        state.target = 0.5;
        let eta = state.eta(now, 0.5);
        assert!(eta.is_some());
        let d = eta.unwrap();
        assert!(d.as_secs() >= 4 && d.as_secs() <= 6);
    }

    #[test]
    fn step_progress_renders() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        StepProgress::new(5).current(2).render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "●");
        assert_eq!(buf[(3, 0)].symbol(), "○");
    }
}
