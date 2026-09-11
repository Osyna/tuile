# Text entry

Single-line input, multi-line textarea, dropdown select, filterable combobox, and multi-select with checkboxes.

![inputs](../screenshots/inputs.png)

## Input

A single-line text field with cursor, horizontal scrolling, selection, history, and optional validation or autocomplete.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = InputState::new();
Input::new()
    .placeholder("Name")
    .focused(true)
    .render(area, buf, &mut state);

if state.take_submitted() {
    let value = state.value();
}
# }
# fn main() {}
```

### State

`InputState` owns:

| Field | Type | Purpose |
|-------|------|---------|
| `value` | `String` | Current text content |
| `cursor` | `usize` | Grapheme index of the cursor |
| `scroll` | `usize` | Horizontal scroll offset |
| `selection` | `Option<(usize, usize)>` | Selection anchor and cursor (if active) |
| `submitted` | `bool` | True when Enter was pressed; read via `take_submitted()` |
| `error` | `Option<String>` | Validation error message (if validator is set) |

Create state with `InputState::new()` or `InputState::with_value("text")`. Read the value with `state.value()`, set it with `state.set_value("text")`, or select all with `state.select_all()`.

### Keys

| Key | Action |
|-----|--------|
| `Left` / `Right` | Move cursor one grapheme |
| `Ctrl-Left` / `Ctrl-Right` | Move to word boundary |
| `Home` / `End` | Jump to line start or end |
| `Ctrl-A` | Select all |
| `Shift-Left` / `Shift-Right` | Extend selection |
| `Backspace` | Delete character before cursor (or selection) |
| `Delete` | Delete character at cursor |
| `Ctrl-Backspace` | Delete word before cursor |
| `Ctrl-U` | Delete from cursor to start |
| `Ctrl-K` | Delete from cursor to end |
| `Ctrl-W` | Delete word before cursor |
| `Ctrl-X` | Cut selection to clipboard |
| `Ctrl-C` | Copy selection to clipboard |
| `Ctrl-V` | Paste from clipboard |
| `Ctrl-Z` | Undo (restores previous history entry) |
| `Tab` | Accept autocomplete suggestion (if suggester is set) |
| `Enter` | Submit (sets `submitted` flag) |
| `Right` at end | Accept suggestion if cursor is at end of line |

### Mouse

Click positions the cursor. Drag selects text. The widget tracks double-click timing but full text selection on double-click depends on the render implementation.

### Options

| Method | Effect |
|--------|--------|
| `placeholder(s)` | Placeholder text shown when empty |
| `password(true)` | Mask input with dots |
| `max_len(n)` | Limit input to `n` graphemes |
| `restrict(r)` | Filter input: `Digits`, `Integer`, `Number`, `Alpha`, `Alnum`, `Custom`, or `None` |
| `validator(fn)` | Validation function `fn(&str) -> Result<(), String>`; errors populate `state.error` |
| `suggester(fn)` | Autocomplete function `fn(&str) -> Option<String>` returning suffix to append |
| `prefix(s)` | Static prefix (e.g. `$` for currency) |
| `suffix(s)` | Static suffix (e.g. `ms` for time) |
| `compact(true)` | Single-line compact mode without tall frame |
| `shape(s)` | Frame shape (see FieldShape section) |
| `tab_accepts(true)` | Let Tab accept suggestions (default: Tab is navigation) |
| `show_error(true)` | Display validation error below the field |
| `select_on_focus(true)` | Select all text when focused |
| `align(a)` | Text alignment: `Left`, `Center`, `Right` |

The `restrict` option filters typed characters. `Digits` allows only `0-9`. `Integer` allows digits and leading `-`. `Number` allows digits, `.`, `-`, `+`, `e`, `E`. `Alpha` allows letters. `Alnum` allows letters and digits. `Custom` uses a custom filter function passed to `state.custom_filter`.

## TextArea

A multi-line text editor with line numbers, syntax highlighting hook, undo/redo, and scrolling.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = TextAreaState::new();
TextArea::new()
    .line_numbers(true)
    .highlight_line(true)
    .focused(true)
    .render(area, buf, &mut state);

let text = state.text();
# }
# fn main() {}
```

### State

`TextAreaState` owns:

