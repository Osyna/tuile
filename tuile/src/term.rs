//! Terminal capability detection: colour depth and Unicode support.
//!
//! The library defaults to truecolour and full Unicode — colours are `Color::Rgb` and glyphs
//! are box-drawing characters. Nothing probes the environment unless you explicitly call
//! [`probe_env`] or [`crate::runtime::run`] (which calls it as part of terminal setup).
//!
//! This is deliberate. A consumer rendering into a `Buffer` for a test or an image gets
//! deterministic output; the agent shell that runs the harness's tests exports `NO_COLOR=1`,
//! and an implicit probe would turn every colour assertion into "default".
//!
//! Call [`probe_env`] to detect from `NO_COLOR` / `COLORTERM` / `TERM`, or [`set_caps`] to
//! override explicitly (for apps with a user setting, or tests that want a specific depth).
//!
//! [`caps`] is cheap — one atomic read — so it sits on the per-cell style path:
//! [`crate::theme::Rgb::color`] calls it every time an `Rgb` becomes a `Color`.

use std::sync::atomic::{AtomicU8, Ordering};

use ratatui_core::style::Color;

/// Colour depth the terminal supports. The default is [`ColorDepth::TrueColor`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ColorDepth {
    /// 24-bit RGB: `Color::Rgb(r, g, b)`.
    #[default]
    TrueColor = 0,
    /// 256-colour palette: 6×6×6 cube + 24 greys. `Color::Indexed(0..=255)`.
    Ansi256 = 1,
    /// 16-colour ANSI palette. `Color::Indexed(0..=15)`.
    Ansi16 = 2,
    /// Attributes only: `Color::Reset`. No colour, only bold/dim/italic/underline.
    None = 3,
}

impl ColorDepth {
    /// Map an RGB triple to a `Color` at this depth.
    pub fn resolve(self, r: u8, g: u8, b: u8) -> Color {
        match self {
            ColorDepth::TrueColor => Color::Rgb(r, g, b),
            ColorDepth::Ansi256 => {
                // 6×6×6 cube (16..=231) + 24 greys (232..=255)
                let grey = (r == g) && (g == b);
                if grey {
                    if r < 8 {
                        Color::Indexed(16) // black
                    } else if r > 247 {
                        Color::Indexed(231) // white
                    } else {
                        let level = ((r as u16 - 8) * 24 / 240) as u8;
                        Color::Indexed(232 + level)
                    }
                } else {
                    let ir = (r as u16 * 5 / 255) as u8;
                    let ig = (g as u16 * 5 / 255) as u8;
                    let ib = (b as u16 * 5 / 255) as u8;
                    Color::Indexed(16 + 36 * ir + 6 * ig + ib)
                }
            }
            ColorDepth::Ansi16 => {
                // Map to the nearest of the 16 ANSI colours by Euclidean distance
                const ANSI16: [(u8, u8, u8); 16] = [
                    (0, 0, 0),       // black
                    (128, 0, 0),     // red
                    (0, 128, 0),     // green
                    (128, 128, 0),   // yellow
                    (0, 0, 128),     // blue
                    (128, 0, 128),   // magenta
                    (0, 128, 128),   // cyan
                    (192, 192, 192), // white
                    (128, 128, 128), // bright black
                    (255, 0, 0),     // bright red
                    (0, 255, 0),     // bright green
                    (255, 255, 0),   // bright yellow
                    (0, 0, 255),     // bright blue
                    (255, 0, 255),   // bright magenta
                    (0, 255, 255),   // bright cyan
                    (255, 255, 255), // bright white
                ];
                let mut best_i = 0;
                let mut best_dist = u32::MAX;
                for (i, &(pr, pg, pb)) in ANSI16.iter().enumerate() {
                    let dr = r as i32 - pr as i32;
                    let dg = g as i32 - pg as i32;
                    let db = b as i32 - pb as i32;
                    let dist = (dr * dr + dg * dg + db * db) as u32;
                    if dist < best_dist {
                        best_dist = dist;
                        best_i = i;
                    }
                }
                Color::Indexed(best_i as u8)
            }
            ColorDepth::None => Color::Reset,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => ColorDepth::TrueColor,
            1 => ColorDepth::Ansi256,
            2 => ColorDepth::Ansi16,
            3 => ColorDepth::None,
            _ => ColorDepth::TrueColor,
        }
    }
}

