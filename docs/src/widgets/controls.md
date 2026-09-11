# Controls

Buttons, toggles and value pickers. This is the family to read first: every widget here
follows the builder-plus-state shape in its simplest form, so the patterns transfer to the
rest of the library.

![the controls page of the showcase](../screenshots/controls.png)

## Button

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = ButtonState::new();

Button::new("Deploy")
    .variant(Variant::Primary)
    .focused(true)
    .now(now)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

A button fires on `Enter`, `Space`, or a click that presses and releases inside it. All three
return `Outcome::Changed`, which is the signal to run the action. A press that slides off the
button before release returns `Consumed`, not `Changed`, so a cancelled click does nothing.

`ButtonState` holds only the hit rectangle and the instant of the last press, which drives a
120 ms flash. Pass `.now(now)` if you want that flash; without it the button still works and
simply does not animate. `state.animating(now)` reports whether the flash is still running.

| Method | Effect |
|---|---|
| `variant(Variant)` | Semantic colour: `Default`, `Primary`, `Secondary`, `Accent`, `Success`, `Warning`, `Error` |
| `style(ButtonStyle)` | `Default` (3D), `Flat`, `Outline`, `Ghost` |
| `compact(bool)` | One row instead of three |
| `icon(&str)` | A glyph before the label |
| `min_width(u16)` | Pad short labels so a row of buttons lines up |
| `full_width(bool)` | Fill the area's width |
| `enabled(bool)` | Disabled buttons ignore input and render muted |

`ButtonGroup::layout(area, n, gap)` splits an area into `n` equal button rectangles, which
saves writing the same `Layout::horizontal` every time.

Four styles exist because they carry different weight. `Default` is the raised look for the
primary action on a screen. `Flat` is the same shape without the 3D edge. `Outline` is a
border and a label, for secondary actions. `Ghost` is a label alone until hovered, for
actions that should not compete for attention.

## Checkbox

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = CheckboxState::new(CheckState::On);

Checkbox::new("Send crash reports")
    .style(CheckStyle::Check)
    .focused(true)
    .render(area, buf, &mut state);

if state.value == CheckState::On { /* ... */ }
# }
# fn main() {}
```

`state.value` is a `CheckState`, not a `bool`: `Off`, `On` or `Indeterminate`. The third
value is for a parent checkbox summarising children that disagree. A plain checkbox never
enters it on its own; call `.tri_state(true)` to let `Space` cycle through all three, or set
it yourself with `state.set_tri_state(..)`.

Six marker styles: `Pill`, `Bracket` (`[x]`), `Box` (`☑`), `Circle` (`●`), `Check` (`✓`),
`Square` (`■`). `.label_first(true)` puts the marker on the right, which suits a settings
list where the labels are the column being scanned.

## Switch

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = SwitchState::new(true);

Switch::new()
    .label("Dark mode")
    .style(SwitchStyle::Pill)
    .now(now)
    .render(area, buf, &mut state);

let on: bool = state.on;
# let _ = on;
# }
# fn main() {}
```

A switch is a boolean with a sliding thumb. `state.on` is the value; `state.anim` is the
tween driving the thumb; `state.animating(now)` tells the runtime to keep redrawing while it
slides.

Styles: `Pill`, `Slim`, `Line`, `Round`, `Text` (`ON` and `OFF`), `Check` (`✓` and `✗`).
`.duration(Duration)` changes the slide time from its default; zero makes it snap, which is
what a reduced-motion mode wants.

Use a switch when the change takes effect immediately, and a checkbox when it takes effect
on submit. That is a convention rather than a rule the library enforces, but it is the one
users read.

## RadioGroup

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = RadioState::new(Some(0));

RadioGroup::new(vec!["Small".into(), "Medium".into(), "Large".into()])
    .title("Size")
    .style(RadioStyle::Dot)
    .focused(true)
    .render(area, buf, &mut state);

let chosen: Option<usize> = state.selected;
# let _ = chosen;
# }
# fn main() {}
```

`state.selected` is the chosen index, `None` until something is picked, and `state.cursor` is
where the keyboard cursor sits.
They differ while the user is moving with the arrow keys before committing with `Space` or
`Enter`. A click sets both.

`.horizontal(true)` lays the options in a row, `.bordered(true)` draws a frame, `.title(..)`
labels it. Four marker styles: `Dot`, `Bracket` (`(•)`), `Check`, `Arrow` (`❯`).

## CheckList

A multi-select list of checkboxes with a cursor, a scroll offset and its own scrollbar:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = CheckListState::default();

CheckList::new(vec!["Logs".into(), "Metrics".into(), "Traces".into()])
    .focused(true)
    .now(now)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The checked set is sized to the option count on the first render, so a state built with
`default()` is safe to hand to a list of any length. The scrollbar appears only when the
options do not fit.

## Segmented

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SegmentedState::new(1);

Segmented::new(vec!["Day".into(), "Week".into(), "Month".into()])
    .style(SegmentedStyle::Filled)
    .focused(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

One row, one choice, all options visible. Styles: `Filled`, `Outline`, `Underline`, `Text`.
Prefer it over a `RadioGroup` when there are two to four short options and vertical space is
tight; prefer the radio group when the options need descriptions.

## Slider and RangeSlider

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = SliderState::new(42.0, 0.0, 100.0, 1.0);

Slider::new()
    .label("Volume")
    .show_value(true)
    .ticks(true)
    .now(now)
    .render(area, buf, &mut state);

let v: f32 = state.value;
# let _ = v;
# }
# fn main() {}
```

`SliderState` carries `value`, `min`, `max` and `step` as public fields, so clamping ranges
live with the value rather than in the builder. `state.set(v)` clamps and snaps to the step,
`state.adjust(delta)` nudges, and `state.value_at(x)` converts a screen column to a value,
which is what dragging uses.

Arrow keys step, `Home` and `End` jump to the ends, a click on the track jumps to that value
and a drag follows the pointer. `.format(..)` controls how the value is printed;
`.shape(FieldShape)` adds a frame, which is off by default so a slider can sit inline in a
settings row.

`RangeSlider` is the two-handle version over a `RangeState`, for a low and high bound.

## Stepper

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = StepperState::new(1, 1, 10, 1);

Stepper::new().focused(true).render(area, buf, &mut state);

state.increment();
state.decrement();
# }
# fn main() {}
```

An integer with a minus and a plus button. It keeps a separate hit rectangle for each button,
so a click lands on the right one. Use it for small bounded counts where a text field would
invite invalid input.

## Rating

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = RatingState::new(3);

Rating::new().max(5).allow_clear(true).focused(true).render(area, buf, &mut state);
# }
# fn main() {}
```

`state.value` is the committed rating and `state.hover_value` is the preview under the
pointer, so hovering the fourth star previews four without committing. `.allow_clear(true)`
lets a click on the current value reset it to zero.

Each star takes two cells. The star glyph is ambiguous-width, which means terminals disagree
about whether it is one cell or two, and the widget reserves two so the row cannot skew. Budget
`max * 2` columns.

## Choosing between them

| You need | Use |
|---|---|
| Run an action | `Button` |
| A setting that applies immediately | `Switch` |
| A setting that applies on submit | `Checkbox` |
| One of several, with room to explain each | `RadioGroup` |
| One of two to four short options, in one row | `Segmented` |
| Several of many | `CheckList` |
| A continuous value | `Slider` |
| A small bounded integer | `Stepper` |
| A one to five score | `Rating` |
