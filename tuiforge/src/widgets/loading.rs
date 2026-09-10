//! Loading states: twenty indeterminate loaders behind one builder, painted skeleton
//! placeholders, and a dimming overlay for "this panel is busy".
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 6);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! Loader::new(LoaderStyle::Scanner).label("Syncing").now(now).render(Rect { height: 1, ..area }, &mut buf);
//! Loader::new(LoaderStyle::Typewriter).label("Indexing the workspace…").now(now).render(Rect { height: 1, ..area }, &mut buf);
//! Loader::new(LoaderStyle::Radar).now(now).render(area, &mut buf); // uses every row
//! Skeleton::new().shape(SkeletonShape::Card).now(now).render(area, &mut buf);
//! LoadingOverlay::new(Loader::new(LoaderStyle::Sweep)).message("Fetching rows…").now(now).render(area, &mut buf);
//! ```
//!
//! Every loader takes `.now(Instant)` (phase from [`crate::anim::EPOCH`]) or an explicit
//! `.elapsed(secs)`, so headless renders are deterministic. Solid cells are painted as
//! background colour; eighth-blocks only appear where a cell is partially filled.

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::anim::{blink, ease_in_out_cubic, pulse, since};
use crate::draw::{LOWER_BLOCKS, blend_area, fill, hbar, put, put_centered, st};
use crate::theme::{self, Rgb, Theme};

fn phase(elapsed: Option<f32>, now: Option<Instant>) -> f32 {
    elapsed.unwrap_or_else(|| since(now.unwrap_or_else(Instant::now)))
}

/// 0→1→0 triangle wave with period 1.
fn tri(t: f32) -> f32 {
    1.0 - (2.0 * t.rem_euclid(1.0) - 1.0).abs()
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// Deterministic pseudo-random 0..1 for a (cell, seed) pair.
fn noise(a: u32, b: u32) -> f32 {
    (hash(a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B)) & 0xffff) as f32 / 65535.0
}

/// Paint one cell as solid colour.
fn cell(buf: &mut Buffer, x: u16, y: u16, c: Rgb) {
    put(buf, x, y, " ", 1, st(c, c));
}

// ───────────────────────────── Loader ─────────────────────────────

/// Which animation a [`Loader`] plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoaderStyle {
    /// Bright cell with a fading tail sweeping back and forth (Knight Rider).
    Scanner,
    /// The same tail travelling one way and wrapping.
    Comet,
    /// Material indeterminate: an eased segment sliding across the track.
    Sweep,
    /// Bar fills from the left, then drains from the left.
    FillDrain,
    /// The whole bar breathes.
    Pulse,
    /// Barber-pole stripes marching right.
    Stripes,
    /// Hue-cycling rainbow flowing along the bar.
    Rainbow,
    /// Three dots chasing along a dotted track.
    Snake,
    /// LED chase: one lit segment with a dim trail over spaced cells.
    Chase,
    /// `▰▰▰▱▱▱` filling and emptying.
    Blocks,
    /// Travelling sine wave of eighth-blocks.
    Wave,
    /// Bouncing spectrum bars; uses every row of its area.
    Equalizer,
    /// A ball bouncing between brackets.
    Bounce,
    /// Expanding rings `(((●)))`.
    Ping,
    /// Scrolling ECG trace with a glowing spike.
    Heartbeat,
    /// `label`, `label.`, `label..`, `label...`
    Ellipsis,
    /// Label with a sweeping highlight band.
    Shimmer,
    /// Label typed out with a cursor, held, cleared, repeated.
    Typewriter,
    /// Matrix rain; uses every row of its area.
    Rain,
    /// Radar sweep over a dot field with blips; uses every row of its area.
    Radar,
}

