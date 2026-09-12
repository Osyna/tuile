# Terminal capabilities

tuile resolves the terminal's **colour depth** and **Unicode level** once, then applies them to
every colour and every border, so a 256-colour terminal, `NO_COLOR`, or a font without box drawing
all get something readable instead of whatever the palette happened to produce.

## The default is deterministic truecolour

The library defaults to **truecolour RGB** and **full Unicode**. Nothing probes the environment
unless you explicitly call `term::probe_env()` or `runtime::run()` (which calls it as part of
terminal setup).

This is deliberate. A consumer rendering into a `Buffer` for a test or an image gets
deterministic output. If capability detection happened implicitly, every colour assertion would
read "default" when run in a CI agent shell that exports `NO_COLOR=1`.

## Automatic detection

`runtime::run()` calls `term::probe_env()` as the first step of terminal setup. It reads:

- **`NO_COLOR`** (any value) → attributes only, no colour
- **`COLORTERM=truecolor`** or `COLORTERM=24bit` → 24-bit RGB
- **`TERM` ending in `-256color`** → 256-colour palette
- **Otherwise** → 16-colour ANSI palette

`probe_env` only decides colour; the Unicode level stays `Full` unless you set it, because no
environment variable reports whether the user's font has box drawing.

## Explicit override

Call `term::set_caps()` to override detection:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {

// Force a colour depth (for app settings or tests)
term::set_caps(TermCaps {
    color: ColorDepth::Ansi256,
    unicode: UnicodeLevel::Full,
});

// Or disable colour entirely
term::set_caps(TermCaps {
    color: ColorDepth::None,
    unicode: UnicodeLevel::Full,
});
# }
```

## How it works

### Colour resolution

Every `Rgb` → `Color` conversion consults `term::caps().color`:

- **`ColorDepth::TrueColor`** → `Color::Rgb(r, g, b)` (the default)
- **`ColorDepth::Ansi256`** → `Color::Indexed(16..=255)` (6×6×6 cube + 24 greys)
- **`ColorDepth::Ansi16`** → `Color::Indexed(0..=15)` (nearest of the 16 ANSI colours)
- **`ColorDepth::None`** → `Color::Reset` (attributes only: bold, dim, italic, underline)

No widget code changes. Every widget calls `Rgb::color()` to resolve theme colours; that one
hook covers the entire library.

### Glyph fallback

With `term::caps().unicode` set to `UnicodeLevel::Ascii`, `draw::Border` swaps its box-drawing
glyphs (`┌─┐│└┘`) for `+ - |`. That is the case that breaks a whole screen at once: a font without
box drawing turns every panel into mojibake. Widget glyphs outside the border tables (`■`, `●`,
`✓`, the sparkline and big-text blocks) are still Unicode-only.

## The NO_COLOR trap

Many CI environments and agent shells export `NO_COLOR=1`. A test that probes the environment and
then asserts on a colour is asserting on `Color::Reset` — it passes while checking nothing. This is
why probing is opt-in, and there are two ways to stay honest in a suite that does probe:

1. Pin the depth in the test:

   ```rust
   #[test]
   fn widget_renders_red() {
       term::set_caps(TermCaps {
           color: ColorDepth::TrueColor,
           unicode: UnicodeLevel::Full,
       });
       // ... now colour assertions work
   }
   ```

2. **Assert on the resolved `Color` in the buffer, not on the source `Rgb`.** The buffer holds
   what actually rendered, capability resolution included.

## Performance

`term::caps()` is one relaxed atomic load: no environment access, no lock, no allocation. It sits
on the per-cell style path, inside every `Rgb::color()` call.
