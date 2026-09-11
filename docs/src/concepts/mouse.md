# Mouse and hit boxes

A mouse event carries a column and a row. Turning that into "the user clicked the Save
button" needs a rectangle to compare against, and the only code that knows where Save landed
is the code that drew it. So widgets record their geometry during `render` and use it during
`handle_mouse`.

You do not have to do anything for this to work. It matters when you write your own widget,
and it explains two behaviours that otherwise look like bugs.

## HitBox

`HitBox` is the tracker a widget stores for one rectangle:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn main() {
let mut hit = HitBox::default();
hit.set_area(Rect::new(10, 5, 20, 3));    // called during render
# }
```

It holds three public fields: `area`, `hover` and `pressed`. Feeding it a mouse event
returns what it saw:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(hit: &mut HitBox, m: MouseEvent) -> Outcome {
match hit.mouse(&m) {
    Hit::Click => Outcome::Changed,          // pressed and released inside
    Hit::Press => Outcome::Consumed,         // pressed inside, not released yet
    Hit::Drag => Outcome::Consumed,          // moved while pressed here
    Hit::Cancel => Outcome::Consumed,        // released outside after pressing inside
    Hit::HoverChanged => Outcome::Consumed,  // pointer entered or left
    Hit::Wheel(delta) => { let _ = delta; Outcome::Consumed }
    Hit::None => Outcome::Ignored,
}
# }
# fn main() {}
```

The press-then-release-inside rule is what makes a click cancellable. Pressing a button and
sliding off before releasing gives `Cancel`, not `Click`, which is the behaviour users expect
from every other interface they use.

`hit.look(focused, enabled)` combines the tracked hover and press flags with the focus flag
your app owns, giving the `Look` that widget styling reads.

## Widgets with many rectangles

A list, a table or a tab bar needs one rectangle per row, cell or tab. Those widgets keep a
`Vec<Rect>` (often paired with the index it maps to) and rebuild it every render:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct MyListState { hits: Vec<Rect>, cursor: usize }
# impl MyListState {
fn render_rows(&mut self, area: Rect, rows: &[&str]) {
    self.hits.clear();
    for (i, _row) in rows.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.bottom() {
            break;                    // only what is visible gets a rectangle
        }
        self.hits.push(Rect::new(area.x, y, area.width, 1));
    }
}

fn on_click(&mut self, m: &MouseEvent) -> Outcome {
    for (i, r) in self.hits.iter().enumerate() {
        if mouse_in(*r, m) && is_left_down(m) {
            self.cursor = i;
            return Outcome::Changed;
        }
    }
    Outcome::Ignored
}
# }
# fn main() {}
```

Clearing and rebuilding the list each frame is deliberate. Scrolled-away rows lose their
rectangles, so a click cannot select a row that is not on screen, and there is no cache to
invalidate.

## The two surprising behaviours

**A widget that has never rendered ignores the mouse.** Its rectangle is still `Rect::ZERO`.
This shows up in tests that feed events to a fresh state, and in a widget hidden behind a tab
that was never drawn. Render once, then click.

**A widget hidden behind an overlay still gets the click** unless you stop dispatching. The
broadcast pattern offers the event to everyone, and a table under a dropdown does not know
the dropdown is there. Handle overlays first and return early when they consume the event;
the [events](events.md) chapter shows the shape.

## The helpers

These are free functions in `core`, re-exported by the prelude:

| Function | Returns |
|---|---|
| `mouse_pos(m)` | The event's `Position` |
| `mouse_in(area, m)` | Whether the event is inside `area` |
| `is_left_down(m)` / `is_left_up(m)` / `is_left_drag(m)` | Left button press, release, drag |
| `is_move(m)` | A bare pointer move |
| `wheel_delta(m)` | `Some(1)` for wheel down, `Some(-1)` for wheel up, `None` otherwise |

Wheel events are not routed by focus. The widget under the pointer scrolls, which is why
scrollable widgets test `mouse_in` before acting on a wheel delta.

## Mouse capture and its cost

`run` enables mouse capture by default. While it is on, the terminal sends events to your app
instead of handling them itself, so the user loses click-to-select-text and middle-click
paste inside your app. Most users know to hold Shift to get the terminal's own selection
back, but if your app is mostly text and rarely interactive, consider turning capture off:

```rust,no_run
# extern crate tuile;
# use tuile::prelude::*;
# struct MyApp;
# impl App for MyApp {
#     fn draw(&mut self, _f: &mut Frame, _n: Instant) {}
#     fn event(&mut self, _e: Event, _n: Instant) -> Flow { Flow::Continue }
# }
# fn main() -> std::io::Result<()> {
run_with(&mut MyApp, RunOptions { mouse: false, ..Default::default() })
# }
```

With capture off, every widget still works from the keyboard. Nothing in the library requires
a mouse.

## Terminal differences

Drag events need a terminal that reports motion while a button is held. Most modern ones do
(kitty, foot, WezTerm, Alacritty, iTerm2, Windows Terminal). Where drag is missing, the
widgets that use it degrade rather than break: a split pane can still be resized from the
keyboard, and a slider still responds to a plain click on the track.
