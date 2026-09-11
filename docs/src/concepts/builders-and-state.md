# Builders and state

Every widget in tuile is split in two: a builder you create fresh each frame, and a state
that lives in your app between frames.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, state: &mut CheckboxState) {
Checkbox::new("Send crash reports")
    .focused(true)
    .render(area, buf, state);
# }
# fn main() {}
```

`Checkbox::new(..)` is the builder. `CheckboxState` is the state. The split is the single
most important thing to understand about the library, because it decides where your data
lives and what you are allowed to mutate.

## The builder is a value, not an object

A builder holds only configuration: the label, the variant, the focus flag, the frame's
`Instant`. It is created, consumed by `render`, and dropped. It does not allocate unless you
hand it an owned `String`, it borrows slices rather than copying them, and it never reads
global mutable state except the fallback theme.

This is why builder chains appear in the middle of a draw function rather than in a struct
field: rebuilding one costs nothing, and it keeps the configuration next to the layout that
feeds it.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, st: &mut ButtonState, enabled: bool) {
// Configuration follows the data it depends on, with no separate "update" step.
Button::new(if enabled { "Deploy" } else { "Deploy (blocked)" })
    .variant(if enabled { Variant::Primary } else { Variant::Default })
    .enabled(enabled)
    .render(area, buf, st);
# }
# fn main() {}
```

## The state owns everything that must survive

A `<Name>State` holds the value and the interaction bookkeeping: hover and press flags, the
cursor position, the scroll offset, running tweens, and the rectangles the widget drew into
last frame.

States are plain data. They derive `Clone` and `Debug`, their useful fields are public, and
they contain no callbacks or parent pointers:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let mut state = InputState::new();
state.set_value("hello");           // write
let text: &str = &state.value;      // read
assert_eq!(text, "hello");
# }
```

That has three consequences worth planning around. You can build a state in a test and assert
on it without a terminal. You can serialise the parts you care about and restore them on the
next run. And you can construct one in a non-default position before the first render, which
is how you restore a scroll offset or a selected row.

## Construction

Most states implement `Default`. Where a widget has an obvious initial value, there is also a
constructor that takes it:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let off = SwitchState::default();      // off
let on = SwitchState::new(true);       // on
let named = InputState::new();
let prefilled = InputState::with_value("root@localhost");
# let _ = (off, on, named, prefilled);
# }
```

Check the widget chapter for the constructor a given state offers. When in doubt,
`Default::default()` is always valid and always inert.

## Why the state must render before it can be clicked

A state learns its geometry during `render`. Mouse handling then compares the event
coordinates against the rectangles it recorded. A state that has never been rendered has a
zero-sized rectangle, so every click misses.

This matters in two situations. If you build a state and feed it mouse events before the
first frame, nothing happens, which is correct but surprising. And if you render a widget
into a different rectangle every frame (a list that scrolls, a table whose columns resize),
the hit rectangles follow automatically, with no invalidation step for you to forget.

## Rendering is a method on the builder

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, st: &mut ListViewState) {
ListView::new(vec![ListEntry::new("alpha"), ListEntry::new("beta")]).render(area, buf, st);
# }
# fn main() {}
```

The signature is `render(self, area: Rect, buf: &mut Buffer, state: &mut S)`. It takes the
builder by value, so a builder cannot be reused by accident.

Stateless widgets drop the last parameter: `Rule::new().render(area, buf)`. The
[widget index](../reference/widget-index.md) marks which is which.

Widgets also implement ratatui's own `Widget` and `StatefulWidget` traits where the signature
allows, so `frame.render_widget(..)` works if you prefer it. The inherent `render` method is
the one used throughout this book because it does not require importing the traits.

## Where the theme comes from

A builder with no `.theme(..)` call reads `theme::current()` at render time. Pass one
explicitly to override a single widget:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, st: &mut ButtonState) {
let danger = Theme::resolve(&ThemeSpec::new("danger", true, Rgb::hex(0xef4444)), None);
Button::new("Delete everything").theme(&danger).render(area, buf, st);
# }
# fn main() {}
```

The [theming chapters](../theming/colour-system.md) cover what a theme contains and how to
build one.

## A rule of thumb for your own code

If a value changes when the user interacts, it belongs in the state. If it changes when your
application logic changes, it belongs in your app struct and gets passed to the builder each
frame. Text typed into a field is the first kind. Whether that field is disabled because a
network request is in flight is the second.
