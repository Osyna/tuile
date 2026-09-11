# Navigation

Widgets for moving between content: tab bars, lists, trees, menus, breadcrumbs, and paginators.

![navigation](../screenshots/navigation.png)

## TabBar

A horizontal tab bar with five visual styles. All support keyboard navigation (Left/Right/Home/End), mouse clicks, and optional closable tabs. The Underline style animates a sliding highlight bar.

### Tab styles

TabBar renders tabs in one of five styles. Pass `.style(TabStyle::...)` to pick one.

| Style | Description |
|-------|-------------|
| `Underline` | Minimal tabs with an animated underline below the active tab (default, 2 rows tall) |
| `Boxed` | Each tab is a bordered box; active tab has no bottom border (3 rows tall) |
| `Pills` | Rounded pill buttons with solid fill on the active tab (1 row tall) |
| `Segmented` | Horizontal segmented control, tabs share borders (1 row tall) |
| `Minimal` | Plain text tabs with subtle highlight on active, no borders (1 row tall) |

### Basic example

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let items = vec!["Home".into(), "Settings".into(), "Help".into()];
let mut state = TabBarState::new(0);
TabBar::new(items)
    .style(TabStyle::Underline)
    .focused(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state holds `active` (the selected tab index). Call `state.set_active(i)` to switch tabs programmatically, or let the user navigate with arrow keys and clicks.

### Animated underline

The Underline style slides a highlight bar from the old tab to the new one. Pass `.now(Instant::now())` each frame to update the animation. The bar tweens both position and width when tabs differ in size.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use std::time::Instant;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let items = vec!["Home".into(), "Projects".into(), "Settings".into()];
let mut state = TabBarState::new(0);
TabBar::new(items)
    .style(TabStyle::Underline)
    .now(now)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The default duration is 150 ms. Without `.now()`, the underline still moves but jumps instantly.

### TabItem options

Tabs accept `TabItem` instead of plain strings when you need icons, badges, or flags. Every `&str` automatically converts to a plain `TabItem`.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let items = vec![
    TabItem::new("All").icon("◈"),
    TabItem::new("Active").badge("3"),
    TabItem::new("Archive").disabled(true),
];
let mut state = TabBarState::new(0);
TabBar::new(items)
    .style(TabStyle::Pills)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

| Method | Effect |
|--------|--------|
| `.icon(s)` | Icon string shown before the label |
| `.badge(s)` | Badge string shown after the label in a colored pill |
| `.disabled(true)` | Tab is visible but not selectable |
| `.closable(true)` | Show an × button that removes the tab when clicked |

### Closable tabs

A closable tab shows an × button on hover. When clicked, the state records the index in `closed` but does not remove the item from your data. You must call `take_closed()` after event handling, remove the item yourself, and clamp the active index if needed.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn example() {
let mut tab_items = vec![
    "Files".into(),
    TabItem::new("Search").closable(true),
    TabItem::new("Git").closable(true),
];
let mut state = TabBarState::new(0);

// in event handler after handle_mouse
if let Some(i) = state.take_closed() {
    if i < tab_items.len() {
        tab_items.remove(i);
        let last = tab_items.len().saturating_sub(1);
        state.set_active(state.active.min(last));
    }
}
# }
# fn main() {}
```

The × hit box is recorded in `state.close_hits`. Clicking outside a closable tab still selects it; the × is the only region that closes.

### Builder options

| Method | Effect |
|--------|--------|
| `.style(TabStyle)` | Visual style (default Underline) |
| `.focused(bool)` | Enables keyboard navigation and focus ring |
| `.now(Instant)` | Current time for underline animation |
| `.theme(Theme)` | Custom theme (default from prelude) |

## TabbedContent

A helper that draws a TabBar and returns the inner content rect. Pair it with a match on `state.active` to render the selected panel.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let tabs = vec!["List".into(), "Tree".into(), "Grid".into()];
let mut state = TabBarState::new(0);

let content_rect = TabbedContent::new(tabs)
    .style(TabStyle::Underline)
    .bordered(true)
    .focused(true)
    .render(area, buf, &mut state);

match state.active {
    0 => { /* render list */ },
    1 => { /* render tree */ },
    2 => { /* render grid */ },
    _ => {}
}
# }
# fn main() {}
```

The `.bordered(true)` option draws a border around the content area. Without it, the content rect fills the remaining space.

## ListView

A vertical scrolling list with cursor, multi-select, fuzzy filtering, details column, separators, and activation on Enter or double-click.

### Basic list

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let entries = vec![
    ListEntry::new("Item 1"),
    ListEntry::new("Item 2"),
    ListEntry::new("Item 3"),
];
let mut state = ListViewState::new();
ListView::new(entries)
    .focused(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state holds `cursor` (the highlighted row index). Arrow keys move the cursor, Enter calls activation (recorded in `state.activated`).

### Details pane

Pass `.details(ListDetail::Right)` to show a right-aligned detail column. Each entry can have an optional detail string.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let entries = vec![
    ListEntry::new("config.toml").detail("1.2K"),
    ListEntry::new("main.rs").detail("4.5K"),
    ListEntry::new("lib.rs").detail("8.3K"),
];
let mut state = ListViewState::new();
ListView::new(entries)
    .details(ListDetail::Right)
    .focused(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

### Icons and variants

Entries support icons, disabled state, and visual variants (Default, Primary, Success, Warning, Error, Accent).

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let entries = vec![
    ListEntry::new("Running").icon("●").variant(Variant::Success),
    ListEntry::new("Stopped").icon("○").variant(Variant::Error),
    ListEntry::new("Archived").disabled(true),
];
let mut state = ListViewState::new();
ListView::new(entries).render(area, buf, &mut state);
# }
# fn main() {}
```

### Separators

A separator is a horizontal line between groups. Set `.separator(true)` on an entry to turn it into a divider. The label is shown as a section heading, or leave it empty for a plain line.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let entries = vec![
    ListEntry::new("Item 1"),
    ListEntry::new("Item 2"),
    ListEntry::new("Group B").separator(true),
    ListEntry::new("Item 3"),
    ListEntry::new("Item 4"),
];
let mut state = ListViewState::new();
ListView::new(entries).render(area, buf, &mut state);
# }
# fn main() {}
```

### Multi-select

Pass `.multi_select(true)` to enable checkboxes. Space toggles the current item, Enter activates it. The state holds a `selected` set of indices.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let entries = vec![
    ListEntry::new("Option A"),
    ListEntry::new("Option B"),
    ListEntry::new("Option C"),
];
let mut state = ListViewState::new();
ListView::new(entries)
    .multi_select(true)
    .highlight(ListHighlight::Bar)
    .render(area, buf, &mut state);

// read state.selected (a BTreeSet<usize>)
# }
# fn main() {}
```

The `ListHighlight` enum controls the cursor appearance. `Bar` fills the entire row; `Side` draws a thin line on the left edge.

### Fuzzy filtering

Pass `.filter(pattern)` to show only entries whose labels match. The filter uses fuzzy matching (characters in order, gaps allowed).

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, pattern: &str) {
let entries = vec![
    ListEntry::new("config.toml"),
    ListEntry::new("Cargo.toml"),
    ListEntry::new("main.rs"),
];
let mut state = ListViewState::new();
ListView::new(entries)
    .filter(pattern)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The filter is applied every render. Filtered-out items are not counted in cursor navigation or selection.

### State methods

| Method | Effect |
|--------|--------|
| `new()` | Create state with cursor at 0 |
| `take_activated()` | Return and clear the activated index (set by Enter or double-click) |

The state also holds `hits` (the widget's drawn rect), `scroll` (top visible row), and `hover` (mouse position).

### Builder options

| Method | Effect |
|--------|--------|
| `.focused(bool)` | Enables keyboard navigation and focus ring |
| `.border(Border)` | Border style (default Round) |
| `.title(s)` | Title shown in the top border |
| `.details(ListDetail)` | Detail column position (None, Right; default None) |
| `.multi_select(bool)` | Show checkboxes (default false) |
| `.highlight(ListHighlight)` | Cursor style (Bar, Side; default Bar) |
| `.filter(s)` | Fuzzy filter pattern (default empty, shows all) |
| `.theme(Theme)` | Custom theme |

## TreeView

A hierarchical tree with expand/collapse, tree guides, icons, and scrolling. Nodes are `TreeNode` structs with children.

### Building a tree

Create nodes with `TreeNode::new(label)`, nest them with `.with_children(vec![...])`, and call `TreeNode::assign_ids(&mut roots)` once to assign unique ids depth-first.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut roots = vec![
    TreeNode::new("src").with_children(vec![
        TreeNode::new("main.rs"),
        TreeNode::new("lib.rs"),
    ]),
    TreeNode::new("Cargo.toml"),
];
TreeNode::assign_ids(&mut roots);

let mut state = TreeViewState::new();
TreeView::new(roots).render(area, buf, &mut state);
# }
# fn main() {}
```

The `assign_ids` method is required before rendering. Without it, every node has id 0 and expand/collapse breaks.

### Icons and details

Nodes accept optional icon and detail strings, shown to the left and right of the label.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut roots = vec![
    TreeNode::new("src").icon("").with_children(vec![
        TreeNode::new("main.rs").icon("").detail("2.4K"),
        TreeNode::new("lib.rs").icon("").detail("1.8K"),
    ]),
];
TreeNode::assign_ids(&mut roots);

let mut state = TreeViewState::new();
TreeView::new(roots).render(area, buf, &mut state);
# }
# fn main() {}
```

### Expand and collapse

The state holds an `expanded` set of node ids. Call `state.expand(id)` and `state.collapse(id)` to change it, or let the user toggle with Enter or click on the expand marker (▸ collapsed, ▾ expanded).

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo() {
let mut state = TreeViewState::new();
state.expand(TreeId(0));
state.expand(TreeId(3));
# }
# fn main() {}
```

Only expanded nodes show their children. The cursor moves through visible rows only.

### Tree guides

The tree draws guides (vertical and corner lines) to show hierarchy. The style matches the border setting (Hair, Thin, Half, Full, Round).

### State methods

| Method | Effect |
|--------|--------|
| `new()` | Create state with empty expanded set |
| `expand(TreeId)` | Add node to expanded set |
| `collapse(TreeId)` | Remove node from expanded set |
| `is_expanded(TreeId)` | Check if node is expanded |
| `take_activated()` | Return and clear the activated node id (set by Enter or double-click) |

The state also holds `cursor` (visible row index), `scroll`, and `hover`.

### Builder options

| Method | Effect |
|--------|--------|
| `.focused(bool)` | Enables keyboard navigation and focus ring |
| `.border(Border)` | Border style (default Round) |
| `.title(s)` | Title shown in the top border |
| `.theme(Theme)` | Custom theme |

## MenuBar and ContextMenu

A menu bar with dropdowns, submenus, keyboard navigation, and context menus. Both use the same item structure but different positioning.

### MenuBar

A horizontal menu bar at the top of the window. Each menu is a `MenuDef` with a title and a list of items.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let menus = vec![
    MenuDef {
        title: "File".to_string(),
        items: vec![
            MenuItem::action(1, "New"),
            MenuItem::action(2, "Open"),
            MenuItem::separator(),
            MenuItem::action(3, "Quit").shortcut("^Q"),
        ],
    },
    MenuDef {
        title: "Edit".to_string(),
        items: vec![
            MenuItem::action(20, "Cut"),
            MenuItem::action(21, "Copy"),
            MenuItem::action(22, "Paste"),
        ],
    },
];

let mut state = MenuBarState::new();
MenuBar::new(menus)
    .focused(true)
    .render(area, buf, &mut state);

state.render_overlay(buf, buf.area);
# }
# fn main() {}
```

Call `state.render_overlay(buf, bounds)` after rendering the menu bar to draw the open dropdown on top of everything else.

### MenuItem types

Menus contain action items, separators, and submenus. An action has an id (any `usize` you choose), a label, and optional shortcut, disabled flag, and checkmark.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo() {
let item = MenuItem::action(10, "Save");
let item_with_shortcut = MenuItem::action(11, "Save As").shortcut("^S");
let checked = MenuItem::action(12, "Show sidebar").checked(true);
let disabled = MenuItem::action(13, "Export").disabled(true);
let separator = MenuItem::separator();
let submenu = MenuItem::submenu("Recent", vec![
    MenuItem::action(20, "file1.rs"),
    MenuItem::action(21, "file2.rs"),
]);
# }
# fn main() {}
```

Submenus open to the right when hovered or navigated to with Right arrow. They nest arbitrarily deep.

### Shortcuts and checkmarks

Shortcuts are display-only strings shown right-aligned in the dropdown. They do not bind keys. Checkmarks show a ✓ to the left of the label when the item is in a checked state.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let menus = vec![MenuDef {
    title: "View".to_string(),
    items: vec![
        MenuItem::action(1, "Sidebar").shortcut("^B").checked(true),
        MenuItem::action(2, "Status bar").checked(false),
    ],
}];
let mut state = MenuBarState::new();
MenuBar::new(menus).render(area, buf, &mut state);
state.render_overlay(buf, buf.area);
# }
# fn main() {}
```

To toggle a checkmark, track the checked state yourself and rebuild the menu items each frame.

### Opening and closing

Call `state.open(index)` to open a menu by index, or `state.close()` to close all. The user can open with F10, click, or Alt+letter (not implemented by the widget, handle in your event loop). Esc closes the menu.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo() {
let mut state = MenuBarState::new();
state.open(0);

if state.is_open() {
    state.close();
}
# }
# fn main() {}
```

### Consuming actions

When an action item is clicked or activated with Enter, its id is recorded in `state`. Call `take_action()` to retrieve and clear it.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo() {
let mut state = MenuBarState::new();

if let Some(action_id) = state.take_action() {
    match action_id {
        1 => { /* handle New */ },
        2 => { /* handle Open */ },
        3 => { /* handle Quit */ },
        _ => {}
    }
}
# }
# fn main() {}
```

This is the same flow as closable tabs: the widget records the event, you take it and act.

### ContextMenu

A floating menu that opens at a cursor position. Build it with `ContextMenu::new(items)` and call `state.open_at(Position { x, y })` to show it.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let items = vec![
    MenuItem::action(1, "Open"),
    MenuItem::action(2, "Rename"),
    MenuItem::separator(),
    MenuItem::action(3, "Delete"),
];

let mut state = ContextMenuState::new();
ContextMenu::new(items)
    .theme(&Theme::default())
    .render(area, buf, &mut state);

state.render_overlay(buf, buf.area);

// open on right-click
# #[allow(unused_variables)]
# let mouse_x = 10u16;
# #[allow(unused_variables)]
# let mouse_y = 5u16;
state.open_at(Position { x: mouse_x, y: mouse_y });
# }
# fn main() {}
```

The context menu also uses `take_action()` to consume activated item ids. Call `state.close()` to dismiss it, or let the user click outside or press Esc.

### MenuBar keyboard navigation

When focused, F10 or click opens the first menu. Arrow keys navigate between menus and items. Right opens a submenu, Left closes it or moves to the previous menu. Enter activates an item, Esc closes all.

## Breadcrumbs

A horizontal breadcrumb trail with clickable segments. Middle segments collapse into ellipsis when space is tight.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let segments = vec![
    "Home".to_string(),
    "Projects".to_string(),
    "tuile".to_string(),
    "src".to_string(),
    "main.rs".to_string(),
];
let mut state = BreadcrumbsState::new();
Breadcrumbs::new(segments)
    .focused(false)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

### Collapsing

When the widget is too narrow to fit all segments, it hides middle segments and shows `... ›` instead. The first and last segments always remain visible. The collapsing logic kicks in when the total width exceeds the area.

### Clicking

When a segment is clicked, its index is recorded in `state`. Call `take_clicked()` to retrieve it.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo() {
let mut state = BreadcrumbsState::new();

if let Some(index) = state.take_clicked() {
    // navigate to segments[index]
}
# }
# fn main() {}
```

The breadcrumbs widget does not navigate for you. You must handle the click and change your view or path state.

### Builder options

| Method | Effect |
|--------|--------|
| `.focused(bool)` | Enables keyboard navigation (Home, End, Enter) |
| `.theme(Theme)` | Custom theme |

## Paginator

A page selector with ellipsis for large page counts. Shows current page, first, last, and a window of nearby pages.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = PaginatorState::new();
Paginator::new(42)
    .window(2)
    .focused(false)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The first argument is the total number of pages. `.window(n)` controls how many pages to show on each side of the current page (default 1). The paginator always shows page 1, page `total`, and `window` pages around `state.page`.

### Navigation

Arrow keys, Home, End, and Page Up/Down move between pages when focused. Clicking a page number jumps to it. The state holds `page` (0-indexed).

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo() {
let mut state = PaginatorState::new();
state.page = 5;
# }
# fn main() {}
```

The paginator does not load or display page content. It only tracks the current page number. You must render the content yourself based on `state.page`.

### Builder options

| Method | Effect |
|--------|--------|
| `.window(usize)` | Number of pages shown around current (default 1) |
| `.focused(bool)` | Enables keyboard navigation |
| `.theme(Theme)` | Custom theme |

## Choosing between navigation widgets

Use `TabBar` for a small fixed set of views (3 to 8 tabs). Use `TabbedContent` when you want the tab bar and content area in one call. Use `ListView` for a dynamic list of items with optional filtering or multi-select. Use `TreeView` for hierarchical data like file trees or nested categories. Use `MenuBar` for commands grouped by category at the top of the window, and `ContextMenu` for right-click actions on a specific item. Use `Breadcrumbs` to show the current path and allow jumping to parent levels. Use `Paginator` for page-based navigation when you cannot fit all content on one screen.

All navigation widgets follow the same handle pattern: render with `&mut state`, handle events with `state.handle_key(k)` or `state.handle_mouse(m)`, and read activation or clicks with `take_activated()` / `take_action()` / `take_clicked()` / `take_closed()`.
