//! Text widgets: labels, rules, badges, links, stat cards and markup parser.
//!
//! ```
//! # use tuiforge::prelude::*;
//! # let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
//! # let th = Theme::default();
//! Label::new("Hello").variant(Variant::Primary).bold(true).render(Rect::new(0, 0, 10, 1), &mut buf);
//! Badge::new("NEW").variant(Variant::Accent).render(Rect::new(12, 0, 5, 1), &mut buf);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::core::{Hit, HitBox, Interactive, Outcome, is_activate, is_press};
use crate::draw::{LOWER_BLOCKS, fill, put, put_aligned, put_centered, st};
use crate::theme::{Rgb, Theme, Variant, self};

// ─────────────────────────────────────────────────────────────────────────────
// Label
// ─────────────────────────────────────────────────────────────────────────────

/// Plain text label with variant tinting, alignment and wrapping.
#[derive(Clone, Debug)]
pub struct Label {
    text: String,
    variant: Option<Variant>,
    muted: bool,
    disabled: bool,
    bold: bool,
    italic: bool,
    underline: bool,
    align: Alignment,
    wrap: bool,
    bg: Option<Rgb>,
    ellipsis: bool,
    theme: Option<Theme>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: None,
            muted: false,
            disabled: false,
            bold: false,
            italic: false,
            underline: false,
            align: Alignment::Left,
            wrap: false,
            bg: None,
            ellipsis: true,
            theme: None,
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = Some(v);
        self
    }
    pub fn muted(mut self, v: bool) -> Self {
        self.muted = v;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn bold(mut self, v: bool) -> Self {
        self.bold = v;
        self
    }
    pub fn italic(mut self, v: bool) -> Self {
        self.italic = v;
        self
    }
    pub fn underline(mut self, v: bool) -> Self {
        self.underline = v;
        self
    }
    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }
    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap = v;
        self
    }
    pub fn bg(mut self, c: Rgb) -> Self {
        self.bg = Some(c);
        self
    }
    pub fn ellipsis(mut self, v: bool) -> Self {
        self.ellipsis = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Height in lines when wrapped to `width`.
    pub fn height_for(&self, width: usize) -> u16 {
        if !self.wrap || width == 0 {
            return 1;
        }
        crate::draw::wrap(&self.text, width).len() as u16
    }
}

impl Widget for Label {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = self.bg.unwrap_or(th.surface);
        fill(buf, area, bg);

        let fg = if self.disabled {
            th.text_disabled
        } else if self.muted {
            th.text_muted
        } else if let Some(v) = self.variant {
            th.text_variant(v)
        } else {
            th.text
        };

        let mut style = st(fg, bg);
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if self.underline {
            style = style.add_modifier(Modifier::UNDERLINED);
        }

        if self.wrap {
            let lines = crate::draw::wrap(&self.text, area.width as usize);
            for (i, line) in lines.iter().enumerate() {
                if i >= area.height as usize {
                    break;
                }
                put_aligned(buf, Rect { x: area.x, y: area.y + i as u16, width: area.width, height: 1 }, line, self.align, style);
            }
        } else {
            let display = if self.ellipsis {
                crate::draw::truncate(&self.text, area.width as usize)
            } else {
                self.text[..self.text.len().min(area.width as usize)].to_string()
            };
            put_aligned(buf, Rect { x: area.x, y: area.y, width: area.width, height: 1 }, &display, self.align, style);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule
// ─────────────────────────────────────────────────────────────────────────────

/// Horizontal or vertical separator line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleStyle {
    Solid,
    Heavy,
    Double,
    Dashed,
    Dotted,
    Ascii,
    Blank,
}

/// Divider line with optional title.
#[derive(Clone, Debug)]
pub struct Rule {
    horizontal: bool,
    style_kind: RuleStyle,
    title: Option<String>,
    title_align: Alignment,
    color: Option<Rgb>,
    theme: Option<Theme>,
}

impl Rule {
    pub fn horizontal() -> Self {
        Self {
            horizontal: true,
            style_kind: RuleStyle::Solid,
            title: None,
            title_align: Alignment::Center,
            color: None,
            theme: None,
        }
    }

