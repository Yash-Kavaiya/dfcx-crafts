//! Agent bundle normalisation and import.
//!
//! * [`normalize_bundle`] assigns resource names under an agent and rewrites
//!   every reference (intents, pages, flows, route groups, webhooks,
//!   generators, playbooks, tools, entity types) so authors can write
//!   bundles using display names.
//! * [`import_cx_directory`] reads an unzipped Conversational Agents /
//!   Dialogflow CX "JSON package" export.
//! * [`default_agent_bundle`] builds the same skeleton the console creates
//!   for a new agent (Default Start Flow, welcome / negative intents).

use cx_core::name::{
    id_of, is_special_page, DEFAULT_NEGATIVE_INTENT_ID, DEFAULT_START_FLOW_ID, DEFAULT_WELCOME_INTENT_ID,
};
use cx_core::{arr, obj, Value};
use std::collections::HashMap;
use std::path::Path;

pub const SYS_ENTITY_PREFIX: &str = "projects/-/locations/-/agents/-/entityTypes/";

fn text_msg(texts: &[&str]) -> Value {
    obj! {"text" => obj!{"text" => Value::from(texts.to_vec())}}
}

/// The skeleton a brand-new agent starts with.
pub fn default_agent_bundle(agent_name: &str, display_name: &str, language: &str, time_zone: &str) -> Value {
    let flow = format!("{}/flows/{}", agent_name, DEFAULT_START_FLOW_ID);
    let welcome = format!("{}/intents/{}", agent_name, DEFAULT_WELCOME_INTENT_ID);
    let negative = format!("{}/intents/{}", agent_name, DEFAULT_NEGATIVE_INTENT_ID);
    let phrase = |t: &str| obj! {"id" => cx_core::uuid::new_v4(), "parts" => arr![obj!{"text" => t}], "repeatCount" => 1};
    let welcome_phrases: Vec<Value> = [
        "hi", "hello", "hey", "howdy", "hey there", "hi there", "greetings", "good morning", "good afternoon",
        "good evening", "just going to say hi", "hello there", "heya", "hello hi", "long time no see",
    ]
    .iter()
    .map(|t| phrase(t))
    .collect();
    obj! {
        "agent" => obj!{
            "name" => agent_name,
            "displayName" => display_name,
            "defaultLanguageCode" => language,
            "supportedLanguageCodes" => arr![],
            "timeZone" => time_zone,
            "startFlow" => flow.clone(),
            "nluSettings" => obj!{"modelType" => "MODEL_TYPE_STANDARD", "classificationThreshold" => 0.3, "modelTrainingMode" => "MODEL_TRAINING_MODE_AUTOMATIC"},
            "speechToTextSettings" => obj!{"enableSpeechAdaptation" => false},
            "advancedSettings" => obj!{"loggingSettings" => obj!{"enableInteractionLogging" => true}},
            "createTime" => cx_core::time::now_rfc3339(),
        },
        "flows" => arr![obj!{
            "name" => flow.clone(),
            "displayName" => "Default Start Flow",
            "description" => "A start flow created along with the agent",
            "transitionRoutes" => arr![obj!{
                "name" => cx_core::uuid::new_v4(),
                "intent" => welcome.clone(),
                "triggerFulfillment" => obj!{"messages" => arr![text_msg(&["Greetings! How can I assist?", "Good day! What can I do for you today?", "Hi! How are you doing?"])]},
            }],
            "eventHandlers" => arr![
                obj!{"name" => cx_core::uuid::new_v4(), "event" => "sys.no-match-default", "triggerFulfillment" => obj!{"messages" => arr![text_msg(&["I didn't get that. Can you say it again?", "I missed what you said. What was that?", "Sorry, could you say that again?", "Sorry, can you say that again?", "Can you say that again?", "Sorry, I didn't get that. Can you rephrase?", "Sorry, what was that?", "One more time?", "What was that?", "Say that one more time?", "I didn't get that. Can you repeat?", "I missed that, say that again?"])]}},
                obj!{"name" => cx_core::uuid::new_v4(), "event" => "sys.no-input-default", "triggerFulfillment" => obj!{"messages" => arr![text_msg(&["I didn't get that. Can you say it again?", "I missed what you said. What was that?", "Sorry, could you say that again?"])]}},
            ],
            "nluSettings" => obj!{"modelType" => "MODEL_TYPE_STANDARD", "classificationThreshold" => 0.3, "modelTrainingMode" => "MODEL_TRAINING_MODE_AUTOMATIC"},
            "pages" => arr![],
            "transitionRouteGroups" => arr![],
        }],
        "intents" => arr![
            obj!{"name" => welcome, "displayName" => "Default Welcome Intent", "priority" => 500000000, "trainingPhrases" => Value::Array(welcome_phrases), "parameters" => arr![]},
            obj!{"name" => negative, "displayName" => "Default Negative Intent", "priority" => 500000000, "isFallback" => true, "trainingPhrases" => arr![], "parameters" => arr![]},
        ],
        "entityTypes" => arr![], "webhooks" => arr![], "transitionRouteGroups" => arr![], "generators" => arr![],
        "playbooks" => arr![], "tools" => arr![], "testCases" => arr![], "environments" => arr![],
    }
}

