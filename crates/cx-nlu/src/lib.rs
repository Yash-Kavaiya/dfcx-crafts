//! Natural-language understanding for Conversational Agents.
//!
//! * [`entities`] – custom entity types (map / list / regexp, fuzzy matching,
//!   excluded phrases) and mention selection.
//! * [`system`] – `@sys.*` system entities (numbers, dates, times, email, …).
//! * [`intents`] – the intent classifier with parameter extraction.

pub mod entities;
pub mod intents;
pub mod search;
pub mod system;
pub mod text;

pub use entities::{Entity, EntityRegistry, EntityTypeDef, Kind, Mention};
pub use intents::{Classifier, IntentDef, IntentMatch, ParamDef, ParamValue, PhrasePart};