| Field | Type | Purpose |
|-------|------|---------|
| `lines` | `Vec<String>` | Lines of text content |
| `cursor` | `(usize, usize)` | Row and column (grapheme indices) |
| `scroll_y` | `usize` | Vertical scroll offset |
| `scroll_x` | `usize` | Horizontal scroll offset |
| `selection` | `Option<(usize, usize, usize, usize)>` | Selection rectangle (row0, col0, row1, col1) |
| `vscroll_state` / `hscroll_state` | `ScrollbarState` | Scrollbar positions |

Create state with `TextAreaState::new()` or `TextAreaState::with_text("multi\nline")`. Read all text with `state.text()`, set it with `state.set_text("text")`, or insert at cursor with `state.insert_str("text")`. The undo and redo stacks store up to 100 snapshots each.

### Keys

| Key | Action |
|-----|--------|
| `Up` / `Down` | Move cursor one line |
| `Left` / `Right` | Move cursor one grapheme (wraps at line boundaries) |
| `Ctrl-Left` / `Ctrl-Right` | Move to word boundary |
| `Home` / `End` | Jump to line start or end |
| `Ctrl-Home` / `Ctrl-End` | Jump to document start or end |
| `PageUp` / `PageDown` | Move 10 lines |
| `Ctrl-A` | Select all |
| `Ctrl-Z` | Undo |
| `Ctrl-Y` | Redo |
| `Ctrl-D` | Duplicate current line |
| `Alt-Up` / `Alt-Down` | Move current line up or down |
| `Ctrl-X` | Cut selection to clipboard |
| `Ctrl-C` | Copy selection to clipboard |
| `Ctrl-V` | Paste from clipboard |
| `Backspace` | Delete character before cursor (or selection) |
| `Delete` | Delete character at cursor |
| `Enter` | Insert newline (with auto-indent) |
| `Tab` | Insert 4 spaces |

### Mouse

Click positions the cursor. Drag selects a rectangular region. Wheel scrolls vertically.

### Options

| Method | Effect |
|--------|--------|
| `line_numbers(true)` | Show line numbers in left gutter |
| `highlight_line(true)` | Highlight the current line |
| `read_only(true)` | Disable editing |
| `tab_size(n)` | Tab width in spaces (default 4) |
| `max_lines(n)` | Limit to `n` lines |
| `placeholder(s)` | Placeholder text shown when empty |
| `shape(s)` | Frame shape (see FieldShape section) |
| `highlighter(fn)` | Syntax highlighting function `fn(&str) -> Vec<(usize, usize, Style)>` returning `(start, end, style)` spans for a line |
| `cursor(style)` | Cursor style: `Block`, `Bar`, `Underline`, or `Outline` |
| `cursor_blink(true)` | Enable cursor blinking |
| `cursor_when_unfocused(true)` | Show cursor even when not focused |
| `show_position(true)` | Display `Ln X, Col Y` position indicator |

The `highlighter` hook is called once per visible line with the line text. Return a list of `(start_byte, end_byte, Style)` tuples. The showcase page includes a simple Rust highlighter that matches keywords and strings.

### Cursor styles

`CursorStyle` controls how the cursor is drawn:

| Variant | Appearance |
|---------|------------|
| `Block` | Inverse cell (default) |
| `Bar` | Thin `▏` bar in cursor color |
| `Underline` | Glyph underlined, bold, accent foreground |
| `Outline` | Cell background set to `cursor_blurred_bg`, foreground unchanged |

## Select

A dropdown with a fixed list of options. Click or press Space/Enter to open, arrow keys to navigate, Enter to choose.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SelectState::new(&["Red", "Green", "Blue"]);
Select::new()
    .placeholder("Pick a color")
    .focused(true)
    .render(area, buf, &mut state);

