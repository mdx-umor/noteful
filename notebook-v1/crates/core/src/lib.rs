//! Notebook core: everything that is not UI. Pure Rust, testable on any machine.

pub mod geom;
pub mod history;
pub mod ink;
pub mod library;
pub mod model;
pub mod pdf;
pub mod recognize;
pub mod render;
pub mod storage;
pub mod templates;
pub mod text;

pub use model::*;
