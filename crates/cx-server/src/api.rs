//! REST API compatible in shape with the Dialogflow CX / Conversational
//! Agents `v3` API, served under `/v3/`.
//!
//! The platform is multi-tenant: any number of projects, locations and
//! agents. Generic CRUD covers every resource collection; custom verbs
//! (`:detectIntent`, `:export`, `:restore`, `:validate`, `:run`,
//! `:batchRun`, `:load`, …) are dispatched explicitly.

use crate::store::{self, bundle, child_collections, public, valid_resource_name, Data, Store};
use cx_core::name::{ancestor, collection_of, id_of, parent_of};
use cx_core::{arr, obj, Value};
use cx_engine::import::{default_agent_bundle, normalize_bundle};
use cx_engine::{Agent, Runtime, SessionState};
use cx_http::{Request, Response};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct ApiError {
    pub code: u16,
    pub status: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(code: u16, message: impl Into<String>) -> Self {
        let status = match code {
            400 => "INVALID_ARGUMENT",
            404 => "NOT_FOUND",
            409 => "ALREADY_EXISTS",
            412 => "FAILED_PRECONDITION",
            501 => "UNIMPLEMENTED",
            _ => "INTERNAL",
        };
        ApiError { code, status, message: message.into() }
    }
    pub fn not_found(name: &str) -> Self {
        Self::new(404, format!("Resource '{}' not found.", name))
    }
    pub fn bad(msg: impl Into<String>) -> Self {
        Self::new(400, msg)
    }
    pub fn to_response(&self) -> Response {
        json_resp(self.code, &obj! {"error" => obj!{"code" => self.code, "message" => self.message.clone(), "status" => self.status}})
    }
}

type ApiResult = Result<Value, ApiError>;

pub fn json_resp(code: u16, v: &Value) -> Response {
    Response::json(code, v.to_string())
}

pub struct Api {
    pub store: Arc<Store>,
    pub runtime: Runtime,
    cache: Mutex<HashMap<String, (u64, Arc<Agent>)>>,
}

fn singular(collection: &str) -> &str {
    match collection {
        "entityTypes" => "Entity type",
        "transitionRouteGroups" => "Route group",
        "testCases" => "Test case",
        "securitySettings" => "Security settings",
        other => other.trim_end_matches('s'),
    }
}

/// Collections whose display names must be unique within the parent.
fn unique_display(collection: &str) -> bool {
    !matches!(collection, "projects" | "results" | "changelogs" | "conversations" | "examples" | "sessions" | "versions" | "continuousTestResults")
}

/// Collections that may not be written through generic CRUD.
fn read_only(collection: &str) -> bool {
    matches!(collection, "results" | "changelogs" | "conversations" | "continuousTestResults")
}

fn operation(response: Value, kind: &str) -> Value {
    obj! {
        "name" => format!("operations/{}", cx_core::uuid::new_v4()),
        "metadata" => obj!{"@type" => kind},
        "done" => true,
        "response" => response,
    }
}

impl Api {
    pub fn new(store: Arc<Store>, runtime: Runtime) -> Self {
        Api { store, runtime, cache: Mutex::new(HashMap::new()) }
    }

    pub fn handle(&self, req: &Request) -> Response {
        let path = req.path.trim_start_matches("/v3/").trim_start_matches("/v3beta1/").trim_end_matches('/').to_string();
        let body = if req.body.is_empty() {
            Ok(Value::object())
        } else {
            Value::parse(&req.body_str()).map_err(|e| ApiError::bad(format!("Invalid JSON body: {}", e)))
        };
        let result = body.and_then(|b| self.route(&req.method, &path, req, &b));
        match result {
            Ok(v) => json_resp(200, &v),
            Err(e) => e.to_response(),
        }
    }

    fn route(&self, method: &str, path: &str, req: &Request, body: &Value) -> ApiResult {
        // custom verb: name:verb
        let (name, verb) = match path.rsplit_once(':') {
            Some((n, v)) if !v.contains('/') => (n.to_string(), Some(v.to_string())),
            _ => (path.to_string(), None),
        };
        if let Some(v) = verb {
            if method != "POST" && !(method == "GET" && v == "export") {
                return Err(ApiError::new(405, "custom methods require POST"));
            }
            return self.action(&name, &v, body);
        }
        // special read-only paths
        if name == "systemEntities" {
            return Ok(obj! {"systemEntities" => Value::Array(cx_nlu::system::SYSTEM_ENTITIES.iter().map(|(n, d)| obj!{"name" => format!("@{}", n), "description" => *d}).collect())});
        }
        if name == "systemFunctions" {
            return Ok(obj! {"functions" => Value::from(cx_engine::expr::FUNCTIONS.to_vec())});
        }
        if name.ends_with("/validationResult") && method == "GET" {
            let target = name.trim_end_matches("/validationResult");
            let d = self.store.data.lock().unwrap();
            return d.get(&format!("{}/_validation", target)).cloned().ok_or_else(|| ApiError::not_found(&name));
        }
        let segs = if name.is_empty() { 0 } else { name.split('/').count() };
        let is_collection = segs % 2 == 1;
        match (method, is_collection) {
            ("GET", true) => self.list(&name, req),
            ("POST", true) => self.create(&name, req, body),
            ("GET", false) => self.get(&name),
            ("PATCH", false) => self.patch(&name, req, body),
            ("DELETE", false) => self.delete(&name),
            _ => Err(ApiError::new(405, format!("{} not supported on '{}'", method, name))),
        }
    }