    pub fn vertical() -> Self {
        Self {
            horizontal: false,
            style_kind: RuleStyle::Solid,
            title: None,
            title_align: Alignment::Center,
            color: None,
            theme: None,
        }
    }

    pub fn style(mut self, s: RuleStyle) -> Self {
        self.style_kind = s;
        self
    }
    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }
    pub fn title_align(mut self, a: Alignment) -> Self {
        self.title_align = a;
        self
    }
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for Rule {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let color = self.color.unwrap_or(th.secondary);

        let glyph = match self.style_kind {
            RuleStyle::Solid if self.horizontal => "─",
            RuleStyle::Solid => "│",
            RuleStyle::Heavy if self.horizontal => "━",
            RuleStyle::Heavy => "┃",
            RuleStyle::Double if self.horizontal => "═",
            RuleStyle::Double => "║",
            RuleStyle::Dashed if self.horizontal => "╌",
            RuleStyle::Dashed => "╎",
            RuleStyle::Dotted if self.horizontal => "┄",
            RuleStyle::Dotted => "┆",
            RuleStyle::Ascii if self.horizontal => "-",
            RuleStyle::Ascii => "|",
            RuleStyle::Blank => " ",
        };

        let style = st(color, th.surface);

        if self.horizontal {
            if let Some(title) = &self.title {
                let title_w = title.width() as u16;
                let space = area.width.saturating_sub(title_w + 2);
                if title_w + 2 > area.width {
                    // too narrow, just draw line
                    for x in 0..area.width {
                        put(buf, area.x + x, area.y, glyph, 1, style);
                    }
                } else {
                    let (left, _right) = match self.title_align {
                        Alignment::Left => (1, space.saturating_sub(1)),
                        Alignment::Right => (space.saturating_sub(1), 1),
                        Alignment::Center => {
                            let l = space / 2;
                            (l, space.saturating_sub(l))
                        }
                    };
                    for x in 0..left {
                        put(buf, area.x + x, area.y, glyph, 1, style);
                    }
                    put(buf, area.x + left, area.y, " ", 1, style);
                    put(buf, area.x + left + 1, area.y, title, title_w, st(th.text, th.surface));
                    put(buf, area.x + left + 1 + title_w, area.y, " ", 1, style);
                    for x in (left + title_w + 2)..area.width {
                        put(buf, area.x + x, area.y, glyph, 1, style);
                    }
                }
            } else {
                for x in 0..area.width {
                    put(buf, area.x + x, area.y, glyph, 1, style);
                }
            }
        } else {
            // vertical
            for y in 0..area.height {
                put(buf, area.x, area.y + y, glyph, 1, style);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Badge, Pill, KeyCap
// ─────────────────────────────────────────────────────────────────────────────

/// Badge rendering style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeStyle {
    Filled,
    Outline,
    Soft,
}

/// Small chip/tag.
#[derive(Clone, Debug)]
pub struct Badge {
    text: String,
    variant: Variant,
    style: BadgeStyle,
    icon: Option<String>,
    theme: Option<Theme>,
}

impl Badge {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: Variant::Default,
            style: BadgeStyle::Filled,
            icon: None,
            theme: None,
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn style(mut self, s: BadgeStyle) -> Self {
        self.style = s;
        self
    }
    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(i.into());
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn width(text: &str) -> u16 {
        (text.width() as u16).saturating_add(2)
    }
}

impl Widget for Badge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let variant_color = th.variant(self.variant);

        let (fg, bg) = match self.style {
            BadgeStyle::Filled => (th.variant(self.variant).text_on(0.9), variant_color),
            BadgeStyle::Outline => (variant_color, th.surface),
            BadgeStyle::Soft => {
                let soft_bg = th.surface.blend(variant_color, 0.2);
                (variant_color, soft_bg)
            }
        };

        fill(buf, area, bg);
        let mut x = area.x;
        if let Some(icon) = &self.icon {
            put(buf, x, area.y, icon, 1, st(fg, bg));
            x += 1 + (icon.width() as u16).min(area.width.saturating_sub(1));
        }
        let remain = area.width.saturating_sub(x - area.x);
        let display = crate::draw::fit(&self.text, remain as usize);
        put_centered(buf, Rect { x, y: area.y, width: remain, height: 1 }, &display, st(fg, bg));
    }
}

/// Pill badge with rounded ends using block glyphs.
#[derive(Clone, Debug)]
pub struct Pill {
    text: String,
    variant: Variant,
    theme: Option<Theme>,
}

impl Pill {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), variant: Variant::Default, theme: None }
    }
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for Pill {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let variant_color = th.variant(self.variant);
        let fg = th.variant(self.variant).text_on(0.9);

        // flat painted pill: bg colour only, no half-block ends (font seams, min-contrast recolouring)
        fill(buf, Rect { height: 1, ..area }, variant_color);
        let display = crate::draw::fit(&self.text, area.width.saturating_sub(2) as usize);
        put_centered(buf, Rect { height: 1, ..area }, &display, st(fg, variant_color));
    }
}