#[derive(Default)]
struct Aliases {
    map: HashMap<String, String>,
}

impl Aliases {
    fn add(&mut self, new_name: &str, old_name: &str, display: &str) {
        for k in [old_name, id_of(old_name), display, id_of(new_name), new_name] {
            if !k.is_empty() {
                self.map.entry(k.to_string()).or_insert_with(|| new_name.to_string());
            }
        }
    }
    fn get(&self, r: &str) -> Option<String> {
        if r.is_empty() {
            return None;
        }
        self.map.get(r).or_else(|| self.map.get(id_of(r))).or_else(|| self.map.get(r.trim_start_matches('@'))).cloned()
    }
}

struct Resolver {
    flows: Aliases,
    pages: HashMap<String, Aliases>,
    all_pages: Aliases,
    intents: Aliases,
    entities: Aliases,
    webhooks: Aliases,
    groups: Aliases,
    flow_groups: HashMap<String, Aliases>,
    generators: Aliases,
    playbooks: Aliases,
    tools: Aliases,
}

fn well_known_id(collection: &str, display: &str) -> Option<&'static str> {
    match (collection, display) {
        ("flows", "Default Start Flow") => Some(DEFAULT_START_FLOW_ID),
        ("intents", "Default Welcome Intent") => Some(DEFAULT_WELCOME_INTENT_ID),
        ("intents", "Default Negative Intent") => Some(DEFAULT_NEGATIVE_INTENT_ID),
        _ => None,
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Picks a resource name for `v` under `parent/collection`.
fn assign(parent: &str, collection: &str, v: &mut Value) -> (String, String) {
    let old = v.str("name").to_string();
    let display = v.str("displayName").to_string();
    let id = if let Some(w) = well_known_id(collection, &display) {
        w.to_string()
    } else if valid_id(id_of(&old)) {
        id_of(&old).to_string()
    } else {
        cx_core::uuid::new_v4()
    };
    let new_name = format!("{}/{}/{}", parent, collection, id);
    v.set("name", new_name.clone());
    (new_name, old)
}

fn arr_mut<'v>(v: &'v mut Value, key: &str) -> Vec<&'v mut Value> {
    match v.get_mut(key).and_then(Value::as_array_mut) {
        Some(a) => a.iter_mut().collect(),
        None => Vec::new(),
    }
}

fn ensure_array(v: &mut Value, key: &str) {
    if !v.get(key).map_or(false, |x| x.is_array()) {
        v.set(key, arr![]);
    }
}

impl Resolver {
    fn entity(&self, r: &str) -> String {
        let t = r.trim_start_matches('@');
        let id = id_of(t);
        if id.starts_with("sys.") {
            return format!("{}{}", SYS_ENTITY_PREFIX, id);
        }
        self.entities.get(t).unwrap_or_else(|| r.to_string())
    }

    fn page(&self, flow: Option<&str>, r: &str) -> String {
        let id = id_of(r);
        if is_special_page(id) {
            // keep a flow-qualified special page when we know the flow
            return match flow {
                Some(f) if !r.contains('/') => format!("{}/pages/{}", f, id),
                Some(f) if r.contains("/flows/") => {
                    let target_flow = cx_core::name::parent_of(r);
                    let tf = self.flows.get(target_flow).unwrap_or_else(|| f.to_string());
                    format!("{}/pages/{}", tf, id)
                }
                _ => r.to_string(),
            };
        }
        if let Some(f) = flow {
            if let Some(p) = self.pages.get(f).and_then(|a| a.get(r)) {
                return p;
            }
        }
        self.all_pages.get(r).unwrap_or_else(|| r.to_string())
    }

