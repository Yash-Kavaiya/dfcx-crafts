//! A compiled, read-only view of an agent bundle used by the runtime.
//!
//! The bundle is the canonical export format:
//! ```json
//! { "agent": {...}, "flows": [{..., "pages": [...], "transitionRouteGroups": [...]}],
//!   "intents": [...], "entityTypes": [...], "webhooks": [...],
//!   "transitionRouteGroups": [...], "generators": [...],
//!   "playbooks": [{..., "examples": [...]}], "tools": [...],
//!   "testCases": [...], "environments": [...] }
//! ```

use cx_core::name::{id_of, DEFAULT_NEGATIVE_INTENT_ID};
use cx_core::Value;
use cx_nlu::system::normalize_type_ref;
use cx_nlu::{Classifier, EntityRegistry, EntityTypeDef, IntentDef, ParamDef, PhrasePart};

#[derive(Debug, Clone)]
pub struct Agent {
    pub name: String,
    pub raw: Value,
    pub bundle: Value,
    pub classifier: Classifier,
    pub threshold: f64,
    /// Search indexes of the data stores carried by the bundle.
    pub indexes: std::collections::HashMap<String, cx_nlu::search::Index>,
}

/// Finds a resource in a list by full name, id or display name.
pub fn find_in<'a>(list: &'a [Value], r: &str) -> Option<&'a Value> {
    if r.is_empty() {
        return None;
    }
    list.iter()
        .find(|v| v.str("name") == r)
        .or_else(|| list.iter().find(|v| id_of(v.str("name")) == r))
        .or_else(|| list.iter().find(|v| v.str("displayName") == r))
        .or_else(|| {
            let id = id_of(r);
            list.iter().find(|v| id_of(v.str("name")) == id)
        })
}