impl LoaderStyle {
    pub const ALL: [LoaderStyle; 20] = [
        LoaderStyle::Scanner,
        LoaderStyle::Comet,
        LoaderStyle::Sweep,
        LoaderStyle::FillDrain,
        LoaderStyle::Pulse,
        LoaderStyle::Stripes,
        LoaderStyle::Rainbow,
        LoaderStyle::Snake,
        LoaderStyle::Chase,
        LoaderStyle::Blocks,
        LoaderStyle::Wave,
        LoaderStyle::Bounce,
        LoaderStyle::Ping,
        LoaderStyle::Heartbeat,
        LoaderStyle::Ellipsis,
        LoaderStyle::Shimmer,
        LoaderStyle::Typewriter,
        LoaderStyle::Equalizer,
        LoaderStyle::Rain,
        LoaderStyle::Radar,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LoaderStyle::Scanner => "Scanner",
            LoaderStyle::Comet => "Comet",
            LoaderStyle::Sweep => "Sweep",
            LoaderStyle::FillDrain => "FillDrain",
            LoaderStyle::Pulse => "Pulse",
            LoaderStyle::Stripes => "Stripes",
            LoaderStyle::Rainbow => "Rainbow",
            LoaderStyle::Snake => "Snake",
            LoaderStyle::Chase => "Chase",
            LoaderStyle::Blocks => "Blocks",
            LoaderStyle::Wave => "Wave",
            LoaderStyle::Equalizer => "Equalizer",
            LoaderStyle::Bounce => "Bounce",
            LoaderStyle::Ping => "Ping",
            LoaderStyle::Heartbeat => "Heartbeat",
            LoaderStyle::Ellipsis => "Ellipsis",
            LoaderStyle::Shimmer => "Shimmer",
            LoaderStyle::Typewriter => "Typewriter",
            LoaderStyle::Rain => "Rain",
            LoaderStyle::Radar => "Radar",
        }
    }

    /// Draws into every row of its area; the others use one row, vertically centred.
    pub fn multi_row(self) -> bool {
        matches!(
            self,
            LoaderStyle::Equalizer | LoaderStyle::Rain | LoaderStyle::Radar
        )
    }

    /// The label *is* the animated text (no bar).
    pub fn textual(self) -> bool {
        matches!(
            self,
            LoaderStyle::Ellipsis | LoaderStyle::Shimmer | LoaderStyle::Typewriter
        )
    }
}

