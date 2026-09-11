# Running the showcase

The `showcase` binary is a 22-page gallery of every widget in the library. It is the fastest
way to see what a widget looks like before you write any code, and the source for each page
is one self-contained file you can copy from.

```text
git clone https://github.com/Osyna/tuile
cd tuile
cargo run --release -p showcase
```

Use `--release`. The debug build redraws animated pages noticeably slower, which makes the
tweens look worse than they are.

![the welcome page](../screenshots/welcome.png)

## Opening a page directly

```text
cargo run --release -p showcase -- --page charts
cargo run --release -p showcase -- --page inputs --theme nord
cargo run --release -p showcase -- --help          # lists the theme names
```

`--page` matches the page title in lower case (`big menus`, `ai tools`, `ai agents` need
quoting in most shells). `--theme` takes any built-in palette name.

## Keys

| Key | Action |
|---|---|
| `]` `[` | Next and previous page |
| `alt+1` to `alt+9` | Jump to a page by number |
| `Ctrl-P` | Command palette (fuzzy page search) |
| `Ctrl-T` | Cycle to the next theme |
| `Ctrl-B` | Toggle the sidebar |
| `F1` | Help overlay |
| `F3` | Toggle reduced motion |
| `Tab` `Shift-Tab` | Move focus inside the page |
| `Esc` | Dismiss all toasts |
| `q` or `Ctrl-C` | Quit |

Individual pages add their own keys, listed in the footer of each page. The Notifications
page fires a different toast style on each of `1` to `9` and `0`; the Big menus page cycles
fonts on `f`, styles on `s` and gradients on `g`; the Loading page pauses on space and
changes speed with `+` and `-`.

Every page is mouse-driven as well. Clicking a widget focuses it, wheel scrolls the thing
under the pointer, and dragging works on sliders, split panes and table column edges.

## The pages

| Page | What it demonstrates |
|---|---|
| Welcome | The library's own front page, big text and a card grid |
| Dashboard | Every family composed into one realistic ops screen |
| Monitor | A btop-style monitor: LED meters, dot-field graphs, mirrored network graph, process tree |
| Controls | Buttons, checkboxes, switches, radios, segmented controls, sliders, steppers, ratings |
| Inputs | Text fields, text areas, selects, comboboxes, multi-selects, all eight field shapes |
| Navigation | Tab bars in five styles, lists, trees, menus, breadcrumbs, paginator |
| Big menus | Four big fonts, ten menu styles, gradients |
| Tables | Sortable data table, cell navigation, key-value lists, big digits |
| Charts | Every chart type with live data |
| Feedback | Progress, spinners, modals, callouts, tooltips, the command palette |
| Notifications | Ten toast styles, six positions, the inbox, banners, inline alerts |
| Loading | Twenty loader styles, skeletons, the loading overlay |
| Spinners | The whole 102-entry catalog, filterable, with the one-liner for each |
| AI | A replayed agent turn: thinking, markdown, tool calls, streaming, approvals |
| AI Tools | Tool timeline, shell and code blocks, edit preview, change set, JSON tree |
| AI Agents | Agent tree and lanes, token and cost meters, context map, compaction, sessions, models |
| AI Composer | Slash commands, mentions, attachments, mode badge, status line, questions, plan, queue |
| Layout | Split panes, scroll views, panels, collapsibles, headers and footers |
| Content | Labels, badges, markup, markdown, logs, calendars, colour pickers, timelines |
| Settings | A complete preferences form in about 300 lines |
| Options | An omp-style settings screen: icon tabs, group sidebar, option list, live preview |
| Themes | All twelve palettes side by side with a live primary-hue override |

## Reading the source

Each page is `showcase/src/pages/<name>.rs` and implements the same small `Page` trait. They
are written to be read: no shared helpers beyond the library itself, no cleverness, and the
layout code sits next to the widget calls it feeds.

If you want to know how a particular screenshot in this book was produced, the page file has
the answer.

## Headless screenshots

The screenshots in this book are captured by a script, not by hand:

```text
cargo xtask shot -s 130x42 -o out.png -- ./target/release/showcase --page charts
cargo xtask shot -s 90x28 -k "Tab Tab Enter" -o out.png -- ./target/release/showcase --page inputs
```

It runs the binary in a private tmux server at an exact size, sends the keys you list, and
rasterises the captured ANSI to a PNG. The [testing](../recipes/testing.md) recipe explains
how to use the same script in CI for your own app.
