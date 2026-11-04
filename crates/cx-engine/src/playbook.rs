//! Playbook execution.
//!
//! With an LLM configured, a playbook runs a bounded reasoning loop: the
//! model sees the goal, instructions, tools, examples and the transcript and
//! answers with a JSON action (`say`, `tool`, `playbook`, `flow`, `end`).
//! Without an LLM the playbook replays its examples deterministically: the
//! user utterance is matched against example user turns and the following
//! agent turns are emitted, which keeps agents testable offline.

use crate::llm::extract_json;
use crate::runtime::Turn;
use crate::session::{Frame, PlaybookState};
use crate::{expr, yaml};
use cx_core::{arr, obj, Value};
use cx_nlu::text::{is_stopword, tokenize};
use std::time::Duration;

fn similarity(a: &str, b: &str) -> f64 {
    let ta: Vec<String> = tokenize(a).into_iter().filter(|t| !is_stopword(&t.norm)).map(|t| t.stem).collect();
    let tb: Vec<String> = tokenize(b).into_iter().filter(|t| !is_stopword(&t.norm)).map(|t| t.stem).collect();
    if ta.is_empty() || tb.is_empty() {
        return if cx_nlu::text::normalize(a) == cx_nlu::text::normalize(b) { 1.0 } else { 0.0 };
    }
    let common = ta.iter().filter(|t| tb.contains(t)).count() as f64;
    2.0 * common / (ta.len() + tb.len()) as f64
}

fn render_steps(steps: &[Value], depth: usize, out: &mut String) {
    for s in steps {
        out.push_str(&format!("{}- {}\n", "  ".repeat(depth), s.str("text")));
        render_steps(s.arr("steps"), depth + 1, out);
    }
}

fn action_text(a: &Value) -> Option<(&'static str, String)> {
    if let Some(u) = a.get("userUtterance") {
        return Some(("user", u.str("text").to_string()));
    }
    if let Some(u) = a.get("agentUtterance") {
        return Some(("assistant", u.str("text").to_string()));
    }
    if let Some(t) = a.get("toolUse") {
        return Some(("tool", format!("{} -> {}", t.str("tool"), t.at("outputActionParameters"))));
    }
    None
}

impl<'a> Turn<'a> {
    fn pb_state(&mut self) -> Option<&mut PlaybookState> {
        self.st.playbook.as_mut()
    }

    pub(crate) fn start_playbook(&mut self, name: &str) {
        let dn = self.agent.playbook(name).map(|p| p.str("displayName").to_string()).unwrap_or_default();
        self.st.playbook = Some(PlaybookState { name: name.to_string(), ..Default::default() });
        self.log("Playbook", obj! {"started" => dn});
        let text = self.user_text.clone();
        if !text.trim().is_empty() {
            self.playbook_user_turn(Some(&text));
        }
    }

    pub(crate) fn playbook_user_turn(&mut self, text: Option<&str>) {
        let Some(name) = self.st.playbook.as_ref().map(|p| p.name.clone()) else { return };
        let Some(pb) = self.agent.playbook(&name) else {
            self.log("Error", obj! {"error" => format!("playbook '{}' not found", name)});
            self.st.playbook = None;
            return;
        };
        if let Some(t) = text {
            if let Some(s) = self.pb_state() {
                s.history.push(obj! {"userUtterance" => obj!{"text" => t}});
            }
        }
        if self.rt.llm.available() {
            self.llm_loop(pb);
        } else {
            self.replay(pb, text);
        }
    }

    pub(crate) fn playbook_tool_result(&mut self, tc: &Value) {
        let Some(pending) = self.pb_state().and_then(|s| s.pending_tool.take()) else {
            self.log("Playbook", obj! {"error" => "no pending tool call"});
            return;
        };
        let mut used = pending.clone();
        used.set("outputActionParameters", tc.at("outputParameters").clone());
        if let Some(s) = self.pb_state() {
            s.history.push(obj! {"toolUse" => used});
        }
        self.playbook_user_turn(None);
    }