    fn commit(&self, d: &Data) {
        self.store.persist(d);
    }

    fn changelog(d: &mut Data, name: &str, action: &str, display: &str) {
        let Some(agent) = ancestor(name, "agents") else { return };
        if agent == name && action == "Delete" {
            return;
        }
        let coll = collection_of(name);
        if matches!(coll, "changelogs" | "results" | "sessions") {
            return;
        }
        let id = cx_core::uuid::new_v4();
        let cl = obj! {
            "name" => format!("{}/changelogs/{}", agent, id),
            "userEmail" => "local-user",
            "displayName" => display,
            "action" => action,
            "type" => singular(coll),
            "resource" => name,
            "createTime" => cx_core::time::now_rfc3339(),
            "languageCode" => "en",
        };
        d.put(&format!("{}/changelogs/{}", agent, id), cl);
    }

    // ---- generic CRUD -----------------------------------------------------

    fn list(&self, coll_path: &str, req: &Request) -> ApiResult {
        let parent = parent_of(&format!("{}/x", coll_path)).to_string();
        let collection = id_of(coll_path).to_string();
        let d = self.store.data.lock().unwrap();
        if !parent.is_empty() && collection != "projects" && d.get(&parent).is_none() && collection_of(&parent) != "locations" && collection_of(&parent) != "sessions" {
            return Err(ApiError::not_found(&parent));
        }
        let mut items: Vec<Value> = d.list(&parent, &collection).into_iter().map(public).collect();
        if collection == "changelogs" || collection == "results" {
            items.reverse();
        }
        if collection == "conversations" {
            return Ok(self.conversations(&d, &parent));
        }
        if let Some(f) = req.query_param("filter") {
            let f = f.to_lowercase();
            items.retain(|v| v.to_string().to_lowercase().contains(&f.replace("displayname:", "").trim().trim_matches('"').to_string()));
        }
        let page_size: usize = req.query_param("pageSize").and_then(|s| s.parse().ok()).filter(|n| *n > 0).unwrap_or(1000);
        let offset: usize = req.query_param("pageToken").and_then(|s| s.parse().ok()).unwrap_or(0);
        let total = items.len();
        let page: Vec<Value> = items.into_iter().skip(offset).take(page_size).collect();
        let mut out = obj! {collection.clone() => Value::Array(page)};
        if offset + page_size < total {
            out.set("nextPageToken", (offset + page_size).to_string());
        }
        Ok(out)
    }

    fn get(&self, name: &str) -> ApiResult {
        let d = self.store.data.lock().unwrap();
        if collection_of(name) == "conversations" {
            let agent = parent_of(name);
            let session = format!("{}/sessions/{}", agent, id_of(name));
            return d.transcripts.get(&session).map(|t| conversation_resource(name, &session, t)).ok_or_else(|| ApiError::not_found(name));
        }
        if collection_of(name) == "locations" && d.get(name).is_none() {
            return Ok(obj! {"name" => name, "locationId" => id_of(name), "displayName" => id_of(name)});
        }
        d.get(name).map(public).ok_or_else(|| ApiError::not_found(name))
    }

    fn defaults_for(collection: &str, v: &mut Value) {
        let now = cx_core::time::now_rfc3339();
        match collection {
            "flows" => {
                if v.get("eventHandlers").is_none() {
                    v.set(
                        "eventHandlers",
                        arr![
                            obj!{"name" => cx_core::uuid::new_v4(), "event" => "sys.no-match-default", "triggerFulfillment" => obj!{"messages" => arr![obj!{"text" => obj!{"text" => arr!["I didn't get that. Can you say it again?", "I missed what you said. What was that?"]}}]}},
                            obj!{"name" => cx_core::uuid::new_v4(), "event" => "sys.no-input-default", "triggerFulfillment" => obj!{"messages" => arr![obj!{"text" => obj!{"text" => arr!["I didn't get that. Can you say it again?"]}}]}},
                        ],
                    );
                }
                if v.get("nluSettings").is_none() {
                    v.set("nluSettings", obj! {"modelType" => "MODEL_TYPE_STANDARD", "classificationThreshold" => 0.3, "modelTrainingMode" => "MODEL_TRAINING_MODE_AUTOMATIC"});
                }
                for k in ["transitionRoutes", "transitionRouteGroups"] {
                    if v.get(k).is_none() {
                        v.set(k, arr![]);
                    }
                }
            }
            "intents" => {
                for k in ["trainingPhrases", "parameters"] {
                    if v.get(k).is_none() {
                        v.set(k, arr![]);
                    }
                }
                if v.get("priority").is_none() {
                    v.set("priority", 500000000);
                }
                for tp in v.get_mut("trainingPhrases").and_then(Value::as_array_mut).into_iter().flatten() {
                    if tp.str("id").is_empty() {
                        tp.set("id", cx_core::uuid::new_v4());
                    }
                    if tp.get("repeatCount").is_none() {
                        tp.set("repeatCount", 1);
                    }
                }
            }
            "entityTypes" => {
                if v.str("kind").is_empty() {
                    v.set("kind", "KIND_MAP");
                }
                if v.get("entities").is_none() {
                    v.set("entities", arr![]);
                }
            }
            "projects" | "agents" | "environments" | "versions" | "testCases" | "securitySettings" => {
                if v.get("createTime").is_none() {
                    v.set("createTime", now.clone());
                }
            }
            _ => {}
        }
        if collection == "environments" || collection == "testCases" {
            v.set("updateTime", now);
        }
    }

