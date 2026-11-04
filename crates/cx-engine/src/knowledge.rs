//! Data stores and knowledge connectors.
//!
//! A bundle may carry `dataStores` (each with `documents`). Pages and flows
//! enable retrieval through `knowledgeConnectorSettings`; when user input
//! matches no intent route, the runtime searches the connected data stores
//! and answers from the best passage (extractively, or grounded through the
//! configured LLM). Playbook tools with a `dataStoreSpec` search the same
//! indexes.

use crate::agent::find_in;
use crate::runtime::{route_target, Turn};
use cx_core::{arr, obj, Value};
use cx_nlu::search::{Hit, Index};
use std::collections::HashMap;

/// Minimum confidence for a knowledge answer.
pub const MIN_CONFIDENCE: f64 = 0.35;

/// Builds a search index for every data store in the bundle, keyed by the
/// data store's resource name.
pub fn build_indexes(bundle: &Value) -> HashMap<String, Index> {
    let mut out = HashMap::new();
    for ds in bundle.arr("dataStores") {
        let faq_store = ds.str("type") == "FAQ" || ds.str("contentConfig") == "FAQ";
        let mut ix = Index::new();
        for doc in ds.arr("documents") {
            let name = doc.str("name");
            let title = if doc.str("title").is_empty() { doc.str("displayName") } else { doc.str("title") };
            let sd = doc.at("structData");
            if !sd.str("question").is_empty() && !sd.str("answer").is_empty() {
                ix.add_faq(name, sd.str("question"), sd.str("answer"), doc.str("uri"));
            } else if faq_store && !doc.str("answer").is_empty() {
                ix.add_faq(name, title, doc.str("answer"), doc.str("uri"));
            } else {
                ix.add_document(name, title, doc.str("uri"), doc.str("content"));
            }
        }
        out.insert(ds.str("name").to_string(), ix);
    }
    out
}

fn hit_json(h: &Hit) -> Value {
    let mut v = obj! {
        "answer" => h.passage.text.clone(),
        "matchConfidence" => (h.confidence * 1000.0).round() / 1000.0,
        "source" => obj!{"document" => h.passage.document.clone(), "title" => h.passage.title.clone(), "uri" => h.passage.uri.clone()},
    };
    if let Some(q) = &h.passage.question {
        v.set("faqQuestion", q.clone());
    }
    v
}

impl<'a> Turn<'a> {
    /// Resolves the data store names of a `dataStoreConnections` list.
    fn connected_stores(&self, conns: &[Value]) -> Vec<String> {
        conns
            .iter()
            .filter_map(|c| {
                let r = c.str("dataStore");
                find_in(self.agent.bundle.arr("dataStores"), r).map(|d| d.str("name").to_string())
            })
            .collect()
    }

    /// Searches the given data stores and merges results by score.
    pub(crate) fn search_stores(&self, stores: &[String], query: &str, k: usize) -> Vec<Hit> {
        let mut hits: Vec<Hit> = stores.iter().filter_map(|s| self.agent.indexes.get(s)).flat_map(|ix| ix.search(query, k)).collect();
        hits.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal).then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)));
        hits.truncate(k);
        hits
    }

    /// Knowledge connector fallback for unmatched input. Returns true when
    /// an answer was produced.
    pub(crate) fn try_knowledge(&mut self, text: &str) -> bool {
        let settings = self
            .current_page()
            .map(|p| p.at("knowledgeConnectorSettings"))
            .filter(|k| k.bool_field("enabled"))
            .or_else(|| self.current_flow().map(|f| f.at("knowledgeConnectorSettings")).filter(|k| k.bool_field("enabled")));
        let Some(settings) = settings else { return false };
        let stores = self.connected_stores(settings.arr("dataStoreConnections"));
        let hits = self.search_stores(&stores, text, 3);
        let top = hits.first().map(|h| h.confidence).unwrap_or(0.0);
        self.log("Knowledge", obj! {"query" => text, "stores" => stores.len(), "topConfidence" => (top * 1000.0).round() / 1000.0});
        if top < MIN_CONFIDENCE {
            return false;
        }
        let mut answers: Vec<Value> = hits.iter().filter(|h| h.confidence >= MIN_CONFIDENCE).map(hit_json).collect();
        // Grounded generation when an LLM is available.
        if self.rt.llm.available() && hits[0].passage.question.is_none() {
            let context: String = hits.iter().enumerate().map(|(i, h)| format!("[{}] {}\n{}\n", i + 1, h.passage.title, h.passage.text)).collect();
            let prompt = format!(
                "Answer the question using only the sources below. If they do not contain the answer, reply exactly NO_ANSWER.\n\nSources:\n{}\nQuestion: {}",
                context, text
            );
            match self.rt.llm.chat("You answer customer questions from provided sources, concisely.", &[("user".into(), prompt)], 0.1) {
                Ok(r) if !r.contains("NO_ANSWER") && !r.trim().is_empty() => {
                    answers.insert(0, obj! {"answer" => r.trim(), "matchConfidence" => (top * 1000.0).round() / 1000.0, "generative" => true, "source" => answers[0].at("source").clone()});
                }
                Ok(_) => return false,
                Err(e) => self.log("Knowledge", obj! {"llmError" => e}),
            }
        }
        self.match_type = "KNOWLEDGE_CONNECTOR";
        self.st.no_match_count = 0;
        // As in Conversational Agents, $request.knowledge.answers is a list of
        // answer strings; full details are reported in queryResult.
        let texts: Vec<Value> = answers.iter().map(|a| Value::from(a.str("answer"))).collect();
        self.knowledge = obj! {"answers" => Value::Array(texts), "questions" => arr![text], "details" => Value::Array(answers.clone())};
        let f = settings.at("triggerFulfillment");
        let has_messages = !f.arr("messages").is_empty() || !f.arr("conditionalCases").is_empty();
        self.pending_target = None;
        self.run_fulfillment(f);
        if !has_messages {
            self.messages.push(obj! {"text" => obj!{"text" => arr![answers[0].str("answer")]}});
        }
        let target = self.pending_target.take().or_else(|| route_target(settings));
        if let Some(t) = target {
            self.transition(t);
        }
        true
    }

    /// Executes a data-store tool for playbooks.
    pub(crate) fn data_store_tool(&self, tool: &Value, input: &Value) -> Result<Value, String> {
        let spec = tool.at("dataStoreSpec");
        let stores = self.connected_stores(spec.arr("dataStoreConnections"));
        if stores.is_empty() {
            return Err(format!("tool '{}' has no resolvable data store connections", tool.str("displayName")));
        }
        let query = match input {
            Value::String(s) => s.clone(),
            other => other.get("query").map(|q| q.to_display()).unwrap_or_else(|| other.to_display()),
        };
        let hits = self.search_stores(&stores, &query, 5);
        Ok(obj! {"results" => Value::Array(hits.iter().map(hit_json).collect())})
    }
}