/// Keyboard key representation (inverted panel style).
#[derive(Clone, Debug)]
pub struct KeyCap {
    key: String,
    theme: Option<Theme>,
}

impl KeyCap {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into(), theme: None }
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for KeyCap {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.panel);
        let display = crate::draw::fit(&self.key, area.width.saturating_sub(2) as usize);
        let style = st(th.footer_key, th.panel).add_modifier(Modifier::BOLD);
        put_centered(buf, Rect { x: area.x + 1, y: area.y, width: area.width.saturating_sub(2), height: 1 }, &display, style);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Link
// ─────────────────────────────────────────────────────────────────────────────

/// Clickable link with visited state.
#[derive(Clone, Debug, Default)]
pub struct LinkState {
    pub url: Option<String>,
    pub visited: bool,
    pub hit: HitBox,
    clicked: bool,
}

impl LinkState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn take_clicked(&mut self) -> bool {
        std::mem::take(&mut self.clicked)
    }
}

impl Interactive for LinkState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if is_press(&key) && is_activate(&key) {
            self.clicked = true;
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        match self.hit.mouse(&m) {
            Hit::Press => {
                self.clicked = true;
                Outcome::Changed
            }
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        }
    }
}

/// Underlined clickable link.
#[derive(Clone, Debug)]
pub struct Link {
    text: String,
    url: Option<String>,
    show_url: bool,
    focused: bool,
    theme: Option<Theme>,
}

impl Link {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), url: None, show_url: false, focused: false, theme: None }
    }
    pub fn url(mut self, u: impl Into<String>) -> Self {
        self.url = Some(u.into());
        self
    }
    pub fn show_url(mut self, v: bool) -> Self {
        self.show_url = v;
        self
    }
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl StatefulWidget for Link {
    type State = LinkState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        state.hit.set_area(area);
        state.url = self.url.clone();

        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        let hover = state.hit.hover;
        let fg = if self.focused || hover {
            th.text_primary
        } else if state.visited {
            th.text_muted
        } else {
            th.link
        };

        let mut display = crate::draw::truncate(&self.text, area.width as usize);
        if self.show_url
            && let Some(url_str) = &self.url
        {
            let full = format!("{} ({})", self.text, url_str);
            display = crate::draw::truncate(&full, area.width as usize);
        }

        let style = st(fg, th.surface).add_modifier(Modifier::UNDERLINED);
        put(buf, area.x, area.y, &display, area.width, style);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StatCard
// ─────────────────────────────────────────────────────────────────────────────

/// Stat card with big value, label, delta and sparkline.
#[derive(Clone, Debug)]
pub struct StatCard {
    value: String,
    label: String,
    delta: Option<(f64, bool)>, // (value, is_positive)
    trend: Vec<f64>,
    variant: Variant,
    icon: Option<String>,
    bordered: bool,
    theme: Option<Theme>,
}

impl StatCard {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            delta: None,
            trend: Vec::new(),
            variant: Variant::Default,
            icon: None,
            bordered: false,
            theme: None,
        }
    }

    pub fn delta(mut self, val: f64, positive: bool) -> Self {
        self.delta = Some((val, positive));
        self
    }
    pub fn trend(mut self, data: &[f64]) -> Self {
        self.trend = data.to_vec();
        self
    }
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(i.into());
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn min_height() -> u16 {
        3
    }
}

