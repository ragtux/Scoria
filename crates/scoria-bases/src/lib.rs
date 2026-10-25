//! The Bases expression language.
//!
//! A `.base` holds statements — filters, formulas, summaries — as strings, and
//! this crate turns one into the tree Obsidian parses it into. Everything here
//! is observed rather than inferred: see
//! [[Specification#Observed Bases expression language, Obsidian 1.14.5 (installer build 1.13.7)]],
//! whose precedence table was read off real parse trees rather than deduced
//! from values.
//!
//! This crate has **no printer**, deliberately. Obsidian keeps a statement's
//! original text beside its tree and writes the text back, so `scoria-formats`
//! holds statements verbatim and never needs a tree to write one. Nothing in
//! the crate graph lets it: `scoria-formats` does not depend on this crate and
//! must not, or a writer could route a statement through an AST and lose the
//! bytes.

pub mod expr;

pub use expr::{parse, parse_node, BinaryOp, Expr, Literal, ParseError};