    fn say(&mut self, text: &str) {
        let rendered = expr::render_template(text, &*self);
        self.messages.push(obj! {"text" => obj!{"text" => arr![rendered.clone()]}});
        if let Some(s) = self.pb_state() {
            s.history.push(obj! {"agentUtterance" => obj!{"text" => rendered}});
        }
    }

    fn end_playbook(&mut self, output: &Value) {
        if let Some(o) = output.as_object() {
            for (k, v) in o.iter() {
                self.st.set_param(k, v.clone());
            }
        }
        let name = self.st.playbook.take().map(|p| p.name).unwrap_or_default();
        self.log("Playbook", obj! {"ended" => name, "output" => output.clone()});
        match self.st.stack.pop() {
            Some(frame) => {
                self.st.flow = frame.flow;
                self.st.page = frame.page;
                if let Some(caller) = frame.playbook {
                    self.st.playbook = Some(PlaybookState { name: caller, ..Default::default() });
                    self.playbook_user_turn(None);
                } else if !self.st.flow.is_empty() {
                    // re-evaluate the calling page
                    self.settle_page();
                }
            }
            None => {
                if self.agent.start_flow().is_none() {
                    self.st.ended = true;
                    self.halted = true;
                }
            }
        }
    }

    fn invoke_flow(&mut self, flow: &str) {
        let Some(f) = self.agent.flow(flow) else {
            self.log("Error", obj! {"error" => format!("flow '{}' not found", flow)});
            return;
        };
        let caller = self.st.playbook.take().map(|p| p.name);
        self.st.stack.push(Frame { flow: self.st.flow.clone(), page: self.st.page.clone(), playbook: caller });
        let n = f.str("name").to_string();
        self.enter_start(&n);
    }

    fn switch_playbook(&mut self, target: &str) -> bool {
        match self.agent.playbook(target) {
            Some(p) => {
                let last_user = self
                    .st
                    .playbook
                    .as_ref()
                    .and_then(|s| s.history.iter().rev().find(|a| a.get("userUtterance").is_some()).cloned());
                let caller = self.st.playbook.as_ref().map(|s| s.name.clone());
                self.st.stack.push(Frame { flow: self.st.flow.clone(), page: self.st.page.clone(), playbook: caller });
                let mut st = PlaybookState { name: p.str("name").to_string(), ..Default::default() };
                if let Some(u) = last_user {
                    st.history.push(u);
                }
                self.st.playbook = Some(st);
                self.log("Playbook", obj! {"handoff" => p.str("displayName")});
                true
            }
            None => {
                self.log("Error", obj! {"error" => format!("playbook '{}' not found", target)});
                false
            }
        }
    }

    // ---- LLM mode ---------------------------------------------------------

    fn playbook_prompt(&self, pb: &Value) -> String {
        let mut s = format!(
            "You are a conversational agent executing the playbook \"{}\".\nGoal: {}\n\nInstructions:\n",
            pb.str("displayName"),
            pb.str("goal")
        );
        render_steps(pb.at("instruction").arr("steps"), 0, &mut s);
        let tools: Vec<&Value> = pb.arr("referencedTools").iter().filter_map(|t| t.as_str()).filter_map(|t| self.agent.tool(t)).collect();
        if !tools.is_empty() {
            s.push_str("\nTools you can call:\n");
            for t in tools {
                let actions = tool_actions(t);
                s.push_str(&format!("- {} ({}): {} actions={:?}\n", t.str("displayName"), tool_kind(t), t.str("description"), actions));
                if let Some(fs) = t.get("functionSpec") {
                    s.push_str(&format!("  input schema: {}\n", fs.at("inputSchema")));
                }
            }
        }
        let pbs: Vec<&str> = pb.arr("referencedPlaybooks").iter().filter_map(|p| p.as_str()).filter_map(|p| self.agent.playbook(p)).map(|p| p.str("displayName")).collect();
        if !pbs.is_empty() {
            s.push_str(&format!("\nPlaybooks you can hand off to: {}\n", pbs.join(", ")));
        }
        let flows: Vec<&str> = pb.arr("referencedFlows").iter().filter_map(|p| p.as_str()).filter_map(|p| self.agent.flow(p)).map(|f| f.str("displayName")).collect();
        if !flows.is_empty() {
            s.push_str(&format!("\nFlows you can hand off to: {}\n", flows.join(", ")));
        }
        if !pb.arr("outputParameterDefinitions").is_empty() {
            let outs: Vec<&str> = pb.arr("outputParameterDefinitions").iter().map(|p| p.str("name")).collect();
            s.push_str(&format!("\nWhen the goal is achieved, end with output parameters: {}\n", outs.join(", ")));
        }
        s.push_str(&format!("\nSession parameters: {}\n", self.st.params));
        let examples = pb.arr("examples");
        if !examples.is_empty() {
            s.push_str("\nExamples:\n");
            for ex in examples.iter().take(5) {
                s.push_str(&format!("# {}\n", ex.str("displayName")));
                for a in ex.arr("actions") {
                    if let Some((role, text)) = action_text(a) {
                        s.push_str(&format!("{}: {}\n", role, text));
                    }
                }
            }
        }
        s.push_str(
            "\nRespond with ONLY one JSON object, one of:\n\
             {\"say\": \"<reply to the user>\"}\n\
             {\"tool\": \"<tool name>\", \"action\": \"<action>\", \"input\": {...}}\n\
             {\"playbook\": \"<playbook name>\"}\n\
             {\"flow\": \"<flow name>\"}\n\
             {\"end\": true, \"say\": \"<optional final reply>\", \"output\": {...}}\n",
        );
        s
    }