/// Unicode support level the terminal has. The default is [`UnicodeLevel::Full`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnicodeLevel {
    /// Box-drawing characters, block elements, and other Unicode glyphs.
    #[default]
    Full = 0,
    /// ASCII fallback: `+ - |` for borders, no fancy glyphs.
    Ascii = 1,
}

impl UnicodeLevel {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => UnicodeLevel::Full,
            1 => UnicodeLevel::Ascii,
            _ => UnicodeLevel::Full,
        }
    }
}

/// Terminal capabilities: colour depth and Unicode support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TermCaps {
    pub color: ColorDepth,
    pub unicode: UnicodeLevel,
}

impl TermCaps {
    fn pack(self) -> u8 {
        (self.color as u8) << 4 | (self.unicode as u8)
    }

    fn unpack(v: u8) -> Self {
        TermCaps {
            color: ColorDepth::from_u8(v >> 4),
            unicode: UnicodeLevel::from_u8(v & 0xf),
        }
    }
}

// Packed as `(color << 4) | unicode` in one byte
static CAPS: AtomicU8 = AtomicU8::new(0);

/// The current terminal capabilities. Cheap: one atomic read, no env access.
pub fn caps() -> TermCaps {
    TermCaps::unpack(CAPS.load(Ordering::Relaxed))
}

/// Explicitly override the terminal capabilities. Use this for app settings or tests.
pub fn set_caps(c: TermCaps) {
    CAPS.store(c.pack(), Ordering::Relaxed);
}

/// Detect capabilities from the environment (`NO_COLOR`, `COLORTERM`, `TERM`) and set them.
/// Returns the detected capabilities.
///
/// Called automatically by [`crate::runtime::run`]; library consumers rendering into a `Buffer`
/// get deterministic truecolor unless they opt in.
///
/// Rules:
/// - `NO_COLOR` (any value) → `ColorDepth::None`
/// - `COLORTERM=truecolor` or `COLORTERM=24bit` → `ColorDepth::TrueColor`
/// - `TERM` ending in `-256color` → `ColorDepth::Ansi256`
/// - Otherwise → `ColorDepth::Ansi16`
pub fn probe_env() -> TermCaps {
    let color = probe_color_depth(
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    );
    let unicode = UnicodeLevel::Full; // No probe; assume full
    let caps = TermCaps { color, unicode };
    set_caps(caps);
    caps
}

