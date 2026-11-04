//! The Conversational Agents runtime.
//!
//! * [`agent`] – compiled agent view over a bundle.
//! * [`runtime`] – `detectIntent` turn processing.
//! * [`expr`] – conditions, templates and `$sys.func` functions.
//! * [`playbook`] – generative playbooks (LLM loop / example replay) and tools.
//! * [`import`] – bundle normalisation and JSON-package import/export.
//! * [`testing`] – test-case runner.
//! * [`validate`] – agent validation.

pub mod agent;
pub mod expr;
pub mod import;
pub mod llm;
pub mod playbook;
pub mod runtime;
pub mod session;
pub mod testing;
pub mod validate;
pub mod yaml;

pub use agent::Agent;
pub use runtime::{response_texts, Runtime};
pub use session::SessionState;