if let Some(idx) = state.selected() {
    let label = state.selected_label().unwrap();
}
# }
# fn main() {}
```

### State

`SelectState` owns:

| Field | Type | Purpose |
|-------|------|---------|
| `options` | `Vec<SelectOption>` | List of selectable options |
| `selected` | `Option<usize>` | Index of selected option |
| `open` | `bool` | True when dropdown is open |
| `highlight` | `usize` | Index of highlighted option in open dropdown |
| `scroll` | `usize` | Scroll offset in dropdown |
| `dropdown_width` | `DropdownWidth` | Width policy: `Auto`, `Field`, or `Fixed(u16)` |

Create state with `SelectState::new(&["A", "B", "C"])` or `SelectState::with_options(vec)`. Read selection with `state.selected()` or `state.selected_label()`. Set selection with `state.set_selected(Some(idx))` or clear with `state.set_selected(None)`. Close the dropdown with `state.close()`.

Each `SelectOption` has a `label` (displayed text), optional `value` (stored data), and `disabled` flag.

### Keys

When closed:

| Key | Action |
|-----|--------|
| `Space` / `Enter` / `Down` | Open dropdown |

When open:

| Key | Action |
|-----|--------|
| `Esc` | Close without selecting |
| `Enter` | Select highlighted option and close |
| `Up` / `Down` | Move highlight (skips disabled options) |
| `Home` / `End` | Jump to first or last option |
| `PageUp` / `PageDown` | Move highlight by 5 |
| Any letter | Typeahead: jump to next option starting with typed prefix (resets after 800ms) |

### Mouse

Click the field to open or close. Click an option to select it. Wheel scrolls the dropdown list. Click outside closes the dropdown (the parent must handle this by checking `state.dropdown_area` and calling `state.close()`).

### Options

| Method | Effect |
|--------|--------|
| `placeholder(s)` | Placeholder text when no option is selected |
| `max_visible(n)` | Maximum visible options before scrolling (default 8) |
| `allow_blank(true)` | Allow deselecting the current option |
| `compact(true)` | Compact mode (single-line field without tall frame) |
| `shape(s)` | Frame shape (see FieldShape section) |

Set `state.dropdown_width` to control dropdown width. `DropdownWidth::Auto` fits the longest option. `DropdownWidth::Field` matches the field width. `DropdownWidth::Fixed(w)` uses a fixed width.

## Combobox

An editable select: type to filter options, arrow keys to navigate, Enter to choose. The input buffer filters the list in real time.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ComboboxState::new(&["Rust", "Python", "JavaScript"]);
Combobox::new()
    .placeholder("Type to filter")
    .focused(true)
    .render(area, buf, &mut state);

if let Some(idx) = state.selected() {
    let label = state.selected_label().unwrap();
}
# }
# fn main() {}
```

### State

`ComboboxState` owns:

| Field | Type | Purpose |
|-------|------|---------|
| `input` | `String` | Current input buffer (filter query) |
| `cursor` | `usize` | Cursor position in input |
| `options` | `Vec<SelectOption>` | All options |
| `filtered` | `Vec<usize>` | Indices of options matching current input |
| `selected` | `Option<usize>` | Index in `options` of selected option |
| `highlight` | `usize` | Index in `filtered` of highlighted option |
| `open` | `bool` | True when dropdown is open |
| `scroll` | `usize` | Scroll offset in dropdown |

Create state with `ComboboxState::new(&["A", "B"])`. The `filtered` list is rebuilt on every keystroke, case-insensitive substring matching. Read selection with `state.selected()` or `state.selected_label()`. The input buffer is cleared when an option is selected.

### Keys

| Key | Action |
|-----|--------|
| `Down` | Open dropdown (if closed) |
| `Esc` | Close dropdown |
| `Enter` | Select highlighted option and close |
| `Up` / `Down` | Navigate filtered options (opens dropdown if closed) |
| `Home` / `End` | Jump to first or last filtered option |
| `Left` / `Right` | Move cursor in input buffer |
| `Backspace` / `Delete` | Edit input buffer |
| Any letter | Append to filter and update filtered list |

### Mouse

Same as Select: click to open/close, click an option to select, wheel scrolls.

### Options

| Method | Effect |
|--------|--------|
| `placeholder(s)` | Placeholder text when input is empty |
| `max_visible(n)` | Maximum visible options before scrolling (default 8) |
| `compact(true)` | Compact mode |
| `shape(s)` | Frame shape (see FieldShape section) |

## MultiSelect

A select with checkboxes. Space toggles the highlighted option, Enter confirms and closes. The state tracks a set of selected indices.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = MultiSelectState::new(&["HTML", "CSS", "JS"]);
MultiSelect::new()
    .placeholder("Select multiple")
    .focused(true)
    .render(area, buf, &mut state);

