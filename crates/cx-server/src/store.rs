//! A flat, name-keyed resource store with JSON-file persistence.
//!
//! Every resource lives under its full resource name
//! (`projects/p/locations/l/agents/a/flows/f/pages/x`). Collections are
//! derived from name prefixes, so one generic CRUD implementation serves
//! all resource types. Fields starting with `_` are internal and are
//! stripped from API responses.

use cx_core::name::{collection_of, id_of, parent_of};
use cx_core::{obj, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct Data {
    pub resources: BTreeMap<String, Value>,
    pub seq: HashMap<String, u64>,
    pub next_seq: u64,
    pub revision: u64,
    /// session name -> SessionState JSON
    pub sessions: HashMap<String, Value>,
    /// session name -> transcript turns
    pub transcripts: HashMap<String, Vec<Value>>,
}

pub struct Store {
    pub data: Mutex<Data>,
    path: Option<PathBuf>,
    /// Set by mutations; the background flusher writes the file when set.
    dirty: AtomicBool,
    /// Serialises file writes (flusher thread vs. shutdown flush).
    write_lock: Mutex<()>,
}

/// Child collections allowed under each collection.
pub fn child_collections(parent_collection: &str) -> &'static [&'static str] {
    match parent_collection {
        "" => &["projects"],
        "projects" => &["locations"],
        "locations" => &["agents", "securitySettings", "dataStores"],
        "dataStores" => &["documents"],
        "agents" => &[
            "flows",
            "intents",
            "entityTypes",
            "webhooks",
            "transitionRouteGroups",
            "generators",
            "playbooks",
            "tools",
            "testCases",
            "environments",
            "changelogs",
            "conversations",
            "sessions",
        ],
        "flows" => &["pages", "transitionRouteGroups", "versions"],
        "playbooks" => &["examples", "versions"],
        "testCases" => &["results"],
        "environments" => &["experiments", "continuousTestResults", "sessions"],
        "sessions" => &["entityTypes"],
        _ => &[],
    }
}

impl Store {
    pub fn open(path: Option<PathBuf>) -> Result<Store, String> {
        let mut data = Data::default();
        if let Some(p) = &path {
            if p.exists() {
                let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
                let v = Value::parse(&text).map_err(|e| format!("{}: {}", p.display(), e))?;
                let mut items: Vec<(u64, String, Value)> = Vec::new();
                if let Some(res) = v.get("resources").and_then(Value::as_object) {
                    for (k, r) in res.iter() {
                        let s = v.at("seq").f64_field(k).unwrap_or(0.0) as u64;
                        items.push((s, k.clone(), r.clone()));
                    }
                }
                for (s, k, r) in items {
                    data.next_seq = data.next_seq.max(s + 1);
                    data.seq.insert(k.clone(), s);
                    data.resources.insert(k, r);
                }
                if let Some(s) = v.get("sessions").and_then(Value::as_object) {
                    for (k, st) in s.iter() {
                        data.sessions.insert(k.clone(), st.clone());
                    }
                }
                if let Some(s) = v.get("transcripts").and_then(Value::as_object) {
                    for (k, t) in s.iter() {
                        data.transcripts.insert(k.clone(), t.as_array().cloned().unwrap_or_default());
                    }
                }
            }
        }
        Ok(Store { data: Mutex::new(data), path, dirty: AtomicBool::new(false), write_lock: Mutex::new(()) })
    }

    pub fn in_memory() -> Store {
        Store { data: Mutex::new(Data::default()), path: None, dirty: AtomicBool::new(false), write_lock: Mutex::new(()) }
    }