impl Widget for StatCard {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 6 || area.height < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.background;
        fill(buf, area, bg);

        if self.bordered {
            crate::draw::Border::Round.draw(buf, area, th.border, bg);
        }

        let inner = if self.bordered {
            Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            }
        } else {
            area
        };

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let mut y = inner.y;

        // Value (bold, big)
        let val_style = st(th.text, bg).add_modifier(Modifier::BOLD);
        let display_val = crate::draw::truncate(&self.value, inner.width as usize);
        if let Some(icon) = &self.icon {
            let icon_w = icon.width() as u16;
            put(buf, inner.x, y, icon, icon_w.min(inner.width), st(th.text_variant(self.variant), bg));
            let remain = inner.width.saturating_sub(icon_w + 1);
            put(buf, inner.x + icon_w + 1, y, &display_val, remain, val_style);
        } else {
            put(buf, inner.x, y, &display_val, inner.width, val_style);
        }
        y += 1;

        if y >= inner.y + inner.height {
            return;
        }

        // Label + delta
        let label_style = st(th.text_muted, bg);
        let mut label_line = self.label.clone();
        if let Some((dval, positive)) = self.delta {
            let arrow = if positive { "▲" } else { "▼" };
            let delta_str = format!(" {} {:.1}%", arrow, dval.abs());
            label_line.push_str(&delta_str);
            let label_w = self.label.width() as u16;
            put(buf, inner.x, y, &self.label, label_w.min(inner.width), label_style);
            let delta_fg = if positive { th.success } else { th.error };
            put(buf, inner.x + label_w, y, &delta_str, inner.width.saturating_sub(label_w), st(delta_fg, bg));
        } else {
            put(buf, inner.x, y, &label_line, inner.width, label_style);
        }
        y += 1;

        // Sparkline
        if !self.trend.is_empty() && y < inner.y + inner.height {
            let spark_w = inner.width.min(self.trend.len() as u16);
            if let (Some(min), Some(max)) = (
                self.trend.iter().copied().min_by(|a, b| a.partial_cmp(b).unwrap()),
                self.trend.iter().copied().max_by(|a, b| a.partial_cmp(b).unwrap()),
            ) {
                let range = (max - min).max(1e-9);
                let start = self.trend.len().saturating_sub(spark_w as usize);
                for (i, val) in self.trend.iter().skip(start).enumerate() {
                    if i >= spark_w as usize {
                        break;
                    }
                    let frac = ((val - min) / range).clamp(0.0, 1.0);
                    let idx = (frac * 8.0).round() as usize;
                    let glyph = LOWER_BLOCKS[idx.min(8)];
                    put(buf, inner.x + i as u16, y, glyph, 1, st(th.text_variant(self.variant), bg));
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StatusLine
// ─────────────────────────────────────────────────────────────────────────────

/// Separator between [`StatusLine`] segments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusSep {
    /// ` › ` - omp's "powerline-thin".
    #[default]
    Chevron,
    /// ` · `
    Dot,
    /// ` │ `
    Pipe,
    /// ` ❯ `
    Arrow,
    /// two spaces
    Space,
}

impl StatusSep {
    fn glyph(self) -> &'static str {
        match self {
            StatusSep::Chevron => " › ",
            StatusSep::Dot => " · ",
            StatusSep::Pipe => " │ ",
            StatusSep::Arrow => " ❯ ",
            StatusSep::Space => "  ",
        }
    }
}

/// One segment: optional icon, text, optional colour (default `th.text_muted`).
#[derive(Clone, Copy, Debug)]
pub struct StatusSegment<'a> {
    pub icon: Option<&'a str>,
    pub text: &'a str,
    pub color: Option<Rgb>,
}

