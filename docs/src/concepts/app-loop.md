# The app loop

`run(&mut app)` is a plain loop over crossterm events. There is no executor, no reactor and
no background thread. Understanding what it does per iteration is enough to predict your
app's CPU use and to decide whether you want it at all.

## What run does

```text
enter raw mode + alternate screen, install the panic hook
enable mouse capture       unless RunOptions { mouse: false }

loop {
    app.update(now)        your per-frame bookkeeping
    terminal.draw(..)      calls app.draw(frame, now)
    wait for an event      timeout depends on app.animating(now)
    app.event(ev, now)     for each event that arrived; Flow::Quit breaks
}

disable mouse capture
leave the alternate screen, restore the terminal mode
```

The wait is the interesting part. When `animating()` returns true the timeout is
`1/fps`, so the loop redraws 60 times a second. When it returns false the timeout is
`idle_redraw` (250 ms by default), enough for a clock or a blinking cursor without burning a
core. With `idle_redraw: None` the loop blocks until the user does something.

## RunOptions

```rust,no_run
# extern crate tuile;
# use tuile::prelude::*;
# struct MyApp;
# impl App for MyApp {
#     fn draw(&mut self, _f: &mut Frame, _n: Instant) {}
#     fn event(&mut self, _e: Event, _n: Instant) -> Flow { Flow::Continue }
# }
# fn main() -> std::io::Result<()> {
run_with(&mut MyApp, RunOptions {
    mouse: true,
    fps: 60,
    idle_redraw: Some(std::time::Duration::from_millis(250)),
})
# }
```

Lower `fps` if you are running over a slow SSH link: 30 looks fine for most tweens and halves
the bytes written. Raising it above 60 gains nothing; terminals do not repaint faster.

## animating: the one method to get right

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct MyApp { dark: SwitchState, save: ButtonState, toasts: Toaster }
# impl MyApp {
fn animating(&self, now: Instant) -> bool {
    self.dark.animating(now) || self.save.animating(now) || self.toasts.animating(now)
}
# }
# fn main() {}
```

Every widget that can move has an `animating(now)` method that reports whether it is still
moving. Or them together. Forget one and its animation freezes halfway until the next
keystroke; return `true` unconditionally and an idle screen costs a core.

Widgets that loop forever (spinners, marquees, shimmer skeletons) animate for as long as they
are on screen. If your app shows one permanently, it will redraw at `fps` permanently, which
is correct and is what the gallery does. Hide the spinner when there is nothing to wait for.

## update

`update(now)` runs before every draw, including idle redraws. It is where time-driven state
that is not a tween belongs:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct MyApp { toasts: Toaster }
# impl MyApp {
fn update(&mut self, now: Instant) {
    self.toasts.tick(now);          // expire toasts whose timeout elapsed
}
# }
# fn main() {}
```

It is also where you drain results from a worker thread. tuile has no opinion about
concurrency: spawn a thread, send over a channel, and pick up whatever arrived in `update`.
Set a flag so `animating` stays true while a request is in flight if you are showing a
spinner.

## Not using the runtime

Nothing in the library requires `App`. Every widget is `render(area, buf, &mut state)`, so
any loop that can produce a `Buffer` works, including an existing ratatui app:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# fn demo(frame: &mut Frame, table: &mut DataTableState) {
// inside your own terminal.draw(|frame| { .. })
let area = frame.area();
let buf = frame.buffer_mut();
let cols = vec![TableColumn::new("Host"), TableColumn::new("Status")];
let rows = vec![TableRow::from(vec!["web-01", "up"])];
DataTable::new(cols, rows).render(area, buf, table);
# }
# fn main() {}
```

Two things the runtime does that you then own: passing a consistent `Instant` to every
animated widget in a frame (use one `Instant::now()` at the top of your draw, not one per
widget), and restoring the terminal when your app panics.

## Panic safety

`run` installs a panic hook that restores the terminal before the message prints. Without it, a
panic inside raw mode leaves the user typing into a shell that echoes nothing, and with mouse
capture still on the terminal keeps emitting escape sequences on every mouse move. The hook
undoes mouse capture, raw mode and the alternate screen, in that order.

If you build your own loop, install an equivalent hook. `ratatui::init()` and
`ratatui::restore()` cover raw mode and the alternate screen but know nothing about mouse
capture, so disable that yourself.

## Quitting

Return `Flow::Quit` from `event`. The loop breaks, mouse capture is disabled, the alternate
screen is left and `run` returns `Ok(())`. There is no separate teardown hook: run your own
cleanup after `run` returns.

```rust,no_run
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct MyApp;
# impl App for MyApp {
#     fn draw(&mut self, _f: &mut Frame, _n: Instant) {}
# fn event(&mut self, ev: Event, _now: Instant) -> Flow {
    match ev {
        Event::Key(k) if ctrl(&k, 'c') => Flow::Quit,
        Event::Key(k) if k.code == KeyCode::Char('q') => Flow::Quit,
        _ => Flow::Continue,
    }
# }
# }
fn main() -> std::io::Result<()> {
    let result = run(&mut MyApp);
    println!("goodbye");        // prints to the restored terminal
    result
}
```