    fn create(&self, coll_path: &str, req: &Request, body: &Value) -> ApiResult {
        let parent = parent_of(&format!("{}/x", coll_path)).to_string();
        let collection = id_of(coll_path).to_string();
        let parent_coll = if parent.is_empty() { "" } else { collection_of(&parent) };
        if !child_collections(parent_coll).contains(&collection.as_str()) {
            return Err(ApiError::not_found(coll_path));
        }
        if read_only(&collection) {
            return Err(ApiError::bad(format!("{} cannot be created directly", collection)));
        }
        if collection == "sessions" {
            return Err(ApiError::bad("sessions are created implicitly by :detectIntent"));
        }
        let mut v = if body.is_object() { body.clone() } else { Value::object() };
        if v.str("displayName").trim().is_empty() && !matches!(collection.as_str(), "examples") {
            if collection == "projects" {
                let pid = v.str("projectId").to_string();
                v.set("displayName", if pid.is_empty() { "My Project".to_string() } else { pid });
            } else {
                return Err(ApiError::bad("displayName is required"));
            }
        }
        // id: explicit query param (<singular>Id), projectId body, or uuid
        let id_param = format!("{}Id", singular(&collection).to_lowercase().replace(' ', ""));
        let requested_id = req
            .query_param(&id_param)
            .or_else(|| req.query_param("id"))
            .map(String::from)
            .or_else(|| v.get("projectId").and_then(Value::as_str).map(String::from))
            .or_else(|| v.get("agentId").and_then(Value::as_str).map(String::from));
        let id = match requested_id {
            Some(i) if !i.is_empty() => i,
            _ if collection == "projects" => slug(v.str("displayName")),
            _ => cx_core::uuid::new_v4(),
        };
        if id.is_empty() || id.contains('/') {
            return Err(ApiError::bad("invalid resource id"));
        }
        let name = if parent.is_empty() { format!("{}/{}", collection, id) } else { format!("{}/{}/{}", parent, collection, id) };
        let mut d = self.store.data.lock().unwrap();
        if !parent.is_empty() && d.get(&parent).is_none() && parent_coll != "locations" && parent_coll != "sessions" {
            return Err(ApiError::not_found(&parent));
        }
        if parent_coll == "locations" {
            // implicit location; ensure project exists
            let project = ancestor(&parent, "projects").unwrap_or_default();
            if d.get(&project).is_none() {
                return Err(ApiError::not_found(&project));
            }
        }
        if d.get(&name).is_some() {
            return Err(ApiError::new(409, format!("Resource '{}' already exists.", name)));
        }
        if unique_display(&collection) {
            let dn = v.str("displayName").to_string();
            if d.list(&parent, &collection).iter().any(|o| o.str("displayName") == dn) {
                return Err(ApiError::new(409, format!("{} with display name '{}' already exists.", singular(&collection), dn)));
            }
        }
        v.set("name", name.clone());
        v.remove("projectId");
        Self::defaults_for(&collection, &mut v);
        if collection == "agents" {
            if v.str("defaultLanguageCode").is_empty() {
                v.set("defaultLanguageCode", "en");
            }
            if v.str("timeZone").is_empty() {
                v.set("timeZone", "UTC");
            }
            let mut skeleton = default_agent_bundle(&name, v.str("displayName"), v.str("defaultLanguageCode"), v.str("timeZone"));
            let mut agent = skeleton.at("agent").clone();
            agent.merge(&v);
            skeleton.set("agent", agent);
            store::restore(&mut d, &name, &skeleton);
        } else if collection == "versions" && parent_coll == "flows" {
            // snapshot the flow
            let flow = d.get(&parent).cloned().unwrap_or_default();
            let pages: Vec<Value> = d.list(&parent, "pages").into_iter().map(public).collect();
            let groups: Vec<Value> = d.list(&parent, "transitionRouteGroups").into_iter().map(public).collect();
            let mut snap = public(&flow);
            snap.set("pages", Value::Array(pages));
            snap.set("transitionRouteGroups", Value::Array(groups));
            v.set("_snapshot", snap);
            v.set("state", "SUCCEEDED");
            v.set("nluSettings", flow.at("nluSettings").clone());
            d.put(&name, v.clone());
        } else if collection == "versions" && parent_coll == "playbooks" {
            let pb = d.get(&parent).cloned().unwrap_or_default();
            let mut snap = public(&pb);
            snap.set("examples", Value::Array(d.list(&parent, "examples").into_iter().map(public).collect()));
            v.set("playbook", snap);
            v.set("createTime", cx_core::time::now_rfc3339());
            d.put(&name, v.clone());
        } else {
            d.put(&name, v.clone());
        }
        Self::changelog(&mut d, &name, "Create", v.str("displayName"));
        let out = public(d.get(&name).unwrap_or(&v));
        self.commit(&d);
        Ok(if collection == "versions" { operation(out, "CreateVersionOperationMetadata") } else { out })
    }

