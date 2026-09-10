//! Spinners and loading indicators: animated feedback for background work.
//!
//! The catalog in [`spinners`] ships every spinner from [yaspin] / cli-spinners (90) plus
//! tuiforge originals; each is a `const` [`SpinnerDef`] you can borrow, look up by name, or
//! replace with your own frames.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 3);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! Spinner::new(&spinners::DOTS).label("Loading…").now(now).render(area, &mut buf);
//! Spinner::new(&spinners::SPARKLE).color(Rgb(255, 200, 0)).now(now).render(area, &mut buf);
//!
//! // your own frames, from anywhere (a const, a config file, a `Vec<String>` you borrow)
//! const PULSE: SpinnerDef = SpinnerDef::new("pulse", 120, &["·", "•", "●", "•"]);
//! Spinner::new(&PULSE).now(now).render(area, &mut buf);
//! Spinner::frames(&["<", "^", ">", "v"], 150).now(now).render(area, &mut buf);
//!
//! // just the glyph, for your own text
//! let glyph = spinners::LINE.frame(anim::since(now));
//! LoadingIndicator::new().now(now).render(area, &mut buf);
//! ```
//!
//! [yaspin]: https://github.com/pavdmyt/yaspin

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::anim::{frame_index, since};
use crate::draw::{put, put_centered, st};
use crate::theme::{self, Rgb, Theme};

pub mod spinners;

/// A spinner: frames plus the delay between them. Every entry in [`spinners`] is one; declare
/// your own with [`SpinnerDef::new`] in a `const`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpinnerDef {
    pub name: &'static str,
    pub interval_ms: u16,
    pub frames: &'static [&'static str],
}

impl SpinnerDef {
    pub const fn new(
        name: &'static str,
        interval_ms: u16,
        frames: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            interval_ms,
            frames,
        }
    }

    /// The frame showing `elapsed` seconds into the loop.
    pub fn frame(&self, elapsed: f32) -> &'static str {
        frame_at(self.frames, self.interval_ms, elapsed)
    }

    /// Widest frame in cells; reserve this much so labels don't jitter.
    pub fn width(&self) -> u16 {
        frames_width(self.frames)
    }

    /// Look a spinner up by its catalog name (`"dots"`, `"bouncingBar"`, `"sparkle"`).
    pub fn by_name(name: &str) -> Option<&'static SpinnerDef> {
        spinners::ALL.iter().copied().find(|d| d.name == name)
    }
}

fn frame_at<'a>(frames: &[&'a str], interval_ms: u16, elapsed: f32) -> &'a str {
    if frames.is_empty() {
        return "";
    }
    let fps = 1000.0 / interval_ms.max(1) as f32;
    frames[frame_index(elapsed, fps, frames.len())]
}

fn frames_width(frames: &[&str]) -> u16 {
    frames.iter().map(|f| f.width()).max().unwrap_or(0) as u16
}

/// Phase clock shared by the loading widgets: explicit `elapsed` wins, otherwise seconds since
/// [`crate::anim::EPOCH`] at `now` (falling back to the wall clock).
fn phase(elapsed: Option<f32>, now: Option<Instant>) -> f32 {
    elapsed.unwrap_or_else(|| since(now.unwrap_or_else(Instant::now)))
}

/// Animated spinner with optional label.
pub struct Spinner<'a> {
    frames: &'a [&'a str],
    interval_ms: u16,
    label: Option<String>,
    color: Option<Rgb>,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl<'a> Spinner<'a> {
    /// A catalog spinner: `Spinner::new(&spinners::DOTS)`.
    pub fn new(def: &'a SpinnerDef) -> Self {
        Self::frames(def.frames, def.interval_ms)
    }

    /// Custom frames shown `interval_ms` apart.
    pub fn frames(frames: &'a [&'a str], interval_ms: u16) -> Self {
        Self {
            frames,
            interval_ms,
            label: None,
            color: None,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    pub fn label(mut self, s: &str) -> Self {
        self.label = Some(s.to_string());
        self
    }

    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    /// Current instant; the phase is measured from [`crate::anim::EPOCH`].
    pub fn now(mut self, now: Instant) -> Self {
        self.now = Some(now);
        self
    }

    /// Explicit phase in seconds (overrides `now`).
    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Cells the glyph column takes (widest frame).
    pub fn width(&self) -> u16 {
        frames_width(self.frames)
    }
}

impl Widget for Spinner<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = phase(self.elapsed, self.now);
        let glyph = frame_at(self.frames, self.interval_ms, el);
        let slot = frames_width(self.frames).min(area.width);
        let fg = self.color.unwrap_or(th.primary);
        let bg = th.background;

        put(buf, area.x, area.y, glyph, slot, st(fg, bg));

        if let Some(lbl) = &self.label {
            let x = area.x + slot + 1;
            if x < area.right() {
                put(buf, x, area.y, lbl, area.right() - x, st(th.text, bg));
            }
        }
    }
}

/// Textual LoadingIndicator: five dots pulsing through a gradient.
pub struct LoadingIndicator {
    dots: usize,
    color_a: Option<Rgb>,
    color_b: Option<Rgb>,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            dots: 5,
            color_a: None,
            color_b: None,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    pub fn dots(mut self, n: usize) -> Self {
        self.dots = n.max(1);
        self
    }

