//! The embedded single-page web console.

use cx_http::{Request, Response};

const INDEX: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");
const APP_CSS: &str = include_str!("../assets/app.css");

pub fn serve(req: &Request) -> Response {
    if req.method != "GET" && req.method != "HEAD" {
        return Response::text(405, "method not allowed");
    }
    match req.path.as_str() {
        "/static/app.js" => Response::new(200, "application/javascript; charset=utf-8", APP_JS).with_header("Cache-Control", "no-cache"),
        "/static/app.css" => Response::new(200, "text/css; charset=utf-8", APP_CSS).with_header("Cache-Control", "no-cache"),
        "/favicon.ico" => Response::new(200, "image/svg+xml", FAVICON),
        "/" => Response::redirect("/projects"),
        // Every other path is a console route rendered client-side.
        _ => Response::html(INDEX),
    }
}

const FAVICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="#1a73e8"/><path d="M8 10h16v9H14l-5 4v-4H8z" fill="#fff"/></svg>"##;
