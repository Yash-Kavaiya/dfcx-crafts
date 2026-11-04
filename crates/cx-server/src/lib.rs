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
}

/// Builds the request handler (exposed for tests).
pub fn handler(api: Arc<Api>, api_key: Option<String>) -> Arc<cx_http::Handler> {
    Arc::new(move |req: &Request| -> Response {
        let started = std::time::Instant::now();
        let resp = route(&api, api_key.as_deref(), req);
        if std::env::var("CXA_LOG").map_or(true, |v| v != "0") {
            eprintln!("{} {} -> {} ({} ms)", req.method, req.path, resp.status, started.elapsed().as_millis());
        }
        resp.with_header("Access-Control-Allow-Origin", "*")
            .with_header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Goog-User-Project")
            .with_header("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
    })
}

fn route(api: &Api, api_key: Option<&str>, req: &Request) -> Response {
    if req.method == "OPTIONS" {
        return Response::text(204, "");
    }
    if req.path == "/healthz" {
        return Response::json(200, r#"{"status":"ok"}"#);
    }
    if req.path.starts_with("/v3/") || req.path.starts_with("/v3beta1/") || req.path == "/v3" {
        if let Some(k) = api_key {
            let ok = req.header("Authorization").map_or(false, |h| h.trim_start_matches("Bearer ").trim() == k)
                || req.query_param("key") == Some(k);
            if !ok {
                return api::ApiError::new(401, "missing or invalid API key").to_response().with_header("WWW-Authenticate", "Bearer");
            }
        }
        return api.handle(req);
    }
    ui::serve(req)
}

/// Runs the platform server (blocking).
pub fn run(cfg: Config) -> Result<(), String> {
    let store = Arc::new(store::Store::open(cfg.data_file.clone())?);
    let api = Arc::new(Api::new(store, cfg.runtime));
    let server = Server::bind(&cfg.addr).map_err(|e| format!("bind {}: {}", cfg.addr, e))?;
    eprintln!("Conversational Agents platform listening on http://{}", server.addr);
    eprintln!("  console: http://{}/projects", server.addr);
    eprintln!("  API:     http://{}/v3/projects", server.addr);
    if let Some(p) = &cfg.data_file {
        eprintln!("  data:    {}", p.display());
    } else {
        eprintln!("  data:    in-memory (not persisted)");
    }
    server.run(handler(api, cfg.api_key));
    Ok(())
}