impl<'a> StatusSegment<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { icon: None, text, color: None }
    }
    pub fn icon(mut self, i: &'a str) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }
}

/// One-row status line of separated segments with optional right-aligned text - the strip
/// under a composer (`π › Opus 5 › /tmp › 4.0%/1M › (sub)            omp`).
///
/// ```no_run
/// use tuiforge::prelude::*;
/// # let area = Rect::new(0, 0, 60, 1);
/// # let mut buf = Buffer::empty(area);
/// StatusLine::new(&[
///     StatusSegment::new("π"),
///     StatusSegment::new("Opus 5").icon("◕").color(Rgb(255, 140, 0)),
///     StatusSegment::new("/tmp").icon("⌂"),
///     StatusSegment::new("4.0%/1M"),
/// ]).sep(StatusSep::Chevron).right("omp").render(area, &mut buf);
/// ```
#[derive(Clone, Debug)]
pub struct StatusLine<'a> {
    segments: &'a [StatusSegment<'a>],
    sep: StatusSep,
    right: Option<&'a str>,
    right_color: Option<Rgb>,
    bg: Option<Rgb>,
    theme: Option<Theme>,
}

impl<'a> StatusLine<'a> {
    pub fn new(segments: &'a [StatusSegment<'a>]) -> Self {
        Self { segments, sep: StatusSep::Chevron, right: None, right_color: None, bg: None, theme: None }
    }
    pub fn sep(mut self, s: StatusSep) -> Self {
        self.sep = s;
        self
    }
    /// Right-aligned text (default colour `th.accent`).
    pub fn right(mut self, r: &'a str) -> Self {
        self.right = Some(r);
        self
    }
    pub fn right_color(mut self, c: Rgb) -> Self {
        self.right_color = Some(c);
        self
    }
    /// Paint a background strip (default: transparent over `th.background`).
    pub fn bg(mut self, c: Rgb) -> Self {
        self.bg = Some(c);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = self.bg.unwrap_or(th.background);
        let row = Rect { height: 1, ..area };
        fill(buf, row, bg);

        let mut limit = area.right();
        if let Some(r) = self.right {
            let w = r.width() as u16 + 1;
            put(buf, area.right().saturating_sub(w), area.y, &format!("{r} "), w, st(self.right_color.unwrap_or(th.accent), bg).add_modifier(Modifier::BOLD));
            limit = area.right().saturating_sub(w + 1);
        }

        let mut x = area.x + 1;
        for (i, seg) in self.segments.iter().enumerate() {
            let color = seg.color.unwrap_or(th.text_muted);
            let mut text = String::new();
            if let Some(icon) = seg.icon {
                text.push_str(icon);
                text.push(' ');
            }
            text.push_str(seg.text);
            let sep_w = if i > 0 { self.sep.glyph().width() as u16 } else { 0 };
            let w = text.width() as u16;
            if x + sep_w + w > limit {
                break;
            }
            if i > 0 {
                x += put(buf, x, area.y, self.sep.glyph(), sep_w, st(th.text_disabled, bg));
            }
            x += put(buf, x, area.y, &text, w, st(color, bg));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Markup parser
// ─────────────────────────────────────────────────────────────────────────────

/// Mini console markup parser: `[b]bold[/b]`, `[i]italic[/]`, `[primary]`, `[#rrggbb]`, etc.
pub struct Markup;

impl Markup {
    /// Parse markup string into ratatui Text with styles applied.
    pub fn parse(input: &str, th: &Theme) -> Text<'static> {
        let mut lines = Vec::new();
        for raw_line in input.lines() {
            lines.push(Self::parse_line(raw_line, th));
        }
        if lines.is_empty() {
            lines.push(Line::raw(""));
        }
        Text::from(lines)
    }

    fn parse_line(input: &str, th: &Theme) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut stack: Vec<(TagKind, Style)> = Vec::new();
        let mut current_style = Style::default().fg(th.text.color()).bg(th.surface.color());
        let mut current = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        // flush the pending text as one span (adjacent same-style text stays merged)
        let flush = |spans: &mut Vec<Span<'static>>, current: &mut String, style: Style| {
            if !current.is_empty() {
                spans.push(Span::styled(std::mem::take(current), style));
            }
        };

        while i < chars.len() {
            if chars[i] == '[' {
                if i + 1 < chars.len() && chars[i + 1] == '[' {
                    current.push('[');
                    i += 2;
                    continue;
                }
                if let Some((tag, end)) = Self::parse_tag(&chars, i, th) {
                    flush(&mut spans, &mut current, current_style);
                    match tag {
                        ParsedTag::Open(kind, style) => {
                            stack.push((kind, current_style));
                            current_style = style;
                        }
                        ParsedTag::Close => {
                            if let Some((_, prev_style)) = stack.pop() {
                                current_style = prev_style;
                            }
                        }
                    }
                    i = end;
                    continue;
                }
                // unknown tag or no closing bracket: literal text
            }
            current.push(chars[i]);
            i += 1;
        }
        flush(&mut spans, &mut current, current_style);
        Line::from(spans)
    }

    fn parse_tag(chars: &[char], start: usize, th: &Theme) -> Option<(ParsedTag, usize)> {
        if chars[start] != '[' {
            return None;
        }
        let end_pos = chars.iter().skip(start + 1).position(|&c| c == ']')?;
        let tag_end = start + 1 + end_pos;
        let tag_text: String = chars[start + 1..tag_end].iter().collect();

        if tag_text == "/" {
            return Some((ParsedTag::Close, tag_end + 1));
        }

        let is_close = tag_text.starts_with('/');
        if is_close {
            return Some((ParsedTag::Close, tag_end + 1));
        }

        let base_style = Style::default().fg(th.text.color()).bg(th.surface.color());

        // parse open tag
        let (kind, style) = if tag_text == "b" {
            (TagKind::Bold, base_style.add_modifier(Modifier::BOLD))
        } else if tag_text == "i" {
            (TagKind::Italic, base_style.add_modifier(Modifier::ITALIC))
        } else if tag_text == "u" {
            (TagKind::Underline, base_style.add_modifier(Modifier::UNDERLINED))
        } else if tag_text == "s" {
            (TagKind::Strike, base_style.add_modifier(Modifier::CROSSED_OUT))
        } else if tag_text == "dim" {
            (TagKind::Dim, base_style.add_modifier(Modifier::DIM))
        } else if tag_text == "reverse" {
            (TagKind::Reverse, base_style.add_modifier(Modifier::REVERSED))
        } else if tag_text == "primary" {
            (TagKind::Color, base_style.fg(th.primary.color()))
        } else if tag_text == "secondary" {
            (TagKind::Color, base_style.fg(th.secondary.color()))
        } else if tag_text == "accent" {
            (TagKind::Color, base_style.fg(th.accent.color()))
        } else if tag_text == "success" {
            (TagKind::Color, base_style.fg(th.success.color()))
        } else if tag_text == "warning" {
            (TagKind::Color, base_style.fg(th.warning.color()))
        } else if tag_text == "error" {
            (TagKind::Color, base_style.fg(th.error.color()))
        } else if tag_text == "muted" {
            (TagKind::Color, base_style.fg(th.text_muted.color()))
        } else if tag_text.starts_with('#') && tag_text.len() == 7 {
            if let Ok(r) = u8::from_str_radix(&tag_text[1..3], 16) {
                if let Ok(g) = u8::from_str_radix(&tag_text[3..5], 16) {
                    if let Ok(b) = u8::from_str_radix(&tag_text[5..7], 16) {
                        (TagKind::Color, base_style.fg(Rgb(r, g, b).color()))
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else if tag_text.starts_with("on #") && tag_text.len() == 10 {
            if let Ok(r) = u8::from_str_radix(&tag_text[4..6], 16) {
                if let Ok(g) = u8::from_str_radix(&tag_text[6..8], 16) {
                    if let Ok(b) = u8::from_str_radix(&tag_text[8..10], 16) {
                        (TagKind::BgColor, base_style.bg(Rgb(r, g, b).color()))
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            // unknown tag, render literally
            return None;
        };

        Some((ParsedTag::Open(kind, style), tag_end + 1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TagKind {
    Bold,
    Italic,
    Underline,
    Strike,
    Dim,
    Reverse,
    Color,
    BgColor,
}

enum ParsedTag {
    Open(TagKind, Style),
    Close,
}

/// Label that renders markup.
#[derive(Clone, Debug)]
pub struct MarkupLabel {
    text: String,
    wrap: bool,
    theme: Option<Theme>,
}

impl MarkupLabel {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), wrap: false, theme: None }
    }
    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for MarkupLabel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        let parsed = Markup::parse(&self.text, &th);
        let lines: Vec<Line> = if self.wrap {
            // simple wrap: re-parse after wrapping plain text
            let plain = self.text.replace("[b]", "").replace("[/b]", "").replace("[i]", "").replace("[/i]", "");
            let wrapped = crate::draw::wrap(&plain, area.width as usize);
            wrapped.iter().map(|l| Markup::parse_line(l, &th)).collect()
        } else {
            parsed.lines.into_iter().collect()
        };

        for (i, line) in lines.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }
            let y = area.y + i as u16;
            let mut x = area.x;
            for span in &line.spans {
                let w = span.content.width() as u16;
                if x + w > area.x + area.width {
                    break;
                }
                put(buf, x, y, &span.content, w, span.style);
                x += w;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_wraps_correctly() {
        let l = Label::new("hello world test").wrap(true);
        assert_eq!(l.height_for(10), 2);
        assert_eq!(l.height_for(20), 1);
        assert_eq!(Label::new("hello world test").height_for(10), 1, "no wrap → one line");
    }

    #[test]
    fn markup_parses_tags() {
        let th = Theme::default();
        let text = Markup::parse("[b]bold[/b] [i]italic[/] plain", &th);
        assert_eq!(text.lines.len(), 1);
        let line = &text.lines[0];
        let contents: Vec<&str> = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(contents, vec!["bold", " ", "italic", " plain"]);
        assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(line.spans[2].style.add_modifier.contains(Modifier::ITALIC));
        assert!(!line.spans[3].style.add_modifier.contains(Modifier::ITALIC));
        // unknown tags and unterminated brackets are literal, never loop forever
        let odd = Markup::parse("[nope]x[ y", &th);
        let joined: String = odd.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, "[nope]x[ y");
    }

    #[test]
    fn markup_handles_nesting() {
        let th = Theme::default();
        let text = Markup::parse("[b][i]both[/][/]", &th);
        assert_eq!(text.lines.len(), 1);
    }

    #[test]
    fn markup_escapes_brackets() {
        let th = Theme::default();
        let text = Markup::parse("[[b] not a tag", &th);
        assert_eq!(text.lines[0].spans[0].content, "[b] not a tag");
    }

    #[test]
    fn badge_width_correct() {
        assert_eq!(Badge::width("NEW"), 5);
        assert_eq!(Badge::width(""), 2);
    }
}
