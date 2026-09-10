//! Textual's design system, ported: a small base palette expands into every derived role
//! (CIE-Lab shades, `auto N%` contrast text, panel/boost, cursor/border/scrollbar colours).
//!
//! There is one process-wide *current* theme ([`current`] / [`set`]) so widgets never need a
//! theme argument; every builder still accepts an explicit `.theme(&Theme)` override.

use std::sync::RwLock;

use ratatui::style::Color;

// ───────────────────────────── colour ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const WHITE: Rgb = Rgb(255, 255, 255);
pub const BLACK: Rgb = Rgb(0, 0, 0);

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Self((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// Parse `#rgb`, `#rrggbb`, `rgb`, `rrggbb`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        match s.len() {
            3 => {
                let v = u32::from_str_radix(s, 16).ok()?;
                let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
                Some(Self((r * 17) as u8, (g * 17) as u8, (b * 17) as u8))
            }
            6 => u32::from_str_radix(s, 16).ok().map(Self::hex),
            _ => None,
        }
    }

    pub fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }

    pub fn from_color(c: Color) -> Option<Self> {
        match c {
            Color::Rgb(r, g, b) => Some(Self(r, g, b)),
            _ => None,
        }
    }

    pub fn to_hex_string(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// Human perceptual brightness, 0 = black, 1 = white.
    pub fn brightness(self) -> f32 {
        (299.0 * self.0 as f32 + 587.0 * self.1 as f32 + 114.0 * self.2 as f32) / 255_000.0
    }

    /// Linear interpolation towards `other` by `f` (0 = self, 1 = other).
    pub fn blend(self, other: Rgb, f: f32) -> Rgb {
        let f = f.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f).round() as u8;
        Rgb(
            mix(self.0, other.0),
            mix(self.1, other.1),
            mix(self.2, other.2),
        )
    }

    pub fn inverse(self) -> Rgb {
        Rgb(255 - self.0, 255 - self.1, 255 - self.2)
    }

    /// White or black, whichever contrasts best with this colour.
    pub fn contrast_text(self) -> Rgb {
        if self.brightness() < 0.5 {
            WHITE
        } else {
            BLACK
        }
    }

    /// Textual `auto N%`: contrast text composited over this colour at `alpha`.
    pub fn text_on(self, alpha: f32) -> Rgb {
        self.blend(self.contrast_text(), alpha)
    }

    /// Lighten by shifting CIE-Lab L* by `amount * 100` (negative darkens).
    pub fn lighten(self, amount: f32) -> Rgb {
        let (l, a, b) = self.to_lab();
        Rgb::from_lab(l + amount * 100.0, a, b)
    }

    pub fn darken(self, amount: f32) -> Rgb {
        self.lighten(-amount)
    }

    /// Textual `$color-lighten-N` / `$color-darken-N` (N may be negative): 15% L* per step.
    pub fn shade(self, n: i32) -> Rgb {
        self.lighten(0.15 * n as f32)
    }

    /// Hue 0..360, saturation 0..1, lightness 0..1.
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
        let h = h.rem_euclid(360.0) / 360.0;
        let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        if s == 0.0 {
            let v = (l * 255.0).round() as u8;
            return Rgb(v, v, v);
        }
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let f = |mut t: f32| {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            let v = if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            };
            (v * 255.0).round() as u8
        };
        Rgb(f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
    }

    /// (hue 0..360, saturation 0..1, lightness 0..1)
    pub fn to_hsl(self) -> (f32, f32, f32) {
        let (r, g, b) = (
            self.0 as f32 / 255.0,
            self.1 as f32 / 255.0,
            self.2 as f32 / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if max == min {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    fn to_lab(self) -> (f32, f32, f32) {
        let lin = |c: u8| {
            let c = c as f32 / 255.0;
            if c > 0.04045 {
                ((c + 0.055) / 1.055).powf(2.4)
            } else {
                c / 12.92
            }
        };
        let (r, g, b) = (lin(self.0), lin(self.1), lin(self.2));
        let x = (r * 41.24 + g * 35.76 + b * 18.05) / 95.047;
        let y = (r * 21.26 + g * 71.52 + b * 7.22) / 100.0;
        let z = (r * 1.93 + g * 11.92 + b * 95.05) / 108.883;
        let f = |t: f32| {
            if t > 0.008856 {
                t.cbrt()
            } else {
                7.787 * t + 16.0 / 116.0
            }
        };
        let (x, y, z) = (f(x), f(y), f(z));
        (116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z))
    }

    fn from_lab(l: f32, a: f32, b: f32) -> Rgb {
        let y = (l + 16.0) / 116.0;
        let x = a / 500.0 + y;
        let z = y - b / 200.0;
        let off = 16.0 / 116.0;
        let y = if y > 0.206_893 {
            y.powi(3)
        } else {
            (y - off) / 7.787
        };
        let x = if x > 0.206_893 {
            0.95047 * x.powi(3)
        } else {
            0.122059 * (x - off)
        };
        let z = if z > 0.206_893 {
            1.08883 * z.powi(3)
        } else {
            0.139827 * (z - off)
        };
        let r = x * 3.2406 + y * -1.5372 + z * -0.4986;
        let g = x * -0.9689 + y * 1.8758 + z * 0.0415;
        let b = x * 0.0557 + y * -0.2040 + z * 1.0570;
        let gamma = |c: f32| {
            let c = if c > 0.0031308 {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            } else {
                12.92 * c
            };
            (c * 255.0).round().clamp(0.0, 255.0) as u8
        };
        Rgb(gamma(r), gamma(g), gamma(b))
    }
}

impl From<Rgb> for Color {
    fn from(c: Rgb) -> Self {
        c.color()
    }
}

/// Sample a multi-stop gradient at `t` in 0..=1.
pub fn gradient(stops: &[Rgb], t: f32) -> Rgb {
    match stops {
        [] => BLACK,
        [one] => *one,
        _ => {
            let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
            let i = (t.floor() as usize).min(stops.len() - 2);
            stops[i].blend(stops[i + 1], t - i as f32)
        }
    }
}

// ───────────────────────────── palette specs ─────────────────────────────

/// Raw palette, as authored in `textual/theme.py`. Build custom ones with [`ThemeSpec::new`].
#[derive(Clone, Copy, Debug)]
pub struct ThemeSpec {
    pub name: &'static str,
    pub dark: bool,
    pub primary: Rgb,
    pub secondary: Option<Rgb>,
    pub warning: Option<Rgb>,
    pub error: Option<Rgb>,
    pub success: Option<Rgb>,
    pub accent: Option<Rgb>,
    pub foreground: Option<Rgb>,
    pub background: Option<Rgb>,
    pub surface: Option<Rgb>,
    pub panel: Option<Rgb>,
}

impl ThemeSpec {
    pub const fn new(name: &'static str, dark: bool, primary: Rgb) -> Self {
        ThemeSpec {
            name,
            dark,
            primary,
            secondary: None,
            warning: None,
            error: None,
            success: None,
            accent: None,
            foreground: None,
            background: None,
            surface: None,
            panel: None,
        }
    }
    pub const fn secondary(mut self, c: Rgb) -> Self {
        self.secondary = Some(c);
        self
    }
    pub const fn warning(mut self, c: Rgb) -> Self {
        self.warning = Some(c);
        self
    }
    pub const fn error(mut self, c: Rgb) -> Self {
        self.error = Some(c);
        self
    }
    pub const fn success(mut self, c: Rgb) -> Self {
        self.success = Some(c);
        self
    }
    pub const fn accent(mut self, c: Rgb) -> Self {
        self.accent = Some(c);
        self
    }
    pub const fn foreground(mut self, c: Rgb) -> Self {
        self.foreground = Some(c);
        self
    }
    pub const fn background(mut self, c: Rgb) -> Self {
        self.background = Some(c);
        self
    }
    pub const fn surface(mut self, c: Rgb) -> Self {
        self.surface = Some(c);
        self
    }
    pub const fn panel(mut self, c: Rgb) -> Self {
        self.panel = Some(c);
        self
    }
    pub fn resolve(&self) -> Theme {
        Theme::resolve(self, None)
    }
}

macro_rules! theme {
    ($name:literal, dark = $dark:expr, primary = $primary:literal $(, $field:ident = $val:literal)* $(,)?) => {
        ThemeSpec { $($field: Some(Rgb::hex($val)),)* ..ThemeSpec::new($name, $dark, Rgb::hex($primary)) }
    };
}

pub const BUILTIN: &[ThemeSpec] = &[
    theme!(
        "textual-dark",
        dark = true,
        primary = 0x0178D4,
        secondary = 0x004578,
        accent = 0xffa62b,
        warning = 0xffa62b,
        error = 0xba3c5b,
        success = 0x4EBF71,
        foreground = 0xe0e0e0
    ),
    theme!(
        "textual-light",
        dark = false,
        primary = 0x004578,
        secondary = 0x0178D4,
        accent = 0xffa62b,
        warning = 0xffa62b,
        error = 0xba3c5b,
        success = 0x4EBF71,
        surface = 0xD8D8D8,
        panel = 0xD0D0D0,
        background = 0xE0E0E0
    ),
    theme!(
        "nord",
        dark = true,
        primary = 0x88C0D0,
        secondary = 0x81A1C1,
        accent = 0xB48EAD,
        foreground = 0xD8DEE9,
        background = 0x2E3440,
        success = 0xA3BE8C,
        warning = 0xEBCB8B,
        error = 0xBF616A,
        surface = 0x3B4252,
        panel = 0x434C5E
    ),
    theme!(
        "gruvbox",
        dark = true,
        primary = 0x85A598,
        secondary = 0xA89A85,
        warning = 0xfe8019,
        error = 0xfb4934,
        success = 0xb8bb26,
        accent = 0xfabd2f,
        foreground = 0xfbf1c7,
        background = 0x282828,
        surface = 0x3c3836,
        panel = 0x504945
    ),
    theme!(
        "catppuccin-mocha",
        dark = true,
        primary = 0xF5C2E7,
        secondary = 0xcba6f7,
        warning = 0xFAE3B0,
        error = 0xF28FAD,
        success = 0xABE9B3,
        accent = 0xfab387,
        foreground = 0xcdd6f4,
        background = 0x181825,
        surface = 0x313244,
        panel = 0x45475a
    ),
    theme!(
        "dracula",
        dark = true,
        primary = 0xBD93F9,
        secondary = 0x6272A4,
        warning = 0xFFB86C,
        error = 0xFF5555,
        success = 0x50FA7B,
        accent = 0xFF79C6,
        background = 0x282A36,
        surface = 0x2B2E3B,
        panel = 0x313442,
        foreground = 0xF8F8F2
    ),
    theme!(
        "tokyo-night",
        dark = true,
        primary = 0xBB9AF7,
        secondary = 0x7AA2F7,
        warning = 0xE0AF68,
        error = 0xF7768E,
        success = 0x9ECE6A,
        accent = 0xFF9E64,
        foreground = 0xa9b1d6,
        background = 0x1A1B26,
        surface = 0x24283B,
        panel = 0x414868
    ),
    theme!(
        "monokai",
        dark = true,
        primary = 0xAE81FF,
        secondary = 0xF92672,
        accent = 0x66D9EF,
        warning = 0xFD971F,
        error = 0xF92672,
        success = 0xA6E22E,
        foreground = 0xd6d6d6,
        background = 0x272822,
        surface = 0x2e2e2e,
        panel = 0x3E3D32
    ),
    theme!(
        "flexoki",
        dark = true,
        primary = 0x205EA6,
        secondary = 0x24837B,
        warning = 0xAD8301,
        error = 0xAF3029,
        success = 0x66800B,
        accent = 0x9B76C8,
        background = 0x100F0F,
        surface = 0x1C1B1A,
        panel = 0x282726,
        foreground = 0xFFFCF0
    ),
    theme!(
        "catppuccin-latte",
        dark = false,
        primary = 0x8839EF,
        secondary = 0xDC8A78,
        warning = 0xDF8E1D,
        error = 0xD20F39,
        success = 0x40A02B,
        accent = 0xFE640B,
        foreground = 0x4C4F69,
        background = 0xEFF1F5,
        surface = 0xE6E9EF,
        panel = 0xCCD0DA
    ),
    theme!(
        "solarized-light",
        dark = false,
        primary = 0x268bd2,
        secondary = 0x2aa198,
        warning = 0xcb4b16,
        error = 0xdc322f,
        success = 0x859900,
        accent = 0x6c71c4,
        foreground = 0x586e75,
        background = 0xfdf6e3,
        surface = 0xeee8d5,
        panel = 0xeee8d5
    ),
    theme!(
        "rose-pine",
        dark = true,
        primary = 0xc4a7e7,
        secondary = 0x31748f,
        warning = 0xf6c177,
        error = 0xeb6f92,
        success = 0x9ccfd8,
        accent = 0xebbcba,
        foreground = 0xe0def4,
        background = 0x191724,
        surface = 0x1f1d2e,
        panel = 0x26233a
    ),
];

pub fn theme_names() -> Vec<&'static str> {
    BUILTIN.iter().map(|t| t.name).collect()
}