    fn fulfillment(&self, f: &mut Value) {
        if !f.is_object() {
            return;
        }
        if let Some(w) = f.get("webhook").and_then(Value::as_str).map(String::from) {
            if !w.is_empty() {
                f.set("webhook", self.webhooks.get(&w).unwrap_or(w));
            }
        }
        for g in arr_mut(f, "generators") {
            let r = g.str("generator").to_string();
            if let Some(n) = self.generators.get(&r) {
                g.set("generator", n);
            }
        }
    }

    fn handler(&self, h: &mut Value, flow: Option<&str>) {
        if h.str("name").is_empty() || h.str("name").contains(' ') {
            h.set("name", cx_core::uuid::new_v4());
        }
        if let Some(i) = h.get("intent").and_then(Value::as_str).map(String::from) {
            if !i.is_empty() {
                h.set("intent", self.intents.get(&i).unwrap_or(i));
            }
        }
        if let Some(p) = h.get("targetPage").and_then(Value::as_str).map(String::from) {
            if !p.is_empty() {
                h.set("targetPage", self.page(flow, &p));
            }
        }
        if let Some(f) = h.get("targetFlow").and_then(Value::as_str).map(String::from) {
            if !f.is_empty() {
                h.set("targetFlow", self.flows.get(&f).unwrap_or(f));
            }
        }
        if let Some(p) = h.get("targetPlaybook").and_then(Value::as_str).map(String::from) {
            if !p.is_empty() {
                h.set("targetPlaybook", self.playbooks.get(&p).unwrap_or(p));
            }
        }
        if let Some(tf) = h.get_mut("triggerFulfillment") {
            self.fulfillment(tf);
        }
    }

    fn route_holder(&self, v: &mut Value, flow: Option<&str>) {
        for r in arr_mut(v, "transitionRoutes") {
            self.handler(r, flow);
        }
        for h in arr_mut(v, "eventHandlers") {
            self.handler(h, flow);
        }
        let groups: Vec<String> = v.arr("transitionRouteGroups").iter().filter_map(|g| g.as_str().map(String::from)).collect();
        if v.get("transitionRouteGroups").is_some() {
            let resolved: Vec<Value> = groups
                .iter()
                .map(|g| {
                    let local = flow.and_then(|f| self.flow_groups.get(f)).and_then(|a| a.get(g));
                    Value::from(local.or_else(|| self.groups.get(g)).unwrap_or_else(|| g.clone()))
                })
                .collect();
            v.set("transitionRouteGroups", Value::Array(resolved));
        }
    }
}

