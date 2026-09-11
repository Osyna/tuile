# Charts

Sparklines, graphs, heatmaps, gauges, and activity grids for visualizing time-series and 2D data. Every chart widget is stateless (render only, no event handling) and autoscales its data to fit the available area.

![charts](../screenshots/charts.png)

## SparkChart

A compact inline chart that shows the last N values from a slice. Width-limited: if there are more values than cells, only the most recent ones appear. Height scales automatically to the data range unless you set `.min()` and `.max()`.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let data = vec![10.0, 15.0, 22.0, 18.0, 30.0, 28.0, 35.0];
SparkChart::new(&data)
    .style(SparkStyle::Bars)
    .baseline(true)
    .show_last_value(true)
    .render(area, buf);
# }
# fn main() {}
```

NaN values in the slice are skipped (they do not occupy cells). An empty or all-NaN slice renders blank.

### Styles

| Style | Appearance |
|-------|------------|
| `Bars` | Block glyphs `▁▂▃▄▅▆▇█` (default) |
| `Line` | Braille line connecting values |
| `Area` | Braille line with dimmed fill below |
| `Field` | btop-style dot field: every dot below the value is lit |

`.gradient(&[color1, color2, ...])` interpolates across the width (Bars, Line, Area) or by height per column (Field). `.baseline(true)` draws a thin baseline at the bottom. `.mirrored(true)` hangs the graph from the top edge (upload half of a mirrored network graph).

The widget needs at least 1 column and 1 row. Typical heights: 1 row for Bars/Field, 2 or more for Line/Area.

## BarGraph

Grouped vertical or horizontal bars. Each `BarGroup` has a label and a vector of values (one per series).

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let groups = vec![
    BarGroup {
        label: "Q1".into(),
        values: vec![30.0, 45.0, 25.0],
    },
    BarGroup {
        label: "Q2".into(),
        values: vec![35.0, 50.0, 30.0],
    },
];
BarGraph::new(&groups)
    .series_names(&["APAC", "EMEA", "AMER"])
    .show_values(true)
    .bar_width(4)
    .gap(1)
    .render(area, buf);
# }
# fn main() {}
```

Values scale to the tallest bar unless you set `.max(value)`. `.horizontal(true)` draws bars left to right. `.show_values(true)` prints each bar's value at its end. `.axis(false)` hides the baseline and tick marks. `.colors(&[rgb1, rgb2, ...])` sets series colors (defaults cycle through theme variants).

A bar group with an empty `values` vector renders as blank space. NaN values are skipped within a group.

The widget needs at least 5 columns and 3 rows for vertical bars, or 5 rows and 10 columns for horizontal bars with labels.

## LineGraph

Multi-series line chart with axes, grid, and legend. Each `LineSeries` has a name, points `(x, y)`, optional color, and a `LineStyle`.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let series = vec![
    LineSeries {
        name: "CPU".into(),
        points: vec![(0.0, 20.0), (1.0, 35.0), (2.0, 50.0), (3.0, 65.0)],
        color: None,
        style: LineStyle::Line,
    },
    LineSeries {
        name: "Memory".into(),
        points: vec![(0.0, 10.0), (1.0, 25.0), (2.0, 40.0), (3.0, 55.0)],
        color: None,
        style: LineStyle::Area,
    },
];
LineGraph::new(&series)
    .grid(true)
    .legend(LegendPos::TopRight)
    .x_labels(5)
    .y_labels(5)
    .render(area, buf);
# }
# fn main() {}
```

Axes and grid are drawn in braille. `.x_bounds(min, max)` and `.y_bounds(min, max)` fix the range (defaults autoscale to the data). `.label_fmt(|v| format!("{v:.1}"))` customizes tick labels. `.grid(true)` draws a faint grid. `.legend(LegendPos::Hidden)` removes the legend. `.title("Title")` centers a title above the graph.

| LineStyle | Rendering |
|-----------|-----------|
| `Line` | Braille line (default) |
| `Area` | Braille line with dimmed fill below |
| `Points` | Braille dots at each point |

An empty series or a series with fewer than two points renders blank. NaN coordinates are skipped (creating gaps in Line/Area styles).

The widget needs at least 12 columns and 6 rows for a readable chart with axes.

## ScatterPlot

Thin wrapper over `LineGraph` that forces every series to `LineStyle::Points`. Same API as `LineGraph` but without `.grid()`, `.legend()`, or `.title()`.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let series = vec![
    LineSeries {
        name: "A".into(),
        points: vec![(0.0, 10.0), (1.0, 20.0), (2.0, 15.0)],
        color: None,
        style: LineStyle::Points,
    },
];
ScatterPlot::new(&series)
    .x_bounds(0.0, 3.0)
    .y_bounds(0.0, 30.0)
    .render(area, buf);
# }
# fn main() {}
```

Same scaling and NaN behavior as `LineGraph`. Minimum area: 8 columns and 4 rows.

## Heatmap

2D grid of colored cells. Values are `&[Vec<f64>]` (rows, then columns within each row). Each cell is painted with a background color interpolated from the gradient stops.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let data = vec![
    vec![10.0, 20.0, 30.0],
    vec![15.0, 25.0, 35.0],
];
let gradient = [Rgb(50, 100, 200), Rgb(200, 100, 50)];
Heatmap::new(&data)
    .row_labels(&["Mon", "Tue"])
    .col_labels(&["A", "B", "C"])
    .gradient(&gradient)
    .show_values(true)
    .render(area, buf);
