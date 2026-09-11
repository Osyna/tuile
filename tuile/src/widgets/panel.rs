//! Panel container with borders, titles, padding, shadows, and background variants.
//! Returns the inner content rect. Also provides `Card` and `Section` presets.
//!
//! ```no_run
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 60, 15);
//! # let mut buf = Buffer::empty(area);
//! let inner = Panel::new().title("Settings").border(Border::Round).shadow(true).render(area, &mut buf);
//! // draw content into `inner`
//! ```

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Rect};

use crate::draw::{Border, fill, put, put_aligned, shadow, st};
use crate::theme::{self, Rgb, Theme, Variant};
use ratatui_core::style::Modifier;

/// Panel background fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelBg {
    /// No fill (transparent).
    #[default]
    Transparent,
    /// `theme.surface`.
    Surface,
    /// `theme.panel`.
    Panel,
    /// `theme.boost`.
    Boost,
    /// Custom colour.
    Custom(Rgb),
}

/// Panel builder (Widget-like, but returns inner rect instead of implementing `Widget`).
#[derive(Clone, Debug)]
pub struct Panel {
    title: Option<String>,
    title_align: Alignment,
    subtitle: Option<String>,
    title_right: Option<String>,
    footer: Option<String>,
    border: Option<Border>,
    border_color: Option<Rgb>,
    variant: Option<Variant>,
    focused: bool,
    background: PanelBg,
    padding: (u16, u16),
    shadow_enabled: bool,
    badge: Option<String>,
    theme: Option<Theme>,
}

impl Panel {
    pub fn new() -> Self {
        Self {
            title: None,
            title_align: Alignment::Left,
            subtitle: None,
            title_right: None,
            footer: None,
            border: Some(Border::Round),
            border_color: None,
            variant: None,
            focused: false,
            background: PanelBg::default(),
            padding: (1, 0),
            shadow_enabled: false,
            badge: None,
            theme: None,
        }
    }

    /// Preset: card (Surface bg, Round border, shadow).
    pub fn card() -> Self {
        Self::new()
            .background(PanelBg::Surface)
            .border(Border::Round)
            .shadow(true)
    }

