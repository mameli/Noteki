//! The rules of Noteki's editor, without GPUI: the text with its selection,
//! cursor movement, and the vertical geometry of lines of different heights.
//! The element that draws them is `crate::editor`.

pub mod buffer;
pub mod code;
pub mod display;
pub mod find;
pub mod format;
pub mod history;
pub mod lines;
pub mod movement;
pub mod styling;
pub mod table;

pub use buffer::{EditBuffer, Motion, Selection};
