# Big text and menus

Big text rendered in box-drawing or pixel fonts, plus vertical menus with animated selection styles and a grouped settings list.

![bigmenus](../screenshots/bigmenus.png)

## BigText

Renders text at 3 or 5 rows per line in one of four fonts. Use it for headings, splash screens and menu items.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
BigText::new("TUILE")
    .font(BigFont::Block5)
    .gradient(&[Rgb(230, 60, 60), Rgb(255, 160, 40)])
    .render(area, buf);
# }
# fn main() {}
```

### Fonts

Four fonts available. Lowercase letters are uppercased. Unsupported characters fall back to `?`.

**Box3** (default): 3-row heavy box-drawing font. Solid strokes in every terminal font. Supports A-Z, 0-9, and `.,:-_+!?/[]<>`. Most letters are 3 cells wide. Example rendering:

```text
┏━┓┳ ┳┏━┓
┣━┫┣━┫┃┫ 
┻ ┻┻ ┻┗━┛
```

**Block5**: 5-row figlet-style font with painted solid cells (background paint, not foreground glyphs). Supports A-Z, 0-9, and `.,:-_+!/[]<>`. Most letters are 5 cells wide, numbers 4 or 5 wide. Example rendering:

```text
 ### 
#   #
#####
#   #
#   #
```

**Half3**: 3-row font using half-blocks `▀▄` for partial cells with painted solids. Same character set as Block5. Pixel fonts already carry a 1-cell gap after each glyph. Most letters are 5 cells wide plus gap. Example rendering:

```text
 ▀▀▀ 
█▀▀█
█  █
```

**Thin3**: 3-row light box-drawing font (`─│╭╮╰╯`) for elegant headings. Same character set as Box3. Most letters are 3 cells wide. Example rendering:

```text
╭─╮┬ ┬╭─╮
├─┤├─┤│ 
╯ ╯╯ ╯╰─╯
```

The pixel fonts (Block5, Half3) include a 1-cell trailing gap in their width calculation. Line fonts (Box3, Thin3) add a 1-cell gap between glyphs by default.

### Builder options

| Method | Effect |
|--------|--------|
| `font(BigFont)` | Choose the font (default `Box3`) |
| `color(Rgb)` | Flat color for the text |
| `gradient(&[Rgb])` | Left-to-right color stops across the text |
| `bg(Rgb)` | Background behind glyph strokes (default `th.background`); pass the panel color when drawing on a card |
| `spacing(u16)` | Cells between glyphs (default: the font's natural gap, 1 for line fonts, 0 for pixel fonts) |
| `align(Alignment)` | Left, Center, or Right (default Left) |
| `bold(bool)` | Apply bold modifier (default false) |
| `theme(&Theme)` | Override the default theme |

`BigText::width_of(text, font, spacing)` returns the cell width a string takes in a given font.

## BigTitle

A title with optional subtitle and rule.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
BigTitle::new("WELCOME")
    .subtitle("Select an option to begin")
    .font(BigFont::Block5)
    .gradient(&[Rgb(100, 200, 255), Rgb(180, 100, 255)])
    .rule(true)
    .render(area, buf);
# }
# fn main() {}
```

The title is center-aligned. If a subtitle is set, it appears 2 rows below the title. If `rule(true)`, a horizontal rule is drawn after the subtitle (or title if no subtitle).

| Method | Effect |
|--------|--------|
| `subtitle(&str)` | Optional subtitle below the title |
| `font(BigFont)` | Choose the font (default `Block5`) |
| `gradient(&[Rgb])` | Left-to-right color stops across the title |
| `rule(bool)` | Draw a horizontal rule after the text (default false) |
| `theme(&Theme)` | Override the default theme |

`height()` returns the total rows the title occupies with its current settings.

## BigMenu

A vertical menu of big-font items with many rendering styles.

![options](../screenshots/options.png)

### Basic usage

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = BigMenuState::default();
BigMenu::new(&["START", "OPTIONS", "QUIT"])
    .style(BigMenuStyle::Arrows)
    .focused(true)
    .render(area, buf, &mut state);