# }
# fn main() {}
```

`.gradient(&[rgb1, rgb2, ...])` maps values from the data range to colors. `.null_color(rgb)` sets the color for NaN cells (default dark gray). `.cell_width(w)` controls cell width in columns (default 2). `.show_values(true)` prints each value centered in its cell. `.legend(true)` draws a gradient bar at the bottom.

NaN values render with the null color. Rows with different lengths are padded with NaN. An empty or all-NaN dataset renders blank.

Minimum area: `(cols * cell_width) + (row_labels.len() as u16 * 2)` columns, `rows + 2` rows (for labels). Compact mode (cell_width 1) fits more data but values become unreadable.

## ActivityGraph

GitHub-style contribution grid: 7 rows (days) and up to 52 columns (weeks). Values are `&[u8]` where each byte is a 0 to 4 intensity level for one day. Up to 364 days (52 weeks) are shown.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut days = vec![0u8; 364];
for (i, v) in days.iter_mut().enumerate() {
    *v = ((i * 7 + i / 7) % 5) as u8;
}
ActivityGraph::new(&days).render(area, buf);
# }
# fn main() {}
```

Each cell is 2 columns wide (painted background plus a space for separation). Weeks are columns, days of the week are rows (Monday at the top). `.levels(&[rgb0, rgb1, rgb2, rgb3, rgb4])` sets the 5 colors (defaults to theme shades). Values outside 0..=4 are clamped.

An empty slice renders blank. Minimum area: 14 columns (7 weeks), 7 rows.

## Meter

Horizontal gauge showing a 0.0 to 1.0 value with optional label, percent, and suffix.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
Meter::new()
    .value(0.73)
    .label("CPU:")
    .show_percent(true)
    .suffix("8 cores")
    .style(MeterStyle::Blocks)
    .render(area, buf);
# }
# fn main() {}
```

| Method | Effect |
|--------|--------|
| `.value(f32)` | 0.0 to 1.0 (clamped) |
| `.label(s)` | Left label |
| `.suffix(s)` | Right label after the bar |
| `.show_percent(bool)` | Print percent inside or after the bar |
| `.thresholds(&[(f32, Variant)])` | Color by value ranges (e.g. green below 0.7, red above 0.9) |
| `.color(rgb)` | Fixed bar color (overrides thresholds unless a gradient is set) |
| `.gradient(&[rgb, ...])` | Interpolate across the bar (LED styles color each cell by position, solid styles by value) |
| `.compact(bool)` | No label spacing (for tight layouts) |

### Meter styles

| Style | Appearance |
|-------|------------|
| `Line` | Horizontal line `━━━━╺━━` (default) |
| `Block` | Solid painted fill |
| `Segments(n)` | n segments with gaps |
| `Blocks` | btop-style LED row: `■■■■□□` (empties dimmed) |
| `Dots` | Compact LED row: `●●●◌◌` |

Thresholds take the highest matching `(threshold_value, variant)` pair. A gradient overrides both color and thresholds. Gradients on LED styles (Blocks, Dots) color each cell by its position along the bar, not by the meter's value.

Minimum area: 10 columns for a readable bar, 1 row.

## RadialGauge

Experimental semicircular gauge drawn with braille arcs. Shows a 0.0 to 1.0 value as a sweep from the left to right end of a half-circle.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
RadialGauge::new()
    .value(0.65)
    .label("Speed")
    .thickness(2)
    .render(area, buf);
# }
# fn main() {}
```

`.thickness(u16)` sets the arc width in braille dot rows (default 2). `.label(s)` prints centered text below the gauge.

This widget may be removed if the braille arc does not look good in all terminals. Minimum area: 12 columns, 6 rows.

## BrailleCanvas

Low-level 2×4 dot rasterizer used internally by `LineGraph` and `ScatterPlot`. Each terminal cell holds 8 braille dots arranged in a 2-column by 4-row grid.

```text
⠁ ⠈  (dots 0-3 left column, 4-7 right column, bottom to top)
⠂ ⠐
⠄ ⠠
⡀ ⢀
```

The braille unicode code is U+2800 plus a bit pattern where bit 0 is the top-left dot and bit 7 is the bottom-right. `BrailleCanvas::new(width, height)` creates a canvas where width and height are in braille dots (multiply terminal cell dimensions by 2 and 4). `.set_dot(x, y, color)` lights a dot. `.line(x1, y1, x2, y2, color)` draws a line. `.render(area, buf)` writes the result.

Most users do not call this directly. `LineGraph` and `ScatterPlot` use it to draw high-resolution plots.

## Gradients

Two gradient helpers are available:

| Helper | Use |
|--------|-----|
| `gradient_stops(&[rgb, ...])` | Builder method on widgets: interpolates colors across the data range or width |
| `color_gradient(stops, t)` | Free function: samples a gradient at position t (0.0 to 1.0) |

`color_gradient` linearly interpolates RGB components between stops. With two stops, it is a linear blend. With three or more, it finds the pair bracketing t and blends within that segment.

```rust
# extern crate tuile;
# use tuile::theme::{Rgb, gradient as color_gradient};
# fn demo() {
let stops = [Rgb(0, 255, 0), Rgb(255, 255, 0), Rgb(255, 0, 0)];
let low = color_gradient(&stops, 0.0);
let mid = color_gradient(&stops, 0.5);
let high = color_gradient(&stops, 1.0);
# }
# fn main() {}
```

Widgets that accept `.gradient(&[rgb, ...])` use this function internally to map data or position to color. One-stop gradients are solid (all values get that color). Zero-stop gradients fall back to the theme primary color.

## When not to use a chart widget

Use `Table` or `ListView` when the data is tabular with row labels, or when you need selection and scrolling. Use `BigText` for large single numbers. Use `Meter` for a single gauge, not a bar chart with one bar. For custom shapes or plots that do not fit these widgets, draw directly into the buffer or compose a `BrailleCanvas`.
