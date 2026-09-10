//! The app runner: terminal setup/teardown (with panic-safe restore), mouse capture, an
//! event loop that only redraws at 60 fps while something animates, and a local clock.

use std::io::{self, stdout};
use std::process::Command;
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flow {
    #[default]
    Continue,
    Quit,
}

/// Implement this and call [`run`].
pub trait App {
    fn draw(&mut self, frame: &mut Frame, now: Instant);

    /// Key releases are filtered out already; everything else is forwarded.
    fn event(&mut self, ev: Event, now: Instant) -> Flow;

    /// Called once per loop before drawing (timers, tweens, async results).
    fn update(&mut self, _now: Instant) {}

    /// `true` while any tween/spinner is live → the loop redraws at [`RunOptions::fps`].
    fn animating(&self, _now: Instant) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RunOptions {
    pub mouse: bool,
    /// Redraw rate while [`App::animating`] is true.
    pub fps: u32,
    /// Redraw interval when idle (clocks, blinking cursors). `None` blocks until an event.
    pub idle_redraw: Option<Duration>,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions { mouse: true, fps: 60, idle_redraw: Some(Duration::from_millis(250)) }
    }
}

pub fn run<A: App>(app: &mut A) -> io::Result<()> {
    run_with(app, RunOptions::default())
}

pub fn run_with<A: App>(app: &mut A, opts: RunOptions) -> io::Result<()> {
    let mut terminal = ratatui::init();
    if opts.mouse {
        execute!(stdout(), EnableMouseCapture)?;
    }
    let result = event_loop(&mut terminal, app, opts);
    if opts.mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    result
}

fn event_loop<A: App>(terminal: &mut ratatui::DefaultTerminal, app: &mut A, opts: RunOptions) -> io::Result<()> {
    let frame = Duration::from_secs_f64(1.0 / opts.fps.max(1) as f64);
    loop {
        let now = Instant::now();
        app.update(now);
        terminal.draw(|f| app.draw(f, now))?;
        let wait = if app.animating(now) { frame } else { opts.idle_redraw.unwrap_or(Duration::from_secs(3600)) };
        if !event::poll(wait)? {
            continue;
        }
        // drain bursts (mouse moves, pastes) before redrawing
        loop {
            let ev = event::read()?;
            let skip = matches!(&ev, Event::Key(k) if k.kind == KeyEventKind::Release);
            if !skip && app.event(ev, Instant::now()) == Flow::Quit {
                return Ok(());
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
        }
    }
}

// ───────────────────────────── wall clock ─────────────────────────────

/// Local time as `(hours, minutes, seconds)` — no timezone crate, uses `date +%z` once.
pub fn local_hms() -> (u32, u32, u32) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + tz_offset_secs();
    let day = secs.rem_euclid(86_400) as u32;
    (day / 3600, day % 3600 / 60, day % 60)
}

/// Local calendar date `(year, month, day)`.
pub fn local_ymd() -> (i32, u32, u32) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + tz_offset_secs();
    civil_from_days(secs.div_euclid(86_400))
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + (m <= 2) as i64) as i32, m, d)
}

/// (year, month, day) → days since 1970-01-01.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = y as i64 - (m <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

static TZ_OFFSET: LazyLock<i64> = LazyLock::new(|| {
    // ponytail: `date +%z` instead of a timezone crate; UTC if unavailable.
    let out = Command::new("date").arg("+%z").output().ok();
    let s = out.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let s = s.trim();
    if s.len() != 5 {
        return 0;
    }
    let sign = if s.starts_with('-') { -1 } else { 1 };
    let h: i64 = s[1..3].parse().unwrap_or(0);
    let m: i64 = s[3..5].parse().unwrap_or(0);
    sign * (h * 3600 + m * 60)
});

fn tz_offset_secs() -> i64 {
    *TZ_OFFSET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28), 2);
    }
}