/// Normalises a bundle so that it belongs to `agent_name` with fully
/// qualified, consistent resource names and references.
pub fn normalize_bundle(input: &Value, agent_name: &str) -> Value {
    let mut b = input.clone();
    if !b.is_object() {
        b = Value::object();
    }
    for k in ["flows", "intents", "entityTypes", "webhooks", "transitionRouteGroups", "generators", "playbooks", "tools", "testCases", "environments"] {
        ensure_array(&mut b, k);
    }
    // Agent
    let mut agent = b.get("agent").cloned().unwrap_or_else(Value::object);
    agent.set("name", agent_name);
    if agent.str("displayName").is_empty() {
        agent.set("displayName", id_of(agent_name));
    }
    if agent.str("defaultLanguageCode").is_empty() {
        agent.set("defaultLanguageCode", "en");
    }
    if agent.str("timeZone").is_empty() {
        agent.set("timeZone", "UTC");
    }
    if b.arr("flows").is_empty() && b.arr("playbooks").is_empty() {
        let d = default_agent_bundle(agent_name, agent.str("displayName"), agent.str("defaultLanguageCode"), agent.str("timeZone"));
        b.set("flows", d.at("flows").clone());
        let mut intents = b.arr("intents").to_vec();
        for di in d.arr("intents") {
            if !intents.iter().any(|i| i.str("displayName") == di.str("displayName")) {
                intents.push(di.clone());
            }
        }
        b.set("intents", Value::Array(intents));
    }

    let mut r = Resolver {
        flows: Aliases::default(),
        pages: HashMap::new(),
        all_pages: Aliases::default(),
        intents: Aliases::default(),
        entities: Aliases::default(),
        webhooks: Aliases::default(),
        groups: Aliases::default(),
        flow_groups: HashMap::new(),
        generators: Aliases::default(),
        playbooks: Aliases::default(),
        tools: Aliases::default(),
    };
    // Pass 1: names
    let simple: [(&str, fn(&mut Resolver) -> &mut Aliases); 7] = [
        ("intents", |r| &mut r.intents),
        ("entityTypes", |r| &mut r.entities),
        ("webhooks", |r| &mut r.webhooks),
        ("transitionRouteGroups", |r| &mut r.groups),
        ("generators", |r| &mut r.generators),
        ("playbooks", |r| &mut r.playbooks),
        ("tools", |r| &mut r.tools),
    ];
    for (coll, sel) in simple {
        for v in arr_mut(&mut b, coll) {
            let display = v.str("displayName").to_string();
            let (new, old) = assign(agent_name, coll, v);
            sel(&mut r).add(&new, &old, &display);
        }
    }
    for coll in ["testCases", "environments"] {
        for v in arr_mut(&mut b, coll) {
            assign(agent_name, coll, v);
        }
    }
    for f in arr_mut(&mut b, "flows") {
        let display = f.str("displayName").to_string();
        let (fname, old) = assign(agent_name, "flows", f);
        r.flows.add(&fname, &old, &display);
        ensure_array(f, "pages");
        ensure_array(f, "transitionRouteGroups");
        let mut pa = Aliases::default();
        for p in arr_mut(f, "pages") {
            let pd = p.str("displayName").to_string();
            let (pn, po) = assign(&fname, "pages", p);
            pa.add(&pn, &po, &pd);
            r.all_pages.add(&pn, &po, "");
        }
        let mut ga = Aliases::default();
        for g in arr_mut(f, "transitionRouteGroups") {
            let gd = g.str("displayName").to_string();
            let (gn, go) = assign(&fname, "transitionRouteGroups", g);
            ga.add(&gn, &go, &gd);
        }
        r.pages.insert(fname.clone(), pa);
        r.flow_groups.insert(fname, ga);
    }
    // display names of pages are only unique per flow; register them
    // globally as a fallback (first wins)
    let page_displays: Vec<(String, String)> = b
        .arr("flows")
        .iter()
        .flat_map(|f| f.arr("pages").iter().map(|p| (p.str("name").to_string(), p.str("displayName").to_string())).collect::<Vec<_>>())
        .collect();
    for (n, d) in page_displays {
        r.all_pages.add(&n, "", &d);
    }

    // Pass 2: references
    for f in arr_mut(&mut b, "flows") {
        let fname = f.str("name").to_string();
        r.route_holder(f, Some(&fname));
        for p in arr_mut(f, "pages") {
            r.route_holder(p, Some(&fname));
            if let Some(ef) = p.get_mut("entryFulfillment") {
                r.fulfillment(ef);
            }
            if let Some(form) = p.get_mut("form") {
                for fp in arr_mut(form, "parameters") {
                    let et = fp.str("entityType").to_string();
                    if !et.is_empty() {
                        fp.set("entityType", r.entity(&et));
                    }
                    if let Some(fb) = fp.get_mut("fillBehavior") {
                        if let Some(ip) = fb.get_mut("initialPromptFulfillment") {
                            r.fulfillment(ip);
                        }
                        for h in arr_mut(fb, "repromptEventHandlers") {
                            r.handler(h, Some(&fname));
                        }
                    }
                }
            }
        }
        for g in arr_mut(f, "transitionRouteGroups") {
            r.route_holder(g, Some(&fname));
        }
    }
    for g in arr_mut(&mut b, "transitionRouteGroups") {
        r.route_holder(g, None);
    }
    for it in arr_mut(&mut b, "intents") {
        for p in arr_mut(it, "parameters") {
            let et = p.str("entityType").to_string();
            p.set("entityType", r.entity(&et));
        }
        for tp in arr_mut(it, "trainingPhrases") {
            if tp.str("id").is_empty() {
                tp.set("id", cx_core::uuid::new_v4());
            }
            if tp.get("repeatCount").is_none() {
                tp.set("repeatCount", 1);
            }
        }
        if it.get("trainingPhrases").is_none() {
            it.set("trainingPhrases", arr![]);
        }
        if it.get("parameters").is_none() {
            it.set("parameters", arr![]);
        }
    }
    for e in arr_mut(&mut b, "entityTypes") {
        if e.str("kind").is_empty() {
            e.set("kind", "KIND_MAP");
        }
        if e.str("autoExpansionMode").is_empty() {
            e.set("autoExpansionMode", "AUTO_EXPANSION_MODE_UNSPECIFIED");
        }
    }
    for p in arr_mut(&mut b, "playbooks") {
        let pname = p.str("name").to_string();
        for (key, al) in [("referencedTools", &r.tools), ("referencedPlaybooks", &r.playbooks), ("referencedFlows", &r.flows)] {
            if let Some(a) = p.get_mut(key).and_then(Value::as_array_mut) {
                for x in a.iter_mut() {
                    if let Some(s) = x.as_str().map(String::from) {
                        *x = Value::from(al.get(&s).unwrap_or(s));
                    }
                }
            }
        }
        ensure_array(p, "examples");
        for ex in arr_mut(p, "examples") {
            assign(&pname, "examples", ex);
            for a in arr_mut(ex, "actions") {
                for (field, key, al) in [("toolUse", "tool", &r.tools), ("playbookInvocation", "playbook", &r.playbooks), ("flowInvocation", "flow", &r.flows)] {
                    if let Some(x) = a.get_mut(field) {
                        let s = x.str(key).to_string();
                        if !s.is_empty() {
                            x.set(key, al.get(&s).unwrap_or(s));
                        }
                    }
                }
            }
        }
    }
    for tc in arr_mut(&mut b, "testCases") {
        if let Some(cfg) = tc.get_mut("testConfig") {
            let f = cfg.str("flow").to_string();
            if !f.is_empty() {
                cfg.set("flow", r.flows.get(&f).unwrap_or(f));
            }
            let p = cfg.str("page").to_string();
            if !p.is_empty() {
                cfg.set("page", r.page(None, &p));
            }
        }
    }
    let sf = agent.str("startFlow").to_string();
    let sp = agent.str("startPlaybook").to_string();
    if !sf.is_empty() {
        agent.set("startFlow", r.flows.get(&sf).unwrap_or(sf));
    } else if sp.is_empty() {
        if let Some(f) = b.arr("flows").first() {
            let n = b.arr("flows").iter().find(|f| id_of(f.str("name")) == DEFAULT_START_FLOW_ID).unwrap_or(f).str("name").to_string();
            agent.set("startFlow", n);
        }
    }
    if !sp.is_empty() {
        agent.set("startPlaybook", r.playbooks.get(&sp).unwrap_or(sp));
    }
    b.set("agent", agent);
    b
}

