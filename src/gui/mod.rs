//! Desktop presentation and a renderer-independent simulation controller.
#[cfg(feature = "gui")]
mod app;
#[cfg(feature = "gui")]
mod charts;
pub mod controller;
#[cfg(feature = "gui")]
mod views;
#[cfg(feature = "gui")]
pub use app::EconomyApp;
