# Theme roles

The tuile theme system resolves a small base palette into semantic colour roles for every surface, text, border and chrome element. Every role on `Theme` is a plain `Rgb` value you can read directly; most are derived by CIE-Lab lightness transformations or auto-contrast blending.

## Base palette

Set directly by `ThemeSpec`:

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Primary colour | `primary` | `ThemeSpec.primary` | Borders, buttons, accents (Primary variant) |
| Secondary colour | `secondary` | `ThemeSpec.secondary` or `primary` | Secondary variant buttons, links |
| Warning colour | `warning` | `ThemeSpec.warning` or `primary` | Warning variant elements |
| Error colour | `error` | `ThemeSpec.error` or `secondary` | Error variant elements |
| Success colour | `success` | `ThemeSpec.success` or `secondary` | Success variant elements |
| Accent colour | `accent` | `ThemeSpec.accent` or `primary` | Accent variant highlights |
| Background | `background` | `ThemeSpec.background` or `#121212` (dark) / `#efefef` (light) | Page background |
| Foreground | `foreground` | `ThemeSpec.foreground` or `background.inverse()` | Maximum contrast text reference |

## Surface roles

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Surface | `surface` | `ThemeSpec.surface` or `#1e1e1e` (dark) / `#f5f5f5` (light) | Default widget background |
| Panel | `panel` | `surface.blend(primary, 0.1)` + 4% contrast tint (dark) | Containers, panels, cards |
| Boost | `boost` | `surface.blend(foreground, 0.04)` | Nested containers (Textual `$boost`) |

## Text roles

All text roles are derived for readability over the background. The `auto N%` notation means `background.text_on(N)`, which blends the best-contrast text (white or black) at the given alpha.

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Default text | `text` | `background.text_on(0.87)` | Primary body text (auto 87%) |
| Muted text | `text_muted` | `background.text_on(0.60)` | Secondary labels, help text (auto 60%) |
| Disabled text | `text_disabled` | `background.text_on(0.38)` | Disabled controls (auto 38%) |
| Primary text | `text_primary` | `foreground.blend(primary, 0.66)` | Tinted text for Primary variant |
| Secondary text | `text_secondary` | `foreground.blend(secondary, 0.66)` | Tinted text for Secondary variant |
| Accent text | `text_accent` | `foreground.blend(accent, 0.66)` | Tinted text for Accent variant |
| Success text | `text_success` | `foreground.blend(success, 0.66)` | Tinted text for Success variant |
| Warning text | `text_warning` | `foreground.blend(warning, 0.66)` | Tinted text for Warning variant |
| Error text | `text_error` | `foreground.blend(error, 0.66)` | Tinted text for Error variant |

## Border and cursor roles

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Border (focused) | `border` | `primary` | Focused widget borders |
| Border (blurred) | `border_blurred` | `surface.darken(0.025)` | Unfocused borders (CIE-Lab −2.5% L*) |
| Cursor background | `cursor_bg` | `primary` | Text cursor fill colour |
| Cursor foreground | `cursor_fg` | `primary.text_on(0.9)` | Text under cursor (auto 90% on cursor_bg) |
| Cursor blurred | `cursor_blurred_bg` | `surface.blend(primary, 0.3)` | Unfocused cursor (30% primary over surface) |

## Interaction roles

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Hover background | `hover_bg` | `surface.blend(foreground, 0.08)` | Button/item hover state |
| Selection background | `selection_bg` | `surface.blend(primary, 0.5)` | Selected text, list items |
| Link colour | `link` | `secondary.blend(foreground, 0.2)` | Hyperlinks |

## Scrollbar roles

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Scrollbar thumb | `scrollbar` | `background.darken(0.15).blend(primary, 0.4)` | Scrollbar handle (40% primary on darkened bg) |
| Scrollbar hover | `scrollbar_hover` | `background.darken(0.15).blend(primary, 0.6)` | Hovered scrollbar thumb (60% primary) |
| Scrollbar track | `scrollbar_bg` | `background.darken(0.15)` | Scrollbar track (CIE-Lab −15% L*) |

## Special widget roles

| Role | Field | Derived from | Used for |
|------|-------|--------------|----------|
| Toast background | `toast_bg` | `panel.lighten(0.15)` | Toast/notification background (CIE-Lab +15% L*) |
| Footer background | `footer_bg` | `panel` | KeyFooter background |
| Footer key | `footer_key` | `accent` | KeyFooter key binding colour |
| Footer description | `footer_desc` | `panel.text_on(0.87)` | KeyFooter description text (auto 87% on panel) |
| Markdown code background | `markdown_code_bg` | `surface.blend(foreground, 0.06)` | Inline code blocks in Markdown |

## Helper methods

### Theme::shade

```text
pub fn shade(c: Rgb, n: i32) -> Rgb
```

Textual's `$color-lighten-N` / `$color-darken-N`: shifts CIE-Lab L* by 15% per step. Positive `n` lightens, negative darkens. Matches Textual's lightness transformations (e.g. `shade(primary, 1)` = +15% L*, `shade(primary, -2)` = −30% L*).

### Rgb::text_on

```text
pub fn text_on(self, alpha: f32) -> Rgb
```

Textual's `auto N%`: blends the best-contrast text (white or black, chosen by `self.contrast_text()`) over this colour at the given alpha (0.0 = transparent, 1.0 = fully opaque). Use this to generate readable text over any background: `bg.text_on(0.87)` gives primary readable text, `bg.text_on(0.60)` gives muted secondary text.

## Choosing the right role

| What you are drawing | Role to use |
|---|---|
| Text over the page background | `text`, `text_muted` or `text_disabled` |
| Text over a widget surface or panel | `surface.text_on(0.87)` or `panel.text_on(0.87)` |
| Text inside a coloured button or badge | `variant_colour.text_on(0.9)` |
| Text tinted to carry meaning | `text_primary`, `text_success`, `text_error` and the rest of that set |
| A border | `border` when focused, `border_blurred` when not |
| A hover state | blend `hover_bg` over the widget surface |
| A lighter or darker step of any colour | `Theme::shade(colour, steps)`, which matches Textual's 15% L\* step, rather than raw `lighten()` |