    pub fn colors(mut self, a: Rgb, b: Rgb) -> Self {
        self.color_a = Some(a);
        self.color_b = Some(b);
        self
    }

    pub fn now(mut self, now: Instant) -> Self {
        self.now = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for LoadingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for LoadingIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = phase(self.elapsed, self.now);
        let bg = th.background;
        let color = self.color_a.unwrap_or(th.primary);
        let color_hi = self.color_b.unwrap_or(color.lighten(0.1));

        let w = self.dots as u16 * 2 - 1;
        let x0 = area.x + area.width.saturating_sub(w) / 2;

        for i in 0..self.dots {
            let blend = (el * 0.8 - i as f32 / 8.0).rem_euclid(1.0);
            let t = 1.0 - (blend - 0.5).abs() * 2.0;
            let c = bg
                .blend(color, 0.1 * (1.0 - t))
                .blend(color_hi, t.powf(2.0));
            put(buf, x0 + i as u16 * 2, area.y, "●", 1, st(c, bg));
        }
    }
}

/// Horizontally scrolling text marquee.
pub struct Marquee {
    text: String,
    speed: f32,
    gap: u16,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl Marquee {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            speed: 10.0,
            gap: 3,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    pub fn speed(mut self, cells_per_sec: f32) -> Self {
        self.speed = cells_per_sec;
        self
    }

    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }

    pub fn now(mut self, now: Instant) -> Self {
        self.now = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for Marquee {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = phase(self.elapsed, self.now);
        let fg = th.text;
        let bg = th.background;

        let padded = format!("{}  {:width$}", self.text, "", width = self.gap as usize);
        let len = padded.chars().count();
        let offset = ((el * self.speed) as usize) % len;

        let displayed: String = padded
            .chars()
            .cycle()
            .skip(offset)
            .take(area.width as usize)
            .collect();
        put(buf, area.x, area.y, &displayed, area.width, st(fg, bg));
    }
}

/// Simple blinking cursor or dot.
pub struct Blinker {
    glyph: &'static str,
    period: f32,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl Blinker {
    pub fn new() -> Self {
        Self {
            glyph: "●",
            period: 1.0,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    pub fn glyph(mut self, g: &'static str) -> Self {
        self.glyph = g;
        self
    }

    pub fn period(mut self, p: f32) -> Self {
        self.period = p;
        self
    }

    pub fn now(mut self, now: Instant) -> Self {
        self.now = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for Blinker {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Blinker {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = phase(self.elapsed, self.now);
        let visible = (el % self.period) < (self.period / 2.0);
        if visible {
            put_centered(buf, area, self.glyph, st(th.primary, th.background));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_well_formed() {
        assert_eq!(spinners::YASPIN.len(), 90);
        assert_eq!(
            spinners::ALL.len(),
            spinners::YASPIN.len() + spinners::ORIGINALS.len()
        );
        let mut names = std::collections::HashSet::new();
        for d in spinners::ALL {
            assert!(!d.frames.is_empty(), "{} has no frames", d.name);
            assert!(d.interval_ms > 0, "{} has no interval", d.name);
            assert!(names.insert(d.name), "duplicate spinner name {}", d.name);
        }
        assert_eq!(
            SpinnerDef::by_name("bouncingBar").map(|d| d.frames.len()),
            Some(spinners::BOUNCING_BAR.frames.len())
        );
        assert!(SpinnerDef::by_name("nope").is_none());
    }

    #[test]
    fn frame_advances_with_time_and_wraps() {
        let d = &spinners::DOTS;
        assert_eq!(d.frame(0.0), d.frames[0]);
        assert_eq!(d.frame(0.08), d.frames[1]);
        assert_eq!(d.frame(0.08 * d.frames.len() as f32 + 0.001), d.frames[0]);
    }

    #[test]
    fn label_starts_after_the_widest_frame() {
        // bouncingBar frames are 6 cells wide; the label must not overlap any of them
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        Spinner::new(&spinners::BOUNCING_BAR)
            .label("Load")
            .elapsed(0.0)
            .render(buf.area, &mut buf);
        assert_eq!(buf[(7, 0)].symbol(), "L");
        // emoji frames are two cells (+ cli-spinners' trailing pad space): the label follows the measured width
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        Spinner::new(&spinners::MOON)
            .label("Load")
            .elapsed(0.0)
            .render(buf.area, &mut buf);
        assert_eq!(spinners::MOON.width(), 3);
        assert_eq!(buf[(4, 0)].symbol(), "L");
    }

    #[test]
    fn custom_frames_render() {
        let frames = ["a".to_string(), "b".to_string()];
        let borrowed: Vec<&str> = frames.iter().map(String::as_str).collect();
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        Spinner::frames(&borrowed, 100)
            .elapsed(0.1)
            .render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "b");
    }

    #[test]
    fn loading_indicator_renders() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        LoadingIndicator::new()
            .elapsed(0.5)
            .render(buf.area, &mut buf);
        assert_ne!(buf[(5, 0)].symbol(), " ");
    }
}
