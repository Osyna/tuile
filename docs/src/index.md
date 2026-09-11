# tuile

tuile is a widget library and design system for [ratatui](https://ratatui.rs). It gives you
108 widget types, a colour system that expands ten colours into thirty semantic roles, tweened
animation, mouse handling with hit rectangles, overlays, and a 22-page gallery app that
exercises all of it.

![the dashboard page of the showcase app](screenshots/dashboard.png)

ratatui hands you a terminal buffer and a small set of drawing widgets. Everything above that
line, a focus ring, hover states, scroll offsets, dropdowns that render on top, dialogs,
toasts, a palette that works in both light and dark terminals, animation that does not burn a
core while idle, is left to your app. Most TUI projects build that layer once, badly, and then
live with it. tuile is that layer, ported from [Textual](https://textual.textualize.io) and
implemented in plain ratatui with no runtime and no CSS engine.

## What using it looks like

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut dark = SwitchState::new(true);

Switch::new()
    .label("Dark mode")
    .focused(true)
    .now(now)
    .render(area, buf, &mut dark);
# }
# fn main() {}
```

Three ideas carry the whole library, and the [core concepts](concepts/builders-and-state.md)
chapters cover them in order:

1. A builder configures one frame. A state lives between frames and owns the value, the hover
   flag, the scroll offset and any running tween.
2. Events go into a state and come back as an [`Outcome`](concepts/events.md): `Ignored`,
   `Consumed` or `Changed`.
3. Focus belongs to your app. Hover belongs to the widget.

Once you know one widget you know the shape of all of them, so the widget chapters are
reference material you can read out of order.

## Where to start

| If you want to | Read |
|---|---|
| Add tuile to a project | [Installation](start/install.md) |
| Build something end to end | [Your first app](start/first-app.md) |
| See every widget running | [Running the showcase](start/showcase.md) |
| Understand the model | [Builders and state](concepts/builders-and-state.md) |
| Change the colours | [The colour system](theming/colour-system.md) |
| Find the widget you need | [Widget index](reference/widget-index.md) |
| Write your own widget | [Writing your own widget](recipes/custom-widget.md) |

## Scope, and what this is not

tuile draws. It does not manage your application state, own your data, or talk to a network.
There is no reactive graph, no style sheet, no macro DSL. A widget is a struct you configure
and hand a buffer, which means you can drop a single `DataTable` into an existing ratatui app
without adopting anything else.

Two dependencies beyond ratatui: `unicode-width` and `unicode-segmentation`. Both are used for
the same reason, terminal cells are not bytes and not chars, and getting that wrong is how a
TUI ends up with a skewed row.

The design language is Textual's. The code is not: nothing here is a binding, a port of their
Python, or affiliated with Textualize.

## Versioning

Pre-1.0. The widget contract (builder plus state, `Interactive`, cached rects) is stable and
every widget follows it, but individual builder methods can still be renamed between 0.x
releases. The [FAQ](reference/faq.md) says what that means in practice.
