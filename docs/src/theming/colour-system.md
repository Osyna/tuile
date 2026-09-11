# The colour system

You give tuile up to ten colours. It derives about thirty from them. That derivation is the
design system, and it is a port of Textual's `ColorSystem`.

The point is that a widget never asks for "grey 40". It asks for `text_muted`, and the theme
decides what that means in a dark terminal versus a light one. Swap the palette and every
widget follows, including widgets written after the palette was chosen.

## The base palette

A `ThemeSpec` has one required colour and nine optional ones:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let spec = ThemeSpec::new("ocean", true, Rgb::hex(0x0178D4))   // name, dark, primary
    .secondary(Rgb::hex(0x004578))
    .accent(Rgb::hex(0xffa62b))
    .background(Rgb::hex(0x1e1e2e));
let theme = spec.resolve();
assert_eq!(theme.name, "ocean");
# }
```

| Field | Required | Default when omitted |
|---|---|---|
| `name` | yes | |
| `dark` | yes | |
| `primary` | yes | |
| `secondary` | no | falls back to `primary` |
| `warning`, `error`, `success`, `accent` | no | fall back to `primary` |
| `background` | no | near-black for a dark theme, near-white for a light one |
| `foreground` | no | the inverse of the background |
| `surface` | no | derived from the background |
| `panel` | no | surface blended 10% toward primary, lifted slightly in dark themes |

A one-colour theme is legal and looks coherent, because everything unstated is derived from
`primary` and the background. It is also monotonous: three or four well-chosen colours is the
sweet spot.

## What gets derived

`Theme::resolve(&spec, primary_override)` computes the full role set. Four mechanisms do all
the work.

**CIE-Lab lightness steps.** `Rgb::shade(n)` moves L\* by 15% per step, so `shade(1)` is one
step lighter and `shade(-2)` two steps darker. Doing this in Lab rather than RGB keeps hue
and saturation stable: darkening a blue in RGB drifts it toward black-grey, in Lab it stays
blue.

**Contrast-aware text.** `Rgb::text_on(alpha)` picks white or black by the background's
brightness, then composites it at `alpha`. That is where the text ramp comes from:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let bg = Rgb::hex(0x1e1e2e);
let _text = bg.text_on(0.87);           // primary text
let _muted = bg.text_on(0.60);          // secondary text
let _disabled = bg.text_on(0.38);       // disabled text
# }
```

Those three alphas are fixed in the library, matching Textual's. A light theme gets dark text
from the same code, with no branch in any widget.

**Blending toward an accent.** Interaction roles are the surface mixed with something:
`hover_bg` is the surface 8% toward the contrast colour, `selection_bg` is the surface 50%
toward primary, `cursor_blurred_bg` is 30% toward primary. Tinting rather than replacing is
what keeps a selected row readable in every palette.

**Semantic text colours.** `text_primary`, `text_success`, `text_error` and friends are the
contrast colour blended 66% toward the matching base colour. They are readable on the
background, unlike the raw `error` red, which is chosen for fills.

## Reading a theme

`Theme` is `Copy`, so hold it in a local for the frame:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let th = theme::current();
fill(buf, area, th.background);
put(buf, area.x, area.y, "Ready", area.width, st(th.text_success, th.background));
# }
# fn main() {}
```

`st(fg, bg)` builds a ratatui `Style` from two `Rgb` values. The
[theme roles](../reference/theme-roles.md) table lists every field with what it is derived
from and what it is for.

## Variants

Six semantic variants map to base colours through `th.variant(v)`:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let th = theme::current();
assert_eq!(th.variant(Variant::Success), th.success);
assert_eq!(th.variant(Variant::Default), th.surface);
# }
```

`Default` resolves to the surface colour, which is deliberate: a default button is a filled
neutral shape, not a coloured one. Widgets that need a visible colour for `Default` (an
outline button, whose surface-coloured border would be invisible) substitute a text colour
instead.

## Switching at runtime

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
theme::set_by_name("nord");                   // false if the name is unknown
let names: Vec<&str> = theme::theme_names();  // every built-in
assert!(names.contains(&"nord"));

theme::set(Theme::resolve(&ThemeSpec::new("mine", true, Rgb::hex(0x7c3aed)), None));
# theme::set_by_name("textual-dark");
# }
```

`theme::current()` is the process-wide fallback every widget reads when no `.theme(..)` was
passed. It is behind a lock, read once per widget per frame. Two globals exist in the whole
library and this is one of them; the other is the animation epoch.

## Contrast is derived, not checked

The system produces readable text for any reasonable palette because the text roles are
computed from the background. It does not measure the result against a contrast standard such
as WCAG. A palette with a mid-grey background and a mid-grey primary will produce legal but
uncomfortable output, and nothing in the library will warn you. Look at the Themes page of
the showcase with your palette loaded before shipping it.
