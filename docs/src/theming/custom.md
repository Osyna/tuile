# Your own theme

A theme is a `ThemeSpec` you resolve. The minimum is a name, a mode and one colour:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let mine = ThemeSpec::new("acme", true, Rgb::hex(0x7c3aed)).resolve();
theme::set(mine);
# theme::set_by_name("textual-dark");
# }
```

Everything else is derived. That is usually too uniform for a real product, so add the
colours that carry meaning in your domain and leave the rest:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let spec = ThemeSpec::new("acme", true, Rgb::hex(0x7c3aed))
    .secondary(Rgb::hex(0x2dd4bf))
    .accent(Rgb::hex(0xfbbf24))
    .success(Rgb::hex(0x22c55e))
    .warning(Rgb::hex(0xf59e0b))
    .error(Rgb::hex(0xef4444))
    .background(Rgb::hex(0x0f0f17))
    .surface(Rgb::hex(0x1a1a24));
theme::set(spec.resolve());
# theme::set_by_name("textual-dark");
# }
```

Every builder method is `const`, so a theme can be a constant:

```rust
# extern crate tuile;
# use tuile::prelude::*;
const ACME: ThemeSpec = ThemeSpec::new("acme", true, Rgb::hex(0x7c3aed))
    .accent(Rgb::hex(0xfbbf24));
# fn main() { let _ = ACME.resolve(); }
```

## Colour construction

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let a = Rgb::hex(0x7c3aed);
let b = Rgb(124, 58, 237);
assert_eq!(a, b);

let lighter = a.shade(1);        // +15% L* in CIE-Lab
let darker = a.shade(-2);        // -30%
let mixed = a.blend(b, 0.5);     // linear interpolation
let on_top = a.text_on(0.87);    // readable text for this background
# let _ = (lighter, darker, mixed, on_top);
# }
```

`Rgb::hex` takes the same `0xRRGGBB` you would paste from a design tool. There is no CSS
string parser, and no named colours.

## Starting from a built-in

Copy a palette and change what you need:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let mut spec = *theme::BUILTIN.iter().find(|s| s.name == "gruvbox").expect("ships with the crate");
spec.name = "gruvbox-hot";
spec.error = Some(Rgb::hex(0xff0033));
theme::set(spec.resolve());
# theme::set_by_name("textual-dark");
# }
```

`ThemeSpec` is `Copy` with public fields, so this needs no builder call at all.

## Per-widget themes

Every builder takes `.theme(&Theme)`, which overrides the global for that widget:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, st: &mut ButtonState) {
let danger = ThemeSpec::new("danger", true, Rgb::hex(0xef4444)).resolve();
Button::new("Delete").theme(&danger).render(area, buf, st);
# }
# fn main() {}
```

Resolve the theme once and keep it in your app struct. Resolving it every frame repeats the
whole Lab expansion for no benefit.

A per-widget theme is the right tool for a panel that must stand apart, a preview pane
showing another palette, or a diff view with its own colours. For anything larger, set the
global theme.

## Letting users pick

Offer `theme::theme_names()` in a select and call `set_by_name` on change:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct Prefs { picker: SelectState }
# impl Prefs {
fn on_theme_picked(&mut self, name: &str) {
    if !theme::set_by_name(name) {
        // unknown name, keep the current theme
    }
}
# }
# fn main() {}
```

`set_by_name` returns `false` rather than panicking or falling back silently, so a stale name
in a config file leaves the user with the previous theme instead of a surprise.

To persist a custom theme, store the spec's ten colours, not the resolved theme. The
resolution rules can change between versions; the palette you chose will not.

## What you cannot theme

The derivation is fixed. You choose the base colours; you do not choose that `text_muted` is
60% alpha, or that hover is an 8% tint. Widgets read named roles, so changing a role means
changing every widget that reads it.

If you need one role to differ, build a `Theme` by resolving a spec and then assigning the
field directly. `Theme` is a plain struct with public fields:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let mut th = ThemeSpec::new("acme", true, Rgb::hex(0x7c3aed)).resolve();
th.hover_bg = Rgb::hex(0x2a2a3a);       // stronger hover than the 8% default
theme::set(th);
# theme::set_by_name("textual-dark");
# }
```

That is supported and occasionally the right answer. Be aware you are stepping outside the
derivation, so the override will not follow when you change the base palette.
