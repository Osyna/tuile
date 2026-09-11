# Animation

Animation in tuile is a pure function of time. Nothing runs on a timer, nothing mutates in
the background. A widget is handed an `Instant` and computes where it should be at that
instant. That single decision is why animated widgets are testable and why screenshots are
reproducible.

Two clocks exist, and picking the right one is the whole API.

## One-shot animations: Tween

A `Tween` interpolates a `f32` from where it is now to a target over a duration. Widgets use
it for switch thumbs, progress bars, sliding underlines and expanding panels.

```rust
# extern crate tuile;
# use tuile::prelude::*;
# use std::time::Duration;
# fn main() {
let now = Instant::now();
let mut t = Tween::new(0.0);

t.go(1.0, now, Duration::from_millis(180));
assert_eq!(t.target(), 1.0);
assert!(t.active(now));

let done = now + Duration::from_millis(200);
assert_eq!(t.value(done), 1.0);
assert!(!t.active(done));
# }
```

`go` starts from the current value, not from the previous start value. Interrupting a tween
halfway therefore never jumps: flicking a switch back and forth reverses smoothly from
wherever the thumb happens to be.

| Method | Effect |
|---|---|
| `new(v)` / `fixed(v)` | A tween parked at `v` |
| `with_easing(e)` | Builder-style easing, default `InOutCubic` |
| `go(to, now, dur)` | Retarget from the current value; `dur` of zero jumps |
| `go_with(to, now, dur, easing)` | Retarget and change the curve for this leg |
| `set(v)` | Jump with no animation |
| `value(now)` | Where it is at `now` |
| `target()` | Where it is heading |
| `active(now)` | Still moving |
| `progress(now)` | 0 to 1 for the current leg |

Eight easing curves: `Linear`, `InOutCubic` (default), `OutCubic`, `InCubic`, `InOutSine`,
`OutBack`, `OutBounce`, `OutElastic`. `OutBack` overshoots slightly and settles, which suits
a small element appearing; `OutElastic` overshoots a lot and is worth using once per app at
most.

## Looping animations: the shared epoch

Spinners, shimmers, marquees and pulses do not have a start and an end. They sample a
process-wide epoch, so every looping animation on screen stays in phase without your app
tracking a start time:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Spinner::new(&spinners::DOTS).now(now).render(area, buf);
# }
# fn main() {}
```

`anim::since(now)` is the seconds elapsed since `anim::EPOCH`, which is fixed the first time
it is read. Three helpers sample it:

| Function | Returns |
|---|---|
| `pulse(elapsed, period)` | A smooth 0 to 1 to 0 oscillation |
| `blink(elapsed, period)` | `true` for the first half of each period |
| `frame_index(elapsed, fps, frames)` | The frame to show in a looping sequence |

## now versus elapsed

Animated builders take one of two calls, and the difference matters:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Spinner::new(&spinners::DOTS).now(now).render(area, buf);        // phase from the epoch
Spinner::new(&spinners::DOTS).elapsed(1.25).render(area, buf);   // exactly 1.25s in
# }
# fn main() {}
```

`.now(instant)` is what an app passes. `.elapsed(secs)` pins the animation to an exact point
and is what tests and the screenshot tool use, because it makes rendering deterministic: the
same `elapsed` always produces the same cells.

If you take a screenshot of an animated widget without pinning it, the image changes between
runs and any test comparing rendered output will flake.

## Telling the runtime you are moving

The loop only redraws at 60 fps while `App::animating` returns true:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# struct MyApp { switch: SwitchState, progress: ProgressState }
# impl MyApp {
fn animating(&self, now: Instant) -> bool {
    self.switch.animating(now) || self.progress.animating(now)
}
# }
# fn main() {}
```

Every state that can move answers `animating(now)`. The [app loop](app-loop.md) chapter
covers what happens when you get this wrong in either direction.

## Reduced motion

Some users cannot tolerate movement, and some terminals over a slow link cannot keep up with
it. The library has no global switch for this; the honest answer is that you implement it by
choosing not to animate. In practice that means passing a fixed `elapsed`, or skipping the
`.now(..)` call so the widget renders its resting state, and setting durations to zero so
tweens jump.

The showcase does this on `F3` and its page code is a working reference for the pattern.

## One instant per frame

Sample `Instant::now()` once per frame and pass the same value everywhere. Calling it per
widget means the widgets in a frame disagree about the time by microseconds, which is
invisible for a tween but makes multi-part animations (a row of bars, a wave) shear.

The `App::draw` signature hands you the frame instant for exactly this reason.
