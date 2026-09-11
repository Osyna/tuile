//! # tuile
//!
//! Textual-grade building blocks for [ratatui]: a ported design system (themes to derived
//! colour roles), 108 widget types with keyboard and mouse handling built in, tweened
//! animation, overlays (modals, toasts, palettes, dropdowns), AI/LLM chat components, a
//! 102-spinner catalog and an app runtime that only redraws when something moves.
//!
//! ## Import
//!
//! ```toml
//! [dependencies]
//! tuile = { git = "https://github.com/irvin/tuile" }
//! ```
//!
//! `use tuile::prelude::*;` is the one import: every widget, `Theme`/`theme`, `draw`/`layout`
//! helpers, `anim`, and the ratatui + crossterm types you touch (`Rect`, `Buffer`, `Frame`,
//! `Event`, `KeyCode`…). The crates themselves are re-exported as [`ratatui_core`] and
//! [`crossterm`] so an app needs no version pins of its own.
//!
//! The dependency is [`ratatui-core`][ratatui], not the `ratatui` facade: upstream splits the
//! project so that applications use `ratatui` and widget libraries use `ratatui-core`. The types
//! are identical, so these widgets render into a `Frame` from a `ratatui` app unchanged.
//!
//! ```no_run
//! use tuile::prelude::*;
//! use std::time::Instant;
//!
//! struct Demo { on: SwitchState }
//!
//! impl App for Demo {
//!     fn draw(&mut self, frame: &mut Frame, now: Instant) {
//!         let area = center(frame.area(), 20, 3);
//!         Switch::new().now(now).focused(true).render(area, frame.buffer_mut(), &mut self.on);
//!     }
//!     fn event(&mut self, ev: Event, now: Instant) -> Flow {
//!         if let Event::Key(k) = &ev { if k.code == KeyCode::Char('q') { return Flow::Quit; } }
//!         self.on.handle(&ev);
//!         Flow::Continue
//!     }
//!     fn animating(&self, now: Instant) -> bool { self.on.animating(now) }
//! }
//! ```
//!
//! ## Reuse and customize
//!
//! Every widget is a **builder** (per-frame configuration, consumed by `render`) plus a
//! **`<Name>State`** (values, hover, tweens, scroll: plain `Clone + Debug` data with public
//! fields) that implements [`core::Interactive`]: `state.handle(&event)` → [`core::Outcome`].
//!
//! * **Theme.** [`theme::set_by_name`] / [`theme::set`] switch every widget at once;
//!   `.theme(&Theme)` overrides one. Build palettes with [`theme::ThemeSpec::new`] and
//!   [`theme::Theme::resolve`]; every derived role (`text_muted`, `border_blurred`, `cursor_bg`…)
//!   is a public `Rgb` field.
//! * **Time.** Pass the frame `Instant` to `.now(..)`. Looping animations phase from
//!   [`anim::EPOCH`]; `.elapsed(secs)` makes them deterministic for tests and screenshots.
//! * **Content.** Builders borrow (`&str`, `&[T]`, `&'static SpinnerDef`); nothing is copied
//!   until it is drawn. Custom spinners: `SpinnerDef::new("name", ms, &frames)` or
//!   `Spinner::frames(&frames, ms)`. Custom looks: [`draw::Border`] styles, [`theme::Variant`]s.
//! * **Extend.** New widgets use the same public primitives ([`draw`], [`layout`], [`anim`],
//!   [`core::HitBox`], [`core::Focus`]); the contract is at <https://osyna.github.io/tuile/reference/widget-contract.html>.
//!
//! [ratatui]: https://ratatui.rs

// A terminal widget library has no reason to reach for unsafe; the crate contains none.
#![forbid(unsafe_code)]

pub mod anim;
pub mod core;
pub mod draw;
pub mod fuzzy;
pub mod layout;
pub mod runtime;
pub mod theme;
pub mod widgets;

pub use crossterm;
pub use ratatui_core;

/// The `ratatui-crossterm` backend, for apps that build their own [`Terminal`].
///
/// [`Terminal`]: ratatui_core::terminal::Terminal
pub use ratatui_crossterm;

/// Everything an app usually needs.
pub mod prelude {
    pub use std::time::{Duration, Instant};

    pub use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    pub use ratatui_core::buffer::Buffer;
    pub use ratatui_core::layout::{
        Alignment, Constraint, Direction, Layout, Margin, Position, Rect,
    };
    pub use ratatui_core::style::{Modifier, Style};
    pub use ratatui_core::terminal::Frame;
    pub use ratatui_core::text::{Line, Span, Text};
    pub use ratatui_core::widgets::{StatefulWidget, Widget};

    pub use crate::anim::{self, Clock, Easing, Tween, blink, elapsed, frame_index, pulse, since};
    pub use crate::core::*;
    pub use crate::draw::{
        Border, Edge, FieldShape, fill, put, put_centered, put_right, st, truncate, wrap,
    };
    pub use crate::layout::{Overlay, center, center_h, columns, pad, popup_below, stack};
    pub use crate::runtime::{App, Flow, RunOptions, run, run_with};
    pub use crate::theme::{self, Rgb, Theme, ThemeSpec, Variant};
    pub use crate::widgets::*;
}
