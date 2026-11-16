//! Noteki GPUI prototype: a native Markdown vault reader/editor built on GPUI Kit.
//!
//! `core` holds everything that does not depend on GPUI (vault I/O, links, parsing,
//! documents, tabs) so it can be unit-tested; `app` is the GPUI view.

pub mod app;
pub mod core;
pub mod theme;
