# Your first app

This chapter builds a small settings card: a switch, a text field, a button, and a toast when
you save. It is the `minimal` example in the repository, taken apart. Run the finished
version with `cargo run -p tuile --example minimal`.

We build it in five steps, and each step compiles.

## 1. The App trait

An app is a struct that implements four methods, two of which have defaults:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# struct Demo;
impl App for Demo {
    fn draw(&mut self, frame: &mut Frame, now: Instant) {
        # let _ = (frame, now);
        // paint one frame
    }

    fn event(&mut self, ev: Event, now: Instant) -> Flow {
        # let _ = (ev, now);
        Flow::Continue
    }

    fn update(&mut self, _now: Instant) {}          // optional: timers, tweens, async results

    fn animating(&self, _now: Instant) -> bool {    // optional: true while something moves
        false
    }
}
# fn main() {}
```

`draw` gets a ratatui `Frame` and the `Instant` for this frame. Every animated widget takes
that same instant, which is what keeps a screen full of spinners in phase.

`event` gets one crossterm event. Key releases are filtered out before you see them, so you
do not have to check `KeyEventKind`. Return `Flow::Quit` to exit the loop.

`animating` is the frame-rate switch. Return `true` while a tween or spinner is live and the
runtime redraws at 60 fps; return `false` and it goes back to blocking on input. Get this
wrong in the lazy direction and your animations stutter; get it wrong in the eager direction
and you spin a core on an idle screen.

## 2. State lives in your struct

Widgets are rebuilt every frame, so anything that must survive a frame lives in your app:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
#[derive(Clone, Copy, PartialEq)]
enum Id { Dark, Name, Save }

struct Demo {
    dark: SwitchState,
    name: InputState,
    save: ButtonState,
    focus: Focus<Id>,
    toasts: Toaster,
}
# fn main() {}
```

`Focus<Id>` is the tab ring. Any `Copy + PartialEq` type works as an id; a small enum is the
usual choice because the compiler then catches a widget you forgot to wire up.

The states own more than the value. `InputState` holds the cursor, the selection, the scroll
offset and the undo stack. `SwitchState` holds the on flag and the tween that slides the
thumb. You can read and write their public fields directly.

## 3. Drawing

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Dark, Name, Save }
# struct Demo { dark: SwitchState, name: InputState, save: ButtonState, focus: Focus<Id>, toasts: Toaster }
# impl Demo {
fn draw(&mut self, frame: &mut Frame, now: Instant) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    let th = theme::current();
    fill(buf, area, th.background);

    let card = center(area, 46, 13);
    let inner = Border::Round.draw_titled(
        buf, card, th.border_blurred, th.background, "tuile", Alignment::Left,
    );
    let [a, b, c] = Layout::vertical([Constraint::Length(3); 3]).areas(pad(inner, 1, 0));

    Switch::new()
        .label("Dark mode")
        .focused(self.focus.is(Id::Dark))
        .now(now)
        .render(a, buf, &mut self.dark);
    Input::new()
        .placeholder("Your name")
        .focused(self.focus.is(Id::Name))
        .now(now)
        .render(b, buf, &mut self.name);
    Button::new("Save")
        .variant(Variant::Primary)
        .focused(self.focus.is(Id::Save))
        .now(now)
        .render(Rect { width: 16, ..c }, buf, &mut self.save);

    ToastStack::new().now(now).render(area, buf, &mut self.toasts);
}
# }
# fn main() {}
```

Four things in that block are worth naming.

`theme::current()` returns the process-wide theme by value. `Theme` is `Copy`, so holding it
in a local for the frame costs nothing.

`Border::draw_titled` draws the frame and returns the rectangle inside it. Every border helper
returns its inner area, so layout code reads top to bottom without a separate `inner()` call.

`.focused(..)` is passed in by you. Widgets never decide their own focus, because only your
app knows what the Tab order is.

The toast stack renders over the whole area, last. Overlays are just widgets drawn after
everything else; the [overlays](../concepts/overlays.md) chapter covers the cases where that
is not enough.

## 4. Events

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Dark, Name, Save }
# struct Demo { dark: SwitchState, name: InputState, save: ButtonState, focus: Focus<Id>, toasts: Toaster }
# impl Demo {
fn event(&mut self, ev: Event, _now: Instant) -> Flow {
    if let Event::Key(k) = &ev {
        if ctrl(k, 'c') {
            return Flow::Quit;
        }
        if self.focus.handle_key(*k).is_consumed() {   // Tab and Shift-Tab
            return Flow::Continue;
        }
    }

    let out = match (&ev, self.focus.current()) {
        (Event::Key(_), Some(Id::Dark)) => self.dark.handle(&ev),
        (Event::Key(_), Some(Id::Name)) => self.name.handle(&ev),
        (Event::Key(_), Some(Id::Save)) => self.save.handle(&ev),
        // Mouse events go to every widget: each one checks the rect it drew.
        (Event::Mouse(_), _) => self.dark.handle(&ev) | self.name.handle(&ev) | self.save.handle(&ev),
        _ => Outcome::Ignored,
    };

    if out.is_changed() && self.focus.is(Id::Save) {
        self.toasts.success(format!("Saved {:?}", self.name.value));
    }
    Flow::Continue
}
# }
# fn main() {}
```

Keys go to the focused widget only. Mouse events go to all of them, because a click carries
its own coordinates and each state knows the rectangle it last drew into. `Outcome` implements
`BitOr`, so combining the results of several widgets is one expression.

`Changed` means the value moved: the switch flipped, the text changed, the button fired. That
is the signal to act on. `Consumed` means the widget handled the event and wants a redraw but
nothing you care about changed, which is what you get from hover moves and cursor blinks.

## 5. Wiring the loop

```rust,no_run
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Dark, Name, Save }
# struct Demo { dark: SwitchState, name: InputState, save: ButtonState, focus: Focus<Id>, toasts: Toaster }
# impl App for Demo {
#     fn draw(&mut self, _f: &mut Frame, _n: Instant) {}
#     fn event(&mut self, _e: Event, _n: Instant) -> Flow { Flow::Continue }
#     fn update(&mut self, now: Instant) { self.toasts.tick(now); }
#     fn animating(&self, now: Instant) -> bool {
#         self.dark.animating(now) || self.save.animating(now) || self.toasts.animating(now)
#     }
# }
fn main() -> std::io::Result<()> {
    theme::set_by_name("textual-dark");
    run(&mut Demo {
        dark: SwitchState::new(true),
        name: InputState::new(),
        save: ButtonState::default(),
        focus: Focus::new([Id::Dark, Id::Name, Id::Save]),
        toasts: Toaster::new(),
    })
}
# fn main2() {}
```

`run` enters the alternate screen, turns on raw mode and mouse capture, installs a panic hook
that restores the terminal, and loops. `run_with(app, RunOptions { .. })` is the same thing
with the knobs exposed:

| Field | Default | Effect |
|---|---|---|
| `mouse` | `true` | Enable mouse capture. Turn it off if you want the terminal's own text selection. |
| `fps` | `60` | Redraw rate while `animating()` is true. |
| `idle_redraw` | `Some(250ms)` | Redraw interval when idle, for clocks and blinking cursors. `None` blocks until the next event. |

The `update(now)` method is where per-frame bookkeeping goes. Here it drives toast timeouts.
It runs before `draw` on every iteration, including idle redraws.

## What to read next

The [core concepts](../concepts/builders-and-state.md) chapters expand each of the five steps.
If you would rather learn by reading working code, the
[showcase](showcase.md) has a page per widget family and the source for each page is a single
self-contained file.