    fn llm_loop(&mut self, pb_initial: &'a Value) {
        let mut pb = pb_initial;
        for _ in 0..8 {
            if self.halted {
                return;
            }
            let system = self.playbook_prompt(pb);
            let history = self.st.playbook.as_ref().map(|s| s.history.clone()).unwrap_or_default();
            let mut msgs: Vec<(String, String)> = Vec::new();
            for a in &history {
                if let Some((role, text)) = action_text(a) {
                    let role = if role == "tool" { "user".to_string() } else { role.to_string() };
                    let text = if a.get("toolUse").is_some() { format!("[tool result] {}", text) } else { text };
                    msgs.push((role, text));
                }
            }
            if msgs.is_empty() {
                msgs.push(("user".into(), "(conversation start)".into()));
            }
            let temp = pb.path("llmModelSettings.temperature").and_then(Value::as_f64).unwrap_or(0.2);
            let reply = match self.rt.llm.chat(&system, &msgs, temp) {
                Ok(r) => r,
                Err(e) => {
                    self.log("Playbook", obj! {"error" => e});
                    self.say("Sorry, something went wrong on my side.");
                    return;
                }
            };
            let action = extract_json(&reply).unwrap_or_else(|| obj! {"say" => reply.trim()});
            self.log("Playbook", obj! {"playbook" => pb.str("displayName"), "action" => action.clone()});
            if !action.str("tool").is_empty() {
                let tool_ref = action.str("tool").to_string();
                let Some(tool) = self.agent.tool(&tool_ref) else {
                    if let Some(s) = self.pb_state() {
                        s.history.push(obj! {"toolUse" => obj!{"tool" => tool_ref.clone(), "outputActionParameters" => obj!{"error" => "unknown tool"}}});
                    }
                    continue;
                };
                let act = action.str("action").to_string();
                let input = action.at("input").clone();
                if tool.get("functionSpec").is_some() {
                    let call = obj! {"tool" => tool.str("name"), "action" => act.clone(), "inputActionParameters" => input.clone()};
                    self.messages.push(obj! {"toolCall" => obj!{"tool" => tool.str("name"), "action" => act, "inputParameters" => input}});
                    if let Some(s) = self.pb_state() {
                        s.pending_tool = Some(call);
                    }
                    return;
                }
                let out = match self.execute_tool(tool, &act, &input) {
                    Ok(v) => v,
                    Err(e) => obj! {"error" => e},
                };
                if let Some(s) = self.pb_state() {
                    s.history.push(obj! {"toolUse" => obj!{"tool" => tool.str("name"), "action" => act, "inputActionParameters" => input, "outputActionParameters" => out}});
                }
                continue;
            }
            if !action.str("playbook").is_empty() {
                let target = action.str("playbook").to_string();
                if self.switch_playbook(&target) {
                    pb = self.agent.playbook(&target).unwrap();
                }
                continue;
            }
            if !action.str("flow").is_empty() {
                let f = action.str("flow").to_string();
                self.invoke_flow(&f);
                return;
            }
            if action.bool_field("end") {
                let say = action.str("say").to_string();
                if !say.is_empty() {
                    self.say(&say);
                }
                let out = action.at("output").clone();
                self.end_playbook(&out);
                return;
            }
            let say = action.str("say").to_string();
            self.say(if say.is_empty() { reply.trim() } else { &say });
            return;
        }
        self.log("Playbook", obj! {"error" => "step limit reached"});
    }

