# Built-in palettes

Twelve palettes ship with the crate. Nine are dark, three are light.

![the themes page, showing every palette side by side](../screenshots/themes.png)

| Name | Mode | Primary | Notes |
|---|---|---|---|
| `textual-dark` | dark | `#0178D4` | The default. Textual's own dark palette |
| `textual-light` | light | `#004578` | Textual's light palette |
| `nord` | dark | `#88C0D0` | Cool blue-grey, low contrast |
| `gruvbox` | dark | `#85A598` | Warm, muted, high legibility |
| `catppuccin-mocha` | dark | `#F5C2E7` | Pastel on a soft dark background |
| `catppuccin-latte` | light | `#8839EF` | The light member of the same family |
| `dracula` | dark | `#BD93F9` | Purple on near-black |
| `tokyo-night` | dark | `#BB9AF7` | Deep blue background, violet accents |
| `monokai` | dark | `#AE81FF` | The classic editor palette |
| `flexoki` | dark | `#205EA6` | Paper-inspired, ink-like text |
| `solarized-light` | light | `#268bd2` | Solarized's light half |
| `rose-pine` | dark | `#c4a7e7` | Muted mauve and pine |

## Using one

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
theme::set_by_name("nord");                  // returns false for an unknown name
# theme::set_by_name("textual-dark");
# }
```

Set it once at startup, before the first frame. Widgets read `theme::current()` at render
time, so changing it mid-run takes effect on the next redraw with no invalidation step.

Getting a palette without installing it globally:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let gruvbox: Theme = theme::builtin("gruvbox").expect("gruvbox ships with the crate");
assert_eq!(gruvbox.name, "gruvbox");
# }
```

`theme::builtin(name)` resolves the palette for you and returns the finished `Theme`. The
unresolved specs are in `theme::BUILTIN`, which is what you want when a built-in palette is
the starting point for a theme of your own.

## Listing them

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
for name in theme::theme_names() {
    println!("{name}");
}
# }
```

This is what a theme picker in your app should iterate, rather than a hard-coded list that
goes stale when the crate adds a palette. The showcase's `Ctrl-T` cycle and its Themes page
both use it.

## Dark and light

`spec.dark` is a declaration, not a computation. It tells the resolver which direction to
derive in: dark themes lift the panel colour slightly toward the contrast colour, light
themes do not. The text ramp itself keys off the background's brightness, so a light palette
gets dark text automatically.

If you build a light theme and mark it `dark = true`, the text will still be readable because
the ramp measures the background. The panel and boost surfaces will be subtly wrong.

## Overriding one colour

`Theme::resolve` takes an optional primary override, which re-derives everything that depends
on primary while keeping the rest of the palette:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let nord = theme::BUILTIN.iter().find(|s| s.name == "nord").unwrap();
let pink = Theme::resolve(nord, Some(Rgb::hex(0xff79c6)));
assert_ne!(pink.border, theme::builtin("nord").unwrap().border);
# }
```

Borders, cursors, selections and the scrollbar all follow, because each is derived from
primary. This is the hue slider on the showcase's Themes page.

For anything more than the primary, build a spec of your own; the
[next chapter](custom.md) covers it.

## Terminal caveats

Every palette is true colour (24-bit). In a terminal limited to 256 colours, ratatui's
backend maps each colour to the nearest palette entry, which flattens the small lightness
steps the design system relies on: a hover row and an unhovered row can land on the same
index. The library does not detect this and has no 256-colour palette of its own.

Terminals with a minimum-contrast setting (iTerm2 and some others) adjust foreground colours
that sit too close to their background. That is why the library paints filled areas as
background colours with space characters rather than as foreground block glyphs; a block
glyph gets recoloured by that feature and a painted background does not.