pub fn builtin(name: &str) -> Option<Theme> {
    BUILTIN
        .iter()
        .find(|t| t.name == name)
        .map(|t| Theme::resolve(t, None))
}

// ───────────────────────────── resolved theme ─────────────────────────────

/// Semantic colour role, Textual's button/toast/label variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Variant {
    #[default]
    Default,
    Primary,
    Secondary,
    Accent,
    Success,
    Warning,
    Error,
}

/// Resolved palette with every derived role Textual's `ColorSystem.generate` produces.
/// Plain data (`Copy`): pass it by reference to builders, copy it into state freely.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    pub primary: Rgb,
    pub secondary: Rgb,
    pub warning: Rgb,
    pub error: Rgb,
    pub success: Rgb,
    pub accent: Rgb,
    pub foreground: Rgb,
    pub background: Rgb,
    pub surface: Rgb,
    pub panel: Rgb,
    /// Slightly boosted surface, Textual `$boost`, for nested containers.
    pub boost: Rgb,
    // derived text roles (`auto 87% / 60% / 38%`)
    pub text: Rgb,
    pub text_muted: Rgb,
    pub text_disabled: Rgb,
    pub text_primary: Rgb,
    pub text_secondary: Rgb,
    pub text_accent: Rgb,
    pub text_success: Rgb,
    pub text_warning: Rgb,
    pub text_error: Rgb,
    // chrome
    pub border: Rgb,
    pub border_blurred: Rgb,
    pub cursor_bg: Rgb,
    pub cursor_fg: Rgb,
    pub cursor_blurred_bg: Rgb,
    pub hover_bg: Rgb,
    pub scrollbar: Rgb,
    pub scrollbar_hover: Rgb,
    pub scrollbar_bg: Rgb,
    pub toast_bg: Rgb,
    pub link: Rgb,
    pub selection_bg: Rgb,
    pub footer_bg: Rgb,
    pub footer_key: Rgb,
    pub footer_desc: Rgb,
    pub markdown_code_bg: Rgb,
}