    // ---- tools ------------------------------------------------------------

    pub(crate) fn execute_tool(&mut self, tool: &Value, action: &str, input: &Value) -> Result<Value, String> {
        if tool.get("dataStoreSpec").is_some() {
            let r = self.data_store_tool(tool, input);
            self.log("Tool", obj! {"tool" => tool.str("displayName"), "kind" => "dataStore", "ok" => r.is_ok()});
            return r;
        }
        let Some(spec) = tool.get("openApiSpec") else {
            return Err(format!("tool '{}' type is not executable server-side", tool.str("displayName")));
        };
        let schema_text = spec.str("textSchema");
        let schema = Value::parse(schema_text).or_else(|_| yaml::parse(schema_text)).map_err(|e| format!("invalid OpenAPI schema: {}", e))?;
        let base = schema.at("servers")[0].str("url").trim_end_matches('/').to_string();
        let mut found: Option<(String, String, Value)> = None;
        if let Some(paths) = schema.get("paths").and_then(Value::as_object) {
            for (path, item) in paths.iter() {
                if let Some(ops) = item.as_object() {
                    for (method, op) in ops.iter() {
                        if op.str("operationId") == action || (action.is_empty() && found.is_none()) {
                            found = Some((path.clone(), method.to_uppercase(), op.clone()));
                        }
                    }
                }
            }
        }
        let (path, method, op) = found.ok_or_else(|| format!("operation '{}' not found in tool schema", action))?;
        let mut url_path = path.clone();
        let mut query = Vec::new();
        let mut remaining = input.as_object().cloned().unwrap_or_default();
        for p in op.arr("parameters") {
            let n = p.str("name");
            if let Some(v) = remaining.remove(n) {
                match p.str("in") {
                    "path" => url_path = url_path.replace(&format!("{{{}}}", n), &cx_http::percent_encode(&expr::display(&v))),
                    "query" => query.push(format!("{}={}", cx_http::percent_encode(n), cx_http::percent_encode(&expr::display(&v)))),
                    _ => {}
                }
            }
        }
        let mut url = format!("{}{}", base, url_path);
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if let Some(ak) = spec.at("authentication").get("apiKeyConfig") {
            match ak.str("requestLocation") {
                "QUERY_STRING" => query.push(format!("{}={}", ak.str("keyName"), cx_http::percent_encode(ak.str("apiKey")))),
                _ => headers.push((ak.str("keyName").to_string(), ak.str("apiKey").to_string())),
            }
        }
        if !query.is_empty() {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(&query.join("&"));
        }
        let body = if method == "GET" || method == "DELETE" {
            String::new()
        } else if let Some(b) = remaining.get("requestBody") {
            b.to_string()
        } else {
            Value::Object(remaining).to_string()
        };
        let (code, resp) = self.rt.transport.send(&method, &url, &headers, &body, Duration::from_secs(10))?;
        self.log("Tool", obj! {"tool" => tool.str("displayName"), "action" => action, "url" => url, "status" => code});
        let parsed = Value::parse(&resp).unwrap_or(Value::String(resp));
        if code >= 300 {
            return Err(format!("HTTP {}: {}", code, parsed));
        }
        Ok(parsed)
    }

    // ---- offline example replay -------------------------------------------

