//! Turn processing: implements `detectIntent` semantics of Conversational
//! Agents (Dialogflow CX) — handler scope and evaluation order, form
//! filling, event handlers, the flow stack, fulfillment (presets, messages,
//! conditional responses, generators, webhooks) and playbook hand-off.

use crate::agent::Agent;
use crate::expr::{self, Scope};
use crate::llm::{base64, HttpTransport, Llm, NoLlm, Transport};
use crate::session::{Frame, SessionState};
use cx_core::name::{id_of, is_special_page, START_PAGE};
use cx_core::{arr, obj, Map, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Shared runtime services.
#[derive(Clone)]
pub struct Runtime {
    pub llm: Arc<dyn Llm>,
    pub transport: Arc<dyn Transport>,
    pub max_transitions: usize,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime { llm: Arc::new(NoLlm), transport: Arc::new(HttpTransport), max_transitions: 64 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Target {
    Page(String),
    Flow(String),
    Playbook(String),
}

pub(crate) fn route_target(r: &Value) -> Option<Target> {
    let p = r.str("targetPage");
    let f = r.str("targetFlow");
    let pb = r.str("targetPlaybook");
    if !p.is_empty() {
        Some(Target::Page(p.to_string()))
    } else if !f.is_empty() {
        Some(Target::Flow(f.to_string()))
    } else if !pb.is_empty() {
        Some(Target::Playbook(pb.to_string()))
    } else {
        None
    }
}

/// Per-turn working state.
pub struct Turn<'a> {
    pub(crate) agent: &'a Agent,
    pub(crate) rt: &'a Runtime,
    pub(crate) st: &'a mut SessionState,
    pub(crate) session: String,
    pub(crate) response_id: String,
    pub(crate) messages: Vec<Value>,
    pub(crate) diag: Vec<Value>,
    pub(crate) intent: Option<(String, String, f64)>,
    pub(crate) intent_params: Map,
    pub(crate) match_type: &'static str,
    pub(crate) event: Option<String>,
    pub(crate) user_text: String,
    pub(crate) input_kind: &'static str,
    pub(crate) language: String,
    pub(crate) transitions: usize,
    pub(crate) webhook_statuses: Vec<Value>,
    pub(crate) webhook_payloads: Vec<Value>,
    pub(crate) pending_target: Option<Target>,
    pub(crate) pending_events: Vec<String>,
    pub(crate) disable_webhook: bool,
    pub(crate) request_payload: Value,
    pub(crate) generative: Map,
    pub(crate) alternatives: Vec<Value>,
    pub(crate) halted: bool,
}

impl<'a> Scope for Turn<'a> {
    fn lookup(&self, path: &str) -> Value {
        if let Some(rest) = path.strip_prefix("session.params.") {
            return self.st.param(rest).clone();
        }
        if path == "session.id" || path == "request.session-id" {
            return Value::from(id_of(&self.session));
        }
        if path == "page.params.status" {
            return if self.form_complete() { Value::from("FINAL") } else { Value::Null };
        }
        if let Some(rest) = path.strip_prefix("page.params.") {
            return self.st.param(rest).clone();
        }
        if let Some(rest) = path.strip_prefix("flow.") {
            return self.st.param(rest).clone();
        }
        if let Some(rest) = path.strip_prefix("intent.params.") {
            let (id, field) = match rest.rsplit_once('.') {
                Some((id, f)) if f == "original" || f == "resolved" => (id, f),
                _ => (rest, "resolved"),
            };
            return self.intent_params.get(id).map(|p| p.at(field).clone()).unwrap_or_default();
        }
        match path {
            "intent.display-name" | "intent.displayName" => {
                return self.intent.as_ref().map(|i| Value::from(i.1.clone())).unwrap_or_default()
            }
            "intent.name" => return self.intent.as_ref().map(|i| Value::from(i.0.clone())).unwrap_or_default(),
            "request.user-utterance" | "request.text" => return Value::from(self.user_text.clone()),
            "request.language" => return Value::from(self.language.clone()),
            "sys.no-match-count" => return Value::from(self.st.no_match_count),
            "sys.no-input-count" => return Value::from(self.st.no_input_count),
            _ => {}
        }
        if let Some(rest) = path.strip_prefix("request.generative.") {
            return self.generative.get(rest).cloned().unwrap_or_default();
        }
        if let Some(rest) = path.strip_prefix("request.payload.") {
            return self.request_payload.path(rest).cloned().unwrap_or_default();
        }
        Value::Null
    }
}

impl Runtime {
    pub fn new(llm: Arc<dyn Llm>) -> Self {
        Runtime { llm, ..Default::default() }
    }

    /// Processes one `DetectIntentRequest` and returns a
    /// `DetectIntentResponse`.
    pub fn detect_intent(&self, agent: &Agent, session: &str, st: &mut SessionState, req: &Value) -> Value {
        let mut t = Turn {
            agent,
            rt: self,
            st,
            session: session.to_string(),
            response_id: cx_core::uuid::new_v4(),
            messages: Vec::new(),
            diag: Vec::new(),
            intent: None,
            intent_params: Map::new(),
            match_type: "MATCH_TYPE_UNSPECIFIED",
            event: None,
            user_text: String::new(),
            input_kind: "text",
            language: agent.language().to_string(),
            transitions: 0,
            webhook_statuses: Vec::new(),
            webhook_payloads: Vec::new(),
            pending_target: None,
            pending_events: Vec::new(),
            disable_webhook: false,
            request_payload: Value::Null,
            generative: Map::new(),
            alternatives: Vec::new(),
            halted: false,
        };
        t.run(req);
        t.finish(req)
    }
}

impl<'a> Turn<'a> {
    pub(crate) fn log(&mut self, step: &str, detail: Value) {
        self.diag.push(obj! {"step" => self.diag.len() + 1, "type" => step, "detail" => detail});
    }

    fn run(&mut self, req: &Value) {
        let qp = req.at("queryParams");
        let qi = req.at("queryInput");
        if let Some(l) = qi.get("languageCode").and_then(Value::as_str) {
            self.language = l.to_string();
        }
        if self.st.ended {
            let keep = self.st.session_entity_types.clone();
            *self.st = SessionState::new();
            self.st.session_entity_types = keep;
            self.log("Session", Value::from("previous session ended; starting a new conversation"));
        }
        self.st.turn += 1;
        if let Some(params) = qp.get("parameters").and_then(Value::as_object) {
            for (k, v) in params.iter() {
                self.st.set_param(k, v.clone());
            }
        }
        if qp.get("sessionEntityTypes").map_or(false, |v| v.is_array()) {
            self.st.session_entity_types = qp.arr("sessionEntityTypes").to_vec();
        }
        self.disable_webhook = qp.bool_field("disableWebhook");
        self.request_payload = qp.at("payload").clone();
        let cp = qp.str("currentPage");
        if !cp.is_empty() {
            if let Some((f, p)) = self.agent.page_any(cp) {
                self.st.flow = f.str("name").to_string();
                self.st.page = p.str("name").to_string();
                self.st.stack.clear();
                self.st.requested_param = None;
            } else if id_of(cp) == START_PAGE {
                let flow = cx_core::name::parent_of(cp).to_string();
                if let Some(f) = self.agent.flow(&flow) {
                    self.st.flow = f.str("name").to_string();
                    self.st.page = START_PAGE.into();
                }
            }
        }
        if self.st.flow.is_empty() && self.st.playbook.is_none() {
            if let Some(f) = self.agent.start_flow() {
                self.st.flow = f;
                self.st.page = START_PAGE.into();
            } else if let Some(p) = self.agent.start_playbook() {
                self.st.playbook = Some(crate::session::PlaybookState { name: p, ..Default::default() });
            }
        }

        if let Some(text) = qi.get("text") {
            self.user_text = text.str("text").to_string();
            self.input_kind = "text";
            if self.user_text.trim().is_empty() {
                self.no_input();
            } else if self.st.playbook.is_some() {
                let text = self.user_text.clone();
                self.match_type = "PLAYBOOK";
                self.playbook_user_turn(Some(&text));
            } else {
                let text = self.user_text.clone();
                self.handle_text(&text);
            }
        } else if let Some(dtmf) = qi.get("dtmf") {
            self.input_kind = "dtmf";
            let digits = format!("{}{}", dtmf.str("digits"), dtmf.str("finishDigit"));
            self.user_text = dtmf.str("digits").to_string();
            if digits.is_empty() {
                self.no_input();
            } else {
                let text = self.user_text.clone();
                self.handle_text(&text);
            }
        } else if let Some(ev) = qi.get("event") {
            self.input_kind = "event";
            let name = ev.str("event").to_string();
            self.event = Some(name.clone());
            self.match_type = "EVENT";
            if name == "sys.no-input-default" || name.starts_with("sys.no-input") {
                self.no_input();
            } else if !self.handle_event(&name) {
                self.log("Event", obj! {"event" => name, "handled" => false});
            }
        } else if let Some(it) = qi.get("intent") {
            self.input_kind = "intent";
            let r = it.str("intent").to_string();
            self.direct_intent(&r);
        } else if let Some(tc) = qi.get("toolCallResult") {
            self.input_kind = "toolCallResult";
            self.match_type = "PLAYBOOK";
            self.playbook_tool_result(tc);
        } else {
            self.no_input();
        }
        self.drain_events();
    }

    fn drain_events(&mut self) {
        let mut guard = 0;
        while let Some(ev) = (!self.pending_events.is_empty()).then(|| self.pending_events.remove(0)) {
            guard += 1;
            if guard > 10 || self.halted {
                break;
            }
            if !self.handle_event(&ev) {
                self.log("Event", obj! {"event" => ev, "handled" => false});
            }
        }
    }

    fn no_input(&mut self) {
        self.match_type = "NO_INPUT";
        self.st.no_input_count += 1;
        let n = self.st.no_input_count;
        let specific = format!("sys.no-input-{}", n);
        if !self.handle_event(&specific) && !self.handle_event("sys.no-input-default") {
            self.log("Event", obj! {"event" => "sys.no-input-default", "handled" => false});
        }
    }

    fn no_match(&mut self) {
        self.match_type = "NO_MATCH";
        self.st.no_match_count += 1;
        let n = self.st.no_match_count;
        let specific = format!("sys.no-match-{}", n);
        if !self.handle_event(&specific) && !self.handle_event("sys.no-match-default") {
            self.log("Event", obj! {"event" => "sys.no-match-default", "handled" => false});
        }
    }

    // ---- scope helpers ------------------------------------------------------

    pub(crate) fn current_flow(&self) -> Option<&'a Value> {
        self.agent.flow(&self.st.flow)
    }

    pub(crate) fn current_page(&self) -> Option<&'a Value> {
        if self.st.page.is_empty() || is_special_page(id_of(&self.st.page)) {
            return None;
        }
        self.agent.page(&self.st.flow, &self.st.page).or_else(|| self.agent.page_any(&self.st.page).map(|(_, p)| p))
    }

    fn form_params(&self) -> &'a [Value] {
        self.current_page().map(|p| p.at("form").arr("parameters")).unwrap_or(&[])
    }

    fn form_complete(&self) -> bool {
        self.form_params().iter().filter(|p| p.bool_field("required")).all(|p| !self.st.param(p.str("displayName")).is_null())
    }

    /// Routes in scope, in evaluation order. `with_intent` selects intent
    /// routes (true) or condition-only routes (false).
    fn routes_in_scope(&self, with_intent: bool) -> Vec<(&'a Value, String)> {
        let mut out = Vec::new();
        let flow_name = self.st.flow.clone();
        let push_from = |holder: &'a Value, origin: &str, out: &mut Vec<(&'a Value, String)>| {
            for r in holder.arr("transitionRoutes") {
                let has_intent = !r.str("intent").is_empty();
                if has_intent == with_intent {
                    out.push((r, origin.to_string()));
                }
            }
            for g in holder.arr("transitionRouteGroups") {
                if let Some(group) = g.as_str().and_then(|g| self.agent.route_group(&flow_name, g)) {
                    for r in group.arr("transitionRoutes") {
                        let has_intent = !r.str("intent").is_empty();
                        if has_intent == with_intent {
                            out.push((r, format!("{} > route group '{}'", origin, group.str("displayName"))));
                        }
                    }
                }
            }
        };
        if let Some(p) = self.current_page() {
            push_from(p, &format!("page '{}'", p.str("displayName")), &mut out);
        }
        if let Some(f) = self.current_flow() {
            let on_start = id_of(&self.st.page) == START_PAGE || self.st.page.is_empty();
            // Flow-level condition routes are only evaluated on the start page.
            if with_intent || on_start {
                push_from(f, &format!("flow '{}'", f.str("displayName")), &mut out);
            }
        }
        out
    }

    // ---- input handling -----------------------------------------------------

    fn handle_text(&mut self, text: &str) {
        let overrides = self.agent.session_entity_defs(&self.st.session_entity_types);
        let results = self.agent.classifier.classify(text, &overrides);
        let threshold = self.agent.threshold_for(&self.st.flow);
        let routes = self.routes_in_scope(true);
        self.alternatives = results
            .iter()
            .take(5)
            .map(|r| obj! {"intent" => r.key.clone(), "displayName" => r.display_name.clone(), "confidence" => (r.confidence * 1000.0).round() / 1000.0})
            .collect();
        let in_scope = |key: &str| routes.iter().any(|(r, _)| self.agent.intent(r.str("intent")).map_or(false, |i| i.str("name") == key));
        let negative_top = results.first().map_or(false, |r| r.confidence >= threshold && self.agent.is_negative_intent(&r.key));
        let best = if negative_top {
            None
        } else {
            results.iter().find(|r| r.confidence >= threshold && !self.agent.is_negative_intent(&r.key) && in_scope(&r.key))
        };

        // Form filling for the parameter currently being requested.
        let mut fill = None;
        if let Some(req_param) = self.st.requested_param.clone() {
            if let Some(p) = self.form_params().iter().find(|p| p.str("displayName") == req_param) {
                let et = self.agent.entity_key(p.str("entityType"));
                fill = self.agent.classifier.extract(text, &et, p.bool_field("isList"), &overrides).map(|v| (p, v));
            }
        }
        self.log(
            "NLU",
            obj! {
                "input" => text,
                "topIntent" => results.first().map(|r| r.display_name.clone()),
                "topConfidence" => results.first().map(|r| (r.confidence * 1000.0).round() / 1000.0),
                "threshold" => threshold,
                "matchedInScope" => best.map(|b| b.display_name.clone()),
            },
        );

        let intent_wins = match (&best, &fill) {
            (Some(b), Some(_)) => b.confidence >= 0.75,
            (Some(_), None) => true,
            _ => false,
        };
        if intent_wins {
            let m = best.unwrap().clone();
            self.match_type = "INTENT";
            self.st.no_match_count = 0;
            self.set_intent(&m.key, &m.display_name, m.confidence, &m.params);
            for (route, origin) in routes {
                let Some(i) = self.agent.intent(route.str("intent")) else { continue };
                if i.str("name") != m.key {
                    continue;
                }
                if self.check_condition(route.str("condition")) {
                    self.fire(route, &origin, "Route");
                    return;
                }
            }
        } else if let Some((p, v)) = fill {
            self.match_type = "PARAMETER_FILLING";
            self.st.no_match_count = 0;
            let name = p.str("displayName").to_string();
            self.log("Form", obj! {"filled" => name.clone(), "value" => v.resolved.clone(), "original" => v.original.clone()});
            self.st.set_param(&name, v.resolved);
            self.st.requested_param = None;
            self.st.reprompt_count = 0;
            // Opportunistically fill other empty form parameters.
            let others: Vec<&Value> = self
                .form_params()
                .iter()
                .filter(|q| q.str("displayName") != name && self.st.param(q.str("displayName")).is_null())
                .collect();
            for q in others {
                let et = self.agent.entity_key(q.str("entityType"));
                if cx_nlu::system::is_open(&et) {
                    continue;
                }
                if let Some(v) = self.agent.classifier.extract(text, &et, q.bool_field("isList"), &overrides) {
                    self.log("Form", obj! {"filled" => q.str("displayName"), "value" => v.resolved.clone()});
                    self.st.set_param(q.str("displayName"), v.resolved);
                }
            }
            self.settle_page();
            return;
        }
        if self.evaluate_condition_routes() {
            return;
        }
        self.no_match();
    }

    fn set_intent(&mut self, key: &str, display: &str, confidence: f64, params: &[(String, cx_nlu::ParamValue)]) {
        self.intent = Some((key.to_string(), display.to_string(), confidence));
        for (id, pv) in params {
            self.intent_params.insert(id.clone(), obj! {"original" => pv.original.clone(), "resolved" => pv.resolved.clone()});
            // Intent parameters propagate to session (and thus form) parameters.
            self.st.set_param(id, pv.resolved.clone());
        }
        self.log(
            "Intent",
            obj! {
                "intent" => display,
                "confidence" => (confidence * 1000.0).round() / 1000.0,
                "parameters" => Value::Object(self.intent_params.clone()),
            },
        );
    }

    fn direct_intent(&mut self, r: &str) {
        let Some(intent) = self.agent.intent(r) else {
            self.log("Intent", obj! {"error" => format!("unknown intent '{}'", r)});
            self.no_match();
            return;
        };
        self.match_type = "DIRECT_INTENT";
        let key = intent.str("name").to_string();
        let display = intent.str("displayName").to_string();
        self.set_intent(&key, &display, 1.0, &[]);
        for (route, origin) in self.routes_in_scope(true) {
            if self.agent.intent(route.str("intent")).map_or(false, |i| i.str("name") == key) && self.check_condition(route.str("condition")) {
                self.fire(route, &origin, "Route");
                return;
            }
        }
        if !self.evaluate_condition_routes() {
            self.no_match();
        }
    }

    pub(crate) fn check_condition(&mut self, cond: &str) -> bool {
        match expr::eval_condition(cond, &*self) {
            Ok(b) => b,
            Err(e) => {
                self.log("Error", obj! {"condition" => cond, "error" => e});
                false
            }
        }
    }

    /// Evaluates condition-only routes in scope; fires the first true one.
    fn evaluate_condition_routes(&mut self) -> bool {
        if self.halted {
            return true;
        }
        for (route, origin) in self.routes_in_scope(false) {
            let cond = route.str("condition");
            if cond.trim().is_empty() {
                continue;
            }
            if self.check_condition(cond) {
                self.fire(route, &origin, "Condition route");
                return true;
            }
        }
        false
    }

    /// Finds and fires an event handler in scope.
    pub(crate) fn handle_event(&mut self, event: &str) -> bool {
        let mut scopes: Vec<(&'a Value, String)> = Vec::new();
        if let Some(rp) = self.st.requested_param.clone() {
            if let Some(p) = self.form_params().iter().find(|p| p.str("displayName") == rp) {
                for h in p.at("fillBehavior").arr("repromptEventHandlers") {
                    scopes.push((h, format!("parameter '{}'", rp)));
                }
            }
        }
        if let Some(p) = self.current_page() {
            for h in p.arr("eventHandlers") {
                scopes.push((h, format!("page '{}'", p.str("displayName"))));
            }
        }
        if let Some(f) = self.current_flow() {
            for h in f.arr("eventHandlers") {
                scopes.push((h, format!("flow '{}'", f.str("displayName"))));
            }
        }
        for (h, origin) in scopes {
            if h.str("event") == event && self.check_condition(h.str("condition")) {
                self.event = Some(event.to_string());
                self.fire(h, &origin, &format!("Event handler ({})", event));
                return true;
            }
        }
        false
    }

    /// Runs a route or handler: fulfillment then transition.
    pub(crate) fn fire(&mut self, handler: &Value, origin: &str, kind: &str) {
        if self.halted {
            return;
        }
        self.log(
            kind,
            obj! {
                "origin" => origin,
                "intent" => handler.get("intent").cloned(),
                "condition" => handler.get("condition").cloned(),
                "event" => handler.get("event").cloned(),
                "target" => route_target(handler).map(|t| format!("{:?}", t)),
            },
        );
        self.pending_target = None;
        self.run_fulfillment(handler.at("triggerFulfillment"));
        // Events raised by the fulfillment (e.g. webhook.error) are handled
        // in the current scope; a handled event pre-empts the transition.
        if !self.pending_events.is_empty() {
            let events = std::mem::take(&mut self.pending_events);
            let mut handled = false;
            for ev in events {
                if !self.halted && self.handle_event(&ev) {
                    handled = true;
                }
            }
            if handled {
                return;
            }
        }
        let target = self.pending_target.take().or_else(|| route_target(handler));
        if let Some(t) = target {
            self.transition(t);
        }
    }

    // ---- transitions ------------------------------------------------------

    pub(crate) fn transition(&mut self, t: Target) {
        if self.halted {
            return;
        }
        self.transitions += 1;
        if self.transitions > self.rt.max_transitions {
            self.log("Error", Value::from("too many transitions in one turn (possible loop); halting"));
            self.halted = true;
            return;
        }
        match t {
            Target::Page(p) => match id_of(&p) {
                cx_core::name::END_SESSION => {
                    self.log("Transition", obj! {"to" => "END_SESSION"});
                    self.st.ended = true;
                    self.st.requested_param = None;
                    self.halted = true;
                }
                "END_FLOW" | "END_FLOW_WITH_HUMAN_ESCALATION" | "END_FLOW_WITH_FAILURE" | "END_FLOW_WITH_CANCELLATION" => {
                    let id = id_of(&p).to_string();
                    if id == "END_FLOW_WITH_HUMAN_ESCALATION" {
                        self.messages.push(obj! {"liveAgentHandoff" => obj!{"metadata" => obj!{}}});
                    }
                    self.end_flow(&id);
                }
                cx_core::name::PREVIOUS_PAGE => {
                    let prev = self.st.previous_page.clone();
                    if prev.is_empty() || id_of(&prev) == START_PAGE {
                        let f = self.st.flow.clone();
                        self.enter_start(&f);
                    } else {
                        self.enter_page(&prev);
                    }
                }
                cx_core::name::CURRENT_PAGE => {
                    let cur = self.st.page.clone();
                    if id_of(&cur) == START_PAGE {
                        let f = self.st.flow.clone();
                        self.enter_start(&f);
                    } else {
                        self.enter_page(&cur);
                    }
                }
                START_PAGE => {
                    // `.../flows/<f>/pages/START_PAGE` may name another flow.
                    let flow = cx_core::name::parent_of(&p);
                    let f = if !flow.is_empty() { self.agent.flow(flow).map(|f| f.str("name").to_string()) } else { None };
                    let f = f.unwrap_or_else(|| self.st.flow.clone());
                    self.enter_start(&f);
                }
                _ => {
                    let flow = self.st.flow.clone();
                    let found = self.agent.page(&flow, &p).map(|pg| (flow.clone(), pg)).or_else(|| {
                        self.agent.page_any(&p).map(|(f, pg)| (f.str("name").to_string(), pg)).or_else(|| {
                            // display-name lookup across flows
                            self.agent.flows().iter().find_map(|f| crate::agent::find_in(f.arr("pages"), &p).map(|pg| (f.str("name").to_string(), pg)))
                        })
                    });
                    match found {
                        Some((f, pg)) => {
                            self.st.flow = f;
                            let n = pg.str("name").to_string();
                            self.enter_page(&n);
                        }
                        None => self.log("Error", obj! {"error" => format!("target page '{}' not found", p)}),
                    }
                }
            },
            Target::Flow(f) => match self.agent.flow(&f) {
                Some(flow) => {
                    let n = flow.str("name").to_string();
                    self.st.stack.push(Frame { flow: self.st.flow.clone(), page: self.st.page.clone(), playbook: None });
                    self.enter_start(&n);
                }
                None => self.log("Error", obj! {"error" => format!("target flow '{}' not found", f)}),
            },
            Target::Playbook(p) => match self.agent.playbook(&p) {
                Some(pb) => {
                    let n = pb.str("name").to_string();
                    self.st.stack.push(Frame { flow: self.st.flow.clone(), page: self.st.page.clone(), playbook: None });
                    self.start_playbook(&n);
                }
                None => self.log("Error", obj! {"error" => format!("target playbook '{}' not found", p)}),
            },
        }
    }

    pub(crate) fn enter_start(&mut self, flow: &str) {
        self.st.previous_page = self.st.page.clone();
        self.st.flow = flow.to_string();
        self.st.page = START_PAGE.into();
        self.st.requested_param = None;
        let dn = self.current_flow().map(|f| f.str("displayName").to_string()).unwrap_or_default();
        self.log("Transition", obj! {"to" => format!("flow '{}' (Start Page)", dn)});
        self.evaluate_condition_routes();
    }

    fn enter_page(&mut self, page: &str) {
        self.st.previous_page = self.st.page.clone();
        self.st.page = page.to_string();
        self.st.requested_param = None;
        self.st.reprompt_count = 0;
        let Some(p) = self.current_page() else {
            self.log("Error", obj! {"error" => format!("page '{}' not found", page)});
            return;
        };
        self.log("Transition", obj! {"to" => format!("page '{}'", p.str("displayName"))});
        for fp in p.at("form").arr("parameters") {
            let dv = fp.at("fillBehavior").get("defaultValue").or_else(|| fp.get("defaultValue"));
            if let Some(dv) = dv {
                if self.st.param(fp.str("displayName")).is_null() && !dv.is_null() {
                    let v = expr::eval_value(dv, &*self);
                    self.st.set_param(fp.str("displayName"), v);
                }
            }
        }
        self.pending_target = None;
        self.run_fulfillment(p.at("entryFulfillment"));
        if let Some(t) = self.pending_target.take() {
            self.transition(t);
            return;
        }
        if !self.pending_events.is_empty() {
            self.drain_events();
            if self.halted {
                return;
            }
        }
        self.settle_page();
    }

    /// After entry or a form update: evaluate condition routes, then prompt
    /// for the next required parameter.
    pub(crate) fn settle_page(&mut self) {
        if self.halted {
            return;
        }
        if self.evaluate_condition_routes() {
            return;
        }
        let page_name = self.st.page.clone();
        for fp in self.form_params() {
            let name = fp.str("displayName");
            if fp.bool_field("required") && self.st.param(name).is_null() {
                self.st.requested_param = Some(name.to_string());
                self.log("Form", obj! {"requesting" => name});
                self.pending_target = None;
                self.run_fulfillment(fp.at("fillBehavior").at("initialPromptFulfillment"));
                if let Some(t) = self.pending_target.take() {
                    if self.st.page == page_name {
                        self.transition(t);
                    }
                }
                return;
            }
        }
        self.st.requested_param = None;
    }

    fn end_flow(&mut self, how: &str) {
        let dn = self.current_flow().map(|f| f.str("displayName").to_string()).unwrap_or_default();
        self.log("Transition", obj! {"to" => how, "flow" => dn});
        match self.st.stack.pop() {
            Some(frame) => {
                self.st.flow = frame.flow;
                self.st.page = frame.page;
                self.st.requested_param = None;
                if let Some(pb) = frame.playbook {
                    // a playbook invoked this flow: resume it
                    self.st.playbook = Some(crate::session::PlaybookState { name: pb, ..Default::default() });
                    self.playbook_user_turn(None);
                } else {
                    self.settle_page();
                }
            }
            None => {
                self.st.ended = true;
                self.halted = true;
            }
        }
    }

    // ---- fulfillment ------------------------------------------------------

    pub(crate) fn run_fulfillment(&mut self, f: &Value) {
        if !f.is_object() {
            return;
        }
        let start = self.messages.len();
        for a in f.arr("setParameterActions") {
            let v = expr::eval_value(a.at("value"), &*self);
            let key = a.str("parameter").trim_start_matches("$session.params.").to_string();
            self.log("Parameter preset", obj! {"parameter" => key.clone(), "value" => v.clone()});
            self.st.set_param(&key, v);
        }
        for m in f.arr("messages") {
            if let Some(rendered) = self.render_message(m) {
                self.messages.push(rendered);
            }
        }
        for cc in f.arr("conditionalCases") {
            let cases = cc.arr("cases").to_vec();
            self.run_cases(&cases);
        }
        for g in f.arr("generators") {
            self.run_generator(g);
        }
        let wh = f.str("webhook");
        if !wh.is_empty() {
            if self.disable_webhook {
                self.log("Webhook", obj! {"webhook" => wh, "skipped" => "disabled by request"});
            } else {
                self.call_webhook(wh, f.str("tag"), start);
            }
        }
    }

    fn run_cases(&mut self, cases: &[Value]) {
        for c in cases {
            if self.check_condition(c.str("condition")) {
                for content in c.arr("caseContent") {
                    if let Some(m) = content.get("message") {
                        if let Some(r) = self.render_message(m) {
                            self.messages.push(r);
                        }
                    }
                    if let Some(ac) = content.get("additionalCases") {
                        let nested = ac.arr("cases").to_vec();
                        self.run_cases(&nested);
                    }
                }
                return;
            }
        }
    }

    pub(crate) fn render_message(&mut self, m: &Value) -> Option<Value> {
        let mut out = Value::object();
        if let Some(ch) = m.get("channel") {
            out.set("channel", ch.clone());
        }
        if let Some(t) = m.get("text") {
            let variants: Vec<String> = t.arr("text").iter().filter_map(Value::as_str).map(|s| expr::render_template(s, &*self)).collect();
            if variants.is_empty() {
                return None;
            }
            let pick = (cx_core::uuid::random_u64() as usize) % variants.len();
            let mut tv = obj! {"text" => arr![variants[pick].clone()]};
            if t.bool_field("allowPlaybackInterruption") {
                tv.set("allowPlaybackInterruption", true);
            }
            out.set("text", tv);
            return Some(out);
        }
        if let Some(p) = m.get("payload") {
            out.set("payload", expr::eval_value(p, &*self));
            return Some(out);
        }
        if let Some(a) = m.get("outputAudioText") {
            let mut a2 = a.clone();
            if let Some(s) = a.get("text").and_then(Value::as_str) {
                a2.set("text", expr::render_template(s, &*self));
            }
            if let Some(s) = a.get("ssml").and_then(Value::as_str) {
                a2.set("ssml", expr::render_template(s, &*self));
            }
            out.set("outputAudioText", a2);
            return Some(out);
        }
        if let Some(ka) = m.get("knowledgeInfoCard") {
            out.set("knowledgeInfoCard", ka.clone());
            return Some(out);
        }
        // conversationSuccess, liveAgentHandoff, playAudio, mixedAudio,
        // telephonyTransferCall, endInteraction: pass through with params.
        let mut any = false;
        if let Some(obj) = m.as_object() {
            for (k, v) in obj.iter() {
                if k != "channel" {
                    out.set(k.clone(), expr::eval_value(v, &*self));
                    any = true;
                }
            }
        }
        any.then_some(out)
    }

    fn run_generator(&mut self, g: &Value) {
        let Some(gen) = self.agent.generator(g.str("generator")) else {
            self.log("Error", obj! {"error" => format!("generator '{}' not found", g.str("generator"))});
            return;
        };
        let mut prompt = gen.at("promptText").str("text").to_string();
        // Placeholders: inputParameters maps placeholder name -> value.
        if let Some(inputs) = g.get("inputParameters").and_then(Value::as_object) {
            let mut pairs: Vec<(String, String)> = inputs
                .iter()
                .map(|(k, v)| (k.trim_start_matches('$').to_string(), expr::display(&expr::eval_value(v, &*self))))
                .collect();
            pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
            for (k, v) in pairs {
                prompt = prompt.replace(&format!("${}", k), &v);
            }
        }
        let prompt = expr::render_template(&prompt, &*self);
        let temperature = gen.path("modelParameter.temperature").and_then(Value::as_f64).unwrap_or(0.3);
        let out_param = g.str("outputParameter").to_string();
        let result = self.rt.llm.chat("You are a helpful assistant. Follow the instructions in the prompt.", &[("user".into(), prompt.clone())], temperature);
        let text = match result {
            Ok(t) => t.trim().to_string(),
            Err(e) => {
                self.log("Generator", obj! {"generator" => gen.str("displayName"), "error" => e});
                return;
            }
        };
        self.log("Generator", obj! {"generator" => gen.str("displayName"), "output" => text.clone()});
        if let Some(rest) = out_param.strip_prefix("$request.generative.") {
            self.generative.insert(rest, Value::from(text));
        } else {
            let key = out_param.trim_start_matches("$session.params.").trim_start_matches('$').to_string();
            if !key.is_empty() {
                self.st.set_param(&key, Value::from(text));
            }
        }
    }

    // ---- webhooks ---------------------------------------------------------

    fn form_info(&self) -> Value {
        let infos: Vec<Value> = self
            .form_params()
            .iter()
            .map(|p| {
                let v = self.st.param(p.str("displayName")).clone();
                obj! {
                    "displayName" => p.str("displayName"),
                    "required" => p.bool_field("required"),
                    "state" => if v.is_null() { "EMPTY" } else { "FILLED" },
                    "value" => v,
                }
            })
            .collect();
        obj! {"parameterInfo" => Value::Array(infos)}
    }

    pub(crate) fn page_ref(&self) -> Value {
        if let Some(p) = self.current_page() {
            obj! {"name" => p.str("name"), "displayName" => p.str("displayName")}
        } else if !self.st.flow.is_empty() {
            let name = if self.st.page.contains('/') { self.st.page.clone() } else { format!("{}/pages/{}", self.st.flow, if self.st.page.is_empty() { START_PAGE } else { &self.st.page }) };
            let dn = match id_of(&name) {
                START_PAGE => "Start Page".to_string(),
                other => other.to_string(),
            };
            obj! {"name" => name, "displayName" => dn}
        } else {
            Value::Null
        }
    }

    fn webhook_request(&self, tag: &str) -> Value {
        let mut req = obj! {
            "detectIntentResponseId" => self.response_id.clone(),
            "languageCode" => self.language.clone(),
            "fulfillmentInfo" => obj!{"tag" => tag},
            "pageInfo" => {
                let mut p = self.page_ref();
                if p.is_object() {
                    let name = p.str("name").to_string();
                    p.remove("name");
                    p.set("currentPage", name);
                    p.set("formInfo", self.form_info());
                }
                p
            },
            "sessionInfo" => obj!{"session" => self.session.clone(), "parameters" => self.st.params.clone()},
            "messages" => Value::Array(self.messages.clone()),
            "payload" => self.request_payload.clone(),
        };
        match self.input_kind {
            "event" => req.set("triggerEvent", self.event.clone()),
            "intent" => req.set("triggerIntent", self.intent.as_ref().map(|i| i.0.clone())),
            "dtmf" => req.set("dtmfDigits", self.user_text.clone()),
            _ => req.set("text", self.user_text.clone()),
        }
        if let Some((name, dn, conf)) = &self.intent {
            let params: Map = self
                .intent_params
                .iter()
                .map(|(k, v)| (k.clone(), obj! {"originalValue" => v.at("original").clone(), "resolvedValue" => v.at("resolved").clone()}))
                .collect();
            req.set(
                "intentInfo",
                obj! {"lastMatchedIntent" => name.clone(), "displayName" => dn.clone(), "confidence" => *conf, "parameters" => Value::Object(params)},
            );
        }
        req
    }

    fn call_webhook(&mut self, wh_ref: &str, tag: &str, msg_start: usize) {
        let Some(wh) = self.agent.webhook(wh_ref) else {
            self.log("Webhook", obj! {"error" => format!("webhook '{}' not found", wh_ref)});
            self.pending_events.push("webhook.error".into());
            return;
        };
        if wh.bool_field("disabled") {
            self.log("Webhook", obj! {"webhook" => wh.str("displayName"), "skipped" => "webhook disabled"});
            return;
        }
        let gws = wh.at("genericWebService");
        let uri = gws.str("uri").to_string();
        let timeout_s = wh.str("timeout").trim_end_matches('s').parse::<f64>().unwrap_or(5.0);
        let mut headers: Vec<(String, String)> = gws
            .get("requestHeaders")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.to_display())).collect())
            .unwrap_or_default();
        if !gws.str("username").is_empty() {
            headers.push(("Authorization".into(), format!("Basic {}", base64(format!("{}:{}", gws.str("username"), gws.str("password")).as_bytes()))));
        }
        let flexible = gws.str("webhookType") == "FLEXIBLE";
        let (method, body) = if flexible {
            let m = match gws.str("httpMethod") {
                "" => "POST",
                other => other,
            }
            .to_string();
            (m, expr::render_template(gws.str("requestBody"), &*self))
        } else {
            ("POST".to_string(), self.webhook_request(tag).to_string())
        };
        headers.push(("Content-Type".into(), "application/json".into()));
        let started = Instant::now();
        let result = self.rt.transport.send(&method, &uri, &headers, &body, Duration::from_secs_f64(timeout_s));
        let latency = format!("{:.3}s", started.elapsed().as_secs_f64());
        let dn = wh.str("displayName").to_string();
        let ok_body = match result {
            Ok((code, body)) if code < 300 => Some(body),
            Ok((code, body)) => {
                self.log("Webhook", obj! {"webhook" => dn.clone(), "tag" => tag, "status" => code, "error" => body});
                self.webhook_statuses.push(obj! {"code" => code, "message" => format!("webhook '{}' returned HTTP {}", dn, code)});
                None
            }
            Err(e) => {
                self.log("Webhook", obj! {"webhook" => dn.clone(), "tag" => tag, "error" => e.clone()});
                self.webhook_statuses.push(obj! {"code" => 14, "message" => e});
                None
            }
        };
        let Some(body) = ok_body else {
            self.pending_events.push("webhook.error".into());
            return;
        };
        let parsed = if body.trim().is_empty() { Ok(Value::object()) } else { Value::parse(&body) };
        let v = match parsed {
            Ok(v) => v,
            Err(e) => {
                self.log("Webhook", obj! {"webhook" => dn.clone(), "error" => format!("invalid JSON response: {}", e)});
                self.webhook_statuses.push(obj! {"code" => 13, "message" => "invalid webhook response"});
                self.pending_events.push("webhook.error".into());
                return;
            }
        };
        self.webhook_statuses.push(obj! {"code" => 0, "message" => "OK"});
        self.log("Webhook", obj! {"webhook" => dn, "tag" => tag, "latency" => latency, "response" => v.clone()});
        if flexible {
            if let Some(map) = gws.get("parameterMapping").and_then(Value::as_object) {
                for (param, path) in map.iter() {
                    if let Some(val) = json_path(&v, path.as_str().unwrap_or("")) {
                        self.st.set_param(param, val.clone());
                    }
                }
            }
            return;
        }
        self.apply_webhook_response(&v, msg_start);
    }

    pub(crate) fn apply_webhook_response(&mut self, v: &Value, msg_start: usize) {
        let fr = v.at("fulfillmentResponse");
        let msgs = fr.arr("messages").to_vec();
        if !msgs.is_empty() {
            if fr.str("mergeBehavior") == "REPLACE" {
                self.messages.truncate(msg_start);
            }
            self.messages.extend(msgs);
        }
        if let Some(params) = v.at("sessionInfo").get("parameters").and_then(Value::as_object) {
            for (k, val) in params.iter() {
                self.st.set_param(k, val.clone());
            }
        }
        for pi in v.at("pageInfo").at("formInfo").arr("parameterInfo") {
            let name = pi.str("displayName");
            match pi.str("state") {
                "INVALID" => {
                    self.st.set_param(name, Value::Null);
                }
                _ => {
                    if let Some(val) = pi.get("value") {
                        self.st.set_param(name, val.clone());
                    }
                }
            }
        }
        if let Some(p) = v.get("payload") {
            self.webhook_payloads.push(p.clone());
        }
        if !v.str("targetPage").is_empty() {
            self.pending_target = Some(Target::Page(v.str("targetPage").to_string()));
        } else if !v.str("targetFlow").is_empty() {
            self.pending_target = Some(Target::Flow(v.str("targetFlow").to_string()));
        }
    }

    // ---- response ---------------------------------------------------------

    fn finish(mut self, req: &Value) -> Value {
        if self.st.ended && !self.messages.iter().any(|m| m.get("endInteraction").is_some()) {
            self.messages.push(obj! {"endInteraction" => obj!{}});
        }
        let qi = req.at("queryInput");
        let mut qr = obj! {"languageCode" => self.language.clone()};
        match self.input_kind {
            "event" => qr.set("triggerEvent", self.event.clone()),
            "intent" => qr.set("triggerIntent", qi.at("intent").str("intent")),
            "dtmf" => qr.set("dtmf", qi.at("dtmf").clone()),
            "toolCallResult" => qr.set("toolCallResult", qi.at("toolCallResult").clone()),
            _ => qr.set("text", self.user_text.clone()),
        }
        qr.set("parameters", self.st.params.clone());
        qr.set("responseMessages", Value::Array(self.messages.clone()));
        if !self.webhook_statuses.is_empty() {
            qr.set("webhookStatuses", Value::Array(self.webhook_statuses.clone()));
        }
        if !self.webhook_payloads.is_empty() {
            qr.set("webhookPayloads", Value::Array(self.webhook_payloads.clone()));
        }
        if self.st.playbook.is_none() {
            qr.set("currentPage", self.page_ref());
            if let Some(f) = self.current_flow() {
                qr.set("currentFlow", obj! {"name" => f.str("name"), "displayName" => f.str("displayName")});
            }
        } else if let Some(pb) = self.st.playbook.as_ref().and_then(|p| self.agent.playbook(&p.name)) {
            qr.set("currentPlaybook", obj! {"name" => pb.str("name"), "displayName" => pb.str("displayName")});
        }
        let mut m = obj! {"matchType" => self.match_type, "resolvedInput" => self.user_text.clone()};
        if let Some((name, dn, conf)) = &self.intent {
            let iv = obj! {"name" => name.clone(), "displayName" => dn.clone()};
            qr.set("intent", iv.clone());
            qr.set("intentDetectionConfidence", (conf * 1000.0).round() / 1000.0);
            m.set("intent", iv);
            m.set("confidence", (conf * 1000.0).round() / 1000.0);
            let params: Map = self.intent_params.iter().map(|(k, v)| (k.clone(), v.at("resolved").clone())).collect();
            m.set("parameters", Value::Object(params));
        }
        if let Some(e) = &self.event {
            m.set("event", e.clone());
        }
        qr.set("match", m);
        let mut di = obj! {
            "Execution Sequence" => Value::Array(self.diag.clone()),
            "Session Id" => id_of(&self.session),
            "Response Id" => self.response_id.clone(),
            "Session Ended" => self.st.ended,
        };
        if !self.alternatives.is_empty() {
            di.set("Alternative Matched Intents", Value::Array(self.alternatives.clone()));
        }
        qr.set("diagnosticInfo", di);
        obj! {"responseId" => self.response_id.clone(), "queryResult" => qr, "responseType" => "FINAL"}
    }
}

/// Evaluates a simple JSON path like `$.a.b[0].c`.
pub fn json_path<'v>(v: &'v Value, path: &str) -> Option<&'v Value> {
    let p = path.trim().trim_start_matches('$').trim_start_matches('.');
    let normalized = p.replace('[', ".").replace(']', "");
    if normalized.is_empty() {
        return Some(v);
    }
    v.path(&normalized)
}

/// Convenience: concatenated text of all text messages in a response.
pub fn response_texts(resp: &Value) -> Vec<String> {
    resp.at("queryResult")
        .arr("responseMessages")
        .iter()
        .filter_map(|m| m.get("text"))
        .flat_map(|t| t.arr("text").iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>())
        .collect()
}
