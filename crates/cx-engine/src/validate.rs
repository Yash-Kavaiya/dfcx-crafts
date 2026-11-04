//! Agent validation (the console's "Validate agent" panel).

use crate::agent::Agent;
use crate::expr;
use cx_core::name::{id_of, is_special_page};
use cx_core::{obj, Value};
use cx_nlu::text::normalize;
use std::collections::{HashMap, HashSet};

struct Collector {
    flow_msgs: HashMap<String, Vec<Value>>,
    agent_msgs: Vec<Value>,
}

impl Collector {
    fn push(&mut self, flow: Option<&str>, severity: &str, resource_type: &str, resources: Vec<String>, detail: String) {
        let m = obj! {
            "resourceType" => resource_type,
            "resourceNames" => Value::Array(resources.iter().map(|r| obj!{"name" => r.clone()}).collect()),
            "severity" => severity,
            "detail" => detail,
        };
        match flow {
            Some(f) => self.flow_msgs.entry(f.to_string()).or_default().push(m),
            None => self.agent_msgs.push(m),
        }
    }
}

/// Validates an agent; returns an `AgentValidationResult`.
pub fn validate(agent: &Agent) -> Value {
    let mut c = Collector { flow_msgs: HashMap::new(), agent_msgs: Vec::new() };
    let mut referenced_intents: HashSet<String> = HashSet::new();

    let check_fulfillment = |c: &mut Collector, flow: &str, owner: &str, f: &Value| {
        if !f.is_object() {
            return;
        }
        let wh = f.str("webhook");
        if !wh.is_empty() && agent.webhook(wh).is_none() {
            c.push(Some(flow), "ERROR", "WEBHOOK", vec![owner.into()], format!("Fulfillment references unknown webhook '{}'", wh));
        }
        for g in f.arr("generators") {
            if agent.generator(g.str("generator")).is_none() {
                c.push(Some(flow), "ERROR", "GENERATOR", vec![owner.into()], format!("Fulfillment references unknown generator '{}'", g.str("generator")));
            }
        }
        fn cases(c: &mut Collector, flow: &str, owner: &str, cs: &[Value]) {
            for case in cs {
                if let Err(e) = expr::validate(case.str("condition")) {
                    c.push(Some(flow), "ERROR", "FULFILLMENT", vec![owner.into()], format!("Invalid conditional response condition '{}': {}", case.str("condition"), e));
                }
                for content in case.arr("caseContent") {
                    if let Some(ac) = content.get("additionalCases") {
                        cases(c, flow, owner, ac.arr("cases"));
                    }
                }
            }
        }
        for cc in f.arr("conditionalCases") {
            cases(c, flow, owner, cc.arr("cases"));
        }
    };

    for flow in agent.flows() {
        let fname = flow.str("name").to_string();
        let mut targeted: HashSet<String> = HashSet::new();
        let mut check_handler = |c: &mut Collector, owner: &str, h: &Value, referenced: &mut HashSet<String>| {
            let intent = h.str("intent");
            if !intent.is_empty() {
                match agent.intent(intent) {
                    Some(i) => {
                        referenced.insert(i.str("name").to_string());
                    }
                    None => c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE", vec![owner.into()], format!("Route references unknown intent '{}'", intent)),
                }
            }
            if let Err(e) = expr::validate(h.str("condition")) {
                c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE", vec![owner.into()], format!("Invalid condition '{}': {}", h.str("condition"), e));
            }
            if h.get("event").is_none() && intent.is_empty() && h.str("condition").trim().is_empty() && h.get("transitionRoutes").is_none() {
                c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE", vec![owner.into()], "Route needs an intent or a condition".into());
            }
            let tp = h.str("targetPage");
            if !tp.is_empty() {
                if is_special_page(id_of(tp)) {
                } else if let Some((_, p)) = agent.page_any(tp).or_else(|| agent.page(&fname, tp).map(|p| (flow, p))) {
                    targeted.insert(p.str("name").to_string());
                } else {
                    c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE", vec![owner.into()], format!("Route targets unknown page '{}'", tp));
                }
            }
            let tf = h.str("targetFlow");
            if !tf.is_empty() && agent.flow(tf).is_none() {
                c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE", vec![owner.into()], format!("Route targets unknown flow '{}'", tf));
            }
            let tpb = h.str("targetPlaybook");
            if !tpb.is_empty() && agent.playbook(tpb).is_none() {
                c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE", vec![owner.into()], format!("Route targets unknown playbook '{}'", tpb));
            }
            check_fulfillment(c, &fname, owner, h.at("triggerFulfillment"));
        };
        let holders: Vec<(&Value, String)> = std::iter::once((flow, format!("{} (Start Page)", flow.str("displayName"))))
            .chain(flow.arr("pages").iter().map(|p| (p, p.str("displayName").to_string())))
            .collect();
        for (holder, label) in &holders {
            for r in holder.arr("transitionRoutes") {
                check_handler(&mut c, label, r, &mut referenced_intents);
            }
            for h in holder.arr("eventHandlers") {
                check_handler(&mut c, label, h, &mut referenced_intents);
            }
            for g in holder.arr("transitionRouteGroups") {
                match g.as_str().and_then(|g| agent.route_group(&fname, g)) {
                    Some(group) => {
                        for r in group.arr("transitionRoutes") {
                            check_handler(&mut c, &format!("{} > {}", label, group.str("displayName")), r, &mut referenced_intents);
                        }
                    }
                    None => c.push(Some(&fname), "ERROR", "TRANSITION_ROUTE_GROUP", vec![label.clone()], format!("Unknown route group '{}'", g)),
                }
            }
            check_fulfillment(&mut c, &fname, label, holder.at("entryFulfillment"));
            for fp in holder.at("form").arr("parameters") {
                let et = fp.str("entityType");
                let key = agent.entity_key(et);
                if !key.starts_with("sys.") && agent.entity_type(&key).is_none() {
                    c.push(Some(&fname), "ERROR", "PAGE", vec![label.clone()], format!("Form parameter '{}' uses unknown entity type '{}'", fp.str("displayName"), et));
                } else if key.starts_with("sys.") && !cx_nlu::system::SYSTEM_ENTITIES.iter().any(|(n, _)| *n == key) {
                    c.push(Some(&fname), "WARNING", "PAGE", vec![label.clone()], format!("System entity '{}' is not supported by this runtime", key));
                }
                if fp.bool_field("required") && fp.at("fillBehavior").at("initialPromptFulfillment").arr("messages").is_empty()
                    && fp.at("fillBehavior").at("initialPromptFulfillment").str("webhook").is_empty()
                {
                    c.push(Some(&fname), "WARNING", "PAGE", vec![label.clone()], format!("Required parameter '{}' has no initial prompt", fp.str("displayName")));
                }
                check_fulfillment(&mut c, &fname, label, fp.at("fillBehavior").at("initialPromptFulfillment"));
                for h in fp.at("fillBehavior").arr("repromptEventHandlers") {
                    check_handler(&mut c, label, h, &mut referenced_intents);
                }
            }
        }
        for p in flow.arr("pages") {
            if !targeted.contains(p.str("name")) {
                // pages may also be targeted from other flows / route groups
                let elsewhere = agent.flows().iter().any(|f| {
                    f.arr("pages").iter().chain(std::iter::once(f)).any(|h| h.arr("transitionRoutes").iter().chain(h.arr("eventHandlers")).any(|r| r.str("targetPage") == p.str("name")))
                }) || agent.bundle.arr("transitionRouteGroups").iter().any(|g| g.arr("transitionRoutes").iter().any(|r| r.str("targetPage") == p.str("name")))
                    || flow.arr("transitionRouteGroups").iter().any(|g| g.arr("transitionRoutes").iter().any(|r| r.str("targetPage") == p.str("name")));
                if !elsewhere {
                    c.push(Some(&fname), "WARNING", "PAGE", vec![p.str("displayName").into()], "Page is unreachable: no route or handler targets it".into());
                }
            }
        }
    }
    for g in agent.bundle.arr("transitionRouteGroups") {
        for r in g.arr("transitionRoutes") {
            if let Some(i) = agent.intent(r.str("intent")) {
                referenced_intents.insert(i.str("name").to_string());
            }
        }
    }

    // Intents
    let mut seen: HashMap<String, String> = HashMap::new();
    for it in agent.intents() {
        let dn = it.str("displayName").to_string();
        if agent.is_negative_intent(it.str("name")) {
            continue;
        }
        let phrases = it.arr("trainingPhrases");
        if phrases.is_empty() {
            c.push(None, "WARNING", "INTENT", vec![dn.clone()], "Intent has no training phrases".into());
        } else if phrases.len() < 5 {
            c.push(None, "INFO", "INTENT", vec![dn.clone()], format!("Intent has only {} training phrases; at least 10-20 are recommended", phrases.len()));
        }
        for tp in phrases {
            let text: String = tp.arr("parts").iter().map(|p| p.str("text")).collect();
            let n = normalize(&text);
            if n.is_empty() {
                continue;
            }
            if let Some(other) = seen.get(&n) {
                if other != &dn {
                    c.push(None, "WARNING", "INTENT_TRAINING_PHRASE", vec![dn.clone(), other.clone()], format!("Training phrase '{}' appears in multiple intents", text));
                }
            } else {
                seen.insert(n, dn.clone());
            }
            for part in tp.arr("parts") {
                let pid = part.str("parameterId");
                if !pid.is_empty() && !it.arr("parameters").iter().any(|p| p.str("id") == pid) {
                    c.push(None, "ERROR", "INTENT_TRAINING_PHRASE", vec![dn.clone()], format!("Annotation references undefined parameter '{}'", pid));
                }
            }
        }
        for p in it.arr("parameters") {
            let key = agent.entity_key(p.str("entityType"));
            if !key.starts_with("sys.") && agent.entity_type(&key).is_none() {
                c.push(None, "ERROR", "INTENT_PARAMETER", vec![dn.clone()], format!("Parameter '{}' uses unknown entity type '{}'", p.str("id"), p.str("entityType")));
            }
        }
        if !referenced_intents.contains(it.str("name")) && !dn.starts_with("Default ") {
            c.push(None, "INFO", "INTENT", vec![dn.clone()], "Intent is not used by any route".into());
        }
    }
    // Entity types
    for e in agent.entity_types() {
        let dn = e.str("displayName").to_string();
        if e.arr("entities").is_empty() {
            c.push(None, "WARNING", "ENTITY_TYPE", vec![dn.clone()], "Entity type has no entities".into());
        }
        if e.str("kind") == "KIND_REGEXP" {
            for ent in e.arr("entities") {
                if let Err(err) = cx_regex::Regex::new(ent.str("value")) {
                    c.push(None, "ERROR", "ENTITY_TYPE", vec![dn.clone()], format!("Invalid regular expression '{}': {}", ent.str("value"), err));
                }
            }
        }
    }
    // Webhooks
    for w in agent.webhooks() {
        let uri = w.at("genericWebService").str("uri");
        if uri.is_empty() && w.get("serviceDirectory").is_none() {
            c.push(None, "ERROR", "WEBHOOK", vec![w.str("displayName").into()], "Webhook has no URI".into());
        } else if uri.starts_with("https://") {
            c.push(None, "INFO", "WEBHOOK", vec![w.str("displayName").into()], "HTTPS webhooks require a local TLS-terminating proxy in this runtime".into());
        }
    }
    // Playbooks
    for p in agent.playbooks() {
        let dn = p.str("displayName").to_string();
        if p.str("goal").is_empty() {
            c.push(None, "WARNING", "PLAYBOOK", vec![dn.clone()], "Playbook has no goal".into());
        }
        for t in p.arr("referencedTools") {
            if agent.tool(t.as_str().unwrap_or("")).is_none() {
                c.push(None, "ERROR", "PLAYBOOK", vec![dn.clone()], format!("Playbook references unknown tool '{}'", t.to_display()));
            }
        }
    }

    let flow_results: Vec<Value> = agent
        .flows()
        .iter()
        .map(|f| {
            let msgs = c.flow_msgs.remove(f.str("name")).unwrap_or_default();
            obj! {"name" => format!("{}/validationResult", f.str("name")), "flowDisplayName" => f.str("displayName"), "validationMessages" => Value::Array(msgs), "updateTime" => cx_core::time::now_rfc3339()}
        })
        .collect();
    let mut counts = obj! {"ERROR" => 0, "WARNING" => 0, "INFO" => 0};
    for m in c.agent_msgs.iter().chain(flow_results.iter().flat_map(|f| f.arr("validationMessages").iter())) {
        let s = m.str("severity").to_string();
        let n = counts.f64_field(&s).unwrap_or(0.0) + 1.0;
        counts.set(s, n);
    }
    obj! {
        "name" => format!("{}/validationResult", agent.name),
        "flowValidationResults" => Value::Array(flow_results),
        "agentValidationMessages" => Value::Array(c.agent_msgs),
        "summary" => counts,
    }
}
