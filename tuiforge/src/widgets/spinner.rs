//! Spinners and loading indicators: animated feedback for background work.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 3);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! Spinner::new(SpinnerKind::Dots).label("Loading…").now(now).render(area, &mut buf);
//! LoadingIndicator::new().now(now).render(area, &mut buf);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::anim::{elapsed, frame_index};
use crate::draw::{put, put_centered, st};
use crate::theme::{self, Rgb, Theme};

/// Spinner animation variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinnerKind {
    Dots,
    Dots2,
    Line,
    Arc,
    Bounce,
    BouncingBar,
    Braille,
    Moon,
    Clock,
    Arrow,
    Toggle,
    Aesthetic,
    Circle,
    SquareCorners,
    Triangle,
    Pulse,
    Grow,
}

impl SpinnerKind {
    pub const ALL: &'static [SpinnerKind] = &[
        SpinnerKind::Dots,
        SpinnerKind::Dots2,
        SpinnerKind::Line,
        SpinnerKind::Arc,
        SpinnerKind::Bounce,
        SpinnerKind::BouncingBar,
        SpinnerKind::Braille,
        SpinnerKind::Moon,
        SpinnerKind::Clock,
        SpinnerKind::Arrow,
        SpinnerKind::Toggle,
        SpinnerKind::Aesthetic,
        SpinnerKind::Circle,
        SpinnerKind::SquareCorners,
        SpinnerKind::Triangle,
        SpinnerKind::Pulse,
        SpinnerKind::Grow,
    ];

    fn frames(&self) -> &'static [&'static str] {
        match self {
            SpinnerKind::Dots => &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            SpinnerKind::Dots2 => &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
            SpinnerKind::Line => &["-", "\\", "|", "/"],
            SpinnerKind::Arc => &["◜", "◠", "◝", "◞", "◡", "◟"],
            SpinnerKind::Bounce => &["⠁", "⠂", "⠄", "⠂"],
            SpinnerKind::BouncingBar => &["[    ]", "[=   ]", "[==  ]", "[=== ]", "[ ===]", "[  ==]", "[   =]", "[    ]", "[   =]", "[  ==]", "[ ===]", "[=== ]", "[==  ]", "[=   ]"],
            SpinnerKind::Braille => &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲", "⠳", "⠓"],
            SpinnerKind::Moon => &["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"],
            SpinnerKind::Clock => &["🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚", "🕛"],
            SpinnerKind::Arrow => &["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
            SpinnerKind::Toggle => &["⊶", "⊷"],
            SpinnerKind::Aesthetic => &["▰▱▱▱▱▱▱", "▰▰▱▱▱▱▱", "▰▰▰▱▱▱▱", "▰▰▰▰▱▱▱", "▰▰▰▰▰▱▱", "▰▰▰▰▰▰▱", "▰▰▰▰▰▰▰", "▰▱▱▱▱▱▱"],
            SpinnerKind::Circle => &["◡", "⊙", "◠"],
            SpinnerKind::SquareCorners => &["◰", "◳", "◲", "◱"],
            SpinnerKind::Triangle => &["◢", "◣", "◤", "◥"],
            SpinnerKind::Pulse => &["●", "◉", "⦿", "⦿", "◉"],
            SpinnerKind::Grow => &["▁", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃"],
        }
    }

    fn interval(&self) -> f32 {
        match self {
            SpinnerKind::Dots | SpinnerKind::Dots2 | SpinnerKind::Braille => 0.08,
            SpinnerKind::Line => 0.13,
            SpinnerKind::Arc => 0.10,
            SpinnerKind::Bounce => 0.12,
            SpinnerKind::BouncingBar => 0.08,
            SpinnerKind::Moon | SpinnerKind::Clock => 0.08,
            SpinnerKind::Arrow => 0.10,
            SpinnerKind::Toggle => 0.25,
            SpinnerKind::Aesthetic => 0.09,
            SpinnerKind::Circle => 0.12,
            SpinnerKind::SquareCorners | SpinnerKind::Triangle => 0.32,
            SpinnerKind::Pulse => 0.12,
            SpinnerKind::Grow => 0.07,
        }
    }
}

/// Animated spinner with optional label.
pub struct Spinner {
    kind: SpinnerKind,
    label: Option<String>,
    color: Option<Rgb>,
    epoch: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl Spinner {
    pub fn new(kind: SpinnerKind) -> Self {
        Self { kind, label: None, color: None, epoch: None, elapsed: None, theme: None }
    }

    pub fn label(mut self, s: &str) -> Self {
        self.label = Some(s.to_string());
        self
    }

    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    pub fn now(mut self, now: Instant) -> Self {
        self.epoch = Some(now);
        self
    }

    pub fn epoch(mut self, e: Instant) -> Self {
        self.epoch = Some(e);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Widget for Spinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = self.elapsed.or_else(|| self.epoch.map(|e| elapsed(e, Instant::now()))).unwrap_or(0.0);
        let frames = self.kind.frames();
        let fps = 1.0 / self.kind.interval();
        let idx = frame_index(el, fps, frames.len());
        let fg = self.color.unwrap_or(th.primary);
        let bg = th.background;

        let glyph = frames[idx];
        let mut x = area.x;
        
        if area.width >= 3 {
            put(buf, x, area.y, glyph, area.width, st(fg, bg));
            x += glyph.chars().count() as u16;
        }

        if let Some(lbl) = &self.label
            && x < area.right() {
                let w = area.right().saturating_sub(x + 1);
                if w > 0 {
                    put(buf, x + 1, area.y, lbl, w, st(th.text, bg));
                }
            }
    }
}

/// Textual LoadingIndicator: five dots pulsing through a gradient.
pub struct LoadingIndicator {
    dots: usize,
    color_a: Option<Rgb>,
    color_b: Option<Rgb>,
    epoch: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl LoadingIndicator {
    pub fn new() -> Self {
        Self { dots: 5, color_a: None, color_b: None, epoch: None, elapsed: None, theme: None }
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
        self.epoch = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
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
        let el = self.elapsed.or_else(|| self.epoch.map(|e| elapsed(e, Instant::now()))).unwrap_or(0.0);
        let bg = th.background;
        let color = self.color_a.unwrap_or(th.primary);
        let color_hi = self.color_b.unwrap_or(color.lighten(0.1));

        let w = self.dots as u16 * 2 - 1;
        let x0 = area.x + area.width.saturating_sub(w) / 2;

        for i in 0..self.dots {
            let blend = (el * 0.8 - i as f32 / 8.0).rem_euclid(1.0);
            let t = 1.0 - (blend - 0.5).abs() * 2.0;
            let c = bg.blend(color, 0.1 * (1.0 - t)).blend(color_hi, t.powf(2.0));
            put(buf, x0 + i as u16 * 2, area.y, "●", 1, st(c, bg));
        }
    }
}

/// Skeleton placeholder with animated shimmer.
pub struct Skeleton {
    lines: Vec<u16>,
    shape: SkeletonShape,
    epoch: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonShape {
    Text,
    Card,
    Avatar,
}

impl Skeleton {
    pub fn new() -> Self {
        Self { lines: vec![60, 45, 50], shape: SkeletonShape::Text, epoch: None, elapsed: None, theme: None }
    }

    pub fn lines(mut self, widths: &[u16]) -> Self {
        self.lines = widths.to_vec();
        self
    }

    pub fn shape(mut self, s: SkeletonShape) -> Self {
        self.shape = s;
        self
    }

    pub fn now(mut self, now: Instant) -> Self {
        self.epoch = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Skeleton {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = self.elapsed.or_else(|| self.epoch.map(|e| elapsed(e, Instant::now()))).unwrap_or(0.0);
        let base = th.surface;
        let hl = th.text_muted.blend(base, 0.7);

        let band_pos = (el * 2.0).rem_euclid(1.0);

        for (i, &w) in self.lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let width = w.min(area.width);
            for x in 0..width {
                let rel = x as f32 / area.width as f32;
                let dist = (rel - band_pos).abs();
                let fg = if dist < 0.1 { base.blend(hl, (1.0 - dist * 10.0).powf(2.0)) } else { base };
                put(buf, area.x + x, y, "─", 1, st(fg, th.background));
            }
        }
    }
}

/// Horizontally scrolling text marquee.
pub struct Marquee {
    text: String,
    speed: f32,
    gap: u16,
    epoch: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl Marquee {
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string(), speed: 10.0, gap: 3, epoch: None, elapsed: None, theme: None }
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
        self.epoch = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Widget for Marquee {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = self.elapsed.or_else(|| self.epoch.map(|e| elapsed(e, Instant::now()))).unwrap_or(0.0);
        let fg = th.text;
        let bg = th.background;

        let padded = format!("{}  {:width$}", self.text, "", width = self.gap as usize);
        let len = padded.chars().count();
        let offset = ((el * self.speed) as usize) % len;

        let mut displayed = String::new();
        for _ in 0..2 {
            displayed.push_str(&padded);
        }
        let chars: Vec<char> = displayed.chars().cycle().skip(offset).take(area.width as usize).collect();
        put(buf, area.x, area.y, &chars.iter().collect::<String>(), area.width, st(fg, bg));
    }
}

/// Simple blinking cursor or dot.
pub struct Blinker {
    glyph: &'static str,
    period: f32,
    epoch: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl Blinker {
    pub fn new() -> Self {
        Self { glyph: "●", period: 1.0, epoch: None, elapsed: None, theme: None }
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
        self.epoch = Some(now);
        self
    }

    pub fn elapsed(mut self, e: f32) -> Self {
        self.elapsed = Some(e);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
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
        let el = self.elapsed.or_else(|| self.epoch.map(|e| elapsed(e, Instant::now()))).unwrap_or(0.0);
        let visible = (el % self.period) < (self.period / 2.0);
        if visible {
            let fg = th.primary;
            let bg = th.background;
            put_centered(buf, area, self.glyph, st(fg, bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_frame_cycling() {
        let kind = SpinnerKind::Dots;
        let frames = kind.frames();
        assert_eq!(frames.len(), 10);
        let idx = frame_index(0.0, 12.5, frames.len());
        assert_eq!(idx, 0);
        let idx = frame_index(0.08, 12.5, frames.len());
        assert!(idx > 0 && idx < frames.len());
    }

    #[test]
    fn loading_indicator_renders() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        LoadingIndicator::new().elapsed(0.5).render(buf.area, &mut buf);
        assert_ne!(buf[(5, 0)].symbol(), " ");
    }
}