    /// Records that the store changed. Writing happens off the request
    /// path: the background flusher (see [`Store::start_flusher`]) writes
    /// the file at most every flush interval, and [`Store::flush`] runs on
    /// shutdown. Requests therefore never wait on disk or Cloud Storage.
    pub fn persist(&self, _d: &Data) {
        if self.path.is_some() {
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Writes the state file now if anything changed (atomic via rename).
    pub fn flush(&self) -> std::io::Result<()> {
        let Some(p) = &self.path else { return Ok(()) };
        let _w = self.write_lock.lock().unwrap();
        if !self.dirty.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        // Serialise under the data lock (no deep clone), write outside it.
        let text = {
            let d = self.data.lock().unwrap();
            serialize(&d)
        };
        let result = (|| {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = p.with_extension("json.tmp");
            std::fs::write(&tmp, text)?;
            std::fs::rename(&tmp, p)
        })();
        if result.is_err() {
            self.dirty.store(true, Ordering::SeqCst); // retry on the next tick
        }
        result
    }

    /// Starts the background writer thread.
    pub fn start_flusher(self: &Arc<Self>, interval: Duration) {
        if self.path.is_none() {
            return;
        }
        let store = Arc::clone(self);
        std::thread::Builder::new()
            .name("store-flush".into())
            .spawn(move || loop {
                std::thread::sleep(interval);
                if store.is_dirty() {
                    if let Err(e) = store.flush() {
                        eprintln!("store: write failed: {}", e);
                    }
                }
            })
            .expect("spawn flusher");
    }
}

fn push_json_str(out: &mut String, s: &str) {
    out.push_str(&Value::String(s.to_string()).to_string());
}

/// Streams the whole state as JSON without cloning resources.
pub fn serialize(d: &Data) -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str("{\"version\":1,\"resources\":{");
    for (i, (k, v)) in d.resources.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, k);
        out.push(':');
        out.push_str(&v.to_string());
    }
    out.push_str("},\"seq\":{");
    for (i, (k, _)) in d.resources.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, k);
        out.push(':');
        out.push_str(&d.seq.get(k).copied().unwrap_or(0).to_string());
    }
    out.push_str("},\"sessions\":{");
    for (i, (k, v)) in d.sessions.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, k);
        out.push(':');
        out.push_str(&v.to_string());
    }
    out.push_str("},\"transcripts\":{");
    for (i, (k, t)) in d.transcripts.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, k);
        out.push_str(":[");
        for (j, e) in t.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&e.to_string());
        }
        out.push(']');
    }
    out.push_str("}}");
    out
}

impl Data {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.resources.get(name)
    }

    pub fn put(&mut self, name: &str, v: Value) {
        if !self.seq.contains_key(name) {
            self.seq.insert(name.to_string(), self.next_seq);
            self.next_seq += 1;
        }
        self.resources.insert(name.to_string(), v);
        self.revision += 1;
    }

    /// Deletes a resource and everything beneath it.
    pub fn delete_tree(&mut self, name: &str) -> usize {
        let prefix = format!("{}/", name);
        // Keys sharing the textual prefix (e.g. `a/bank-eu` vs `a/bank/...`)
        // interleave in sort order, so scan the whole prefix range and filter.
        let keys: Vec<String> = self
            .resources
            .range(name.to_string()..)
            .take_while(|(k, _)| k.starts_with(name))
            .filter(|(k, _)| *k == name || k.starts_with(&prefix))
            .map(|(k, _)| k.clone())
            .collect();
        for k in &keys {
            self.resources.remove(k);
            self.seq.remove(k);
        }
        self.sessions.retain(|k, _| !k.starts_with(&prefix));
        self.transcripts.retain(|k, _| !k.starts_with(&prefix));
        self.revision += 1;
        keys.len()
    }

    /// Lists direct children of `parent` in `collection`, in creation order.
    pub fn list(&self, parent: &str, collection: &str) -> Vec<&Value> {
        let prefix = if parent.is_empty() { format!("{}/", collection) } else { format!("{}/{}/", parent, collection) };
        let mut items: Vec<(&String, &Value)> = self
            .resources
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .filter(|(k, _)| !k[prefix.len()..].contains('/'))
            .collect();
        items.sort_by_key(|(k, _)| self.seq.get(*k).copied().unwrap_or(0));
        items.into_iter().map(|(_, v)| v).collect()
    }

    pub fn list_names(&self, parent: &str, collection: &str) -> Vec<String> {
        self.list(parent, collection).iter().map(|v| v.str("name").to_string()).collect()
    }
}

