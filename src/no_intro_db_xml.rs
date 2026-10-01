//! Typed streaming reader for No-Intro DATABASE-EXPORT XML.
//!
//! [`read_with`] walks the XML event stream once, retains the document header
//! and one [`DatabaseGame`] at a time, and delivers each game to the caller.
//! It accepts a single `<datafile>` root with an optional nested `<header>`,
//! or the observed sibling `<header>` then `<datafile>` framing. The two
//! [`NoIntroDatabaseMode`] values identify ordinary observed-compatible input
//! and the narrow decoded U+0000 recovery interpretation.
//!
//! The callback API does not persist a catalog or claim native database
//! publication. [`ValidatedNoIntroDatabase`] is returned only after complete
//! XML EOF, so callers can use that value as the parser's completion proof.

mod model;
mod reader;

pub use crate::xml_reader::DeclaredText;
pub use model::*;
pub use reader::{RecoveryWarnings, ValidatedNoIntroDatabase, read_with};
