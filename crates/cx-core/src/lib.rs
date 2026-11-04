//! Core primitives shared by every crate in the workspace.
//!
//! The project is intentionally dependency-free: this crate provides an
//! order-preserving JSON value type with a parser/serializer, RFC 3339 time
//! helpers, random UUIDs and resource-name helpers.

pub mod json;
pub mod name;
pub mod time;
pub mod uuid;

pub use json::{Map, Value};