/// Removes internal (`_`-prefixed) fields recursively at the top level.
pub fn public(v: &Value) -> Value {
    match v.as_object() {
        Some(m) => Value::Object(m.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), v.clone())).collect()),
        None => v.clone(),
    }
}

/// Builds the export bundle for an agent.
pub fn bundle(d: &Data, agent: &str) -> Option<Value> {
    let a = d.get(agent)?;
    let list = |coll: &str| -> Value { Value::Array(d.list(agent, coll).into_iter().map(public).collect()) };
    let flows: Vec<Value> = d
        .list(agent, "flows")
        .into_iter()
        .map(|f| {
            let n = f.str("name");
            let mut fv = public(f);
            fv.set("pages", Value::Array(d.list(n, "pages").into_iter().map(public).collect()));
            fv.set("transitionRouteGroups", Value::Array(d.list(n, "transitionRouteGroups").into_iter().map(public).collect()));
            fv
        })
        .collect();
    let playbooks: Vec<Value> = d
        .list(agent, "playbooks")
        .into_iter()
        .map(|p| {
            let mut pv = public(p);
            pv.set("examples", Value::Array(d.list(p.str("name"), "examples").into_iter().map(public).collect()));
            pv
        })
        .collect();
    let mut out = obj! {
        "agent" => public(a),
        "flows" => Value::Array(flows),
        "intents" => list("intents"),
        "entityTypes" => list("entityTypes"),
        "webhooks" => list("webhooks"),
        "transitionRouteGroups" => list("transitionRouteGroups"),
        "generators" => list("generators"),
        "playbooks" => Value::Array(playbooks),
        "tools" => list("tools"),
        "testCases" => list("testCases"),
        "environments" => list("environments"),
    };
    attach_data_stores(d, agent, &mut out);
    Some(out)
}

/// Adds the location's data stores that the bundle references (knowledge
/// connectors, data-store tools), with their documents.
pub fn attach_data_stores(d: &Data, agent: &str, b: &mut Value) {
    let location = cx_core::name::parent_of(agent);
    let text = b.to_string();
    let stores: Vec<Value> = d
        .list(location, "dataStores")
        .into_iter()
        .filter(|ds| text.contains(&format!("\"{}\"", ds.str("name"))))
        .map(|ds| {
            let mut v = public(ds);
            v.set("documents", Value::Array(d.list(ds.str("name"), "documents").into_iter().map(public).collect()));
            v
        })
        .collect();
    b.set("dataStores", Value::Array(stores));
}

/// Replaces the content of `agent` with a normalised bundle. Keeps
/// changelogs, conversations and environments unless the bundle has them.
pub fn restore(d: &mut Data, agent: &str, normalized: &Value) {
    let keep = ["changelogs", "conversations", "sessions"];
    for coll in child_collections("agents") {
        if keep.contains(coll) || (*coll == "environments" && normalized.arr("environments").is_empty()) {
            continue;
        }
        for n in d.list_names(agent, coll) {
            d.delete_tree(&n);
        }
    }
    let mut a = normalized.at("agent").clone();
    if let Some(existing) = d.get(agent) {
        if let Some(ct) = existing.get("createTime") {
            a.set("createTime", ct.clone());
        }
    }
    a.set("name", agent);
    d.put(agent, a);
    for f in normalized.arr("flows") {
        let mut fv = f.clone();
        let pages = fv.remove("pages").unwrap_or_default();
        let groups = fv.remove("transitionRouteGroups").unwrap_or_default();
        let n = fv.str("name").to_string();
        d.put(&n, fv);
        for p in pages.as_array().cloned().unwrap_or_default() {
            d.put(p.str("name"), p.clone());
        }
        for g in groups.as_array().cloned().unwrap_or_default() {
            d.put(g.str("name"), g.clone());
        }
    }
    for p in normalized.arr("playbooks") {
        let mut pv = p.clone();
        let exs = pv.remove("examples").unwrap_or_default();
        d.put(&pv.str("name").to_string(), pv.clone());
        for e in exs.as_array().cloned().unwrap_or_default() {
            d.put(e.str("name"), e.clone());
        }
    }
    for ds in normalized.arr("dataStores") {
        let n = ds.str("name").to_string();
        if n.is_empty() || d.get(&n).is_some() {
            continue; // shared location resource: never overwrite an existing store
        }
        let mut dv = ds.clone();
        let docs = dv.remove("documents").unwrap_or_default();
        d.put(&n, dv);
        for doc in docs.as_array().cloned().unwrap_or_default() {
            d.put(doc.str("name"), doc.clone());
        }
    }
    for coll in ["intents", "entityTypes", "webhooks", "transitionRouteGroups", "generators", "tools", "testCases", "environments"] {
        for v in normalized.arr(coll) {
            d.put(v.str("name"), v.clone());
        }
    }
}

