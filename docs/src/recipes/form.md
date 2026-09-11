# A settings form

A form is a focus ring, a column of widgets, and a submit path. This recipe builds one with
validation, a dirty flag and a confirmation dialog. The finished version of this pattern is
the Settings page of the showcase, in about 300 lines.

![the settings page](../screenshots/settings.png)

## The shape

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
#[derive(Clone, Copy, PartialEq)]
enum Field { Name, Email, Notify, Retention, Save }

struct Settings {
    name: InputState,
    email: InputState,
    notify: SwitchState,
    retention: SliderState,
    save: ButtonState,
    focus: Focus<Field>,
    dirty: bool,
}

impl Settings {
    fn new() -> Self {
        Settings {
            name: InputState::with_value("ada"),
            email: InputState::with_value("ada@example.com"),
            notify: SwitchState::new(true),
            retention: SliderState::new(30.0, 1.0, 365.0, 1.0),
            save: ButtonState::new(),
            focus: Focus::new([Field::Name, Field::Email, Field::Notify, Field::Retention, Field::Save]),
            dirty: false,
        }
    }
}
# fn main() { let _ = Settings::new(); }
```

Load the current values into the states at construction. The states are the form: there is no
separate model to keep in sync, and reading the form back is reading the states.

## Laying it out

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Field { Name, Email, Notify, Retention, Save }
# struct Settings { name: InputState, email: InputState, notify: SwitchState, retention: SliderState, save: ButtonState, focus: Focus<Field>, dirty: bool }
# impl Settings {
fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
    let th = theme::current();
    let card = center(area, 56, 18);
    let inner = Border::Round.draw_titled(
        buf, card, th.border_blurred, th.background, "Settings", Alignment::Left,
    );

    let rows = stack(pad(inner, 2, 1), &[3, 3, 1, 3, 1], 1);

    // Input has no label of its own: draw one and give the field the rest of the row.
    let label = st(th.text_muted, th.background);
    let field = |r: Rect| Rect { x: r.x + 10, width: r.width.saturating_sub(10), ..r };

    put(buf, rows[0].x, rows[0].y, "Name", 10, label);
    Input::new()
        .placeholder("your name")
        .focused(self.focus.is(Field::Name))
        .now(now)
        .render(field(rows[0]), buf, &mut self.name);

    put(buf, rows[1].x, rows[1].y, "Email", 10, label);
    Input::new()
        .validator(|v| if v.contains('@') { Ok(()) } else { Err("must contain @".into()) })
        .focused(self.focus.is(Field::Email))
        .now(now)
        .render(field(rows[1]), buf, &mut self.email);

    Switch::new()
        .label("Email notifications")
        .focused(self.focus.is(Field::Notify))
        .now(now)
        .render(rows[2], buf, &mut self.notify);

    Slider::new()
        .label("Retention")
        .show_value(true)
        .format(|v| format!("{v:.0} days"))
        .focused(self.focus.is(Field::Retention))
        .now(now)
        .render(rows[3], buf, &mut self.retention);

    Button::new(if self.dirty { "Save changes" } else { "Saved" })
        .variant(if self.dirty { Variant::Primary } else { Variant::Default })
        .enabled(self.dirty)
        .focused(self.focus.is(Field::Save))
        .now(now)
        .render(Rect { width: 18, ..rows[4] }, buf, &mut self.save);
}
# }
# fn main() {}
```

`stack(area, &heights, gap)` is the layout helper for a column of fixed-height rows with a
gap. It saves building a `Layout` with one `Constraint` per row, and the heights read as a
list of row sizes.

The save button carries the dirty state in three places at once: its label, its variant and
its enabled flag. Deriving all three from one bool is what keeps them consistent.

## Events and the dirty flag

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# #[derive(Clone, Copy, PartialEq)] enum Field { Name, Email, Notify, Retention, Save }
# struct Settings { name: InputState, email: InputState, notify: SwitchState, retention: SliderState, save: ButtonState, focus: Focus<Field>, dirty: bool }
# impl Settings {
fn event(&mut self, ev: &Event) -> Outcome {
    if let Event::Key(k) = ev
        && self.focus.handle_key(*k).is_consumed()
    {
        return Outcome::Consumed;
    }

    let out = match ev {
        Event::Key(_) => match self.focus.current() {
            Some(Field::Name) => self.name.handle(ev),
            Some(Field::Email) => self.email.handle(ev),
            Some(Field::Notify) => self.notify.handle(ev),
            Some(Field::Retention) => self.retention.handle(ev),
            Some(Field::Save) => self.save.handle(ev),
            None => Outcome::Ignored,
        },
        Event::Mouse(_) => {
            self.name.handle(ev)
                | self.email.handle(ev)
                | self.notify.handle(ev)
                | self.retention.handle(ev)
                | self.save.handle(ev)
        }
        _ => Outcome::Ignored,
    };

    if out.is_changed() {
        if self.focus.is(Field::Save) {
            self.submit();
        } else {
            self.dirty = true;
        }
    }
    out
}

fn submit(&mut self) {
    // read the states; they are the form
    let _name = self.name.value.clone();
    let _email = self.email.value.clone();
    let _notify = self.notify.on;
    let _days = self.retention.value as u32;
    self.dirty = false;
}
# }
# fn main() {}
```

One `Changed` handler covers every field. Which widget changed comes from the focus ring, so
adding a field means adding an enum variant and a render call, not another branch here.

## Validation

`Input::validator` takes a function from the current text to `Result<(), String>`. The widget
runs it on every edit, stores the error in `state.error` and renders the field in the error
colour with the message underneath.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, state: &mut InputState) {
Input::new()
    .validator(|v| {
        if v.is_empty() {
            Err("required".into())
        } else if !v.contains('@') {
            Err("must contain @".into())
        } else {
            Ok(())
        }
    })
    .render(area, buf, state);
# }
# fn main() {}
```

It is a `fn` pointer, not a closure that captures, so the rule cannot depend on another
field's value. For cross-field rules (a confirmation password, an end date after a start
date) check in your submit path and surface the result yourself, for example with an
`InlineAlert` above the button.

Gate submission on validity:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct Settings { name: InputState, email: InputState }
# impl Settings {
fn valid(&self) -> bool {
    self.name.error.is_none() && self.email.error.is_none()
}
# }
# fn main() {}
```

## Confirming a destructive save

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct Page { confirm: ModalState }
# impl Page {
fn draw_confirm(&mut self, area: Rect, buf: &mut Buffer) {
    Modal::confirm("Apply changes?", "This restarts the agent.")
        .render(area, buf, &mut self.confirm);
}
# }
# fn main() {}
```

Render the modal after the form so it paints on top, and give it the event before the form so
the form does not react while it is open. The [overlays](../concepts/overlays.md) chapter
covers both halves.

## What to reach for

| Field type | Widget |
|---|---|
| Free text | `Input` |
| Long text | `TextArea` |
| One of a few | `RadioGroup` or `Segmented` |
| One of many | `Select` or `Combobox` |
| Several of many | `MultiSelect` or `CheckList` |
| On or off, applied immediately | `Switch` |
| On or off, applied on submit | `Checkbox` |
| A bounded number | `Slider` or `Stepper` |
| A date | `DatePicker` |

For a settings screen with sections and dozens of rows, `OptionList` is a better fit than a
hand-built form: it renders `label  value` rows, cycles bool, choice and int values in place,
and exposes a group index a sidebar can follow. The Options page of the showcase is that
pattern.