    fn patch(&self, name: &str, req: &Request, body: &Value) -> ApiResult {
        if !valid_resource_name(name) {
            return Err(ApiError::not_found(name));
        }
        if read_only(collection_of(name)) {
            return Err(ApiError::bad("resource is read-only"));
        }
        let mut d = self.store.data.lock().unwrap();
        let mut cur = d.get(name).cloned().ok_or_else(|| ApiError::not_found(name))?;
        let mask: Vec<String> = req.query_param("updateMask").map(|m| m.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
        if mask.is_empty() {
            if let Some(m) = body.as_object() {
                for (k, v) in m.iter() {
                    if k != "name" && !k.starts_with('_') {
                        cur.set(k.clone(), v.clone());
                    }
                }
            }
        } else {
            for path in &mask {
                let val = body.path(path).cloned();
                let parts: Vec<&str> = path.split('.').collect();
                let mut target = &mut cur;
                for p in &parts[..parts.len() - 1] {
                    target = target.entry(p);
                    if !target.is_object() {
                        *target = Value::object();
                    }
                }
                let last = parts[parts.len() - 1];
                match val {
                    Some(v) => target.set(last, v),
                    None => {
                        target.remove(last);
                    }
                }
            }
        }
        let coll = collection_of(name).to_string();
        if unique_display(&coll) {
            let dn = cur.str("displayName").to_string();
            if d.list(parent_of(name), &coll).iter().any(|o| o.str("displayName") == dn && o.str("name") != name) {
                return Err(ApiError::new(409, format!("{} with display name '{}' already exists.", singular(&coll), dn)));
            }
        }
        cur.set("name", name);
        if matches!(coll.as_str(), "environments" | "testCases") {
            cur.set("updateTime", cx_core::time::now_rfc3339());
        }
        Self::defaults_for(&coll, &mut cur);
        d.put(name, cur.clone());
        Self::changelog(&mut d, name, "Update", cur.str("displayName"));
        self.commit(&d);
        Ok(public(&cur))
    }

    fn delete(&self, name: &str) -> ApiResult {
        let mut d = self.store.data.lock().unwrap();
        if collection_of(name) == "conversations" {
            let session = format!("{}/sessions/{}", parent_of(name), id_of(name));
            d.transcripts.remove(&session);
            d.sessions.remove(&session);
            self.commit(&d);
            return Ok(Value::object());
        }
        let dn = d.get(name).map(|v| v.str("displayName").to_string()).ok_or_else(|| ApiError::not_found(name))?;
        d.delete_tree(name);
        Self::changelog(&mut d, name, "Delete", &dn);
        self.commit(&d);
        Ok(Value::object())
    }

    fn conversations(&self, d: &Data, agent: &str) -> Value {
        let mut convs: Vec<Value> = d
            .transcripts
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}/", agent)))
            .map(|(k, t)| conversation_resource(&format!("{}/conversations/{}", agent, id_of(k)), k, t))
            .collect();
        convs.sort_by(|a, b| b.str("startTime").cmp(a.str("startTime")));
        for c in convs.iter_mut() {
            c.remove("interactions");
        }
        obj! {"conversations" => Value::Array(convs)}
    }

    // ---- compiled agents --------------------------------------------------

    /// Compiles (and caches) an agent, optionally applying an
    /// environment's flow versions.
    pub fn compiled(&self, agent: &str, environment: Option<&str>) -> Result<Arc<Agent>, ApiError> {
        let d = self.store.data.lock().unwrap();
        let key = format!("{}|{}", agent, environment.unwrap_or(""));
        if let Some((rev, a)) = self.cache.lock().unwrap().get(&key) {
            if *rev == d.revision {
                return Ok(a.clone());
            }
        }
        let mut b = bundle(&d, agent).ok_or_else(|| ApiError::not_found(agent))?;
        if let Some(env) = environment.filter(|e| id_of(e) != "draft" && !e.is_empty()) {
            let env_name = if env.contains('/') { env.to_string() } else { format!("{}/environments/{}", agent, env) };
            let e = d.get(&env_name).ok_or_else(|| ApiError::not_found(&env_name))?;
            let mut flows = b.arr("flows").to_vec();
            for vc in e.arr("versionConfigs") {
                let vname = vc.str("version");
                if let Some(snap) = d.get(vname).and_then(|v| v.get("_snapshot")) {
                    let fname = format!("{}/flows/{}", agent, id_of(parent_of(vname)));
                    if let Some(slot) = flows.iter_mut().find(|f| f.str("name") == fname) {
                        *slot = snap.clone();
                    }
                }
            }
            b.set("flows", Value::Array(flows));
        }
        let compiled = Arc::new(Agent::compile(&b).map_err(|e| ApiError::new(500, e))?);
        self.cache.lock().unwrap().insert(key, (d.revision, compiled.clone()));
        Ok(compiled)
    }

