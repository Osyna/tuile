//! Every widget, re-exported flat: `use tuiforge::widgets::*;`.
//!
//! Conventions shared by all of them:
//! * builders are cheap values consumed by `render`; state lives in `<Name>State` and
//!   implements [`crate::core::Interactive`] (`handle_key` / `handle_mouse` → [`crate::core::Outcome`]);
//! * every builder has `.theme(&Theme)`; without it the process-wide [`crate::theme::current`] is used;
//! * focus is owned by the app (`.focused(bool)`), hover is tracked by the state's [`crate::core::HitBox`];
//! * anything animated samples an `Instant` passed to the builder (`.now(instant)`), never `Instant::now()`.

pub mod scrollbar;

// controls
pub mod button;
pub mod slider;
pub mod toggle;
// text entry
pub mod input;
pub mod select;
pub mod textarea;
// navigation
pub mod breadcrumbs;
pub mod list;
pub mod menu;
pub mod tabs;
pub mod tree;
// data
pub mod charts;
pub mod digits;
pub mod table;
// feedback & overlays
pub mod modal;
pub mod palette;
pub mod progress;
pub mod spinner;
pub mod toast;
pub mod tooltip;
// layout & chrome
pub mod collapsible;
pub mod footer;
pub mod header;
pub mod panel;
pub mod scroll;
pub mod split;
// content
pub mod calendar;
pub mod color;
pub mod log;
pub mod markdown;
pub mod steps;
pub mod text;

pub use breadcrumbs::*;
pub use button::*;
pub use calendar::*;
pub use charts::*;
pub use collapsible::*;
pub use color::*;
pub use digits::*;
pub use footer::*;
pub use header::*;
pub use input::*;
pub use list::*;
pub use log::*;
pub use markdown::*;
pub use menu::*;
pub use modal::*;
pub use palette::*;
pub use panel::*;
pub use progress::*;
pub use scroll::*;
pub use scrollbar::*;
pub use select::*;
pub use slider::*;
pub use spinner::*;
pub use split::*;
pub use steps::*;
pub use table::*;
pub use tabs::*;
pub use text::*;
pub use textarea::*;
pub use toast::*;
pub use toggle::*;
pub use tooltip::*;
pub use tree::*;