fn probe_color_depth(
    no_color: Option<&str>,
    colorterm: Option<&str>,
    term: Option<&str>,
) -> ColorDepth {
    if no_color.is_some() {
        return ColorDepth::None;
    }
    if let Some(ct) = colorterm
        && (ct == "truecolor" || ct == "24bit")
    {
        return ColorDepth::TrueColor;
    }
    if let Some(t) = term
        && t.ends_with("-256color")
    {
        return ColorDepth::Ansi256;
    }
    ColorDepth::Ansi16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_maps_env_to_depth() {
        assert_eq!(
            probe_color_depth(Some("1"), None, None),
            ColorDepth::None,
            "NO_COLOR forces None"
        );
        assert_eq!(
            probe_color_depth(None, Some("truecolor"), None),
            ColorDepth::TrueColor,
            "COLORTERM=truecolor"
        );
        assert_eq!(
            probe_color_depth(None, Some("24bit"), None),
            ColorDepth::TrueColor,
            "COLORTERM=24bit"
        );
        assert_eq!(
            probe_color_depth(None, None, Some("xterm-256color")),
            ColorDepth::Ansi256,
            "TERM ends in -256color"
        );
        assert_eq!(
            probe_color_depth(None, None, Some("xterm")),
            ColorDepth::Ansi16,
            "bare TERM defaults to Ansi16"
        );
        assert_eq!(
            probe_color_depth(None, None, None),
            ColorDepth::Ansi16,
            "no env defaults to Ansi16"
        );
        // NO_COLOR wins even if others set
        assert_eq!(
            probe_color_depth(Some(""), Some("truecolor"), Some("xterm-256color")),
            ColorDepth::None,
            "NO_COLOR overrides everything"
        );
    }

    #[test]
    fn resolve_truecolor_is_identity() {
        let c = ColorDepth::TrueColor.resolve(128, 64, 192);
        assert_eq!(c, Color::Rgb(128, 64, 192));
    }

    #[test]
    fn resolve_ansi256_maps_to_cube_and_greys() {
        // Black
        assert_eq!(ColorDepth::Ansi256.resolve(0, 0, 0), Color::Indexed(16));
        // White
        assert_eq!(
            ColorDepth::Ansi256.resolve(255, 255, 255),
            Color::Indexed(231)
        );
        // Grey
        let grey = ColorDepth::Ansi256.resolve(128, 128, 128);
        assert!(matches!(grey, Color::Indexed(i) if (232..=255).contains(&i)));
        // A color in the 6×6×6 cube
        let red = ColorDepth::Ansi256.resolve(255, 0, 0);
        assert!(matches!(red, Color::Indexed(i) if (16..232).contains(&i)));
    }

    #[test]
    fn resolve_ansi16_picks_nearest() {
        // Should map to black
        assert_eq!(ColorDepth::Ansi16.resolve(0, 0, 0), Color::Indexed(0));
        // Should map to white or bright white
        let white = ColorDepth::Ansi16.resolve(255, 255, 255);
        assert!(matches!(white, Color::Indexed(7) | Color::Indexed(15)));
        // Red-ish should map to red or bright red
        let red = ColorDepth::Ansi16.resolve(200, 0, 0);
        assert!(matches!(red, Color::Indexed(1) | Color::Indexed(9)));
    }

    #[test]
    fn resolve_none_always_reset() {
        assert_eq!(ColorDepth::None.resolve(128, 64, 192), Color::Reset);
        assert_eq!(ColorDepth::None.resolve(0, 0, 0), Color::Reset);
        assert_eq!(ColorDepth::None.resolve(255, 255, 255), Color::Reset);
    }

    #[test]
    fn caps_roundtrip() {
        for &color in &[
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::None,
        ] {
            for &unicode in &[UnicodeLevel::Full, UnicodeLevel::Ascii] {
                let caps = TermCaps { color, unicode };
                let packed = caps.pack();
                let unpacked = TermCaps::unpack(packed);
                assert_eq!(caps, unpacked, "roundtrip {caps:?}");
            }
        }
    }

    #[test]
    fn set_and_get_caps() {
        // Save original
        let original = caps();

        set_caps(TermCaps {
            color: ColorDepth::Ansi256,
            unicode: UnicodeLevel::Ascii,
        });
        assert_eq!(caps().color, ColorDepth::Ansi256);
        assert_eq!(caps().unicode, UnicodeLevel::Ascii);

        // Restore
        set_caps(original);
    }

    use std::sync::Mutex;

    // Guard to prevent parallel tests from conflicting on global caps
    static CAPS_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn color_depths_produce_distinct_output() {
        use crate::draw;
        use crate::theme::Rgb;
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let _guard = CAPS_LOCK.lock().unwrap();
        let original = caps();

        let test_color = Rgb(80, 160, 240);
        let area = Rect::new(0, 0, 5, 3);

        // Render at each depth and collect the resulting fg color
        let mut results = vec![];
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::None,
        ] {
            set_caps(TermCaps {
                color: depth,
                unicode: UnicodeLevel::Full,
            });

            let mut buf = Buffer::empty(area);
            draw::fill(&mut buf, area, test_color);

            // Check a painted cell
            let cell = &buf[(1, 1)];
            results.push((depth, cell.bg));
        }

        set_caps(original);

        // All four depths must produce distinct results
        assert_eq!(
            results[0].1,
            Color::Rgb(80, 160, 240),
            "TrueColor should be RGB"
        );
        assert!(
            matches!(results[1].1, Color::Indexed(i) if i >= 16),
            "Ansi256 should be Indexed in cube/greys: got {:?}",
            results[1].1
        );
        assert!(
            matches!(results[2].1, Color::Indexed(i) if i <= 15),
            "Ansi16 should be Indexed 0..=15: got {:?}",
            results[2].1
        );
        assert_eq!(results[3].1, Color::Reset, "None should be Reset");

        // Distinct: TrueColor != Ansi256 != Ansi16 != None
        assert_ne!(results[0].1, results[1].1, "TrueColor vs Ansi256");
        assert_ne!(results[1].1, results[2].1, "Ansi256 vs Ansi16");
        assert_ne!(results[2].1, results[3].1, "Ansi16 vs None");
    }

    #[test]
    fn color_depth_none_is_attributes_only() {
        use crate::draw;
        use crate::theme::Rgb;
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let _guard = CAPS_LOCK.lock().unwrap();
        let original = caps();

        set_caps(TermCaps {
            color: ColorDepth::None,
            unicode: UnicodeLevel::Full,
        });

        let area = Rect::new(0, 0, 5, 3);
        let mut buf = Buffer::empty(area);
        draw::fill(&mut buf, area, Rgb(200, 100, 50));

        set_caps(original);

        // Under ColorDepth::None, both fg and bg should be Reset
        let cell = &buf[(1, 1)];
        assert_eq!(
            cell.fg,
            Color::Reset,
            "fg should be Reset under ColorDepth::None"
        );
        assert_eq!(
            cell.bg,
            Color::Reset,
            "bg should be Reset under ColorDepth::None"
        );
        // But the glyph is still painted (space for fill)
        assert_eq!(cell.symbol(), " ", "symbol should still be present");
    }

    #[test]
    fn unicode_level_ascii_makes_borders_ascii() {
        use crate::draw::Border;
        use crate::theme::Rgb;
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let _guard = CAPS_LOCK.lock().unwrap();
        let original = caps();

        set_caps(TermCaps {
            color: ColorDepth::TrueColor,
            unicode: UnicodeLevel::Ascii,
        });

        let area = Rect::new(0, 0, 5, 3);
        let mut buf = Buffer::empty(area);
        Border::Solid.draw(&mut buf, area, Rgb(200, 200, 200), Rgb(0, 0, 0));

        set_caps(original);

        // Top-left corner should be "+" in ASCII mode, not "┌"
        assert_eq!(buf[(0, 0)].symbol(), "+", "top-left corner should be ASCII");
        // Top edge should be "-" not "─"
        assert_eq!(buf[(1, 0)].symbol(), "-", "top edge should be ASCII");
        // Left edge should be "|" not "│"
        assert_eq!(buf[(0, 1)].symbol(), "|", "left edge should be ASCII");
    }

    #[test]
    fn unicode_level_ascii_keeps_none_and_ascii_borders_unchanged() {
        use crate::draw::Border;

        let _guard = CAPS_LOCK.lock().unwrap();
        let original = caps();

        set_caps(TermCaps {
            color: ColorDepth::TrueColor,
            unicode: UnicodeLevel::Ascii,
        });

        // Border::None, Border::Blank, and Border::Ascii should not be affected
        let none_glyphs = Border::None.glyphs();
        assert_eq!(none_glyphs, [" "; 8], "Border::None unchanged");

        let blank_glyphs = Border::Blank.glyphs();
        assert_eq!(blank_glyphs, [" "; 8], "Border::Blank unchanged");

        let ascii_glyphs = Border::Ascii.glyphs();
        assert_eq!(
            ascii_glyphs,
            ["+", "-", "+", "|", "|", "+", "-", "+"],
            "Border::Ascii unchanged"
        );

        set_caps(original);
    }
}