impl Agent {
    pub fn compile(bundle: &Value) -> Result<Agent, String> {
        let raw = bundle.get("agent").cloned().ok_or("bundle has no 'agent'")?;
        let name = raw.str("name").to_string();
        let entity_defs: Vec<EntityTypeDef> =
            bundle.arr("entityTypes").iter().map(|e| EntityTypeDef::from_json(e.str("name"), e)).collect();
        let registry = EntityRegistry { custom: entity_defs };
        let mut intents = Vec::new();
        for it in bundle.arr("intents") {
            let params: Vec<ParamDef> = it
                .arr("parameters")
                .iter()
                .map(|p| ParamDef {
                    id: p.str("id").to_string(),
                    entity_type: Self::entity_key_in(bundle, p.str("entityType")),
                    is_list: p.bool_field("isList"),
                })
                .collect();
            let phrases = it
                .arr("trainingPhrases")
                .iter()
                .map(|tp| {
                    tp.arr("parts")
                        .iter()
                        .map(|part| PhrasePart {
                            text: part.str("text").to_string(),
                            parameter_id: part.get("parameterId").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from),
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            intents.push(IntentDef { key: it.str("name").to_string(), display_name: it.str("displayName").to_string(), phrases, params });
        }
        let classifier = Classifier::new(intents, registry);
        let threshold = raw.path("nluSettings.classificationThreshold").and_then(Value::as_f64).unwrap_or(0.3);
        let indexes = crate::knowledge::build_indexes(bundle);
        Ok(Agent { name, raw, bundle: bundle.clone(), classifier, threshold, indexes })
    }

    /// Normalises an entity-type reference to the key used by the NLU
    /// (`sys.xxx` or the custom type's full resource name).
    fn entity_key_in(bundle: &Value, r: &str) -> String {
        let n = normalize_type_ref(r);
        if n.starts_with("sys.") {
            return n;
        }
        find_in(bundle.arr("entityTypes"), &n).map(|e| e.str("name").to_string()).unwrap_or(n)
    }

    pub fn entity_key(&self, r: &str) -> String {
        Self::entity_key_in(&self.bundle, r)
    }

    pub fn display_name(&self) -> &str {
        self.raw.str("displayName")
    }
    pub fn language(&self) -> &str {
        let l = self.raw.str("defaultLanguageCode");
        if l.is_empty() {
            "en"
        } else {
            l
        }
    }

    pub fn flows(&self) -> &[Value] {
        self.bundle.arr("flows")
    }
    pub fn intents(&self) -> &[Value] {
        self.bundle.arr("intents")
    }
    pub fn entity_types(&self) -> &[Value] {
        self.bundle.arr("entityTypes")
    }
    pub fn webhooks(&self) -> &[Value] {
        self.bundle.arr("webhooks")
    }
    pub fn generators(&self) -> &[Value] {
        self.bundle.arr("generators")
    }
    pub fn playbooks(&self) -> &[Value] {
        self.bundle.arr("playbooks")
    }
    pub fn tools(&self) -> &[Value] {
        self.bundle.arr("tools")
    }
    pub fn test_cases(&self) -> &[Value] {
        self.bundle.arr("testCases")
    }

    pub fn flow(&self, r: &str) -> Option<&Value> {
        find_in(self.flows(), r)
    }
    pub fn page(&self, flow: &str, r: &str) -> Option<&Value> {
        let f = self.flow(flow)?;
        find_in(f.arr("pages"), r)
    }
    /// Finds a page anywhere in the agent (by full name), returning (flow, page).
    pub fn page_any(&self, r: &str) -> Option<(&Value, &Value)> {
        for f in self.flows() {
            if let Some(p) = f.arr("pages").iter().find(|p| p.str("name") == r) {
                return Some((f, p));
            }
        }
        None
    }
    pub fn intent(&self, r: &str) -> Option<&Value> {
        find_in(self.intents(), r)
    }
    pub fn entity_type(&self, r: &str) -> Option<&Value> {
        find_in(self.entity_types(), r)
    }
    pub fn webhook(&self, r: &str) -> Option<&Value> {
        find_in(self.webhooks(), r)
    }
    pub fn generator(&self, r: &str) -> Option<&Value> {
        find_in(self.generators(), r)
    }
    pub fn playbook(&self, r: &str) -> Option<&Value> {
        find_in(self.playbooks(), r)
    }
    pub fn tool(&self, r: &str) -> Option<&Value> {
        find_in(self.tools(), r)
    }
    /// Route groups are looked up in the flow first, then at agent level.
    pub fn route_group(&self, flow: &str, r: &str) -> Option<&Value> {
        if let Some(f) = self.flow(flow) {
            if let Some(g) = find_in(f.arr("transitionRouteGroups"), r) {
                return Some(g);
            }
        }
        find_in(self.bundle.arr("transitionRouteGroups"), r)
    }

    /// Start flow name (explicit `startFlow`, else the default start flow,
    /// else the first flow).
    pub fn start_flow(&self) -> Option<String> {
        let sf = self.raw.str("startFlow");
        if let Some(f) = self.flow(sf) {
            return Some(f.str("name").to_string());
        }
        if let Some(f) = self.flow(cx_core::name::DEFAULT_START_FLOW_ID).or_else(|| self.flow("Default Start Flow")) {
            return Some(f.str("name").to_string());
        }
        if self.start_playbook().is_some() {
            return None;
        }
        self.flows().first().map(|f| f.str("name").to_string())
    }

    pub fn start_playbook(&self) -> Option<String> {
        let sp = self.raw.str("startPlaybook");
        self.playbook(sp).map(|p| p.str("name").to_string())
    }

    pub fn is_negative_intent(&self, intent_name: &str) -> bool {
        id_of(intent_name) == DEFAULT_NEGATIVE_INTENT_ID
            || self.intent(intent_name).map_or(false, |i| i.str("displayName") == "Default Negative Intent")
    }

    /// Flow-specific classification threshold, falling back to the agent's.
    pub fn threshold_for(&self, flow: &str) -> f64 {
        self.flow(flow)
            .and_then(|f| f.path("nluSettings.classificationThreshold"))
            .and_then(Value::as_f64)
            .unwrap_or(self.threshold)
    }

    /// Builds session-entity overrides (from `sessionEntityTypes`).
    pub fn session_entity_defs(&self, session_entity_types: &[Value]) -> Vec<EntityTypeDef> {
        let mut out = Vec::new();
        for se in session_entity_types {
            // name: .../sessions/<s>/entityTypes/<display name or id>
            let id = id_of(se.str("name"));
            let Some(base) = self.entity_type(id) else { continue };
            let key = base.str("name").to_string();
            let mut synthetic = base.clone();
            synthetic.set("entities", se.at("entities").clone());
            let supplement = se.str("entityOverrideMode") == "ENTITY_OVERRIDE_MODE_SUPPLEMENT";
            let k = if supplement { format!("{}#supplement", key) } else { key };
            out.push(EntityTypeDef::from_json(&k, &synthetic));
        }
        out
    }
}

impl Agent {
    /// Names of parameters whose values must not be stored in logs: form
    /// and intent parameters marked `redact`, and parameters whose entity
    /// type is marked `redact`.
    pub fn redacted_parameters(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let redacted_type = |et: &str| self.entity_type(&self.entity_key(et)).map_or(false, |e| e.bool_field("redact"));
        let mut add = |n: &str| {
            if !n.is_empty() && !out.iter().any(|x| x == n) {
                out.push(n.to_string());
            }
        };
        for f in self.flows() {
            for p in f.arr("pages") {
                for fp in p.at("form").arr("parameters") {
                    if fp.bool_field("redact") || redacted_type(fp.str("entityType")) {
                        add(fp.str("displayName"));
                    }
                }
            }
        }
        for it in self.intents() {
            for p in it.arr("parameters") {
                if p.bool_field("redact") || redacted_type(p.str("entityType")) {
                    add(p.str("id"));
                }
            }
        }
        out
    }
}

/// Masks redacted parameters inside a logged interaction: the parameter
/// entries themselves and any occurrence of their values in text.
pub fn redact_value(v: &Value, params: &[String]) -> Value {
    if params.is_empty() {
        return v.clone();
    }
    const MASK: &str = "[redacted]";
    let mut secrets: Vec<String> = Vec::new();
    fn collect(v: &Value, params: &[String], out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m.iter() {
                    if params.iter().any(|p| p == k) {
                        leaf_strings(x, out);
                    } else {
                        collect(x, params, out);
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| collect(x, params, out)),
            _ => {}
        }
    }
    fn leaf_strings(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) if s.chars().count() >= 3 => out.push(s.clone()),
            Value::Number(n) => {
                let s = cx_core::json::format_number(*n);
                if s.len() >= 3 {
                    out.push(s);
                }
            }
            Value::Object(m) => m.values().for_each(|x| leaf_strings(x, out)),
            Value::Array(a) => a.iter().for_each(|x| leaf_strings(x, out)),
            _ => {}
        }
    }
    collect(v, params, &mut secrets);
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    secrets.dedup();
    fn mask(v: &Value, params: &[String], secrets: &[String]) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, x)| {
                        // the whole value, including {original, resolved} objects
                        if params.iter().any(|p| p == k) {
                            (k.clone(), Value::from(MASK))
                        } else {
                            (k.clone(), mask(x, params, secrets))
                        }
                    })
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(|x| mask(x, params, secrets)).collect()),
            Value::String(s) => {
                let mut t = s.clone();
                for sec in secrets {
                    if t.contains(sec.as_str()) {
                        t = t.replace(sec.as_str(), MASK);
                    }
                }
                Value::String(t)
            }
            other => other.clone(),
        }
    }
    mask(v, params, &secrets)
}

#[cfg(test)]
mod redact_tests {
    use super::*;
    use cx_core::obj;

    #[test]
    fn masks_params_and_their_values_in_text() {
        let entry = obj! {
            "request" => obj!{"queryInput" => obj!{"text" => obj!{"text" => "my card is 4111111111111111"}}},
            "response" => obj!{"queryResult" => obj!{
                "parameters" => obj!{"card" => "4111111111111111", "name" => "Ada"},
                "responseMessages" => cx_core::arr![obj!{"text" => obj!{"text" => cx_core::arr!["Charged card 4111111111111111."]}}],
                "match" => obj!{"parameters" => obj!{"card" => obj!{"original" => "4111111111111111"}}},
            }},
        };
        let r = redact_value(&entry, &["card".to_string()]);
        let s = r.to_string();
        assert!(!s.contains("4111111111111111"), "{}", s);
        assert!(s.contains("Ada"));
        assert_eq!(r.at("response").at("queryResult").at("parameters").str("card"), "[redacted]");
        assert_eq!(r.at("request").at("queryInput").at("text").str("text"), "my card is [redacted]");
    }
}
