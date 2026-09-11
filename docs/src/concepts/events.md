# Events and outcomes

Feeding an event to a widget returns an `Outcome`. It has three values and they mean
different things to your app:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
let _ = Outcome::Ignored;    // the widget did nothing; offer the event to someone else
let _ = Outcome::Consumed;   // handled; a redraw is enough
let _ = Outcome::Changed;    // the value the widget owns changed
# }
```

`Consumed` covers hover moves, scrolling, a cursor blink, an arrow key that moved a list
cursor without picking anything. `Changed` is the one your application logic reacts to: a
toggle flipped, text was edited, a row was selected, a button fired.

## The Interactive trait

Every stateful widget implements `Interactive`:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(state: &mut CheckboxState, key: KeyEvent, m: MouseEvent, ev: &Event) {
state.handle_key(key);     // key events only
state.handle_mouse(m);     // mouse events only
state.handle(ev);          // either: dispatches by variant, filters key releases
# }
# fn main() {}
```

`handle` is the one to reach for in an app. It matches on the event, forwards key presses to
`handle_key` and mouse events to `handle_mouse`, and returns `Ignored` for anything else
(resize, focus, paste). Key releases are dropped, so a widget cannot fire twice on one press
under terminals that report releases.

Widgets that animate on interaction do not take the frame instant here. They stamp the event
with `Instant::now()` and compare it against the `.now(..)` handed to the next `render`.
`ButtonState::pressed_at` works that way: the press is recorded in `handle_mouse`, and the
flash only appears if the builder was given a `now` to measure against.

## Combining outcomes

`Outcome` implements `BitOr`, taking the strongest result:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn main() {
assert_eq!(Outcome::Ignored | Outcome::Consumed, Outcome::Consumed);
assert_eq!(Outcome::Consumed | Outcome::Changed, Outcome::Changed);
# }
```

That is what makes broadcasting a mouse event to a screen of widgets one expression:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# struct Page { name: InputState, email: InputState, save: ButtonState }
# impl Page {
fn on_mouse(&mut self, ev: &Event) -> Outcome {
    self.name.handle(ev) | self.email.handle(ev) | self.save.handle(ev)
}
# }
# fn main() {}
```

There is also `|=`, and the helpers `is_consumed()` (true for `Consumed` and `Changed`) and
`is_changed()` (true only for `Changed`).

## Dispatch: keys go to one widget, mouse goes to all

Keyboard events have no coordinates, so only the focused widget may act on them. Mouse events
carry coordinates, and every state knows the rectangle it drew into, so the cheapest correct
dispatch is to offer the event to everyone and let each widget test its own rectangle.

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Name, Save }
# struct Page { name: InputState, save: ButtonState, focus: Focus<Id> }
# impl Page {
fn event(&mut self, ev: &Event) -> Outcome {
    match ev {
        Event::Key(k) => {
            if self.focus.handle_key(*k).is_consumed() {
                return Outcome::Consumed;          // Tab or Shift-Tab
            }
            match self.focus.current() {
                Some(Id::Name) => self.name.handle(ev),
                Some(Id::Save) => self.save.handle(ev),
                None => Outcome::Ignored,
            }
        }
        Event::Mouse(_) => self.name.handle(ev) | self.save.handle(ev),
        _ => Outcome::Ignored,
    }
}
# }
# fn main() {}
```

The cost of broadcasting is one rectangle test per widget per event, which is not measurable
at terminal scale.

## Order matters for overlapping widgets

When two widgets overlap, the one drawn last is on top, so it must see the event first.
Broadcasting in draw order gives the wrong answer for a dropdown covering a table: both would
consume the click.

Handle overlays before the widgets beneath them, and stop when one consumes:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# struct Page { menu: ContextMenuState, table: DataTableState }
# impl Page {
fn event(&mut self, ev: &Event) -> Outcome {
    let out = self.menu.handle(ev);
    if out.is_consumed() {
        return out;                     // the menu is on top; the table never sees this
    }
    self.table.handle(ev)
}
# }
# fn main() {}
```

The [overlays](overlays.md) chapter covers the drawing half of the same problem.

## Reading what changed

`Changed` says something moved, not what. Read the state:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(dark: &mut SwitchState, ev: &Event) {
if dark.handle(ev).is_changed() {
    theme::set_by_name(if dark.on { "textual-dark" } else { "textual-light" });
}
# }
# fn main() {}
```

Widgets that fire an event rather than hold a value use a `take_*` method that returns the
pending action once and clears it: `take_activated()` on menus and lists,
`take_closed()` on a closable tab bar, `take_action()` on an action toast. Calling `take_*`
is what marks the event as handled, so a widget whose result you never take will keep
reporting it.

## Events you will not see

The runtime filters key releases before `App::event`. Everything else crossterm produces is
forwarded: resize, focus gained and lost, paste, and mouse moves when capture is on. Widgets
ignore the variants they do not care about, so passing them through costs nothing.
