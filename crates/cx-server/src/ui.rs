//! The embedded single-page web console.

use cx_http::{Request, Response};

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

pub fn serve(req: &Request) -> Response {
    if req.method != "GET" && req.method != "HEAD" {
        return Response::text(405, "method not allowed");
    }
    match req.path.as_str() {
        "/static/app.js" => Response::new(200, "application/javascript; charset=utf-8", APP_JS).with_header("Cache-Control", "no-cache"),
        "/static/app.css" => Response::new(200, "text/css; charset=utf-8", APP_CSS).with_header("Cache-Control", "no-cache"),
        "/favicon.ico" => Response::new(200, "image/svg+xml", FAVICON),
        "/messenger.js" => Response::new(200, "application/javascript; charset=utf-8", MESSENGER_JS).with_header("Cache-Control", "public, max-age=300"),
        "/messenger/demo" => demo_page(req),
        "/" => Response::redirect("/projects"),
        // Every other path is a console route rendered client-side.
        _ => Response::html(INDEX),
    }
}

const FAVICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="#1a73e8"/><path d="M8 10h16v9H14l-5 4v-4H8z" fill="#fff"/></svg>"##;