// ---- Conversational Agents JSON package import ------------------------------

fn read_json(p: &Path) -> Result<Value, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    Value::parse(&s).map_err(|e| format!("{}: {}", p.display(), e))
}

fn json_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file() && p.extension().map_or(false, |x| x == "json")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn sub_dirs(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Merges array fields from every JSON file in `dir` into `target[key]`.
fn merge_lang_files(target: &mut Value, dir: &Path, key: &str) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut all = target.arr(key).to_vec();
    for f in json_files(dir) {
        let v = read_json(&f)?;
        all.extend(v.arr(key).iter().cloned());
    }
    target.set(key, Value::Array(all));
    Ok(())
}

/// Reads one "resource directory" (`<dir>/<Name>.json` + sub-folders).
fn resource_dir(dir: &Path) -> Result<Option<Value>, String> {
    let files = json_files(dir);
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let main = files.iter().find(|f| f.file_stem().map_or(false, |s| s.to_string_lossy() == name)).or_else(|| files.first());
    match main {
        Some(f) => Ok(Some(read_json(f)?)),
        None => Ok(None),
    }
}

/// Imports an unzipped Conversational Agents export (JSON package format).
pub fn import_cx_directory(root: &Path) -> Result<Value, String> {
    let agent = read_json(&root.join("agent.json"))?;
    let mut b = obj! {"agent" => agent};
    let mut flows = Vec::new();
    for fd in sub_dirs(&root.join("flows")) {
        let Some(mut flow) = resource_dir(&fd)? else { continue };
        let mut pages = Vec::new();
        for pf in json_files(&fd.join("pages")) {
            pages.push(read_json(&pf)?);
        }
        flow.set("pages", Value::Array(pages));
        let mut groups = Vec::new();
        for gf in json_files(&fd.join("transitionRouteGroups")) {
            groups.push(read_json(&gf)?);
        }
        flow.set("transitionRouteGroups", Value::Array(groups));
        flows.push(flow);
    }
    b.set("flows", Value::Array(flows));
    let mut intents = Vec::new();
    for d in sub_dirs(&root.join("intents")) {
        if let Some(mut it) = resource_dir(&d)? {
            merge_lang_files(&mut it, &d.join("trainingPhrases"), "trainingPhrases")?;
            intents.push(it);
        }
    }
    b.set("intents", Value::Array(intents));
    let mut ets = Vec::new();
    for d in sub_dirs(&root.join("entityTypes")) {
        if let Some(mut e) = resource_dir(&d)? {
            merge_lang_files(&mut e, &d.join("entities"), "entities")?;
            merge_lang_files(&mut e, &d.join("excludedPhrases"), "excludedPhrases")?;
            ets.push(e);
        }
    }
    b.set("entityTypes", Value::Array(ets));
    let flat = |sub: &str| -> Result<Value, String> {
        let dir = root.join(sub);
        let mut out = Vec::new();
        for f in json_files(&dir) {
            out.push(read_json(&f)?);
        }
        for d in sub_dirs(&dir) {
            if let Some(v) = resource_dir(&d)? {
                out.push(v);
            }
        }
        Ok(Value::Array(out))
    };
    b.set("webhooks", flat("webhooks")?);
    b.set("transitionRouteGroups", flat("transitionRouteGroups")?);
    b.set("generators", flat("generators")?);
    b.set("testCases", flat("testCases")?);
    let mut tools = Vec::new();
    for d in sub_dirs(&root.join("tools")) {
        if let Some(mut t) = resource_dir(&d)? {
            // schema files next to the tool definition
            if let Ok(rd) = std::fs::read_dir(&d) {
                for e in rd.filter_map(|e| e.ok()) {
                    let p = e.path();
                    let ext = p.extension().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                    if (ext == "yaml" || ext == "yml") && t.get("openApiSpec").is_some() && t.at("openApiSpec").str("textSchema").is_empty() {
                        let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
                        t.get_mut("openApiSpec").unwrap().set("textSchema", s);
                    }
                }
            }
            tools.push(t);
        }
    }
    for f in json_files(&root.join("tools")) {
        tools.push(read_json(&f)?);
    }
    b.set("tools", Value::Array(tools));
    let mut pbs = Vec::new();
    for d in sub_dirs(&root.join("playbooks")) {
        if let Some(mut p) = resource_dir(&d)? {
            let mut exs = Vec::new();
            for f in json_files(&d.join("examples")) {
                exs.push(read_json(&f)?);
            }
            for sd in sub_dirs(&d.join("examples")) {
                if let Some(v) = resource_dir(&sd)? {
                    exs.push(v);
                }
            }
            p.set("examples", Value::Array(exs));
            pbs.push(p);
        }
    }
    b.set("playbooks", Value::Array(pbs));
    Ok(b)
}