impl Theme {
    pub fn resolve(spec: &ThemeSpec, primary_override: Option<Rgb>) -> Theme {
        let primary = primary_override.unwrap_or(spec.primary);
        let secondary = spec.secondary.unwrap_or(primary);
        let warning = spec.warning.unwrap_or(primary);
        let error = spec.error.unwrap_or(secondary);
        let success = spec.success.unwrap_or(secondary);
        let accent = spec.accent.unwrap_or(primary);
        let (bg_default, surface_default) = if spec.dark {
            (Rgb::hex(0x121212), Rgb::hex(0x1e1e1e))
        } else {
            (Rgb::hex(0xefefef), Rgb::hex(0xf5f5f5))
        };
        let background = spec.background.unwrap_or(bg_default);
        let surface = spec.surface.unwrap_or(surface_default);
        let foreground = spec.foreground.unwrap_or_else(|| background.inverse());
        let contrast = background.contrast_text();
        let panel = spec.panel.unwrap_or_else(|| {
            let p = surface.blend(primary, 0.1);
            if spec.dark {
                p.blend(contrast, 0.04)
            } else {
                p
            }
        });
        let bg_d1 = background.darken(0.15);
        Theme {
            name: spec.name,
            dark: spec.dark,
            primary,
            secondary,
            warning,
            error,
            success,
            accent,
            foreground,
            background,
            surface,
            panel,
            boost: surface.blend(contrast, 0.04),
            text: background.text_on(0.87),
            text_muted: background.text_on(0.60),
            text_disabled: background.text_on(0.38),
            text_primary: contrast.blend(primary, 0.66),
            text_secondary: contrast.blend(secondary, 0.66),
            text_accent: contrast.blend(accent, 0.66),
            text_success: contrast.blend(success, 0.66),
            text_warning: contrast.blend(warning, 0.66),
            text_error: contrast.blend(error, 0.66),
            border: primary,
            border_blurred: surface.darken(0.025),
            cursor_bg: primary,
            cursor_fg: primary.text_on(0.9),
            cursor_blurred_bg: surface.blend(primary, 0.3),
            hover_bg: surface.blend(contrast, 0.08),
            scrollbar: bg_d1.blend(primary, 0.4),
            scrollbar_hover: bg_d1.blend(primary, 0.6),
            scrollbar_bg: bg_d1,
            toast_bg: panel.lighten(0.15),
            link: secondary.blend(contrast, 0.2),
            selection_bg: surface.blend(primary, 0.5),
            footer_bg: panel,
            footer_key: accent,
            footer_desc: panel.text_on(0.87),
            markdown_code_bg: surface.blend(contrast, 0.06),
        }
    }