    // ---- custom methods ---------------------------------------------------

    fn action(&self, name: &str, verb: &str, body: &Value) -> ApiResult {
        // `a/b/coll:verb` acts on a collection; `a/b:verb` on a resource.
        let coll = if name.split('/').count() % 2 == 1 { id_of(name) } else { collection_of(name) };
        match (coll, verb) {
            ("sessions", "detectIntent") | ("sessions", "serverStreamingDetectIntent") => self.detect_intent(name, body),
            ("sessions", "matchIntent") => self.match_intent(name, body),
            ("sessions", "fulfillIntent") => {
                let mi = body.at("matchIntentRequest");
                let intent = body.at("match").at("intent").str("name").to_string();
                let mut req = obj! {"queryParams" => mi.at("queryParams").clone()};
                req.set("queryInput", obj! {"intent" => obj!{"intent" => intent}});
                let resp = self.detect_intent(name, &req)?;
                Ok(obj! {"responseId" => resp.str("responseId"), "queryResult" => resp.at("queryResult").clone()})
            }
            ("agents", "export") => {
                let d = self.store.data.lock().unwrap();
                let b = bundle(&d, name).ok_or_else(|| ApiError::not_found(name))?;
                drop(d);
                if body.str("dataFormat") == "JSON_PACKAGE" || body.str("dataFormat") == "BLOB" {
                    // agentContent is base64 bytes, as in the Google API
                    let zip = cx_engine::import::export_cx_zip(&b).map_err(|e| ApiError::new(500, e))?;
                    return Ok(operation(obj! {"agentContent" => cx_core::base64::encode(&zip)}, "ExportAgentResponse"));
                }
                Ok(operation(obj! {"agentContent" => b}, "ExportAgentResponse"))
            }
            ("agents", "restore") => {
                let content = body.get("agentContent").or_else(|| body.get("bundle")).ok_or_else(|| ApiError::bad("agentContent is required"))?;
                let content = agent_content(content)?;
                let mut d = self.store.data.lock().unwrap();
                if d.get(name).is_none() {
                    return Err(ApiError::not_found(name));
                }
                let n = normalize_bundle(&content, name);
                store::restore(&mut d, name, &n);
                Self::changelog(&mut d, name, "Restore", n.at("agent").str("displayName"));
                self.commit(&d);
                Ok(operation(Value::object(), "RestoreAgent"))
            }
            ("locations", "importAgent") | ("agents", "import") | ("locations", "import") => {
                // create a new agent from a bundle: POST {location}:importAgent or {location}/agents:import
                let location = if coll == "agents" { parent_of(&format!("{}/x", name)).to_string() } else { name.to_string() };
                let location = if collection_of(&location) == "agents" { parent_of(&location).to_string() } else { location };
                let content = agent_content(body.get("agentContent").or_else(|| body.get("bundle")).unwrap_or(body))?;
                let id = body.get("agentId").and_then(Value::as_str).map(String::from).unwrap_or_else(cx_core::uuid::new_v4);
                let agent_name = format!("{}/agents/{}", location, id);
                let mut d = self.store.data.lock().unwrap();
                let project = ancestor(&location, "projects").unwrap_or_default();
                if d.get(&project).is_none() {
                    return Err(ApiError::not_found(&project));
                }
                if d.get(&agent_name).is_some() {
                    return Err(ApiError::new(409, format!("Agent '{}' already exists", agent_name)));
                }
                let mut n = normalize_bundle(&content, &agent_name);
                if n.at("agent").get("createTime").is_none() {
                    let mut a = n.at("agent").clone();
                    a.set("createTime", cx_core::time::now_rfc3339());
                    n.set("agent", a);
                }
                store::restore(&mut d, &agent_name, &n);
                self.commit(&d);
                Ok(public(d.get(&agent_name).unwrap()))
            }
            ("agents", "validate") => {
                let a = self.compiled(name, None)?;
                let r = cx_engine::validate::validate(&a);
                let mut d = self.store.data.lock().unwrap();
                let rev = d.revision;
                d.resources.insert(format!("{}/_validation", name), r.clone());
                d.revision = rev; // validation results do not invalidate caches
                Ok(r)
            }
            ("flows", "validate") => {
                let agent = ancestor(name, "agents").unwrap_or_default();
                let a = self.compiled(&agent, None)?;
                let r = cx_engine::validate::validate(&a);
                let fr = r.arr("flowValidationResults").iter().find(|f| f.str("name") == format!("{}/validationResult", name)).cloned().unwrap_or_default();
                Ok(fr)
            }
            ("flows", "train") => {
                let agent = ancestor(name, "agents").unwrap_or_default();
                self.compiled(&agent, None)?;
                Ok(operation(Value::object(), "TrainFlow"))
            }
            ("flows", "export") => {
                let d = self.store.data.lock().unwrap();
                let f = d.get(name).ok_or_else(|| ApiError::not_found(name))?;
                let mut fv = public(f);
                fv.set("pages", Value::Array(d.list(name, "pages").into_iter().map(public).collect()));
                fv.set("transitionRouteGroups", Value::Array(d.list(name, "transitionRouteGroups").into_iter().map(public).collect()));
                Ok(operation(obj! {"flowContent" => fv}, "ExportFlowResponse"))
            }
            ("versions", "load") => {
                let mut d = self.store.data.lock().unwrap();
                let v = d.get(name).cloned().ok_or_else(|| ApiError::not_found(name))?;
                let snap = v.get("_snapshot").cloned().ok_or_else(|| ApiError::bad("version has no snapshot"))?;
                let flow = parent_of(name).to_string();
                for coll in ["pages", "transitionRouteGroups"] {
                    for n in d.list_names(&flow, coll) {
                        d.delete_tree(&n);
                    }
                }
                let mut fv = snap.clone();
                let pages = fv.remove("pages").unwrap_or_default();
                let groups = fv.remove("transitionRouteGroups").unwrap_or_default();
                fv.set("name", flow.clone());
                d.put(&flow, fv);
                for p in pages.as_array().cloned().unwrap_or_default().into_iter().chain(groups.as_array().cloned().unwrap_or_default()) {
                    let n = p.str("name").to_string();
                    d.put(&n, p);
                }
                Self::changelog(&mut d, &flow, "Load version", v.str("displayName"));
                self.commit(&d);
                Ok(operation(Value::object(), "LoadVersion"))
            }
            ("versions", "compareVersions") => {
                let d = self.store.data.lock().unwrap();
                let base = d.get(name).and_then(|v| v.get("_snapshot")).cloned().unwrap_or_default();
                let target_name = body.str("targetVersion");
                let target = d.get(target_name).and_then(|v| v.get("_snapshot")).cloned().unwrap_or_default();
                Ok(obj! {"baseVersionContentJson" => base.to_pretty(), "targetVersionContentJson" => target.to_pretty(), "compareTime" => cx_core::time::now_rfc3339()})
            }
            ("testCases", "run") => {
                let agent = ancestor(name, "agents").unwrap_or_default();
                let env = body.str("environment").to_string();
                let a = self.compiled(&agent, Some(&env))?;
                let tc = self.store.data.lock().unwrap().get(name).cloned().ok_or_else(|| ApiError::not_found(name))?;
                let result = self.run_and_store(&a, &tc, &env);
                Ok(operation(obj! {"result" => result}, "RunTestCaseResponse"))
            }
            ("testCases", "batchRun") => {
                let agent = parent_of(&format!("{}/x", name)).to_string();
                let agent = ancestor(&agent, "agents").unwrap_or(agent);
                let env = body.str("environment").to_string();
                let a = self.compiled(&agent, Some(&env))?;
                let names: Vec<String> = if body.arr("testCases").is_empty() {
                    self.store.data.lock().unwrap().list_names(&agent, "testCases")
                } else {
                    body.arr("testCases").iter().filter_map(|t| t.as_str().map(String::from)).collect()
                };
                let mut results = Vec::new();
                for n in names {
                    let tc = self.store.data.lock().unwrap().get(&n).cloned();
                    if let Some(tc) = tc {
                        results.push(self.run_and_store(&a, &tc, &env));
                    }
                }
                Ok(operation(obj! {"results" => Value::Array(results)}, "BatchRunTestCasesResponse"))
            }
            ("testCases", "batchDelete") => {
                let mut d = self.store.data.lock().unwrap();
                for n in body.arr("names").iter().filter_map(Value::as_str) {
                    d.delete_tree(n);
                }
                self.commit(&d);
                Ok(Value::object())
            }
            ("testCases", "calculateCoverage") | ("agents", "calculateCoverage") => {
                let agent = ancestor(&format!("{}/x", name), "agents").unwrap_or_default();
                let a = self.compiled(&agent, None)?;
                Ok(coverage(&a, body.str("type")))
            }
            ("intents", "batchDelete") | ("entityTypes", "batchDelete") => {
                let mut d = self.store.data.lock().unwrap();
                for n in body.arr("names").iter().filter_map(Value::as_str) {
                    d.delete_tree(n);
                }
                self.commit(&d);
                Ok(Value::object())
            }
            ("agents", "saveTestCase") => {
                // non-standard helper: build a test case from a session transcript
                let session = body.str("session").to_string();
                let d = self.store.data.lock().unwrap();
                let t = d.transcripts.get(&session).cloned().ok_or_else(|| ApiError::not_found(&session))?;
                drop(d);
                let turns: Vec<(Value, Value)> = t.iter().map(|x| (x.at("request").clone(), x.at("response").clone())).collect();
                let tc = cx_engine::testing::test_case_from_transcript(body.str("displayName"), &turns);
                let req = Request { method: "POST".into(), ..Default::default() };
                self.create(&format!("{}/testCases", name), &req, &tc)
            }
            ("environments", "deployFlow") => {
                let mut d = self.store.data.lock().unwrap();
                let mut env = d.get(name).cloned().ok_or_else(|| ApiError::not_found(name))?;
                let version = body.str("flowVersion").to_string();
                let flow = parent_of(&version).to_string();
                let mut vcs: Vec<Value> = env.arr("versionConfigs").iter().filter(|vc| parent_of(vc.str("version")) != flow).cloned().collect();
                vcs.push(obj! {"version" => version});
                env.set("versionConfigs", Value::Array(vcs));
                env.set("updateTime", cx_core::time::now_rfc3339());
                d.put(name, env);
                self.commit(&d);
                Ok(operation(obj! {"environment" => name}, "DeployFlowResponse"))
            }
            ("environments", "runContinuousTest") => {
                let agent = ancestor(name, "agents").unwrap_or_default();
                let a = self.compiled(&agent, Some(name))?;
                let tcs: Vec<Value> = { self.store.data.lock().unwrap().list(&agent, "testCases").into_iter().cloned().collect() };
                let results: Vec<Value> = tcs.iter().map(|tc| self.run_and_store(&a, tc, name)).collect();
                let passed = results.iter().all(|r| r.str("testResult") == "PASSED");
                let ctr = obj! {
                    "name" => format!("{}/continuousTestResults/{}", name, cx_core::uuid::new_v4()),
                    "result" => if passed { "PASSED" } else { "FAILED" },
                    "testCaseResults" => Value::Array(results.iter().map(|r| Value::from(r.str("name"))).collect()),
                    "runTime" => cx_core::time::now_rfc3339(),
                };
                let mut d = self.store.data.lock().unwrap();
                d.put(ctr.str("name"), ctr.clone());
                self.commit(&d);
                Ok(operation(obj! {"continuousTestResult" => ctr}, "RunContinuousTestResponse"))
            }
            _ => Err(ApiError::new(501, format!("method '{}' is not implemented for '{}'", verb, coll))),
        }
    }

