//! The FilmCraft GPUI frontend (native panels).
//!
//! Thin by design, like `filmcraft-ui-egui`: every project change goes through
//! `filmcraft_engine::Session::execute` by registered command id, the interface reads its colours
//! and sizes from `filmcraft-theme` tokens, and the keyboard map is the engine's own
//! (`Session::shortcuts`). What exists so far is the Program monitor and the Timeline; every other
//! panel is still only in the egui frontend (see `docs/ui-gpui-parity.md`).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

mod chrome;
pub mod geometry;
mod keys;
mod run;
mod timeline;
mod viewer;
pub mod workspace;

pub use run::{FONT_FAMILY, load_fonts, run};
pub use viewer::Shown;
pub use workspace::{Tool, Workspace};
