# Tables and data

Data presentation widgets: a sortable table with row or cell cursors, a simple key-value list, and large seven-segment digits.

![tables](../screenshots/tables.png)

## DataTable

A full-featured data table with sortable columns, row or cell navigation, multi-select, filtering, zebra striping, column resize, scrollbars, and styling hooks. The header row is clickable and resizable.

### Building columns and rows

Columns define layout and behaviour. Rows hold cells.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::{DataTable, DataTableState, TableColumn, TableRow, TableCell, TableCursor};
use tuile::ratatui_core::layout::{Constraint, Alignment};

# fn demo(area: Rect, buf: &mut Buffer) {
let cols = vec![
    TableColumn::new("Name")
        .width(Constraint::Fill(1))
        .sortable(true),
    TableColumn::new("CPU")
        .width(Constraint::Length(8))
        .align(Alignment::Right)
        .sortable(true),
];

let rows = vec![
    TableRow::from(vec!["server-01", "45.2%"]),
    TableRow::from(vec!["server-02", "12.8%"]),
];

let mut state = DataTableState::new();
DataTable::new(cols, rows)
    .cursor(TableCursor::Row)
    .zebra(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

`TableColumn` has five fields:
- `title` (String): header text
- `width` (Constraint): column width constraint; defaults to `Fill(1)`
- `align` (Alignment): cell alignment; defaults to Left
- `sortable` (bool): whether clicking the header or pressing `s` toggles sort; defaults to false
- `key` (usize): unique identifier for column reordering (not used by DataTable itself)

`TableRow::from(vec)` accepts anything that converts to `TableCell`. Strings, `&str`, and `f64` all work. An `f64` cell displays as `{:.1}` and stores the numeric value as `sort_key` for numeric sorting.

`TableCell` has three fields:
- `text` (String): displayed content
- `style` (Option\<Style\>): overrides the default cell style if present
- `sort_key` (Option\<f64\>): numeric sort value; when present, sorting compares numbers instead of text

### Cursor modes

`TableCursor` has four variants:

| Mode | Behaviour |
|------|-----------|
| `None` | No cursor; the table is display-only |
| `Row` | Highlights the entire row; arrow keys move up and down |
| `Cell` | Highlights a single cell; arrows move in all four directions |
| `Column` | Highlights an entire column; left and right arrows move between columns |

Row mode is the most common for selecting records. Cell mode is useful for heatmaps or when individual cell values matter. Column mode is rare.

### Sorting

Clicking a sortable column header or pressing `s` when focused toggles sort order: first click sorts ascending, second descending, third clears the sort. The `sort` field in `DataTableState` holds `Some((col_idx, ascending))` or `None`.

Sort logic:
- If both cells have `sort_key`, compare those numbers.
- Otherwise, compare `text` case-insensitively.
- Missing cells sort last.

The `f64` conversion automatically sets `sort_key`, so a column of floats sorts numerically. For other types (dates, durations, IDs), construct `TableCell` manually:

```rust
# extern crate tuile;
# use tuile::widgets::TableCell;
let uptime_cell = TableCell::new("23d 4h")
    .sort_key(23.0 * 24.0 + 4.0); // sort by hours
# fn main() {}
```

### Selection and activation

Multi-select is off by default. Enable it with `.multi_select(true)`. Space toggles selection of the current row. `state.selected` is a `BTreeSet<usize>` of row indices (into the caller's rows, not the sorted/filtered view).

Enter or double-clicking a row sets `state.activated` to `Some(row_idx)`. Call `state.take_activated()` after handling an event to consume it (returns `Option<usize>`, clearing the field). This pattern avoids processing the same activation twice.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::{DataTable, DataTableState, TableColumn, TableRow, TableCursor};
# use tuile::crossterm::event::{Event, KeyEvent};
# fn demo(state: &mut DataTableState, ev: &Event) -> Outcome {
let out = state.handle(&ev);
if let Some(idx) = state.take_activated() {
    println!("User activated row {}", idx);
}
out
# }
# fn main() {}
```

The row index is into the original rows, not the sorted or filtered order, so it remains stable across sorts.

### Filtering

`state.set_filter(query)` rebuilds the visible row list to include only rows where at least one cell contains the query (case-insensitive substring match). An empty query shows all rows. The showcase page binds `/` to enter filter mode, which calls `set_filter` on each keystroke.

Filtering and sorting compose: the table first filters, then sorts the surviving rows.

### Zebra striping and row numbers

`.zebra(true)` alternates row background colours (based on the visible row index after filtering and sorting).

`.show_row_numbers(true)` prepends a column displaying 1-based row numbers. The numbers reflect the visible order, so they change when sorting or filtering.

### Borders and spacing

`.borders(TableBorders)` controls separator lines:

| Variant | Rendering |
|---------|-----------|
| `None` | No lines (default) |
| `Horizontal` | Lines between rows |
| `Vertical` | Lines between columns |
| `All` | Grid |

`.column_gap(n)` sets the spacing between columns (defaults to 2 cells). `.padding(n)` adds left and right padding inside each cell (defaults to 1).

### Column resize

Clicking the boundary between two column headers starts a drag-resize. The mouse position during drag sets the width of the left column. Released widths are stored in `state.width_overrides` (a `HashMap<usize, u16>`) and persist across renders.

### The cell_style hook

`.cell_style(fn)` lets the caller override cell colours based on value. The function receives `(row_idx, col_idx, &TableCell, &Theme)` and returns `Option<Style>`. Returning `Some` replaces the default style; `None` keeps it.

Example from the showcase (a heatmap blending primary and error colours based on cell value):

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::TableCell;
fn heatmap_style(_row: usize, _col: usize, cell: &TableCell, th: &Theme) -> Option<Style> {
    if let Some(v) = cell.sort_key {
        let intensity = ((v - 30.0) / 50.0).clamp(0.0, 1.0) as f32;
        let color = th.primary.blend(th.error, intensity);
        Some(st(color, th.surface))
    } else {
        None
    }
}
# fn main() {}
```

The function must be a plain `fn` pointer (not a closure) because the widget stores it across frames.

### State fields

`DataTableState` exposes the following public fields and methods:

| Field / method | Type / signature | Meaning |
|----------------|------------------|---------|
| `cursor_row` | `usize` | Cursor position in the sorted/filtered view (0-based) |
| `cursor_col` | `usize` | Column index (for Cell or Column cursor modes) |
| `offset_y` | `usize` | Vertical scroll offset in rows |
| `offset_x` | `usize` | Horizontal scroll offset in cells |
| `selected` | `BTreeSet<usize>` | Row indices (into original rows) that are selected |
| `sort` | `Option<(usize, bool)>` | Current sort (column index, ascending) or None |
| `hover_row` | `Option<usize>` | Row index under the mouse in the visible view |
| `filter` | `String` | Current filter query (case-insensitive substring) |
| `visible_len()` | `usize` | Number of rows after filtering |
| `current_row()` | `Option<usize>` | Original row index at `cursor_row`, or None if empty |
| `take_activated()` | `Option<usize>` | Consumes and returns the activated row index (Enter or double-click) |
| `selected_rows()` | `Vec<usize>` | Copy of selected row indices as a vector |
| `set_filter(&str)` | `()` | Updates the filter and marks the order dirty for rebuild |
| `clear_selection()` | `()` | Empties the selection set |

The state rebuilds the internal `order` vector whenever `sort` or `filter` changes (lazy rebuild on next render). The caller never sees `order` directly; use `current_row()` to map visible positions to original indices.

### Keys and mouse

The table responds to keyboard and mouse when focused. All bindings apply to Row and Cell cursor modes; Column mode only handles Left, Right, Enter, and `s`.

| Input | Action |
|-------|--------|
| **Keyboard** | |
| Up / Down | Move cursor row |
| Left / Right | Move cursor column (Cell mode) or do nothing (Row mode) |
| Home | Jump to first row |
| End | Jump to last row |
| Page Up | Move up 10 rows |
| Page Down | Move down 10 rows |
| Enter | Set `activated` to current row (consumed by `take_activated()`) |
| Space | Toggle selection of current row (if `multi_select` is true) |
| `s` | Toggle sort on the current column (if `sortable` is true): ascending → descending → none |
| **Mouse** | |
| Click header | Toggle sort on that column (same as `s`) |
| Click header boundary | Start column resize drag |
| Drag (after boundary click) | Resize the left column; width is stored in `state.width_overrides` |
| Click row | Move cursor to that row and set `activated` if it was already focused |
| Double-click row | Set `activated` (same as Enter) |
| Hover row | Update `state.hover_row` (does not change cursor or selection) |
| Scroll wheel | Scroll the table vertically (adjusts `offset_y`) |
| Scrollbar drag | Scroll to the dragged position |

Mouse events do not require focus. Both the main table and a secondary table can respond to clicks simultaneously if the caller forwards events to both states (as the showcase does).

## KeyValueList

A simple two-column widget for label-value pairs (definition lists, metadata, settings). Not interactive; it is a plain `Widget`, not `StatefulWidget`.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::KeyValueList;

# fn demo(area: Rect, buf: &mut Buffer) {
let items = vec![
    ("Version".to_string(), "1.0.0".to_string()),
    ("Uptime".to_string(), "23d 4h 12m".to_string()),
    ("Memory".to_string(), "8.2 / 16.0 GB".to_string()),
];
KeyValueList::new(items).render(area, buf);
# }
# fn main() {}
```

Labels are bold and muted; values are normal text. The label column width is the longest label (capped at one third of the available width). `.gap(n)` sets the spacing between columns (defaults to 2).

If there are more items than rows in the area, the excess is clipped. Use a scrollable container or split the list if this is a problem.

## Digits

Textual-style 3-row large digits for displaying numbers, time, or short character sequences. Each digit occupies 3 cells wide by 3 rows tall. The character set is ` 0123456789+-^x:ABCDEF$£€()`. Colon (`:`) is 1 cell wide; all others are 3.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::Digits;
use tuile::ratatui_core::layout::Alignment;

# fn demo(area: Rect, buf: &mut Buffer, theme: &Theme) {
Digits::new("12:34:56")
    .color(theme.primary)
    .align(Alignment::Center)
    .render(area, buf);
# }
# fn main() {}
```

`Digits::width(text)` returns the display width in cells (useful for layout). The widget requires at least 3 rows; if the area is shorter, nothing renders.

Options:
- `.color(Rgb)`: glyph foreground colour
- `.variant(Variant)`: alternative to `.color`; uses the theme's variant colour
- `.align(Alignment)`: horizontal alignment (Left, Center, Right); defaults to Left
- `.bold(bool)`: render glyphs bold; defaults to false
- `.bg(Rgb)`: background colour; defaults to theme surface
- `.theme(&Theme)`: override the theme

The showcase uses Digits for a live clock (updated every second via `animating`) and a counter (incremented with Space). The clock is primary-coloured; the counter changes to accent when focused.