let selected = state.selected_indices();
let summary = state.summary();
# }
# fn main() {}
```

### State

`MultiSelectState` owns:

| Field | Type | Purpose |
|-------|------|---------|
| `options` | `Vec<SelectOption>` | List of options |
| `selected` | `Vec<bool>` | One flag per option (true = selected) |
| `open` | `bool` | True when dropdown is open |
| `highlight` | `usize` | Index of highlighted option |
| `scroll` | `usize` | Scroll offset |

Create state with `MultiSelectState::new(&["A", "B"])`. Read selections with `state.selected_indices()` (returns `Vec<usize>` of selected indices) or `state.summary()` (returns `"2 selected"`).

### Keys

When closed:

| Key | Action |
|-----|--------|
| `Space` / `Enter` / `Down` | Open dropdown |

When open:

| Key | Action |
|-----|--------|
| `Esc` | Close (selections remain) |
| `Enter` | Close (selections remain) |
| `Space` | Toggle highlighted option |
| `Up` / `Down` | Move highlight |
| `Home` / `End` | Jump to first or last option |
| `PageUp` / `PageDown` | Move highlight by 5 |

### Mouse

Click the field to open. Click a checkbox to toggle that option. Click outside closes (the parent must handle this).

### Options

| Method | Effect |
|--------|--------|
| `placeholder(s)` | Placeholder text when no options are selected |
| `max_visible(n)` | Maximum visible options before scrolling (default 8) |
| `compact(true)` | Compact mode |
| `shape(s)` | Frame shape (see FieldShape section) |

## FieldShape

The `FieldShape` enum controls the frame drawn around text fields. All five widgets in this chapter accept `shape(s)` as a builder option. The default is `Tall(Edge::Full)`, a Textual-style frame with thick side bars and thin top and bottom lines.

| Variant | Appearance | Use when |
|---------|------------|----------|
| `Tall(edge)` | Thick side bars (`edge` controls thickness) plus thin `▔`/`▁` lines above and below | You want a prominent, Textual-style field (default) |
| `Bars(edge)` | Left and right bars only, no top or bottom | You want vertical separation without horizontal lines |
| `Bar(edge)` | Left bar only | You want a minimal accent (omp-style prompts) |
| `Rule` | A `─` rule above and below, no side bars | You want horizontal separation in a dense form |
| `Round` | Rounded box `╭─╮` / `╰─╯` | You want a softer, friendlier appearance |
| `Prompt` | A `❯` prompt glyph before the first line, no frame | You want a shell-style prompt (single-line only) |
| `Band` | Full-width band in field color with padding rows and muted `›` prompt | You want Claude Code's composer style |
| `None` | No frame | You want inline editing or custom framing |

The `edge` parameter controls bar thickness for `Tall`, `Bars`, and `Bar`:

| Edge | Width | Glyph |
|------|-------|-------|
| `Hair` | 1/8 cell | `▏` / `▕` |
| `Thin` | 2/8 cell | `▎` / `▕` (default) |
| `Half` | 1/2 cell | `▌` / `▐` |
| `Full` | Whole cell | Background painted solid |

For example, `shape(FieldShape::Bar(Edge::Hair))` draws a thin left accent bar. `shape(FieldShape::Prompt)` removes the frame entirely and draws a `❯` prompt.

## Choosing between Select, Combobox, and MultiSelect

Use **Select** when:
- The list is short (under 20 options) and the user knows what they want.
- Typing is slower than arrow keys (e.g. "United States" vs. typing "uni").
- The list is stable and well-known (countries, months, enum values).

Use **Combobox** when:
- The list is long (50+ options) and typing is faster than scrolling.
- The user may not know the exact label (search by substring).
- The list is dynamic or fetched from a search API.

Use **MultiSelect** when:
- The user needs to choose zero or more options.
- The choices are independent (tags, permissions, features).
- You want a compact summary like "3 selected" instead of showing all chosen items.

If the user must pick exactly one from a long list and knows what they want, Combobox is the right choice. If they may not remember the label, Combobox lets them filter. If they need to pick several, MultiSelect is the only option in this family.
