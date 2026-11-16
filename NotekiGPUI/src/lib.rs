//! Noteki GPUI prototype: a native Markdown vault reader/editor built on GPUI Kit.
//!
//! `core` holds everything that does not depend on GPUI (vault I/O, links, parsing,
//! documents, tabs) so it can be unit-tested; `app` is the GPUI view.

// The editor's hidden ranges are lists of ranges, often of one.
#![allow(clippy::single_range_in_vec_init)]

pub mod app;
pub mod core;
pub mod editor;
pub mod theme;