if let Some(i) = state.take_activated() {
    // user pressed Enter or clicked item i
}
# }
# fn main() {}
```

The menu can be built from simple strings or from `BigMenuItem` structs that carry a description, icon, disabled flag, and hotkey.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
# let mut state = BigMenuState::default();
let items = [
    BigMenuItem::new("NEW GAME").icon("▸").hotkey('n'),
    BigMenuItem::new("LOAD").icon("◧").disabled(true),
    BigMenuItem::new("QUIT").icon("◆").hotkey('q'),
];
BigMenu::items(&items)
    .style(BigMenuStyle::Cards)
    .focused(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

### State

`BigMenuState` owns the selection cursor, mouse hit boxes, and animation tweens.

| Field | Purpose |
|-------|---------|
| `selected` | Index of the currently selected item |
| `hits` | Mouse hit boxes (read-only, updated on render) |

| Method | Purpose |
|--------|---------|
| `take_activated()` | Returns `Some(index)` when an item was chosen with Enter/Space/click since the last call |
| `animating(now)` | True if an animation is active (for Underline style tween) |

### Keys and mouse

| Key | Action |
|-----|--------|
| Up, k | Move selection up |
| Down, j | Move selection down |
| Left | Move selection up (wraps to last) |
| Right | Move selection down (wraps to first) |
| Home | Select first item |
| End | Select last item |
| Enter, Space | Activate selected item |

Mouse hover changes the selection. Click activates the item. Wheel scrolls through items.

### Styles

Ten rendering styles, each with different selection markers, animations, and layout.

**Plain** (default): Selected item in accent or gradient, others muted.

**Arrows**: Arrow markers `▸` and `◂` flanking the selected item with animated bounce.

**Boxed**: Round border frame around the selected item.

**Underline**: Underline under the selected item that tweens between items when selection changes.

**Glow**: Selected item bright with a horizontal gradient that sweeps across it, unselected items dimmed.

**Shadow**: Drop shadow (offset right and down), selected item raised in accent.

**Bracket**: Big bracket glyphs `[` and `]` around the selected item.

**Horizontal**: Items laid in one row separated by a gap. Left and Right navigate, Up and Down wrap.

**Cards**: Each item in its own card with label and description (requires `BigMenuItem` with description).

**Retro**: Pulse-blink selected item, `>` prefix cursor, all caps, monochrome.

### Builder options

| Method | Effect |
|--------|--------|
| `style(BigMenuStyle)` | Choose the rendering style (default `Plain`) |
| `gap(u16)` | Blank rows between items (default 1) |
| `focused(bool)` | Draw focus indicator (default false) |
| `align(Alignment)` | Left, Center, or Right (default Center) |
| `color(Rgb)` | Color of the selected item (default `th.accent`) |
| `selected_gradient(&[Rgb])` | Gradient across the selected item instead of flat color |
| `font(BigFont)` | Choose the font (default `Box3`) |
| `now(Instant)` | Pass the current time for animations |
| `theme(&Theme)` | Override the default theme |

`height()` returns the total rows the menu needs to show all items. `width()` returns the widest item including the style's side chrome.

When an item is too wide for the available area, the menu first tries the label with letters touching (spacing 0), then truncates to the longest prefix that fits.

## OptionList

A settings menu with grouped rows, in-place value cycling, and a group index for a sidebar. The shape of omp's settings screen.

### Structure

An `OptionListState` owns a `Vec<OptionGroup>`. Each group has a title and a `Vec<OptionItem>`. Each item has a stable `key`, a visible `label`, a value, and an optional hint.

Values can be Bool, Choice (one of N strings), Int (with min/max/step), Text (read-only, app edits it elsewhere), or Action (a command that reports changed when Enter is pressed).

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = OptionListState::new(vec![
    OptionGroup::new("Theme", vec![
        OptionItem::choice("theme", "Dark Theme", &["titanium", "nord", "dracula"], 0),
        OptionItem::bool("colorblind", "Color-Blind Mode", false),
    ]),
    OptionGroup::new("Display", vec![
        OptionItem::int("fps", "Frame rate", 60, 15, 120, 15),
    ]),
]);
OptionList::new().focused(true).render(area, buf, &mut state);
if let Some(key) = state.take_changed() {
    let theme = state.choice("theme");
}
# }
# fn main() {}
```

### State methods

| Method | Purpose |
|--------|---------|
| `take_changed()` | Returns the key of the row whose value changed (or whose action fired) since the last call |
| `get(&str)` | Returns a reference to the value for a key |
| `get_mut(&str)` | Returns a mutable reference to the value for a key |
| `bool(&str)` | Returns `Some(bool)` if the key is a Bool value |
| `choice(&str)` | Returns the selected option text of a Choice row |
| `int(&str)` | Returns `Some(i64)` if the key is an Int value |
| `group_index()` | Index of the group the cursor is in (for a sidebar) |
| `jump_to_group(usize)` | Move the cursor to the first row of a group |

### Keys and mouse

| Key | Action |
|-----|--------|
| Up, k | Move cursor up |
| Down, j | Move cursor down |
| Home | Jump to first row |
| End | Jump to last row |
| Left, h | Cycle value backward (Bool toggles, Choice moves to previous option, Int decrements by step) |
| Right, l | Cycle value forward |
| Enter | Cycle value forward for Bool/Choice/Int, report changed for Text/Action |

Mouse hover changes the cursor. Click on a row selects it and cycles its value forward. Wheel scrolls through rows.

The cursor is a flat index over all items in group order. Group headers are visible but not selectable.

### Widget options

| Method | Effect |
|--------|--------|
| `focused(bool)` | Draw focus indicator (default false) |
| `value_column(u16)` | Column where values start (default: widest label + 4) |
| `cursor_glyph(&str)` | Glyph shown to the left of the cursor row (default `❯`) |
| `theme(&Theme)` | Override the default theme |

The widget renders group headers in muted text, then each row as `cursor label  value hint`. The hint (if present) appears after the value in muted text only when the cursor is on that row.

The scrollbar appears when the list is taller than the area. `keep_visible` keeps the cursor row visible.