    /// Preset: section (title row with horizontal rule, no box).
    pub fn section() -> Self {
        Self::new().border(Border::None)
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn title_align(mut self, a: Alignment) -> Self {
        self.title_align = a;
        self
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    /// Second title, right-aligned in the top border (btop's `┤ io ├`).
    pub fn title_right(mut self, s: impl Into<String>) -> Self {
        self.title_right = Some(s.into());
        self
    }

    /// Key hints embedded left in the bottom border (`┤ sync ├┤ auto ├`).
    pub fn footer(mut self, s: impl Into<String>) -> Self {
        self.footer = Some(s.into());
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn border_color(mut self, c: Rgb) -> Self {
        self.border_color = Some(c);
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = Some(v);
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    pub fn background(mut self, bg: PanelBg) -> Self {
        self.background = bg;
        self
    }

    pub fn padding(mut self, h: u16, v: u16) -> Self {
        self.padding = (h, v);
        self
    }

    pub fn shadow(mut self, s: bool) -> Self {
        self.shadow_enabled = s;
        self
    }

    pub fn badge(mut self, b: impl Into<String>) -> Self {
        self.badge = Some(b.into());
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    /// Render the panel and return the inner content rect.
    pub fn render(self, area: Rect, buf: &mut Buffer) -> Rect {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.width == 0 || area.height == 0 {
            return Rect::default();
        }

        // Background fill
        let bg = match self.background {
            PanelBg::Transparent => None,
            PanelBg::Surface => Some(th.surface),
            PanelBg::Panel => Some(th.panel),
            PanelBg::Boost => Some(th.boost),
            PanelBg::Custom(c) => Some(c),
        };
        if let Some(color) = bg {
            fill(buf, area, color);
        }

        // Shadow
        if self.shadow_enabled && area.width > 1 && area.height > 1 {
            shadow(buf, area, th.background, 0.3);
        }

        // Border
        let border_color = self
            .border_color
            .or_else(|| self.variant.map(|v| th.variant(v)))
            .unwrap_or({
                if self.focused {
                    th.border
                } else {
                    th.border_blurred
                }
            });

        let inner = if let Some(bord) = self.border {
            if bord == Border::None {
                // Section style: title + rule
                if let Some(ref title) = self.title {
                    if area.height > 0 {
                        let title_bg = bg.unwrap_or(th.background);
                        put_aligned(
                            buf,
                            area,
                            title,
                            self.title_align,
                            st(th.foreground, title_bg),
                        );
                        if area.height > 1 {
                            for x in area.x..area.right() {
                                if x >= area.x
                                    && x < area.right()
                                    && let Some(c) = buf.cell_mut((x, area.y + 1))
                                {
                                    c.set_symbol("─")
                                        .set_fg(border_color.color())
                                        .set_bg(title_bg.color());
                                }
                            }
                        }
                        Rect {
                            x: area.x,
                            y: area.y + 2,
                            width: area.width,
                            height: area.height.saturating_sub(2),
                        }
                    } else {
                        area
                    }
                } else {
                    area
                }
            } else {
                let border_bg = bg.unwrap_or(th.background);
                if let Some(ref title) = self.title {
                    // Combine title and badge
                    let full_title = if let Some(ref badge) = self.badge {
                        format!("{} [{}]", title, badge)
                    } else {
                        title.clone()
                    };
                    // Textual: titles are `$text` bold; only focus / a semantic variant tint them
                    let title_fg = if self.focused || self.variant.is_some() {
                        border_color
                    } else {
                        th.text
                    };
                    bord.draw_titled_with(
                        buf,
                        area,
                        border_color,
                        border_bg,
                        &full_title,
                        self.title_align,
                        st(title_fg, border_bg).add_modifier(Modifier::BOLD),
                    );
                } else {
                    bord.draw(buf, area, border_color, border_bg);
                }

                // Right title in the top border, footer in the bottom border, subtitle bottom-right
                if let Some(t) = &self.title_right {
                    let text = format!(" {t} ");
                    let tw = crate::draw::width(&text) as u16;
                    if area.width > tw + 4 {
                        put(
                            buf,
                            area.right() - tw - 2,
                            area.y,
                            &text,
                            tw,
                            st(th.text, border_bg).add_modifier(Modifier::BOLD),
                        );
                    }
                }
                if let Some(f) = &self.footer {
                    let text = format!(" {f} ");
                    let fw = crate::draw::width(&text) as u16;
                    if area.width > fw + 4 && area.height > 1 {
                        put(
                            buf,
                            area.x + 2,
                            area.bottom() - 1,
                            &text,
                            fw,
                            st(th.text_muted, border_bg),
                        );
                    }
                }
                if let Some(subtitle) = &self.subtitle {
                    let sw = crate::draw::width(subtitle) as u16;
                    if area.width > sw + 4 && area.height > 1 {
                        let sub_x = area.right() - sw - 2;
                        let sub_y = area.bottom() - 1;
                        put(
                            buf,
                            sub_x,
                            sub_y,
                            subtitle,
                            sw,
                            st(th.text_muted, border_bg),
                        );
                    }
                }

                Rect {
                    x: area.x + 1,
                    y: area.y + 1,
                    width: area.width.saturating_sub(2),
                    height: area.height.saturating_sub(2),
                }
            }
        } else {
            area
        };

        // Padding
        let (ph, pv) = self.padding;
        Rect {
            x: inner.x + ph,
            y: inner.y + pv,
            width: inner.width.saturating_sub(ph * 2),
            height: inner.height.saturating_sub(pv * 2),
        }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

/// Standalone `Placeholder` widget: Textual's placeholder block (cycles colours, shows size/name).
#[derive(Clone, Debug)]
pub struct Placeholder {
    variant: PlaceholderVariant,
    index: usize,
    theme: Option<Theme>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlaceholderVariant {
    /// Show name only.
    #[default]
    Default,
    /// Show size (WxH).
    Size,
    /// Show custom text.
    Text,
}

const PLACEHOLDER_COLORS: &[Rgb] = &[
    Rgb(136, 17, 119),  // #881177
    Rgb(170, 51, 85),   // #aa3355
    Rgb(204, 102, 102), // #cc6666
    Rgb(238, 153, 68),  // #ee9944
    Rgb(238, 221, 0),   // #eedd00
    Rgb(153, 221, 85),  // #99dd55
    Rgb(68, 221, 136),  // #44dd88
    Rgb(34, 204, 187),  // #22ccbb
    Rgb(0, 187, 204),   // #00bbcc
    Rgb(0, 153, 204),   // #0099cc
    Rgb(51, 102, 187),  // #3366bb
    Rgb(102, 51, 153),  // #663399
];

impl Placeholder {
    pub fn new() -> Self {
        Self {
            variant: PlaceholderVariant::default(),
            index: 0,
            theme: None,
        }
    }

    pub fn variant(mut self, v: PlaceholderVariant) -> Self {
        self.variant = v;
        self
    }

    pub fn index(mut self, i: usize) -> Self {
        self.index = i;
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer, name: &str) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let color = PLACEHOLDER_COLORS[self.index % PLACEHOLDER_COLORS.len()];
        fill(buf, area, color);

        let text = match self.variant {
            PlaceholderVariant::Default => name.to_string(),
            PlaceholderVariant::Size => format!("{}×{}", area.width, area.height),
            PlaceholderVariant::Text => name.to_string(),
        };

        if !text.is_empty() {
            put_aligned(
                buf,
                area,
                &text,
                Alignment::Center,
                st(Rgb(255, 255, 255), color),
            );
        }
    }
}

impl Default for Placeholder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::buffer::Buffer;

    #[test]
    fn panel_inner_rect() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        let inner = Panel::new()
            .border(Border::Round)
            .padding(2, 1)
            .render(buf.area, &mut buf);
        // Border takes 2 (1 on each side), padding adds 2 more horizontally and 1 vertically on each side
        assert_eq!(inner.x, 3); // 1 (border) + 2 (padding)
        assert_eq!(inner.y, 2); // 1 (border) + 1 (padding)
        assert_eq!(inner.width, 40 - 2 - 4); // -2 for border, -4 for padding
        assert_eq!(inner.height, 20 - 2 - 2); // -2 for border, -2 for padding
    }

    #[test]
    fn panel_no_panic_small() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        let _ = Panel::new().render(buf.area, &mut buf);
        // Should not panic
    }

    #[test]
    fn placeholder_cycles_colors() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        Placeholder::new().index(0).render(buf.area, &mut buf, "A");
        let c1 = buf.cell((10, 5)).map(|c| c.bg).unwrap_or_default();

        let mut buf2 = Buffer::empty(Rect::new(0, 0, 20, 10));
        Placeholder::new()
            .index(1)
            .render(buf2.area, &mut buf2, "B");
        let c2 = buf2.cell((10, 5)).map(|c| c.bg).unwrap_or_default();

        assert_ne!(c1, c2);
    }
}