    fn run_and_store(&self, a: &Agent, tc: &Value, env: &str) -> Value {
        let res = cx_engine::testing::run_test_case(&self.runtime, a, tc, env);
        let mut d = self.store.data.lock().unwrap();
        let rev = d.revision;
        d.put(res.str("name"), res.clone());
        if let Some(mut t) = d.get(tc.str("name")).cloned() {
            t.set("lastTestResult", res.clone());
            d.put(tc.str("name"), t);
        }
        // test results do not change agent content
        d.revision = rev;
        self.commit(&d);
        res
    }

    fn session_agent(name: &str) -> Result<(String, Option<String>), ApiError> {
        let agent = ancestor(name, "agents").ok_or_else(|| ApiError::bad("session name must be under an agent"))?;
        let env = ancestor(name, "environments");
        Ok((agent, env))
    }

    pub fn detect_intent(&self, session: &str, body: &Value) -> ApiResult {
        let (agent, env) = Self::session_agent(session)?;
        let compiled = self.compiled(&agent, env.as_deref())?;
        let mut st = {
            let d = self.store.data.lock().unwrap();
            let mut st = d.sessions.get(session).map(SessionState::from_json).unwrap_or_else(SessionState::new);
            // session entity types stored as resources under the session
            let se: Vec<Value> = d.list(session, "entityTypes").into_iter().map(public).collect();
            if !se.is_empty() {
                st.session_entity_types = se;
            }
            st
        };
        let resp = self.runtime.detect_intent(&compiled, session, &mut st, body);
        let mut d = self.store.data.lock().unwrap();
        d.sessions.insert(session.to_string(), st.to_json());
        let logging = compiled.raw.at("advancedSettings").at("loggingSettings").get("enableInteractionLogging").and_then(Value::as_bool).unwrap_or(true);
        if logging {
            let entry = obj! {"request" => body.clone(), "response" => resp.clone(), "createTime" => cx_core::time::now_rfc3339(), "environment" => env.clone()};
            let t = d.transcripts.entry(session.to_string()).or_default();
            t.push(entry);
            if t.len() > 500 {
                t.remove(0);
            }
        }
        self.commit(&d);
        Ok(resp)
    }

