//! Shared domain types used by every ProtonTrainer module.
//!
//! This crate has no I/O and no dependency on any other workspace crate.
//! Keep it small: a type belongs here only when two or more modules need it.

mod app_id;

pub use app_id::{AppId, AppIdError};
