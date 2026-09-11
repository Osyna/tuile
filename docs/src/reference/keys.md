# Keyboard conventions

tuile follows Textual's keyboard patterns across every widget. Learn them once and they work everywhere.

## Global and focus movement

| Key | Action |
|---|---|
| Tab | Move focus to the next widget in the focus ring |
| Shift-Tab | Move focus to the previous widget |

Focus is managed by your app through `Focus<Id>`. Call `focus.handle_key(key)` to route Tab and Shift-Tab, or implement your own ring. The widgets themselves do not trap Tab.

## Activation

| Key | Action |
|---|---|
| Enter | Activate (buttons, list rows, select options, dialogs, tabs) |
| Space | Activate (buttons, list rows) or toggle (checkboxes, switches, table selection) |

The helper `is_activate(key)` recognizes both. Buttons flash for 120 ms on activation. Lists and selects mark `state.activated` with the chosen index.

## Lists and menus

| Key | Action |
|---|---|
| Up / Down | Move cursor up or down one row |
| j / k | Move cursor down or up (vim-style, in `ListView` and `OptionList`) |
| Home | Jump to the first row |
| End | Jump to the last row |
| PageUp | Move up ten rows (five rows in `CommandPalette`) |
| PageDown | Move down ten rows (five rows in `CommandPalette`) |
| Type-ahead | Type letters to jump to the next matching entry |

Type-ahead resets after a second of inactivity. In `ListView` it is fuzzy; in `Select` and `CommandPalette` it matches prefixes.

`OptionList` has a special value-cycling mode: Left/Right or h/l adjusts the value at the cursor (cycles choices, toggles bools, steps numbers). Enter and Space also step the value forward.

## Text fields

Single-line `Input` and multi-line `TextArea` share the same motion and editing keys.

### Navigation

| Key | Action |
|---|---|
| Left / Right | Move cursor one grapheme |
| Ctrl-Left / Ctrl-Right | Move to previous or next word boundary |
| Home | Move to start of line (or start of document with Ctrl in `TextArea`) |
| End | Move to end of line (or end of document with Ctrl in `TextArea`) |
| Up / Down | Move up or down one line (`TextArea` only) |
| PageUp / PageDown | Move up or down ten lines (`TextArea` only) |

Word boundaries follow the rule in `core::word_boundary`: skip non-alphanumerics, then the word itself. Both widgets use the same function so they cannot drift.

### Editing

| Key | Action |
|---|---|
| Backspace | Delete backward (by word with Ctrl in `Input`) |
| Delete | Delete forward |
| Tab | Accept suggestion (`Input`) or insert four spaces (`TextArea`) |

`Input` shows a greyed-out suggestion from its `suggester` hook; Tab or Right at the end accepts it. `TextArea` has no autocomplete; Tab is indent.

### Clipboard and undo

| Key | Action |
|---|---|
| Ctrl-A | Select all |
| Ctrl-C | Copy selection |
| Ctrl-X | Cut selection |
| Ctrl-V | Paste |
| Ctrl-Z | Undo |
| Ctrl-Y | Redo (`TextArea` only) |

The clipboard is per-widget state, not the system clipboard. `Input` also supports emacs-style shortcuts: Ctrl-U deletes from cursor to start, Ctrl-K deletes to end, Ctrl-W deletes the word before the cursor, and Alt-B / Alt-F move by word.

`TextArea` adds Ctrl-D to duplicate the current line and Alt-Up / Alt-Down to move the line up or down.

## Dropdowns and overlays

| Key | Action |
|---|---|
| Escape | Close the dropdown, dialog, or palette |

`Select` and `MultiSelect` open on Enter, Space, or Down when closed. `Modal` closes on Escape only if `close_on_escape` is true (the default). `CommandPalette` always closes on Escape.

Click outside a dropdown or modal backdrop to dismiss (when `close_on_backdrop` is enabled).

## Tables

| Key | Action |
|---|---|
| Arrows | Move cursor (row or cell depending on `TableCursor` mode) |
| Home / End | Jump to first or last row |
| PageUp / PageDown | Move up or down ten rows |
| Enter | Activate the current row (sets `state.activated`) |
| Space | Toggle selection for the current row |
| s | Toggle sort on the current column |

Header clicks also toggle sort. Column boundaries can be dragged to resize.

## Tabs

| Key | Action |
|---|---|
| Left / Right | Switch to the previous or next tab |
| Home / End | Jump to the first or last tab |
| 1..9 | Jump to tab 1 through 9 |
| Ctrl-W or Delete | Close the current tab (when closable) |
| Space or Enter | Re-activate the current tab (returns `Outcome::Changed`) |

Mouse wheel over the tab bar also cycles tabs.

## Deviations

Most widgets follow the patterns above without change. Two exceptions:

- `CommandPalette` uses Home and End to jump to the start and end of the query input, not the result list. Use PageUp to reach the top of the list.
- `Modal` in prompt mode sends all typing to the text input until you Tab to the buttons. Up/Down do not navigate; Left/Right and Tab move between buttons.

## Showcase app keys

The showcase gallery app itself uses:

| Key | Action |
|---|---|
| ] / [ | Next or previous page |
| Alt-1..9 | Jump to page 1 through 9 |
| Ctrl-P | Open command palette (pages, themes, actions) |
| Ctrl-T | Cycle to the next theme |
| Ctrl-B | Toggle sidebar |
| F1 | Show help overlay |
| F3 | Toggle reduce motion (disables animations) |
| Ctrl-C / Ctrl-Q | Quit |
| q | Quit (when no widget consumed the key) |
| Esc | Dismiss all toasts (when no modal or dropdown is open) |

Inside pages, Tab and Shift-Tab move focus, arrows adjust focused widgets, and Enter/Space activate.