/// An indeterminate loading animation in one of twenty styles.
#[derive(Clone, Debug)]
pub struct Loader {
    style: LoaderStyle,
    label: Option<String>,
    color: Option<Rgb>,
    color2: Option<Rgb>,
    speed: f32,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl Loader {
    pub fn new(style: LoaderStyle) -> Self {
        Self {
            style,
            label: None,
            color: None,
            color2: None,
            speed: 1.0,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    /// Text left of the bar; for textual styles the text being animated.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// Main colour (default `$primary`).
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }

    /// Secondary colour for two-tone styles (default `$accent`).
    pub fn color2(mut self, c: Rgb) -> Self {
        self.color2 = Some(c);
        self
    }

    /// Playback rate; `1.0` is the designed speed.
    pub fn speed(mut self, s: f32) -> Self {
        self.speed = s.max(0.0);
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

struct Palette {
    color: Rgb,
    color2: Rgb,
    bg: Rgb,
    /// Dim track behind bars.
    track: Rgb,
    /// Dim glyph colour for dotted tracks.
    track_fg: Rgb,
    text: Rgb,
    text_muted: Rgb,
}

impl Widget for Loader {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = phase(self.elapsed, self.now) * self.speed;
        let p = Palette {
            color: self.color.unwrap_or(th.primary),
            color2: self.color2.unwrap_or(th.accent),
            bg: th.background,
            track: th.panel,
            track_fg: th.text_muted.blend(th.background, 0.5),
            text: th.text,
            text_muted: th.text_muted,
        };

        if self.style.textual() {
            let text = self.label.as_deref().unwrap_or("Loading");
            let row = Rect {
                y: area.y + area.height / 2,
                height: 1,
                ..area
            };
            match self.style {
                LoaderStyle::Ellipsis => ellipsis(buf, row, text, el, &p),
                LoaderStyle::Shimmer => shimmer(buf, row, text, el, &p),
                _ => typewriter(buf, row, text, el, &p),
            }
            return;
        }

        let mut r = area;
        if let Some(label) = self.label.as_deref().filter(|l| !l.is_empty()) {
            let lw = (label.width() as u16 + 1).min(area.width.saturating_sub(4));
            let y = if self.style.multi_row() {
                area.y
            } else {
                area.y + area.height / 2
            };
            put(buf, area.x, y, label, lw, st(p.text, p.bg));
            r = Rect {
                x: area.x + lw,
                width: area.width - lw,
                ..area
            };
        }
        if !self.style.multi_row() {
            r = Rect {
                y: r.y + r.height / 2,
                height: 1,
                ..r
            };
        }
        if r.width < 3 {
            return;
        }

        match self.style {
            LoaderStyle::Scanner => scanner(buf, r, el, &p, false),
            LoaderStyle::Comet => scanner(buf, r, el, &p, true),
            LoaderStyle::Sweep => sweep(buf, r, el, &p),
            LoaderStyle::FillDrain => fill_drain(buf, r, el, &p),
            LoaderStyle::Pulse => pulse_bar(buf, r, el, &p),
            LoaderStyle::Stripes => stripes(buf, r, el, &p),
            LoaderStyle::Rainbow => rainbow(buf, r, el, &p),
            LoaderStyle::Snake => snake(buf, r, el, &p),
            LoaderStyle::Chase => chase(buf, r, el, &p),
            LoaderStyle::Blocks => blocks(buf, r, el, &p),
            LoaderStyle::Wave => wave(buf, r, el, &p),
            LoaderStyle::Equalizer => equalizer(buf, r, el, &p),
            LoaderStyle::Bounce => bounce(buf, r, el, &p),
            LoaderStyle::Ping => ping(buf, r, el, &p),
            LoaderStyle::Heartbeat => heartbeat(buf, r, el, &p),
            LoaderStyle::Rain => rain(buf, r, el, &p),
            LoaderStyle::Radar => radar(buf, r, el, &p),
            LoaderStyle::Ellipsis | LoaderStyle::Shimmer | LoaderStyle::Typewriter => {
                unreachable!("textual styles handled above")
            }
        }
    }
}

// ── single-row bars ──

fn scanner(buf: &mut Buffer, r: Rect, el: f32, p: &Palette, wrap: bool) {
    let w = r.width as f32;
    let tail = 4.0;
    for i in 0..r.width {
        let x = i as f32;
        let d = if wrap {
            let head = (el * 14.0).rem_euclid(w);
            (head - x).rem_euclid(w)
        } else {
            let head = tri(el * 0.55) * (w - 1.0);
            (x - head).abs()
        };
        let a = (1.0 - d / tail).max(0.0).powf(1.6);
        let c = if a > 0.92 {
            p.color.lighten(0.12)
        } else {
            p.track.blend(p.color, a)
        };
        cell(buf, r.x + i, r.y, c);
    }
}

fn sweep(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let w = r.width as i32;
    let seg = (w / 4).max(2);
    let t = ease_in_out_cubic((el * 0.7).rem_euclid(1.0));
    let start = (t * (w + seg) as f32).round() as i32 - seg;
    for i in 0..w {
        let lit = i >= start && i < start + seg;
        cell(
            buf,
            r.x + i as u16,
            r.y,
            if lit { p.color } else { p.track },
        );
    }
}

fn fill_drain(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let t = (el * 0.45).rem_euclid(1.0);
    if t < 0.5 {
        hbar(
            buf,
            r.x,
            r.y,
            r.width,
            ease_in_out_cubic(t * 2.0),
            p.color,
            p.track,
        );
    } else {
        let drained = (ease_in_out_cubic((t - 0.5) * 2.0) * r.width as f32).round() as u16;
        for i in 0..r.width {
            cell(
                buf,
                r.x + i,
                r.y,
                if i < drained { p.track } else { p.color },
            );
        }
    }
}

fn pulse_bar(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let a = 0.2 + 0.8 * pulse(el, 1.6);
    let c = p.track.blend(p.color, a);
    fill(buf, r, c);
}

fn stripes(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let shift = (el * 8.0) as i32;
    let dark = p.color.darken(0.14);
    for i in 0..r.width {
        let band = (i as i32 - shift).rem_euclid(4) < 2;
        cell(buf, r.x + i, r.y, if band { p.color } else { dark });
    }
}

fn rainbow(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let (_, s, l) = p.color.to_hsl();
    let (s, l) = (s.max(0.55), l.clamp(0.45, 0.6));
    for i in 0..r.width {
        let hue = i as f32 / r.width as f32 * 360.0 - el * 120.0;
        cell(buf, r.x + i, r.y, Rgb::from_hsl(hue, s, l));
    }
}

fn snake(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let w = r.width as i32;
    let head = (el * 10.0) as i32 % w;
    for i in 0..w {
        let behind = (head - i).rem_euclid(w);
        let (sym, c) = match behind {
            0 => ("●", p.color),
            1 => ("●", p.color.blend(p.bg, 0.3)),
            2 => ("●", p.color.blend(p.bg, 0.6)),
            _ => ("·", p.track_fg),
        };
        put(buf, r.x + i as u16, r.y, sym, 1, st(c, p.bg));
    }
}

fn chase(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let n = r.width.div_ceil(2) as i32;
    let head = (el * 7.0) as i32 % n;
    for k in 0..n {
        let behind = (head - k).rem_euclid(n);
        let a = match behind {
            0 => 1.0,
            1 => 0.55,
            2 => 0.25,
            _ => 0.0,
        };
        cell(buf, r.x + (k * 2) as u16, r.y, p.track.blend(p.color, a));
    }
}

fn blocks(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    // 2-cell painted segments with 1-cell gaps, filling then emptying
    let n = (r.width + 1) / 3;
    if n == 0 {
        return;
    }
    let k = (tri(el * 0.35) * n as f32).round() as u16;
    for i in 0..n {
        let c = if i < k { p.color } else { p.track };
        let x = r.x + i * 3;
        cell(buf, x, r.y, c);
        if x + 1 < r.right() {
            cell(buf, x + 1, r.y, c);
        }
    }
}

fn wave(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    for i in 0..r.width {
        let h = ((i as f32 * 0.65 - el * 5.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let idx = 1 + (h * 7.0).round() as usize;
        let c = p.color.blend(p.color2, h);
        put(buf, r.x + i, r.y, LOWER_BLOCKS[idx.min(8)], 1, st(c, p.bg));
    }
}

fn bounce(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    put(buf, r.x, r.y, "[", 1, st(p.text_muted, p.bg));
    put(buf, r.right() - 1, r.y, "]", 1, st(p.text_muted, p.bg));
    let inner = r.width.saturating_sub(2);
    if inner == 0 {
        return;
    }
    let t = (el * 0.6).rem_euclid(1.0);
    let pos_f = tri(t) * (inner - 1) as f32;
    let pos = pos_f.round() as u16;
    let dir: i32 = if t < 0.5 { 1 } else { -1 };
    // ghost one cell behind
    let ghost = pos as i32 - dir;
    if ghost >= 0 && (ghost as u16) < inner {
        put(
            buf,
            r.x + 1 + ghost as u16,
            r.y,
            "∙",
            1,
            st(p.color.blend(p.bg, 0.55), p.bg),
        );
    }
    put(buf, r.x + 1 + pos, r.y, "●", 1, st(p.color, p.bg));
}

fn ping(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    let rings = ((r.width.saturating_sub(1)) / 2).min(3) as i32;
    if rings == 0 {
        return;
    }
    let cx = r.x as i32 + r.width as i32 / 2;
    let t = (el * 0.9).rem_euclid(1.0);
    let grown = (t * (rings as f32 + 1.0)) as i32; // 0..=rings
    put(buf, cx as u16, r.y, "●", 1, st(p.color, p.bg));
    for j in 1..=rings {
        if j > grown {
            break;
        }
        let a = 1.0 - (grown - j) as f32 / (rings as f32 + 0.5);
        let c = p.color.blend(p.bg, 1.0 - a.clamp(0.15, 1.0));
        put(buf, (cx - j) as u16, r.y, "(", 1, st(c, p.bg));
        put(buf, (cx + j) as u16, r.y, ")", 1, st(c, p.bg));
    }
}

fn heartbeat(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    // eighth-block heights of one beat: flat, P wave, QRS spike, dip, flat
    const BEAT: [u8; 22] = [
        1, 1, 1, 1, 1, 2, 1, 1, 3, 8, 2, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    ];
    let offset = (el * 14.0) as usize;
    let dim = p.color.blend(p.bg, 0.45);
    for i in 0..r.width {
        let h = BEAT[(i as usize + offset) % BEAT.len()] as usize;
        let c = if h >= 3 { p.color.lighten(0.1) } else { dim };
        put(buf, r.x + i, r.y, LOWER_BLOCKS[h], 1, st(c, p.bg));
    }
}

// ── textual ──

fn ellipsis(buf: &mut Buffer, r: Rect, text: &str, el: f32, p: &Palette) {
    let n = ((el * 2.5) as usize) % 4;
    let used = put(
        buf,
        r.x,
        r.y,
        text,
        r.width.saturating_sub(3),
        st(p.text, p.bg),
    );
    let dots = ".".repeat(n);
    put(
        buf,
        r.x + used,
        r.y,
        &dots,
        3,
        st(p.color, p.bg).add_modifier(Modifier::BOLD),
    );
}

fn shimmer(buf: &mut Buffer, r: Rect, text: &str, el: f32, p: &Palette) {
    let len = text.chars().count() as f32;
    let band = (el * 0.9).rem_euclid(1.4) / 1.4 * (len + 8.0) - 4.0;
    let mut x = r.x;
    for (i, ch) in text.chars().enumerate() {
        if x >= r.right() {
            break;
        }
        let d = (i as f32 - band).abs();
        let a = (1.0 - d / 3.5).max(0.0).powf(1.4);
        let fg = p.text_muted.blend(p.color.lighten(0.15), a);
        let mut style = st(fg, p.bg);
        if a > 0.5 {
            style = style.add_modifier(Modifier::BOLD);
        }
        let mut s = [0u8; 4];
        x += put(buf, x, r.y, ch.encode_utf8(&mut s), r.right() - x, style);
    }
}

fn typewriter(buf: &mut Buffer, r: Rect, text: &str, el: f32, p: &Palette) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let cps = 14.0;
    let type_t = n as f32 / cps;
    let total = type_t + 1.2 + 0.5;
    let t = el.rem_euclid(total);
    let shown = if t < type_t {
        (t * cps) as usize
    } else if t < type_t + 1.2 {
        n
    } else {
        0
    };
    let visible: String = chars[..shown.min(n)].iter().collect();
    let used = put(
        buf,
        r.x,
        r.y,
        &visible,
        r.width.saturating_sub(1),
        st(p.text, p.bg),
    );
    if blink(el, 0.8) {
        put(buf, r.x + used, r.y, "▎", 1, st(p.color, p.bg));
    }
}

// ── multi-row scenes ──

fn equalizer(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    fill(buf, r, p.bg);
    let bar_w: u16 = if r.width >= 24 { 2 } else { 1 };
    let pitch = bar_w + 1;
    let n = (r.width + 1) / pitch;
    let rows = r.height as f32;
    for k in 0..n {
        let kf = k as f32;
        let level = ((el * (2.2 + (k % 5) as f32 * 0.5) + kf * 1.3).sin() * 0.5 + 0.5) * 0.6
            + ((el * 3.7 + kf * 0.7).sin() * 0.5 + 0.5) * 0.4;
        let eighths = (level.clamp(0.05, 1.0) * rows * 8.0).round() as u16;
        for row in 0..r.height {
            let y = r.bottom() - 1 - row;
            let cell_eighths = eighths.saturating_sub(row * 8).min(8);
            if cell_eighths == 0 {
                continue;
            }
            let frac = row as f32 / rows.max(1.0);
            let c = p.color.blend(p.color2, frac);
            for dx in 0..bar_w {
                let x = r.x + k * pitch + dx;
                if x >= r.right() {
                    break;
                }
                if cell_eighths == 8 {
                    cell(buf, x, y, c);
                } else {
                    put(
                        buf,
                        x,
                        y,
                        LOWER_BLOCKS[cell_eighths as usize],
                        1,
                        st(c, p.bg),
                    );
                }
            }
        }
    }
}

fn rain(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    // ASCII + Latin-1 only: half-width katakana is tofu in most terminal fonts
    const GLYPHS: &[char] = &[
        '0', '1', '2', '3', '4', '5', '7', '8', '9', 'A', 'C', 'E', 'F', 'H', 'K', 'N', 'P', 'R',
        'T', 'V', 'X', 'Z', ':', '·', '=', '*', '+', '-', '¦', '|', '_', '<', '>', '%', '#', '@',
    ];
    fill(buf, r, p.bg);
    if r.height < 2 {
        scanner(buf, r, el, p, true);
        return;
    }
    let h = r.height as f32;
    let tail = (h * 0.7).clamp(3.0, 9.0);
    let head_c = p.color.lighten(0.35);
    for i in 0..r.width {
        let col = i as u32;
        if noise(col, 11) < 0.22 {
            continue; // leave some columns empty so streams read as streams
        }
        let speed = 3.0 + noise(col, 7) * 6.0;
        let start = noise(col, 3) * (h + tail);
        let head = (el * speed + start).rem_euclid(h + tail);
        let tick = (el * 1.5) as u32 + (noise(col, 5) * 4.0) as u32;
        for row in 0..r.height {
            let d = head - row as f32;
            if !(0.0..tail).contains(&d) {
                continue;
            }
            let g = GLYPHS[(hash(
                col.wrapping_mul(31) ^ (row as u32).wrapping_mul(17) ^ tick.wrapping_mul(7),
            ) % GLYPHS.len() as u32) as usize];
            let c = if d < 1.0 {
                head_c
            } else {
                p.bg.blend(p.color, (1.0 - d / tail).powf(1.3))
            };
            let mut s = [0u8; 4];
            put(
                buf,
                r.x + i,
                r.y + row,
                g.encode_utf8(&mut s),
                1,
                st(c, p.bg),
            );
        }
    }
}

fn radar(buf: &mut Buffer, r: Rect, el: f32, p: &Palette) {
    fill(buf, r, p.bg);
    if r.height < 3 {
        scanner(buf, r, el, p, false);
        return;
    }
    let ry = (r.height as f32 - 1.0) / 2.0;
    let rx = (ry * 2.0).min((r.width as f32 - 1.0) / 2.0);
    let cx = r.x as f32 + (r.width as f32 - 1.0) / 2.0;
    let cy = r.y as f32 + ry;
    let beam = el * 2.2;
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            let nx = (x as f32 - cx) / rx;
            let ny = (y as f32 - cy) / ry;
            let d = (nx * nx + ny * ny).sqrt();
            if d > 1.0 {
                continue;
            }
            let ang = ny.atan2(nx);
            let behind = (beam - ang).rem_euclid(std::f32::consts::TAU);
            let a = (1.0 - behind / 2.6).max(0.0).powf(1.8);
            let blip = noise(x as u32, y as u32 ^ 0x5151) > 0.965 && d < 0.9;
            let on_axis_h = (y as f32 - cy).abs() < 0.5;
            let on_axis_v = (x as f32 - cx).abs() < 0.5;
            let ring = (d - 0.5).abs() < 0.06 || d > 0.9;
            let (sym, fg) = if blip {
                ("●", p.color2.blend(p.track_fg, 1.0 - a.max(0.2)))
            } else if on_axis_h && on_axis_v {
                ("┼", p.track_fg.blend(p.color, a))
            } else if on_axis_h {
                ("─", p.track_fg.blend(p.color, a * 0.6))
            } else if on_axis_v {
                ("│", p.track_fg.blend(p.color, a * 0.6))
            } else if ring {
                ("∘", p.track_fg.blend(p.color, a * 0.7))
            } else {
                ("·", p.track_fg.blend(p.color.lighten(0.1), a))
            };
            let bg = p.bg.blend(p.color, a * 0.35);
            put(buf, x, y, sym, 1, st(fg, bg));
        }
    }
}

// ───────────────────────────── Skeleton ─────────────────────────────

/// Painted skeleton placeholder with a diagonal shimmer band.
#[derive(Clone, Debug)]
pub struct Skeleton {
    lines: Vec<u16>,
    shape: SkeletonShape,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

/// What the placeholder stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonShape {
    /// Paragraph: one bar per entry in `.lines(..)` (cells), a blank row between when there's room.
    Text,
    /// Hero image block on top, then a title and two body lines.
    Card,
    /// Avatar block with a name and a handle beside it.
    Avatar,
    /// Stacked avatar rows: a list waiting for its items.
    List,
    /// Header cells and rows of column cells; 2-4 columns depending on width.
    Table,
    /// Bars of varying height along a baseline.
    Chart,
}

impl Skeleton {
    pub fn new() -> Self {
        Self {
            lines: vec![60, 45, 50],
            shape: SkeletonShape::Text,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    /// Bar widths in cells (clamped to the area).
    pub fn lines(mut self, widths: &[u16]) -> Self {
        self.lines = widths.to_vec();
        self
    }

    pub fn shape(mut self, s: SkeletonShape) -> Self {
        self.shape = s;
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

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

struct Shimmer {
    base: Rgb,
    hl: Rgb,
    band: f32,
    area: Rect,
}

impl Shimmer {
    fn color_at(&self, x: u16, y: u16) -> Rgb {
        // a wide, smooth light sweeping left→right with a gentle lean (half a cell per row)
        let rel = (x as f32 - self.area.x as f32 + (y as f32 - self.area.y as f32) * 0.5)
            / self.area.width.max(1) as f32;
        let d = (rel - self.band).abs();
        const HALF: f32 = 0.28;
        if d >= HALF {
            return self.base;
        }
        let a = ((d / HALF * std::f32::consts::PI).cos() + 1.0) * 0.5;
        self.base.blend(self.hl, a)
    }

    fn bar(&self, buf: &mut Buffer, x: u16, y: u16, w: u16) {
        for i in 0..w {
            cell(buf, x + i, y, self.color_at(x + i, y));
        }
    }

    fn block(&self, buf: &mut Buffer, r: Rect) {
        for y in r.top()..r.bottom() {
            self.bar(buf, r.x, y, r.width);
        }
    }

    /// Rows of bars with `widths` in cells, one blank row between when the height allows.
    fn lines(&self, buf: &mut Buffer, r: Rect, widths: &[u16]) {
        let step = if r.height as usize >= widths.len() * 2 - 1 {
            2
        } else {
            1
        };
        for (i, &w) in widths.iter().enumerate() {
            let y = r.y + i as u16 * step;
            if y >= r.bottom() {
                break;
            }
            self.bar(buf, r.x, y, w.min(r.width));
        }
    }
}

/// Column fractions for a table skeleton of the given width.
fn table_columns(width: u16) -> &'static [u16] {
    if width >= 36 {
        &[28, 18, 34, 20]
    } else if width >= 18 {
        &[40, 25, 35]
    } else {
        &[55, 45]
    }
}

impl Widget for Skeleton {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let el = phase(self.elapsed, self.now);
        // bars two shades above the surface, the sweep two more: visible without shouting
        let base = th.surface.blend(th.text_muted, 0.22);
        let sh = Shimmer {
            base,
            hl: base.blend(th.text_muted, 0.45),
            band: (el * 0.45).rem_euclid(1.0) * 1.8 - 0.4,
            area,
        };
        let pct = |p: u16, of: u16| (of as u32 * p as u32 / 100) as u16;

        match self.shape {
            SkeletonShape::Text => sh.lines(buf, area, &self.lines),
            SkeletonShape::Card => {
                // hero block ≈ 40% of the height (min 2 rows), a title, a blank, then body lines
                let hero_h = (area.height * 40 / 100).clamp(2, area.height);
                sh.block(
                    buf,
                    Rect {
                        height: hero_h,
                        ..area
                    },
                );
                let w = area.width;
                let title_y = area.y + hero_h + 1;
                if title_y < area.bottom() {
                    sh.bar(buf, area.x, title_y, pct(60, w));
                }
                let body = Rect {
                    y: title_y + 2,
                    height: area.bottom().saturating_sub(title_y + 2),
                    ..area
                };
                if !body.is_empty() {
                    sh.lines(buf, body, &[pct(100, w), pct(75, w)]);
                }
            }
            SkeletonShape::Avatar | SkeletonShape::List => {
                let av_w = 4u16.min(area.width);
                let tx = area.x + av_w + 1;
                let rest = area.right().saturating_sub(tx);
                let item_h = if self.shape == SkeletonShape::List {
                    3
                } else {
                    area.height.min(2)
                };
                let mut y = area.y;
                while y + 2 <= area.bottom() {
                    sh.block(
                        buf,
                        Rect {
                            x: area.x,
                            y,
                            width: av_w,
                            height: 2,
                        },
                    );
                    if rest > 0 {
                        // name and handle, widths nudged per item so the list isn't a grid
                        let seed = (y - area.y) as u32;
                        sh.bar(buf, tx, y, pct(55 + (noise(seed, 1) * 25.0) as u16, rest));
                        sh.bar(
                            buf,
                            tx,
                            y + 1,
                            pct(30 + (noise(seed, 2) * 15.0) as u16, rest),
                        );
                    }
                    if self.shape == SkeletonShape::Avatar {
                        break;
                    }
                    y += item_h;
                }
            }
            SkeletonShape::Table => {
                let cols = table_columns(area.width);
                let total: u16 = cols.iter().sum();
                let header = Shimmer {
                    base: base.blend(th.text_muted, 0.25),
                    ..sh
                };
                let step = if area.height >= 6 { 2 } else { 1 };
                let mut y = area.y;
                let mut row = 0u32;
                while y < area.bottom() {
                    let mut x = area.x;
                    for (ci, &c) in cols.iter().enumerate() {
                        let col_w = pct(c * 100 / total, area.width).max(2);
                        let cell_w = if row == 0 {
                            col_w.saturating_sub(1)
                        } else {
                            // data cells vary a little; the last column is a short status/number
                            let jitter = if col_w >= 6 {
                                (noise(row, ci as u32) * 3.0) as u16
                            } else {
                                0
                            };
                            col_w.saturating_sub(1 + jitter).max(1)
                        };
                        let painter = if row == 0 { &header } else { &sh };
                        painter.bar(buf, x, y, cell_w.min(area.right().saturating_sub(x)));
                        x += col_w;
                        if x >= area.right() {
                            break;
                        }
                    }
                    y += if row == 0 { 2 } else { step };
                    row += 1;
                }
            }
            SkeletonShape::Chart => {
                if area.height < 2 {
                    sh.bar(buf, area.x, area.y, area.width);
                    return;
                }
                let bar_w: u16 = if area.width >= 30 { 3 } else { 2 };
                let pitch = bar_w + 1;
                let n = (area.width + 1) / pitch;
                let plot_h = area.height - 1;
                for k in 0..n {
                    let level = 0.2 + noise(k as u32, 99) * 0.8;
                    let h = ((level * plot_h as f32).round() as u16).clamp(1, plot_h);
                    let x = area.x + k * pitch;
                    let w = bar_w.min(area.right().saturating_sub(x));
                    sh.block(
                        buf,
                        Rect {
                            x,
                            y: area.y + plot_h - h,
                            width: w,
                            height: h,
                        },
                    );
                }
                // baseline
                let axis = Shimmer {
                    base: base.blend(th.background, 0.5),
                    hl: base,
                    ..sh
                };
                axis.bar(buf, area.x, area.bottom() - 1, area.width);
            }
        }
    }
}

// ───────────────────────────── LoadingOverlay ─────────────────────────────

/// Dim an area and centre a [`Loader`] (plus an optional message) over it: the "this panel
/// is busy" state. Draw it last, over the content it covers.
#[derive(Clone, Debug)]
pub struct LoadingOverlay {
    loader: Loader,
    message: Option<String>,
    dim: f32,
    width: u16,
    now: Option<Instant>,
    elapsed: Option<f32>,
    theme: Option<Theme>,
}

impl LoadingOverlay {
    pub fn new(loader: Loader) -> Self {
        Self {
            loader,
            message: None,
            dim: 0.65,
            width: 24,
            now: None,
            elapsed: None,
            theme: None,
        }
    }

    /// Text under the loader.
    pub fn message(mut self, m: impl Into<String>) -> Self {
        self.message = Some(m.into());
        self
    }

    /// How far the covered content fades towards the background (0 = untouched, 1 = hidden).
    pub fn dim(mut self, f: f32) -> Self {
        self.dim = f.clamp(0.0, 1.0);
        self
    }

    /// Loader width in cells (single-row styles); default 24.
    pub fn width(mut self, w: u16) -> Self {
        self.width = w.max(3);
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

impl Widget for LoadingOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        if self.loader.style.multi_row() {
            // a scene owns the whole area; dimmed slivers around it would read as a glitch
            fill(buf, area, th.background);
        } else {
            blend_area(buf, area, th.background, self.dim);
        }

        let has_msg = self.message.as_deref().is_some_and(|m| !m.is_empty()) && area.height >= 3;
        let mut loader = self.loader;
        loader.theme = Some(th);
        if let Some(n) = self.now {
            loader.now = Some(n);
        }
        if let Some(e) = self.elapsed {
            loader.elapsed = Some(e);
        }

        let multi = loader.style.multi_row();
        let block_h = if has_msg { 3 } else { 1 };
        let top = area.y + area.height.saturating_sub(block_h) / 2;
        let w = if multi {
            area.width.saturating_sub(4)
        } else {
            self.width.min(area.width.saturating_sub(2))
        };
        let x = area.x + (area.width - w) / 2;
        let loader_h = if multi {
            area.height
                .saturating_sub(if has_msg { 4 } else { 2 })
                .max(1)
        } else {
            1
        };
        let ly = if multi { area.y + 1 } else { top };
        loader.render(
            Rect {
                x,
                y: ly,
                width: w,
                height: loader_h,
            },
            buf,
        );

        if has_msg && let Some(m) = &self.message {
            let my = if multi { area.bottom() - 2 } else { top + 2 };
            // clear a pad behind the message so dimmed content doesn't show through the text
            let mw = (m.width() as u16 + 4).min(area.width);
            fill(
                buf,
                Rect {
                    x: area.x + (area.width - mw) / 2,
                    y: my,
                    width: mw,
                    height: 1,
                },
                th.background,
            );
            put_centered(
                buf,
                Rect {
                    y: my,
                    height: 1,
                    ..area
                },
                m,
                st(th.text, th.background),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_renders_in_one_row_and_a_block() {
        for style in LoaderStyle::ALL {
            for (w, h) in [(3u16, 1u16), (12, 1), (40, 1), (40, 6), (7, 3)] {
                let area = Rect::new(0, 0, w, h);
                let mut buf = Buffer::empty(area);
                Loader::new(style)
                    .label("Loading")
                    .elapsed(0.37)
                    .render(area, &mut buf);
                Loader::new(style).elapsed(3.91).render(area, &mut buf);
            }
        }
    }

    #[test]
    fn bars_paint_the_track_and_the_head() {
        // Scanner at t=0 has its head at x=0: the head cell is the accent, far cells are the track
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        let th = Theme::default();
        Loader::new(LoaderStyle::Scanner)
            .elapsed(0.0)
            .theme(&th)
            .render(area, &mut buf);
        assert_eq!(
            buf[(0, 0)].symbol(),
            " ",
            "solid cells are painted, not glyphs"
        );
        assert_eq!(buf[(0, 0)].bg, th.primary.lighten(0.12).color());
        assert_eq!(buf[(19, 0)].bg, th.panel.color());
    }

    #[test]
    fn typewriter_reveals_then_clears() {
        let area = Rect::new(0, 0, 20, 1);
        let text = "Hello";
        let row = |el: f32| {
            let mut buf = Buffer::empty(area);
            Loader::new(LoaderStyle::Typewriter)
                .label(text)
                .elapsed(el)
                .render(area, &mut buf);
            (0..6)
                .map(|x| buf[(x, 0)].symbol().to_string())
                .collect::<String>()
        };
        assert!(row(0.15).starts_with("He"), "{}", row(0.15)); // 14 cps
        assert!(row(0.5).starts_with("Hello"), "{}", row(0.5));
        assert!(
            row(1.8).starts_with(' ') || row(1.8).starts_with('▎'),
            "{}",
            row(1.8)
        ); // cleared phase
    }

    #[test]
    fn skeleton_shapes_paint_inside_the_area() {
        let th = Theme::default();
        for shape in [
            SkeletonShape::Text,
            SkeletonShape::Card,
            SkeletonShape::Avatar,
            SkeletonShape::List,
            SkeletonShape::Table,
            SkeletonShape::Chart,
        ] {
            let area = Rect::new(2, 1, 30, 6);
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
            Skeleton::new()
                .shape(shape)
                .elapsed(0.2)
                .theme(&th)
                .render(area, &mut buf);
            // the row above the area is untouched
            assert_eq!(buf[(2, 0)].bg, ratatui::style::Color::Reset, "{shape:?}");
            // something got painted in the area
            let painted = (area.top()..area.bottom())
                .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
                .filter(|&(x, y)| buf[(x, y)].bg != ratatui::style::Color::Reset)
                .count();
            assert!(painted > 0, "{shape:?}");
        }
    }

    #[test]
    fn overlay_dims_and_centres_message() {
        let th = Theme::default();
        let area = Rect::new(0, 0, 40, 7);
        let mut buf = Buffer::empty(area);
        fill(&mut buf, area, th.primary);
        LoadingOverlay::new(Loader::new(LoaderStyle::Sweep))
            .message("Busy")
            .elapsed(0.0)
            .theme(&th)
            .render(area, &mut buf);
        // corner dimmed towards the background
        assert_ne!(buf[(0, 0)].bg, th.primary.color());
        // message centred on row 4 (block of 3 rows centred in 7: rows 2..5, message at 4)
        let row: String = (0..40).map(|x| buf[(x, 4)].symbol().to_string()).collect();
        assert!(row.contains("Busy"), "{row:?}");
    }
}