/// Validates that a name has the shape `.../<collection>/<id>` and that the
/// collection is allowed under its parent.
pub fn valid_resource_name(name: &str) -> bool {
    let segs: Vec<&str> = name.split('/').collect();
    if segs.len() % 2 != 0 || segs.iter().any(|s| s.is_empty()) {
        return false;
    }
    let mut parent_coll = "";
    for i in (0..segs.len()).step_by(2) {
        if !child_collections(parent_coll).contains(&segs[i]) {
            return false;
        }
        parent_coll = segs[i];
    }
    true
}

pub fn describe(name: &str) -> (String, String, String) {
    (parent_of(name).to_string(), collection_of(name).to_string(), id_of(name).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debounced_persistence_roundtrip() {
        let dir = std::env::temp_dir().join(format!("cxa-store-{}", cx_core::uuid::new_v4()));
        let path = dir.join("state.json");
        let store = Store::open(Some(path.clone())).unwrap();
        {
            let mut d = store.data.lock().unwrap();
            d.put("projects/p", obj! {"name" => "projects/p", "displayName" => "P \"quoted\""});
            d.sessions.insert("projects/p/x/sessions/s".into(), obj! {"turn" => 3});
            d.transcripts.insert("projects/p/x/sessions/s".into(), vec![obj! {"t" => 1}]);
            store.persist(&d);
        }
        assert!(!path.exists(), "nothing is written on the request path");
        store.flush().unwrap();
        assert!(!store.is_dirty());
        let back = Store::open(Some(path.clone())).unwrap();
        let d = back.data.lock().unwrap();
        assert_eq!(d.get("projects/p").unwrap().str("displayName"), "P \"quoted\"");
        assert_eq!(d.sessions.len(), 1);
        assert_eq!(d.transcripts.values().next().unwrap().len(), 1);
        drop(d);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn crud_and_listing() {
        let mut d = Data::default();
        d.put("projects/p", obj! {"name" => "projects/p"});
        d.put("projects/p/locations/global/agents/b", obj! {"name" => "projects/p/locations/global/agents/b"});
        d.put("projects/p/locations/global/agents/a", obj! {"name" => "projects/p/locations/global/agents/a", "_x" => 1});
        d.put("projects/p/locations/global/agents/a/flows/f", obj! {"name" => "projects/p/locations/global/agents/a/flows/f"});
        let agents = d.list("projects/p/locations/global", "agents");
        assert_eq!(agents.len(), 2);
        assert!(agents[0].str("name").ends_with("/b"), "creation order");
        assert_eq!(public(agents[1]).get("_x"), None);
        d.put("projects/p/locations/global/agents/a-2", obj! {"name" => "x"});
        assert_eq!(d.delete_tree("projects/p/locations/global/agents/a"), 2);
        assert!(d.get("projects/p/locations/global/agents/a-2").is_some());
        assert!(valid_resource_name("projects/p/locations/l/agents/a/flows/f/pages/x"));
        assert!(!valid_resource_name("projects/p/locations/l/agents/a/pages/x"));
    }
}
