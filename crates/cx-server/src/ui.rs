//! The embedded single-page web console.

use cx_http::{Request, Response};
use std::sync::OnceLock;

const INDEX: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");
const APP_CSS: &str = include_str!("../assets/app.css");
const MESSENGER_JS: &str = include_str!("../assets/messenger.js");

fn safe_attr(s: &str) -> bool {
    !s.is_empty() && s.len() <= 300 && s.chars().all(|c| c.is_ascii_alphanumeric() || "/_.-".contains(c))
}

/// A standalone page hosting the widget for one agent (`/messenger/demo?agent=...`).
fn demo_page(req: &Request) -> Response {
    let agent = req.query_param("agent").unwrap_or("");
    let env = req.query_param("environment").unwrap_or("");
    if !safe_attr(agent) || (!env.is_empty() && !safe_attr(env)) {
        return Response::text(400, "query parameter 'agent' must be an agent resource name");
    }
    let title = req.query_param("title").unwrap_or("Chat").chars().filter(|c| c.is_alphanumeric() || " -_'".contains(*c)).take(60).collect::<String>();
    let env_attr = if env.is_empty() { String::new() } else { format!(" environment=\"{}\"", env) };
    Response::html(format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title}</title>
<style>:root{{color-scheme:light dark}}body{{margin:0;font:15px system-ui,sans-serif;background:#f1f3f4;color:#202124;display:grid;place-items:center;min-height:100vh}}@media (prefers-color-scheme:dark){{body{{background:#171717;color:#e8eaed}}}}main{{max-width:520px;padding:16px}}code{{word-break:break-all}}</style>
<script src="/messenger.js" defer></script></head><body><main><h1>{title}</h1><p>This page embeds the <code>&lt;cx-messenger&gt;</code> widget for <code>{agent}</code>. Use the chat bubble in the corner.</p></main>
<cx-messenger agent="{agent}"{env_attr} chat-title="{title}" expanded></cx-messenger></body></html>"#
    ))
}

/// A static asset with its precomputed gzip body and content hash.
struct Asset {
    body: &'static [u8],
    gz: Vec<u8>,
    etag: String,
}

fn asset(cell: &'static OnceLock<Asset>, body: &'static str) -> &'static Asset {
    cell.get_or_init(|| {
        let b = body.as_bytes();
        Asset { body: b, gz: cx_core::deflate::gzip(b), etag: format!("\"{:08x}\"", cx_core::zip::crc32(b)) }
    })
}

static JS: OnceLock<Asset> = OnceLock::new();
static CSS: OnceLock<Asset> = OnceLock::new();
static MESSENGER: OnceLock<Asset> = OnceLock::new();
static HTML: OnceLock<String> = OnceLock::new();

/// index.html with content-hashed asset URLs, so the assets themselves can
/// be cached forever (`immutable`) and a deploy still busts the cache.
fn index_html() -> &'static str {
    HTML.get_or_init(|| {
        let js = asset(&JS, APP_JS);
        let css = asset(&CSS, APP_CSS);
        INDEX
            .replace("/static/app.js", &format!("/static/app.js?v={}", js.etag.trim_matches('"')))
            .replace("/static/app.css", &format!("/static/app.css?v={}", css.etag.trim_matches('"')))
    })
}

fn serve_asset(req: &Request, a: &Asset, content_type: &str, cache: &str) -> Response {
    if req.header("If-None-Match") == Some(a.etag.as_str()) {
        return Response::new(304, content_type, Vec::new()).with_header("ETag", &a.etag).with_header("Cache-Control", cache);
    }
    let resp = if cx_http::accepts_gzip(req) {
        Response::new(200, content_type, a.gz.clone()).with_header("Content-Encoding", "gzip")
    } else {
        Response::new(200, content_type, a.body.to_vec())
    };
    resp.with_header("ETag", &a.etag).with_header("Cache-Control", cache).with_header("Vary", "Accept-Encoding")
}

pub fn serve(req: &Request) -> Response {
    if req.method != "GET" && req.method != "HEAD" {
        return Response::text(405, "method not allowed");
    }
    // Versioned URLs (?v=hash) are immutable; unversioned ones revalidate.
    let cache = if req.query_param("v").is_some() { "public, max-age=31536000, immutable" } else { "no-cache" };
    match req.path.as_str() {
        "/static/app.js" => serve_asset(req, asset(&JS, APP_JS), "application/javascript; charset=utf-8", cache),
        "/static/app.css" => serve_asset(req, asset(&CSS, APP_CSS), "text/css; charset=utf-8", cache),
        "/favicon.ico" => Response::new(200, "image/svg+xml", FAVICON).with_header("Cache-Control", "public, max-age=86400"),
        "/messenger.js" => serve_asset(req, asset(&MESSENGER, MESSENGER_JS), "application/javascript; charset=utf-8", "public, max-age=300"),
        "/messenger/demo" => demo_page(req),
        "/" => Response::redirect("/projects"),
        // Every other path is a console route rendered client-side.
        _ => Response::html(index_html()).with_header("Cache-Control", "no-cache"),
    }
}

const FAVICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="#1a73e8"/><path d="M8 10h16v9H14l-5 4v-4H8z" fill="#fff"/></svg>"##;