    fn match_intent(&self, session: &str, body: &Value) -> ApiResult {
        let (agent, env) = Self::session_agent(session)?;
        let a = self.compiled(&agent, env.as_deref())?;
        let text = body.at("queryInput").at("text").str("text").to_string();
        let results = a.classifier.classify(&text, &[]);
        let matches: Vec<Value> = results
            .iter()
            .take(5)
            .filter(|r| r.confidence >= a.threshold)
            .map(|r| {
                let params: cx_core::Map = r.params.iter().map(|(k, v)| (k.clone(), v.resolved.clone())).collect();
                obj! {
                    "intent" => obj!{"name" => r.key.clone(), "displayName" => r.display_name.clone()},
                    "parameters" => Value::Object(params),
                    "resolvedInput" => text.clone(),
                    "matchType" => "INTENT",
                    "confidence" => (r.confidence * 1000.0).round() / 1000.0,
                }
            })
            .collect();
        let matches = if matches.is_empty() { arr![obj! {"matchType" => "NO_MATCH", "resolvedInput" => text.clone()}] } else { Value::Array(matches) };
        Ok(obj! {"text" => text, "matches" => matches})
    }
}

/// Decodes `agentContent`: an inline bundle object, a JSON string, or
/// base64 bytes of a ZIP export / JSON document.
fn agent_content(v: &Value) -> Result<Value, ApiError> {
    match v {
        Value::Object(_) => Ok(v.clone()),
        Value::String(s) => {
            let t = s.trim_start();
            if t.starts_with('{') {
                return Value::parse(t).map_err(|e| ApiError::bad(format!("agentContent: {}", e)));
            }
            let bytes = cx_core::base64::decode(s).map_err(|e| ApiError::bad(format!("agentContent: {}", e)))?;
            cx_engine::import::parse_agent_bytes(&bytes).map_err(|e| ApiError::bad(format!("agentContent: {}", e)))
        }
        _ => Err(ApiError::bad("agentContent must be an object or base64 string")),
    }
}

