# Installation

```toml
[dependencies]
tuile = "0.3"
```

To track main instead of a release, depend on the repository:

```toml
[dependencies]
tuile = { git = "https://github.com/Osyna/tuile" }
```

That is the only dependency you need. tuile re-exports the crates you would otherwise pin
yourself:

```rust
# extern crate tuile;
use tuile::crossterm;         // key and mouse events, terminal control
use tuile::ratatui_core;      // Rect, Buffer, Style, Frame
use tuile::ratatui_crossterm; // the backend, if you build your own Terminal
# fn main() {}
```

tuile depends on `ratatui-core` rather than the `ratatui` facade. Upstream splits the project
exactly this way: applications use `ratatui`, widget libraries use `ratatui-core`. The types are
the same ones the facade re-exports, so tuile widgets render into a `Frame` from a `ratatui` app
without any conversion.

Pinning them yourself is allowed, but the versions must match tuile's or you will get two
incompatible `Buffer` types and a confusing type error at the `render` call. If you already
depend on ratatui, check that it resolves to 0.30.

## Requirements

| | |
|---|---|
| Rust | 1.88 or newer, edition 2024 |
| ratatui | 0.30 |
| Terminal | anything crossterm supports; true colour is used when available |

## The prelude

Every example in this book starts the same way:

```rust
# extern crate tuile;
use tuile::prelude::*;
# fn main() {}
```

The prelude brings in every widget and its state type, the theme API, the drawing and layout
helpers, the `core` vocabulary (`Outcome`, `Focus`, `HitBox`, `Look`), the runtime (`App`,
`Flow`, `run`), and the ratatui types you cannot avoid touching: `Rect`, `Buffer`, `Frame`,
`Line`, `Span`, `Text`, `Style`, `Modifier`, `Constraint`, `Layout`, `Direction`,
`Alignment`, `Position`, `Margin`, and the crossterm event types.

If you prefer explicit imports, everything is reachable through its module:
`tuile::widgets::Button`, `tuile::theme::Theme`, `tuile::draw::Border`, `tuile::layout::center`,
`tuile::core::Outcome`, `tuile::anim::Tween`, `tuile::runtime::run`.

## Check it works

```rust,no_run
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

struct Hello;

impl App for Hello {
    fn draw(&mut self, frame: &mut Frame, _now: Instant) {
        let area = center(frame.area(), 30, 3);
        let buf = frame.buffer_mut();
        let mut state = ButtonState::default();
        Button::new("It works").variant(Variant::Success).render(area, buf, &mut state);
    }

    fn event(&mut self, ev: Event, _now: Instant) -> Flow {
        match ev {
            Event::Key(k) if k.code == KeyCode::Esc => Flow::Quit,
            _ => Flow::Continue,
        }
    }
}

fn main() -> std::io::Result<()> {
    run(&mut Hello)
}
```

`cargo run` should paint a green button in the middle of a cleared terminal, and `Esc` should
put your shell back exactly as it was. The runtime restores the terminal on panic too, so a
crash in your draw code does not leave you typing into a raw-mode shell.

## Examples in the repository

```text
cargo run -p tuile --example minimal    # a switch, an input and a button
cargo run -p tuile --example custom     # a custom theme, a custom spinner, chat widgets
```

Both are short enough to read in one sitting and are compiled by CI, so they always match the
current API.