    /// Textual `$color-lighten-N` / `$color-darken-N` (N may be negative).
    pub fn shade(c: Rgb, n: i32) -> Rgb {
        c.shade(n)
    }

    /// Base colour for a semantic variant.
    pub fn variant(&self, v: Variant) -> Rgb {
        match v {
            Variant::Default => self.surface,
            Variant::Primary => self.primary,
            Variant::Secondary => self.secondary,
            Variant::Accent => self.accent,
            Variant::Success => self.success,
            Variant::Warning => self.warning,
            Variant::Error => self.error,
        }
    }

    /// Readable text tinted with the variant (`$text-primary`, ...), for use over surfaces.
    pub fn text_variant(&self, v: Variant) -> Rgb {
        match v {
            Variant::Default => self.text,
            Variant::Primary => self.text_primary,
            Variant::Secondary => self.text_secondary,
            Variant::Accent => self.text_accent,
            Variant::Success => self.text_success,
            Variant::Warning => self.text_warning,
            Variant::Error => self.text_error,
        }
    }

    /// Focused-widget surface: `$surface` with a 5% foreground tint.
    pub fn focus_bg(&self) -> Rgb {
        self.surface.blend(self.foreground, 0.05)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::resolve(&BUILTIN[0], None)
    }
}

// ───────────────────────────── current theme ─────────────────────────────