fn slug(s: &str) -> String {
    let mut out: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        format!("project-{}", &cx_core::uuid::new_v4()[..8])
    } else {
        out
    }
}

fn conversation_resource(name: &str, session: &str, turns: &[Value]) -> Value {
    let start = turns.first().map(|t| t.str("createTime").to_string()).unwrap_or_default();
    let interactions: Vec<Value> = turns
        .iter()
        .map(|t| {
            obj! {
                "request" => t.at("request").clone(),
                "response" => t.at("response").clone(),
                "createTime" => t.str("createTime"),
            }
        })
        .collect();
    let intents: Vec<Value> = turns.iter().filter_map(|t| t.at("response").at("queryResult").get("intent").cloned()).collect();
    let ended = turns.last().map_or(false, |t| t.at("response").at("queryResult").at("diagnosticInfo").bool_field("Session Ended"));
    obj! {
        "name" => name,
        "session" => session,
        "type" => "AUDIO_OR_TEXT_CONVERSATION",
        "startTime" => start,
        "duration" => format!("{}turns", turns.len()),
        "turnCount" => turns.len(),
        "ended" => ended,
        "intents" => Value::Array(intents),
        "interactions" => Value::Array(interactions),
        "environment" => turns.first().map(|t| t.at("environment").clone()),
    }
}

/// Test coverage over intents, pages or route groups.
fn coverage(a: &Agent, kind: &str) -> Value {
    let tcs = a.test_cases();
    let mut seen_intents = std::collections::HashSet::new();
    let mut seen_pages = std::collections::HashSet::new();
    for tc in tcs {
        let res = tc.at("lastTestResult");
        for t in res.arr("conversationTurns") {
            let o = t.at("virtualAgentOutput");
            seen_intents.insert(o.at("triggeredIntent").str("name").to_string());
            seen_pages.insert(o.at("currentPage").str("name").to_string());
        }
        for t in tc.arr("testCaseConversationTurns") {
            let o = t.at("virtualAgentOutput");
            if let Some(i) = a.intent(o.at("triggeredIntent").str("displayName")).or_else(|| a.intent(o.at("triggeredIntent").str("name"))) {
                seen_intents.insert(i.str("name").to_string());
            }
        }
    }
    match kind {
        "PAGE" | "TRANSITION" => {
            let pages: Vec<Value> = a
                .flows()
                .iter()
                .flat_map(|f| f.arr("pages").iter())
                .map(|p| obj! {"page" => obj!{"name" => p.str("name"), "displayName" => p.str("displayName")}, "covered" => seen_pages.contains(p.str("name"))})
                .collect();
            let covered = pages.iter().filter(|p| p.bool_field("covered")).count();
            let score = if pages.is_empty() { 0.0 } else { covered as f64 / pages.len() as f64 };
            obj! {"agent" => a.name.clone(), "transitionCoverage" => obj!{"transitions" => Value::Array(pages), "coverageScore" => score}}
        }
        _ => {
            let intents: Vec<Value> = a
                .intents()
                .iter()
                .map(|i| obj! {"intent" => i.str("name"), "displayName" => i.str("displayName"), "covered" => seen_intents.contains(i.str("name"))})
                .collect();
            let covered = intents.iter().filter(|i| i.bool_field("covered")).count();
            let score = if intents.is_empty() { 0.0 } else { covered as f64 / intents.len() as f64 };
            obj! {"agent" => a.name.clone(), "intentCoverage" => obj!{"intents" => Value::Array(intents), "coverageScore" => score}}
        }
    }
}
