//! # tuiforge
//!
//! Textual-grade building blocks for [ratatui]: a ported design system (themes → derived
//! colour roles), 40+ widgets with keyboard *and* mouse handling built in, tweened animation,
//! overlays (modals, toasts, palettes, dropdowns) and an app runtime that only redraws when
//! something moves.
//!
//! ```ignore
//! use tuiforge::prelude::*;
//!
//! struct Demo { on: SwitchState }
//!
//! impl App for Demo {
//!     fn draw(&mut self, frame: &mut Frame, now: Instant) {
//!         let area = center(frame.area(), 20, 3);
//!         Switch::new().now(now).focused(true).render(area, frame.buffer_mut(), &mut self.on);
//!     }
//!     fn event(&mut self, ev: Event, now: Instant) -> Flow {
//!         if let Event::Key(k) = &ev && k.code == KeyCode::Char('q') { return Flow::Quit; }
//!         self.on.now(now).handle(&ev);
//!         Flow::Continue
//!     }
//!     fn animating(&self, now: Instant) -> bool { self.on.animating(now) }
//! }
//! ```
//!
//! [ratatui]: https://ratatui.rs

#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod anim;
pub mod core;
pub mod draw;
pub mod fuzzy;
pub mod layout;
pub mod runtime;
pub mod theme;
pub mod widgets;

pub use ratatui;
pub use ratatui::crossterm;

/// Everything an app usually needs.
pub mod prelude {
    pub use std::time::{Duration, Instant};

    pub use ratatui::Frame;
    pub use ratatui::buffer::Buffer;
    pub use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    pub use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect};
    pub use ratatui::style::{Modifier, Style};
    pub use ratatui::text::{Line, Span, Text};
    pub use ratatui::widgets::{StatefulWidget, Widget};

    pub use crate::anim::{Clock, Easing, Tween, blink, elapsed, frame_index, pulse};
    pub use crate::core::*;
    pub use crate::draw::{Border, fill, put, put_centered, put_right, st, truncate, wrap};
    pub use crate::layout::{Overlay, center, center_h, columns, pad, popup_below, stack};
    pub use crate::runtime::{App, Flow, RunOptions, run, run_with};
    pub use crate::theme::{self, Rgb, Theme, ThemeSpec, Variant};
    pub use crate::widgets::*;
}