static CURRENT: RwLock<Option<Theme>> = RwLock::new(None);

/// The process-wide theme every widget falls back to. `textual-dark` until [`set`] is called.
pub fn current() -> Theme {
    if let Ok(g) = CURRENT.read()
        && let Some(t) = g.as_ref()
    {
        return *t;
    }
    Theme::default()
}

pub fn set(theme: Theme) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(theme);
    }
}

/// Switch to a builtin theme by name; returns `false` if unknown.
pub fn set_by_name(name: &str) -> bool {
    match builtin(name) {
        Some(t) => {
            set(t);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_roundtrip_and_shades() {
        let c = Rgb::hex(0x0178D4);
        let (l, a, b) = c.to_lab();
        let back = Rgb::from_lab(l, a, b);
        assert!((back.0 as i32 - c.0 as i32).abs() <= 1);
        assert!((back.1 as i32 - c.1 as i32).abs() <= 1);
        assert!((back.2 as i32 - c.2 as i32).abs() <= 1);
        assert!(c.lighten(0.3).brightness() > c.brightness());
        assert!(c.darken(0.3).brightness() < c.brightness());
        assert_eq!(WHITE.contrast_text(), BLACK);
        let t = Theme::resolve(&BUILTIN[0], None);
        assert!(t.text.brightness() > 0.8);
    }

    #[test]
    fn hsl_roundtrip_and_parse() {
        let c = Rgb::hex(0x4EBF71);
        let (h, s, l) = c.to_hsl();
        let back = Rgb::from_hsl(h, s, l);
        assert!((back.0 as i32 - c.0 as i32).abs() <= 1);
        assert!((back.2 as i32 - c.2 as i32).abs() <= 1);
        assert_eq!(Rgb::parse("#fff"), Some(WHITE));
        assert_eq!(
            Rgb::parse("0178d4"),
            Some(c.blend(c, 0.0)).map(|_| Rgb::hex(0x0178D4))
        );
        assert_eq!(gradient(&[BLACK, WHITE], 0.5), Rgb(128, 128, 128));
    }

    #[test]
    fn current_theme_switches() {
        assert!(set_by_name("nord"));
        assert_eq!(current().name, "nord");
        assert!(!set_by_name("nope"));
        set(Theme::default());
    }
}
