# Custom spinners

The catalog has 102 spinners: the 90 from [cli-spinners](https://github.com/sindresorhus/cli-spinners)
and yaspin, plus 12 written for this crate. Defining your own takes one line, because a
spinner is a name, an interval and a list of frames.

![the spinners page of the showcase](../screenshots/spinners.png)

## Using one from the catalog

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Spinner::new(&spinners::DOTS).label("Loading").now(now).render(area, buf);
# }
# fn main() {}
```

Every entry is a `pub const` in `spinners`, screaming case: `DOTS`, `LINE`, `ARC`,
`BOUNCING_BAR`, `MOON`, `EARTH`, `CLOCK`, and the originals `SPARKLE`, `RING`, `WAVE`,
`EQUALIZER`, `SCANNER`, `SHIMMER`, `DNA`, `MATRIX`. `spinners::ALL` is the whole catalog as a
slice, which is what the showcase's filterable list iterates.

Look one up by name when it comes from a config file:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let def: &SpinnerDef = SpinnerDef::by_name("dots").unwrap_or(&spinners::LINE);
assert_eq!(def.name, "dots");
# }
```

Names are the lower-case cli-spinners names, so a config that says `dots12` finds the same
spinner it would in a Node tool.

## Defining one

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
const PULSE: SpinnerDef = SpinnerDef::new("pulse", 120, &["·", "•", "●", "•"]);

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Spinner::new(&PULSE).now(now).render(area, buf);
# }
# fn main() {}
```

`SpinnerDef::new(name, interval_ms, frames)` is `const`, so a spinner can be a constant with
no allocation and no lazy init. The three fields are public if you prefer struct syntax.

Two rules for the frames. Every frame should be the same display width, or the spinner will
shift the text next to it on each tick; `def.width()` reports the widest frame so you can
check. And the glyphs must exist in the fonts your users have: braille dots are safe almost
everywhere, emoji are not, and anything with the Unicode Emoji property may be drawn two
cells wide by some terminals while `unicode-width` calls it one.

## Frames from data

When the frames come from a config file or are computed, they are not `'static`:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant, frames: &[&str]) {
Spinner::frames(frames, 90).now(now).render(area, buf);
# }
# fn main() {}
```

`Spinner::frames(&[&str], interval_ms)` borrows for the lifetime of the builder. Nothing is
copied; the slice only has to outlive the render call.

## Timing

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Spinner::new(&spinners::DOTS).now(now).render(area, buf);        // phase from the shared epoch
Spinner::new(&spinners::DOTS).elapsed(1.25).render(area, buf);   // pinned, deterministic
# }
# fn main() {}
```

`.now(instant)` measures the phase from the process-wide epoch, so two spinners on screen
with the same interval stay in step without any shared state. `.elapsed(secs)` pins the
animation, which is what tests and the screenshot tool use.

The interval lives in the definition, not the builder. To run the same frames faster, define
another constant; the spinner does not take a speed multiplier.

## Colour and label

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let th = theme::current();
Spinner::new(&spinners::ARC)
    .color(th.accent)
    .label("Compiling")
    .now(now)
    .render(area, buf);
# }
# fn main() {}
```

Without `.color(..)` the spinner uses the theme's primary. The label is drawn after the
frame with one space between, and `.width()` on the builder reports the total cells needed,
which is what you want when reserving space in a status line.

## Regenerating the catalog

The catalog is generated, not hand-written:

```text
cargo xtask spinners     # reads xtask/spinners.json, writes spinner/spinners.rs
```

Add an entry to `xtask/spinners.json` and re-run it to add a spinner to the shipped set.
Editing `spinners.rs` by hand works until the next regeneration overwrites it.
