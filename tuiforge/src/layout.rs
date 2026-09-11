//! Small layout helpers on top of `ratatui::layout`: centring, padding, fixed rows/columns,
//! and an `Overlay` queue for things drawn after everything else (dropdowns, tooltips).

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// A `w`×`h` rectangle centred in `area` (clamped to fit).
pub fn center(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// Horizontal centring only, full height.
pub fn center_h(area: Rect, w: u16) -> Rect {
    let w = w.min(area.width);
    Rect {
        x: area.x + (area.width - w) / 2,
        width: w,
        ..area
    }
}

/// Shrink by `x` cells left/right and `y` cells top/bottom.
pub fn pad(area: Rect, x: u16, y: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(x),
        y: area.y.saturating_add(y),
        width: area.width.saturating_sub(x * 2),
        height: area.height.saturating_sub(y * 2),
    }
}

/// Shrink with individual sides (top, right, bottom, left), CSS order.
pub fn pad_trbl(area: Rect, t: u16, r: u16, b: u16, l: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(l),
        y: area.y.saturating_add(t),
        width: area.width.saturating_sub(l + r),
        height: area.height.saturating_sub(t + b),
    }
}

/// Split into rows of the given heights; the last `Constraint::Fill` takes the rest.
pub fn rows<const N: usize>(area: Rect, constraints: [Constraint; N]) -> [Rect; N] {
    Layout::vertical(constraints).areas(area)
}

pub fn cols<const N: usize>(area: Rect, constraints: [Constraint; N]) -> [Rect; N] {
    Layout::horizontal(constraints).areas(area)
}

/// Fixed-height rows with `gap` between them; the last one takes the remainder.
pub fn stack(area: Rect, heights: &[u16], gap: u16) -> Vec<Rect> {
    let mut out = Vec::with_capacity(heights.len());
    let mut y = area.y;
    for (i, &h) in heights.iter().enumerate() {
        let last = i + 1 == heights.len();
        let avail = area.bottom().saturating_sub(y);
        let h = if last && h == 0 { avail } else { h.min(avail) };
        out.push(Rect {
            x: area.x,
            y,
            width: area.width,
            height: h,
        });
        y = y.saturating_add(h + gap);
    }
    out
}

/// Equal columns with `gap` between them.
pub fn columns(area: Rect, n: usize, gap: u16) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let total_gap = gap * (n as u16 - 1);
    let w = area.width.saturating_sub(total_gap) / n as u16;
    (0..n as u16)
        .map(|i| Rect {
            x: area.x + i * (w + gap),
            y: area.y,
            width: w,
            height: area.height,
        })
        .collect()
}

/// Grid of `cols`×`rows` cells with gaps.
pub fn grid(area: Rect, cols_n: usize, rows_n: usize, gap: u16) -> Vec<Vec<Rect>> {
    let rows_v = if rows_n == 0 {
        Vec::new()
    } else {
        let total_gap = gap * (rows_n as u16 - 1);
        let h = area.height.saturating_sub(total_gap) / rows_n as u16;
        (0..rows_n as u16)
            .map(|i| Rect {
                y: area.y + i * (h + gap),
                height: h,
                ..area
            })
            .collect::<Vec<_>>()
    };
    rows_v
        .into_iter()
        .map(|r| columns(r, cols_n, gap))
        .collect()
}

/// A flow layout: places fixed-size boxes left→right, wrapping to the next line.
pub fn flow(area: Rect, sizes: &[(u16, u16)], gap_x: u16, gap_y: u16) -> Vec<Rect> {
    let mut out = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut line_h) = (area.x, area.y, 0u16);
    for &(w, h) in sizes {
        if x > area.x && x + w > area.right() {
            x = area.x;
            y = y.saturating_add(line_h + gap_y);
            line_h = 0;
        }
        out.push(Rect {
            x,
            y,
            width: w.min(area.width),
            height: h,
        });
        x = x.saturating_add(w + gap_x);
        line_h = line_h.max(h);
    }
    out
}

/// Anchor a `w`×`h` popup next to `anchor`, preferring below, flipping above when it would
/// overflow `bounds`, and sliding horizontally to stay inside.
pub fn popup_below(anchor: Rect, w: u16, h: u16, bounds: Rect) -> Rect {
    let w = w.min(bounds.width);
    let h = h.min(bounds.height);
    let mut x = anchor.x;
    if x + w > bounds.right() {
        x = bounds.right().saturating_sub(w);
    }
    let below = anchor.bottom();
    let y = if below + h <= bounds.bottom() {
        below
    } else if anchor.y >= bounds.y + h {
        anchor.y - h
    } else {
        bounds.bottom().saturating_sub(h)
    };
    Rect {
        x: x.max(bounds.x),
        y: y.max(bounds.y),
        width: w,
        height: h,
    }
}

/// Deferred drawing queue: widgets push closures that must paint *over* everything else
/// (dropdown lists, tooltips, menus). Drain it at the end of the frame.
#[derive(Default)]
pub struct Overlay<'a> {
    // Boxed draw closures; the type is the documentation.
    #[allow(clippy::type_complexity)]
    layers: Vec<Box<dyn FnOnce(&mut Buffer) + 'a>>,
}

impl<'a> Overlay<'a> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, f: impl FnOnce(&mut Buffer) + 'a) {
        self.layers.push(Box::new(f));
    }
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
    pub fn draw(self, buf: &mut Buffer) {
        for l in self.layers {
            l(buf);
        }
    }
}

pub fn direction_split(area: Rect, dir: Direction, first: u16) -> (Rect, Rect) {
    match dir {
        Direction::Horizontal => {
            let first = first.min(area.width);
            (
                Rect {
                    width: first,
                    ..area
                },
                Rect {
                    x: area.x + first,
                    width: area.width - first,
                    ..area
                },
            )
        }
        Direction::Vertical => {
            let first = first.min(area.height);
            (
                Rect {
                    height: first,
                    ..area
                },
                Rect {
                    y: area.y + first,
                    height: area.height - first,
                    ..area
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_flips_and_slides() {
        let bounds = Rect::new(0, 0, 40, 10);
        let r = popup_below(Rect::new(30, 2, 5, 1), 20, 3, bounds);
        assert_eq!((r.x, r.y), (20, 3));
        let r = popup_below(Rect::new(0, 8, 5, 1), 10, 5, bounds);
        assert_eq!(r.y, 3);
        let f = flow(Rect::new(0, 0, 10, 10), &[(6, 1), (6, 1), (3, 2)], 1, 0);
        assert_eq!(f[1].y, 1);
        assert_eq!(f[2].x, 7);
        assert_eq!(center(bounds, 100, 2).width, 40);
    }
}
