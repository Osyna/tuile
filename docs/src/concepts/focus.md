# Focus

Focus belongs to your app, not to the widgets. A widget is told whether it is focused through
`.focused(bool)` and styles itself accordingly. It never decides on its own, because only the
app knows what else is on screen and what the tab order should be.

`Focus<T>` is the small helper that keeps that decision in one place.

```rust
# extern crate tuile;
# use tuile::prelude::*;
# fn main() {
#[derive(Clone, Copy, PartialEq, Debug)]
enum Id { Name, Email, Save }

let mut focus = Focus::new([Id::Name, Id::Email, Id::Save]);
assert!(focus.is(Id::Name));
focus.next();
assert!(focus.is(Id::Email));
focus.prev();
focus.prev();
assert!(focus.is(Id::Save));     // wrapped around
# }
```

`T` is any `Copy + PartialEq`. An enum is the usual choice: adding a field to the enum and
forgetting to render it is then a compile error in the match, rather than a widget you cannot
reach with Tab.

## Wiring it up

Two calls. One in `event` to move the ring, one per widget in `draw` to report the answer:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Name, Save }
# struct Page { name: InputState, save: ButtonState, focus: Focus<Id> }
# impl Page {
fn event(&mut self, ev: &Event) -> Outcome {
    if let Event::Key(k) = ev
        && self.focus.handle_key(*k).is_consumed()
    {
        return Outcome::Consumed;
    }
    match self.focus.current() {
        Some(Id::Name) => self.name.handle(ev),
        Some(Id::Save) => self.save.handle(ev),
        None => Outcome::Ignored,
    }
}

fn draw(&mut self, area: Rect, buf: &mut Buffer) {
    let [a, b] = Layout::vertical([Constraint::Length(3); 2]).areas(area);
    Input::new().focused(self.focus.is(Id::Name)).render(a, buf, &mut self.name);
    Button::new("Save").focused(self.focus.is(Id::Save)).render(b, buf, &mut self.save);
}
# }
# fn main() {}
```

`Focus::handle_key` consumes `Tab` and `Shift-Tab` (crossterm reports the second as
`BackTab`) and ignores everything else. Call it before dispatching to the focused widget, or
a text field will swallow the Tab.

## The API

| Method | Effect |
|---|---|
| `new(order)` | Build from anything that converts into a `Vec<T>`; starts at index 0 |
| `current()` | `Option<T>`, `None` only when the order is empty |
| `is(id)` | The focus test to pass to `.focused(..)` |
| `set(id)` | Jump to an id; returns `false` if it is not in the order |
| `next()` / `prev()` | Step, honouring `wrap` |
| `index()` / `set_index(i)` | Positional access; `set_index` clamps |
| `order()` | The current order as a slice |
| `set_order(order)` | Replace the order, keeping the current id if it survives |
| `wrap` | Public field, default `true`. Set `false` to stop at the ends |

## Dynamic tab orders

A collapsed section or a hidden field should not be a stop on the ring. `set_order` rebuilds
it and keeps the user where they are when possible:

```rust
# extern crate tuile;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Name, Advanced, Timeout, Save }
# fn demo(focus: &mut Focus<Id>, advanced_open: bool) {
let order: Vec<Id> = if advanced_open {
    vec![Id::Name, Id::Advanced, Id::Timeout, Id::Save]
} else {
    vec![Id::Name, Id::Advanced, Id::Save]
};
focus.set_order(order);
# }
# fn main() {}
```

If the focused id is still in the new order, focus follows it. If it disappeared (the user
collapsed the section that held it), focus falls back to index 0. Rebuilding the order every
frame is fine: it is a `Vec` of `Copy` values and a linear search.

## Clicking should move focus

Terminal users expect a click to focus what they clicked. Widgets do not do this for you,
because the widget cannot know its own id. Do it where you dispatch the mouse event:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Id { Name, Save }
# struct Page { name: InputState, save: ButtonState, focus: Focus<Id> }
# impl Page {
fn on_mouse(&mut self, ev: &Event) -> Outcome {
    let n = self.name.handle(ev);
    if n.is_consumed() {
        self.focus.set(Id::Name);
    }
    let s = self.save.handle(ev);
    if s.is_changed() {
        self.focus.set(Id::Save);
    }
    n | s
}
# }
# fn main() {}
```

Use `is_consumed()` for widgets where a press should focus (a text field: the click also
places the cursor) and `is_changed()` for widgets where only a completed action should
(a button: a press that slides off and releases elsewhere is a cancelled click).

## Focus inside a widget

Some widgets have internal cursors: the selected row of a list, the current cell of a table,
the open item of a menu. That is not app focus and `Focus<T>` knows nothing about it. The
widget owns it, moves it with the arrow keys, and only sees those keys while your app says it
is focused.

Nested focus rings work the same way: a panel with three fields can own a `Focus<FieldId>`
and be a single stop in the parent's `Focus<PanelId>`. Forward keys down only while the
parent ring points at that panel, and let the child ring stop wrapping (`wrap = false`) if
you want Tab to escape the panel at its ends.

## Blurred styling

Widgets render differently when not focused, and the difference is a theme role, not an
on-off switch. `border_blurred`, `cursor_blurred_bg` and the muted text roles are what a
blurred widget uses. A text area keeps showing its cursor when blurred, in a dimmer colour,
so a reader can still see where typing would resume. The
[theme roles](../reference/theme-roles.md) table lists the pairs.
