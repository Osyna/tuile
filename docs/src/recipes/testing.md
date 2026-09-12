# Testing a terminal UI

A tuile widget renders into a `Buffer`, which is a plain grid of cells you can construct in a
unit test. No terminal, no PTY, no snapshot framework. That covers most of what is worth
testing; the rest is covered by driving the real binary headlessly and looking at a PNG.

## Rendering into a buffer

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
let mut state = CheckboxState::new(CheckState::On);

Checkbox::new("Enabled").render(buf.area, &mut buf, &mut state);

let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
assert!(row.contains("Enabled"));
# }
```

`Buffer::empty(rect)` is the whole setup. Reading cells back gives you the symbol, the
foreground, the background and the modifiers, so you can assert on colour as well as text.

## Assert what a consumer observes

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let mut state = CheckboxState::new(CheckState::Off);

let out = state.handle_key(KeyEvent::from(KeyCode::Char(' ')));

assert_eq!(out, Outcome::Changed);
assert_eq!(state.value, CheckState::On);

// A key the widget does not use must stay available to the app.
assert_eq!(state.handle_key(KeyEvent::from(KeyCode::Tab)), Outcome::Ignored);
# }
```

The second assertion is the one that catches real bugs. A widget that consumes `Tab` breaks
the focus ring of every app that embeds it, and nothing else in the test suite notices.

Useful things to assert: the value after an event, the `Outcome`, which cell got which
colour, how many hit rectangles a render produced, and what happens at a size too small to
draw in. Things not worth asserting: that a builder field reached the state, that a default
was applied, that rendering "does not panic" for a widget whose contract says more than that.

## Mouse events in tests

A state must render before it can be clicked, because that is when it learns its geometry:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
let mut state = ButtonState::new();

Button::new("Save").render(buf.area, &mut buf, &mut state);

let press = MouseEvent {
    kind: MouseEventKind::Down(MouseButton::Left),
    column: 3,
    row: 1,
    modifiers: KeyModifiers::NONE,
};
let release = MouseEvent { kind: MouseEventKind::Up(MouseButton::Left), ..press };

assert_eq!(state.handle_mouse(press), Outcome::Consumed);
assert_eq!(state.handle_mouse(release), Outcome::Submitted);
# }
```

Press then release, because a click is both. A test that only presses is testing half the
interaction.

## Animated widgets

Pin the clock with `.elapsed(secs)` instead of `.now(instant)`:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));

Spinner::new(&spinners::DOTS).elapsed(0.0).render(buf.area, &mut buf);
let first = buf[(0, 0)].symbol().to_string();

let mut buf2 = Buffer::empty(Rect::new(0, 0, 10, 1));
Spinner::new(&spinners::DOTS).elapsed(0.5).render(buf2.area, &mut buf2);
let later = buf2[(0, 0)].symbol().to_string();

assert_ne!(first, later, "the spinner advanced between 0.0s and 0.5s");
# }
```

With `elapsed` the same input always produces the same cells, so these tests do not flake.
With `now` they would depend on when the suite happened to run.

For tweens, construct the state, call `go(target, now, dur)` with a fixed `now`, and sample
`value(now + d)` at chosen offsets. `Instant` arithmetic is exact and no sleeping is needed.

## Screenshots of the real binary

Unit tests cannot tell you that two panels overlap or that a border is one cell short.
`cargo xtask shot` runs the actual binary in a private tmux server at an exact size, sends
input, and rasterises the result:

```text
cargo xtask shot -s 130x42 -o out.png -- ./target/release/showcase --page charts
cargo xtask shot -s 90x28 -k "Tab Tab Enter" -o out.png -- ./target/release/showcase --page inputs
cargo xtask shot -s 60x16 --text -- ./target/release/showcase --page tables
```

| Flag | Effect |
|---|---|
| `-s COLSxROWS` | Terminal size, default 120x34 |
| `-k "Tab Enter"` | tmux key names sent in order (`C-p`, `M-1`, `BTab`, `Escape`) |
| `--mouse "click 40 12"` | SGR mouse steps: `click`, `down`, `up`, `drag`, `move`, `wheelup`, `wheeldown`, with 0-based coordinates |
| `--script 'Tab;shot a.png;Enter;wait 0.5;shot b.png'` | A sequence with several captures |
| `--text` | Print the captured text instead of writing a PNG |
| `-o PATH` | Where the PNG goes |

`--text` is the mode to use in CI: the output is plain text, so a diff against a committed
file is readable in a pull request, unlike a PNG diff.

Two things the script handles that a hand-rolled tmux capture will not. Agent and CI shells
often export `NO_COLOR=1` or `TERM=dumb`, which makes crossterm strip every colour; the
script scrubs both. And `Escape` is always sent alone with a pause, because crossterm reads
`ESC` immediately followed by another key as `Alt+key`.

## Checking small sizes

Every widget in this library is expected to render without panicking from 60x16 up to
250x70. The cheap way to check your own screens is a loop over sizes with random input:

```text
for size in 60x16 90x28 130x42 200x55; do
  cargo xtask shot -s $size --text -- ./target/release/myapp > /dev/null || echo "FAILED at $size"
done
```

The repository fuzzes every showcase page this way with random keys and mouse events. It is
how the library's no-panic claim is checked, and it has found real bugs that no unit test
would have.
