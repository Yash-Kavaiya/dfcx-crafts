//! Persistent per-session conversation state.

use cx_core::{arr, obj, Value};

/// A caller frame pushed when a flow (or playbook) is entered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub flow: String,
    pub page: String,
    /// When the caller was a playbook, its name.
    pub playbook: Option<String>,
}

/// Runtime state of an active playbook.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybookState {
    pub name: String,
    /// Conversation actions in Conversational Agents example format.
    pub history: Vec<Value>,
    /// Example-replay cursor (example index, next action index).
    pub cursor: Option<(usize, usize)>,
    /// A client-side function tool call awaiting its result.
    pub pending_tool: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionState {
    pub params: Value,
    pub flow: String,
    pub page: String,
    pub previous_page: String,
    pub stack: Vec<Frame>,
    pub requested_param: Option<String>,
    pub reprompt_count: u32,
    pub no_match_count: u32,
    pub no_input_count: u32,
    pub ended: bool,
    pub playbook: Option<PlaybookState>,
    pub turn: u64,
    pub session_entity_types: Vec<Value>,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState { params: Value::object(), ..Default::default() }
    }

    pub fn param(&self, key: &str) -> &Value {
        self.params.path(key).unwrap_or(&cx_core::json::NULL)
    }

    pub fn set_param(&mut self, key: &str, v: Value) {
        if !self.params.is_object() {
            self.params = Value::object();
        }
        // Support nested keys "a.b" like Conversational Agents does.
        let parts: Vec<&str> = key.split('.').collect();
        let mut cur = &mut self.params;
        for (i, p) in parts.iter().enumerate() {
            if i + 1 == parts.len() {
                if v.is_null() {
                    cur.remove(p);
                } else {
                    cur.set(*p, v.clone());
                }
                return;
            }
            let next = cur.entry(p);
            if !next.is_object() {
                *next = Value::object();
            }
            cur = next;
        }
    }

    pub fn to_json(&self) -> Value {
        let frame = |f: &Frame| obj! {"flow" => f.flow.clone(), "page" => f.page.clone(), "playbook" => f.playbook.clone()};
        let pb = self.playbook.as_ref().map(|p| {
            obj! {
                "name" => p.name.clone(),
                "history" => Value::Array(p.history.clone()),
                "cursor" => p.cursor.map(|(a, b)| arr![a, b]),
                "pendingTool" => p.pending_tool.clone(),
            }
        });
        obj! {
            "params" => self.params.clone(),
            "flow" => self.flow.clone(),
            "page" => self.page.clone(),
            "previousPage" => self.previous_page.clone(),
            "stack" => Value::Array(self.stack.iter().map(frame).collect()),
            "requestedParam" => self.requested_param.clone(),
            "repromptCount" => self.reprompt_count,
            "noMatchCount" => self.no_match_count,
            "noInputCount" => self.no_input_count,
            "ended" => self.ended,
            "playbook" => pb,
            "turn" => self.turn,
            "sessionEntityTypes" => Value::Array(self.session_entity_types.clone()),
        }
    }

    pub fn from_json(v: &Value) -> Self {
        let frame = |f: &Value| Frame {
            flow: f.str("flow").to_string(),
            page: f.str("page").to_string(),
            playbook: f.get("playbook").and_then(Value::as_str).map(String::from),
        };
        let playbook = v.get("playbook").filter(|p| p.is_object()).map(|p| PlaybookState {
            name: p.str("name").to_string(),
            history: p.arr("history").to_vec(),
            cursor: p.get("cursor").and_then(Value::as_array).and_then(|c| Some((c.first()?.as_f64()? as usize, c.get(1)?.as_f64()? as usize))),
            pending_tool: p.get("pendingTool").filter(|t| !t.is_null()).cloned(),
        });
        SessionState {
            params: v.get("params").filter(|p| p.is_object()).cloned().unwrap_or_else(Value::object),
            flow: v.str("flow").to_string(),
            page: v.str("page").to_string(),
            previous_page: v.str("previousPage").to_string(),
            stack: v.arr("stack").iter().map(frame).collect(),
            requested_param: v.get("requestedParam").and_then(Value::as_str).map(String::from),
            reprompt_count: v.f64_field("repromptCount").unwrap_or(0.0) as u32,
            no_match_count: v.f64_field("noMatchCount").unwrap_or(0.0) as u32,
            no_input_count: v.f64_field("noInputCount").unwrap_or(0.0) as u32,
            ended: v.bool_field("ended"),
            playbook,
            turn: v.f64_field("turn").unwrap_or(0.0) as u64,
            session_entity_types: v.arr("sessionEntityTypes").to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_nested() {
        let mut s = SessionState::new();
        s.set_param("order.size", Value::from("large"));
        s.set_param("count", Value::from(2));
        s.stack.push(Frame { flow: "f".into(), page: "p".into(), playbook: None });
        s.playbook = Some(PlaybookState { name: "pb".into(), cursor: Some((1, 2)), ..Default::default() });
        assert_eq!(s.param("order.size"), &Value::from("large"));
        let back = SessionState::from_json(&s.to_json());
        assert_eq!(back, s);
        s.set_param("count", Value::Null);
        assert!(s.param("count").is_null());
    }
}
