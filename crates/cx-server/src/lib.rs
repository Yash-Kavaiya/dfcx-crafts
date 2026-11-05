//! The Conversational Agents platform server: a multi-tenant REST API
//! (projects → locations → agents → …) plus the embedded web console.

pub mod api;
pub mod store;
pub mod ui;

use api::Api;
use cx_http::{Request, Response, Server};
use std::path::PathBuf;
use std::sync::Arc;

pub struct Config {
    pub addr: String,
    pub data_file: Option<PathBuf>,
    pub runtime: cx_engine::Runtime,
    /// Optional static bearer token required for `/v3` calls.
    pub api_key: Option<String>,
    /// Let `:detectIntent` / `:matchIntent` through without the API key
    /// (for public chat widgets) while the management API stays protected.
    pub public_sessions: bool,
    /// Agents to create in project `demo` when absent: (agent id, bundle).
    pub seeds: Vec<(String, cx_core::Value)>,
}

/// Access control for the `/v3` API.
#[derive(Clone, Default)]
pub struct Auth {
    pub api_key: Option<String>,
    pub public_sessions: bool,
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn is_session_call(req: &Request) -> bool {
    req.method == "POST"
        && req.path.contains("/sessions/")
        && (req.path.ends_with(":detectIntent") || req.path.ends_with(":matchIntent") || req.path.ends_with(":serverStreamingDetectIntent"))
}

impl Auth {
    fn allows(&self, req: &Request) -> bool {
        let Some(k) = &self.api_key else { return true };
        if self.public_sessions && is_session_call(req) {
            return true;
        }
        let presented = req
            .header("Authorization")
            .map(|h| h.trim().trim_start_matches("Bearer ").trim().to_string())
            .or_else(|| req.header("X-Api-Key").map(|h| h.trim().to_string()))
            .or_else(|| req.query_param("key").map(String::from))
            .unwrap_or_default();
        constant_time_eq(presented.as_bytes(), k.as_bytes())
    }
}

/// Builds the request handler (exposed for tests).
pub fn handler(api: Arc<Api>, api_key: Option<String>) -> Arc<cx_http::Handler> {
    handler_with_auth(api, Auth { api_key, public_sessions: false })
}

/// Builds the request handler with explicit access control.
pub fn handler_with_auth(api: Arc<Api>, auth: Auth) -> Arc<cx_http::Handler> {
    Arc::new(move |req: &Request| -> Response {
        let started = std::time::Instant::now();
        let resp = route(&api, &auth, req);
        if std::env::var("CXA_LOG").map_or(true, |v| v != "0") {
            eprintln!("{} {} -> {} ({} ms)", req.method, req.path, resp.status, started.elapsed().as_millis());
        }
        resp.with_header("Access-Control-Allow-Origin", "*")
            .with_header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Api-Key, X-Goog-User-Project")
            .with_header("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
    })
}

fn route(api: &Api, auth: &Auth, req: &Request) -> Response {
    if req.method == "OPTIONS" {
        return Response::text(204, "");
    }
    if req.path == "/healthz" {
        return Response::json(200, r#"{"status":"ok"}"#);
    }
    if req.path == "/v3/serverInfo" {
        // Lets the console discover whether it must ask for an API key.
        return Response::json(
            200,
            format!(r#"{{"apiKeyRequired":{},"publicSessions":{},"version":"{}"}}"#, auth.api_key.is_some(), auth.public_sessions, env!("CARGO_PKG_VERSION")),
        );
    }
    if req.path.starts_with("/v3/") || req.path.starts_with("/v3beta1/") || req.path == "/v3" {
        if !auth.allows(req) {
            return api::ApiError::new(401, "missing or invalid API key").to_response().with_header("WWW-Authenticate", "Bearer");
        }
        return api.handle(req);
    }
    ui::serve(req)
}

/// Creates the seed agents (in project `demo`) that do not exist yet.
pub fn seed(store: &store::Store, seeds: &[(String, cx_core::Value)]) {
    if seeds.is_empty() {
        return;
    }
    let mut d = store.data.lock().unwrap();
    if d.get("projects/demo").is_none() {
        d.put("projects/demo", cx_core::obj! {"name" => "projects/demo", "displayName" => "Demo", "createTime" => cx_core::time::now_rfc3339()});
    }
    for (id, raw) in seeds {
        let name = format!("projects/demo/locations/global/agents/{}", id);
        if d.get(&name).is_some() {
            continue;
        }
        let mut n = cx_engine::import::normalize_bundle(raw, &name);
        let mut a = n.at("agent").clone();
        a.set("createTime", cx_core::time::now_rfc3339());
        n.set("agent", a);
        store::restore(&mut d, &name, &n);
        eprintln!("seeded agent {}", name);
    }
    store.persist(&d);
}

#[cfg(unix)]
mod shutdown {
    use std::sync::atomic::{AtomicBool, Ordering};
    pub static REQUESTED: AtomicBool = AtomicBool::new(false);
    extern "C" {
        fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    }
    extern "C" fn on_signal(_: i32) {
        // Only async-signal-safe work here: set a flag.
        REQUESTED.store(true, Ordering::SeqCst);
    }
    /// SIGTERM (Cloud Run / Kubernetes shutdown) and SIGINT (Ctrl-C).
    pub fn install() {
        unsafe {
            signal(15, on_signal);
            signal(2, on_signal);
        }
    }
}

/// Flushes the store and exits when SIGTERM/SIGINT arrives, so redeploys and
/// scale-down never lose the last writes.
fn watch_shutdown(store: Arc<store::Store>) {
    #[cfg(unix)]
    {
        shutdown::install();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if shutdown::REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                eprintln!("shutdown requested: flushing state");
                if let Err(e) = store.flush() {
                    eprintln!("final flush failed: {}", e);
                }
                std::process::exit(0);
            }
        });
    }
    #[cfg(not(unix))]
    let _ = store;
}

/// Runs the platform server (blocking).
pub fn run(cfg: Config) -> Result<(), String> {
    let store = Arc::new(store::Store::open(cfg.data_file.clone())?);
    seed(&store, &cfg.seeds);
    let _ = store.flush();
    store.start_flusher(std::time::Duration::from_millis(300));
    watch_shutdown(store.clone());
    let api = Arc::new(Api::new(store, cfg.runtime));
    let server = Server::bind(&cfg.addr).map_err(|e| format!("bind {}: {}", cfg.addr, e))?;
    eprintln!("Conversational Agents platform listening on http://{}", server.addr);
    eprintln!("  console: http://{}/projects", server.addr);
    eprintln!("  API:     http://{}/v3/projects", server.addr);
    match &cfg.data_file {
        Some(p) => eprintln!("  data:    {}", p.display()),
        None => eprintln!("  data:    in-memory (not persisted)"),
    }
    eprintln!(
        "  auth:    {}",
        match (&cfg.api_key, cfg.public_sessions) {
            (None, _) => "open (no API key)",
            (Some(_), false) => "API key required",
            (Some(_), true) => "API key required (sessions public)",
        }
    );
    server.run(handler_with_auth(api, Auth { api_key: cfg.api_key, public_sessions: cfg.public_sessions }));
    Ok(())
}