    fn replay(&mut self, pb: &'a Value, text: Option<&str>) {
        let examples = pb.arr("examples");
        let cursor = self.st.playbook.as_ref().and_then(|s| s.cursor);
        let mut start: Option<(usize, usize)> = None;
        match text {
            Some(t) => {
                if let Some((e, i)) = cursor {
                    if let Some(u) = examples.get(e).and_then(|ex| ex.arr("actions").get(i)).and_then(|a| a.get("userUtterance")) {
                        if similarity(t, u.str("text")) >= 0.3 {
                            start = Some((e, i + 1));
                        }
                    }
                }
                if start.is_none() {
                    let mut best = (0.0, None);
                    for (ei, ex) in examples.iter().enumerate() {
                        for (ai, a) in ex.arr("actions").iter().enumerate() {
                            if let Some(u) = a.get("userUtterance") {
                                let s = similarity(t, u.str("text"));
                                if s > best.0 {
                                    best = (s, Some((ei, ai + 1)));
                                }
                            }
                        }
                    }
                    if best.0 >= 0.25 {
                        start = best.1;
                    }
                }
                if start.is_none() {
                    self.log("Playbook", obj! {"mode" => "example-replay", "matched" => false});
                    let goal = pb.str("goal");
                    let msg = if goal.is_empty() {
                        "Sorry, I didn't quite get that. Could you rephrase?".to_string()
                    } else {
                        format!("Sorry, I didn't quite get that. I can help with: {}", goal)
                    };
                    self.say(&msg);
                    return;
                }
            }
            None => match cursor {
                Some(c) => start = Some(c),
                None => return,
            },
        }
        let (e, i) = start.unwrap();
        let ex = &examples[e];
        self.log("Playbook", obj! {"mode" => "example-replay", "example" => ex.str("displayName")});
        let actions = ex.arr("actions");
        let mut idx = i;
        while idx < actions.len() {
            let a = &actions[idx];
            if a.get("userUtterance").is_some() {
                if let Some(s) = self.pb_state() {
                    s.cursor = Some((e, idx));
                }
                return;
            }
            if let Some(u) = a.get("agentUtterance") {
                let t = u.str("text").to_string();
                self.say(&t);
            } else if let Some(tu) = a.get("toolUse") {
                self.log("Tool", obj! {"tool" => tu.str("tool"), "replayed" => true, "output" => tu.at("outputActionParameters").clone()});
                if let Some(s) = self.pb_state() {
                    s.history.push(a.clone());
                }
            } else if let Some(pi) = a.get("playbookInvocation") {
                let target = pi.str("playbook").to_string();
                if self.switch_playbook(&target) {
                    self.playbook_user_turn(None);
                }
                return;
            } else if let Some(fi) = a.get("flowInvocation") {
                let f = fi.str("flow").to_string();
                if let Some(s) = self.pb_state() {
                    s.cursor = Some((e, idx + 1));
                }
                self.invoke_flow(&f);
                return;
            }
            idx += 1;
        }
        if let Some(s) = self.pb_state() {
            s.cursor = None;
        }
        if let Some(po) = ex.get("playbookOutput") {
            let out = po.at("actionParameters").clone();
            self.end_playbook(&out);
        }
    }
}

fn tool_kind(t: &Value) -> &'static str {
    if t.get("openApiSpec").is_some() {
        "openapi"
    } else if t.get("functionSpec").is_some() {
        "function"
    } else if t.get("dataStoreSpec").is_some() {
        "datastore"
    } else {
        "unknown"
    }
}

/// Lists the action ids (OpenAPI operationIds) a tool exposes.
pub fn tool_actions(t: &Value) -> Vec<String> {
    let Some(spec) = t.get("openApiSpec") else {
        return if t.get("functionSpec").is_some() { vec![t.str("displayName").to_string()] } else { vec![] };
    };
    let text = spec.str("textSchema");
    let Ok(schema) = Value::parse(text).or_else(|_| yaml::parse(text)) else { return vec![] };
    let mut out = Vec::new();
    if let Some(paths) = schema.get("paths").and_then(Value::as_object) {
        for (_, item) in paths.iter() {
            if let Some(ops) = item.as_object() {
                for (_, op) in ops.iter() {
                    let id = op.str("operationId");
                    if !id.is_empty() {
                        out.push(id.to_string());
                    }
                }
            }
        }
    }
    out
}
