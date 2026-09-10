//! Time-based animation: easing curves, retargetable tweens, pulses and blinks.
//! Everything samples an explicit `Instant` so rendering stays pure and testable.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
    Linear,
    #[default]
    InOutCubic,
    OutCubic,
    InCubic,
    InOutSine,
    OutBack,
    OutBounce,
    OutElastic,
}

impl Easing {
    pub fn apply(self, p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Easing::Linear => p,
            Easing::InOutCubic => {
                if p < 0.5 { 4.0 * p * p * p } else { 1.0 - (-2.0 * p + 2.0).powi(3) / 2.0 }
            }
            Easing::OutCubic => 1.0 - (1.0 - p).powi(3),
            Easing::InCubic => p * p * p,
            Easing::InOutSine => -((std::f32::consts::PI * p).cos() - 1.0) / 2.0,
            Easing::OutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (p - 1.0).powi(3) + c1 * (p - 1.0).powi(2)
            }
            Easing::OutBounce => {
                let (n1, d1) = (7.5625, 2.75);
                if p < 1.0 / d1 {
                    n1 * p * p
                } else if p < 2.0 / d1 {
                    let p = p - 1.5 / d1;
                    n1 * p * p + 0.75
                } else if p < 2.5 / d1 {
                    let p = p - 2.25 / d1;
                    n1 * p * p + 0.9375
                } else {
                    let p = p - 2.625 / d1;
                    n1 * p * p + 0.984375
                }
            }
            Easing::OutElastic => {
                if p == 0.0 || p == 1.0 {
                    p
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    2f32.powf(-10.0 * p) * ((p * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
        }
    }
}

pub fn ease_in_out_cubic(p: f32) -> f32 {
    Easing::InOutCubic.apply(p)
}

pub fn ease_out_cubic(p: f32) -> f32 {
    Easing::OutCubic.apply(p)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A retargetable scalar animation. `go` starts from the *current* value, so interrupting an
/// animation mid-flight never jumps.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    dur: Duration,
    easing: Easing,
}

impl Tween {
    pub fn new(v: f32) -> Self {
        Self { from: v, to: v, start: Instant::now(), dur: Duration::ZERO, easing: Easing::default() }
    }

    /// Alias kept for readability at call sites: a tween parked at `v`.
    pub fn fixed(v: f32) -> Self {
        Self::new(v)
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Retarget from the current position; `dur == 0` jumps.
    pub fn go(&mut self, to: f32, now: Instant, dur: Duration) {
        let from = self.value(now);
        self.from = from;
        self.to = to;
        self.start = now;
        self.dur = dur;
    }

    /// Retarget with a specific easing for this leg.
    pub fn go_with(&mut self, to: f32, now: Instant, dur: Duration, easing: Easing) {
        self.easing = easing;
        self.go(to, now, dur);
    }

    /// Jump immediately, no animation.
    pub fn set(&mut self, v: f32) {
        self.from = v;
        self.to = v;
        self.dur = Duration::ZERO;
    }

    pub fn value(&self, now: Instant) -> f32 {
        if self.dur.is_zero() {
            return self.to;
        }
        let p = now.saturating_duration_since(self.start).as_secs_f32() / self.dur.as_secs_f32();
        lerp(self.from, self.to, self.easing.apply(p))
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn active(&self, now: Instant) -> bool {
        now < self.start + self.dur
    }

    /// 0..1 progress of the current leg.
    pub fn progress(&self, now: Instant) -> f32 {
        if self.dur.is_zero() {
            1.0
        } else {
            (now.saturating_duration_since(self.start).as_secs_f32() / self.dur.as_secs_f32()).clamp(0.0, 1.0)
        }
    }
}

impl Default for Tween {
    fn default() -> Self {
        Tween::new(0.0)
    }
}

/// Seconds since `epoch`; the raw clock most looping animations sample.
pub fn elapsed(epoch: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(epoch).as_secs_f32()
}

/// Process-wide epoch for looping animations (spinners, shimmers, marquees). Widgets that take
/// only `.now(instant)` measure their phase from here, so they animate without an app-owned
/// start time. Fixed on first use.
pub static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Seconds between [`EPOCH`] and `now`: the phase clock for looping animations.
pub fn since(now: Instant) -> f32 {
    elapsed(*EPOCH, now)
}

/// Smooth 0→1→0 oscillation with the given period in seconds.
pub fn pulse(elapsed: f32, period: f32) -> f32 {
    if period <= 0.0 {
        return 1.0;
    }
    0.5 - 0.5 * (elapsed / period * std::f32::consts::TAU).cos()
}

/// Cursor-style blink: `true` for the first half of every period.
pub fn blink(elapsed: f32, period: f32) -> bool {
    if period <= 0.0 {
        return true;
    }
    (elapsed / period).fract() < 0.5
}

/// Frame index for a looping sequence of `frames` at `fps`.
pub fn frame_index(elapsed: f32, fps: f32, frames: usize) -> usize {
    if frames == 0 {
        return 0;
    }
    ((elapsed * fps) as usize) % frames
}

/// A wall clock a widget can keep instead of threading `Instant::now()` everywhere.
/// Also records a `started` epoch for looping animations.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub started: Instant,
    pub now: Instant,
}

impl Default for Clock {
    fn default() -> Self {
        let n = Instant::now();
        Clock { started: n, now: n }
    }
}

impl Clock {
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
    }
    pub fn elapsed(&self) -> f32 {
        elapsed(self.started, self.now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_eases_between_targets_and_retargets_smoothly() {
        let t0 = Instant::now();
        let mut t = Tween::new(0.0);
        t.go(1.0, t0, Duration::from_millis(200));
        let mid = t.value(t0 + Duration::from_millis(100));
        assert!(mid > 0.4 && mid < 0.6);
        // retarget half-way: must start from the mid value, not from 0 or 1
        t.go(0.0, t0 + Duration::from_millis(100), Duration::from_millis(200));
        let v = t.value(t0 + Duration::from_millis(100));
        assert!((v - mid).abs() < 1e-4);
        assert_eq!(t.value(t0 + Duration::from_secs(5)), 0.0);
        assert!(!t.active(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn easings_hit_endpoints() {
        for e in [
            Easing::Linear,
            Easing::InOutCubic,
            Easing::OutCubic,
            Easing::InCubic,
            Easing::InOutSine,
            Easing::OutBack,
            Easing::OutBounce,
            Easing::OutElastic,
        ] {
            assert!((e.apply(0.0)).abs() < 1e-5, "{e:?} start");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-4, "{e:?} end");
        }
        assert!(blink(0.1, 1.0));
        assert!(!blink(0.6, 1.0));
        assert_eq!(frame_index(1.0, 10.0, 4), 2);
    }
}
