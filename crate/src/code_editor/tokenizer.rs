//! Backward-compatibility re-exports.
//!
//! All tokenizer functionality has moved to [`super::lang`].
//! This module is kept so that existing imports from `code_editor::tokenizer`
//! continue to work.

pub use super::lang::tokenize_line;
pub use super::token::{Token, TokenKind};
