//! SlimLocalize domain core.
//!
//! This crate is deliberately independent from Tauri, Axum, SQLite and the CLI.
//! It owns the domain model, application services, validation rules and the
//! repository ports consumed by adapters.
//!
//! See `docs/architecture/overview.md` for the dependency rules.

pub mod error;
pub mod events;
pub mod model;
pub mod paths;
pub mod ports;
pub mod service;
pub mod validation;

pub use error::{DomainError, Result};
pub use model::*;
pub use ports::Store;
pub use service::LocalizeService;