/// Writes a bundle as a JSON-package directory (the inverse of
/// [`import_cx_directory`], using display names for references).
pub fn export_cx_directory(bundle: &Value, root: &Path) -> Result<(), String> {
    let w = |p: &Path, v: &Value| -> Result<(), String> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(p, v.to_pretty()).map_err(|e| format!("{}: {}", p.display(), e))
    };
    let safe = |s: &str| -> String { s.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect() };
    let mut agent = bundle.at("agent").clone();
    let sf = agent.str("startFlow").to_string();
    if let Some(f) = crate::agent::find_in(bundle.arr("flows"), &sf) {
        agent.set("startFlow", f.str("displayName"));
    }
    w(&root.join("agent.json"), &agent)?;
    for f in bundle.arr("flows") {
        let dn = safe(f.str("displayName"));
        let dir = root.join("flows").join(&dn);
        let mut flow = f.clone();
        flow.remove("pages");
        flow.remove("transitionRouteGroups");
        let groups: Vec<Value> = f.arr("transitionRouteGroups").to_vec();
        w(&dir.join(format!("{}.json", dn)), &flow)?;
        for p in f.arr("pages") {
            w(&dir.join("pages").join(format!("{}.json", safe(p.str("displayName")))), p)?;
        }
        for g in groups {
            w(&dir.join("transitionRouteGroups").join(format!("{}.json", safe(g.str("displayName")))), &g)?;
        }
    }
    for it in bundle.arr("intents") {
        let dn = safe(it.str("displayName"));
        let mut main = it.clone();
        let tps = main.remove("trainingPhrases").unwrap_or_default();
        let dir = root.join("intents").join(&dn);
        w(&dir.join(format!("{}.json", dn)), &main)?;
        w(&dir.join("trainingPhrases").join("en.json"), &obj! {"trainingPhrases" => tps})?;
    }
    for e in bundle.arr("entityTypes") {
        let dn = safe(e.str("displayName"));
        let mut main = e.clone();
        let ents = main.remove("entities").unwrap_or_default();
        let dir = root.join("entityTypes").join(&dn);
        w(&dir.join(format!("{}.json", dn)), &main)?;
        w(&dir.join("entities").join("en.json"), &obj! {"entities" => ents})?;
    }
    for (coll, items) in [("webhooks", "webhooks"), ("transitionRouteGroups", "transitionRouteGroups"), ("generators", "generators"), ("testCases", "testCases")] {
        for v in bundle.arr(items) {
            w(&root.join(coll).join(format!("{}.json", safe(v.str("displayName")))), v)?;
        }
    }
    for t in bundle.arr("tools") {
        let dn = safe(t.str("displayName"));
        w(&root.join("tools").join(&dn).join(format!("{}.json", dn)), t)?;
    }
    for p in bundle.arr("playbooks") {
        let dn = safe(p.str("displayName"));
        let mut main = p.clone();
        let exs = main.remove("examples").unwrap_or_default();
        let dir = root.join("playbooks").join(&dn);
        w(&dir.join(format!("{}.json", dn)), &main)?;
        for ex in exs.as_array().cloned().unwrap_or_default() {
            w(&dir.join("examples").join(format!("{}.json", safe(ex.str("displayName")))), &ex)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_display_names() {
        let src = Value::parse(
            r#"{
            "agent": {"displayName": "Pizza"},
            "intents": [{"displayName": "order", "trainingPhrases": [{"parts": [{"text": "pizza"}]}],
                         "parameters": [{"id": "size", "entityType": "@size"}, {"id": "n", "entityType": "@sys.number"}]}],
            "entityTypes": [{"displayName": "size", "entities": [{"value": "large", "synonyms": ["large"]}]}],
            "webhooks": [{"displayName": "orders-hook", "genericWebService": {"uri": "http://localhost"}}],
            "flows": [{"displayName": "Default Start Flow",
                       "transitionRoutes": [{"intent": "order", "targetPage": "Confirm"}],
                       "pages": [{"displayName": "Confirm", "entryFulfillment": {"webhook": "orders-hook"},
                                  "transitionRoutes": [{"condition": "true", "targetPage": "END_SESSION"}]}]}]
        }"#,
        )
        .unwrap();
        let a = "projects/p/locations/global/agents/a1";
        let b = normalize_bundle(&src, a);
        let flow = &b.arr("flows")[0];
        assert_eq!(flow.str("name"), format!("{}/flows/{}", a, DEFAULT_START_FLOW_ID));
        let route = &flow.arr("transitionRoutes")[0];
        assert!(route.str("intent").starts_with(&format!("{}/intents/", a)));
        let page = &flow.arr("pages")[0];
        assert_eq!(route.str("targetPage"), page.str("name"));
        assert_eq!(page.arr("transitionRoutes")[0].str("targetPage"), format!("{}/pages/END_SESSION", flow.str("name")));
        assert!(page.at("entryFulfillment").str("webhook").contains("/webhooks/"));
        let it = &b.arr("intents")[0];
        assert!(it.arr("parameters")[0].str("entityType").contains("/entityTypes/"));
        assert_eq!(it.arr("parameters")[1].str("entityType"), format!("{}sys.number", SYS_ENTITY_PREFIX));
        assert_eq!(b.at("agent").str("startFlow"), flow.str("name"));
        // idempotent
        let b2 = normalize_bundle(&b, a);
        assert_eq!(b2.arr("flows")[0].arr("pages")[0].str("name"), page.str("name"));
    }
}
