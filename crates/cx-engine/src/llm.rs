//! Pluggable LLM providers (used by generators and playbooks) and the
//! outbound HTTP transport used for webhooks and OpenAPI tools.

use cx_core::{obj, Value};
use std::time::Duration;

/// A chat-completion capable model.
pub trait Llm: Send + Sync {
    fn name(&self) -> String;
    /// `messages` are (role, content) pairs with roles `user` / `assistant`.
    fn chat(&self, system: &str, messages: &[(String, String)], temperature: f64) -> Result<String, String>;
    fn available(&self) -> bool {
        true
    }
}

/// The offline default: no model configured. Playbooks fall back to
/// deterministic example replay and generators produce no text.
pub struct NoLlm;

impl Llm for NoLlm {
    fn name(&self) -> String {
        "none".into()
    }
    fn chat(&self, _: &str, _: &[(String, String)], _: f64) -> Result<String, String> {
        Err("no LLM provider configured (set CXA_LLM_URL to an OpenAI-compatible endpoint)".into())
    }
    fn available(&self) -> bool {
        false
    }
}

/// Any OpenAI-compatible `/v1/chat/completions` endpoint reachable over
/// plain HTTP (e.g. a local Ollama, vLLM, LiteLLM or llama.cpp server).
pub struct OpenAiCompatible {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
}

impl OpenAiCompatible {
    /// Reads `CXA_LLM_URL`, `CXA_LLM_MODEL`, `CXA_LLM_API_KEY`.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("CXA_LLM_URL").ok().filter(|s| !s.is_empty())?;
        Some(OpenAiCompatible {
            base_url,
            model: std::env::var("CXA_LLM_MODEL").unwrap_or_else(|_| "default".into()),
            api_key: std::env::var("CXA_LLM_API_KEY").ok().filter(|s| !s.is_empty()),
            timeout: Duration::from_secs(60),
        })
    }
}

impl Llm for OpenAiCompatible {
    fn name(&self) -> String {
        format!("openai-compatible:{}", self.model)
    }
    fn chat(&self, system: &str, messages: &[(String, String)], temperature: f64) -> Result<String, String> {
        let mut msgs = vec![obj! {"role" => "system", "content" => system}];
        for (r, c) in messages {
            msgs.push(obj! {"role" => r.clone(), "content" => c.clone()});
        }
        let body = obj! {"model" => self.model.clone(), "messages" => Value::Array(msgs), "temperature" => temperature};
        let mut url = self.base_url.trim_end_matches('/').to_string();
        if !url.ends_with("/chat/completions") {
            url.push_str("/chat/completions");
        }
        let mut headers = vec![];
        if let Some(k) = &self.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {}", k)));
        }
        let resp = cx_http::post_json(&url, &body.to_string(), &headers, self.timeout)?;
        if resp.status >= 300 {
            return Err(format!("LLM endpoint returned HTTP {}: {}", resp.status, resp.body_str()));
        }
        let v = Value::parse(&resp.body_str()).map_err(|e| e.to_string())?;
        v.path("choices.0.message.content").and_then(Value::as_str).map(String::from).ok_or_else(|| "unexpected LLM response shape".into())
    }
}

/// Picks the provider from the environment.
pub fn from_env() -> Box<dyn Llm> {
    match OpenAiCompatible::from_env() {
        Some(p) => Box::new(p),
        None => Box::new(NoLlm),
    }
}

/// Outbound HTTP used by webhooks and OpenAPI tools. Replaceable for tests.
pub trait Transport: Send + Sync {
    fn send(&self, method: &str, url: &str, headers: &[(String, String)], body: &str, timeout: Duration) -> Result<(u16, String), String>;
}

pub struct HttpTransport;

impl Transport for HttpTransport {
    fn send(&self, method: &str, url: &str, headers: &[(String, String)], body: &str, timeout: Duration) -> Result<(u16, String), String> {
        let r = cx_http::request(method, url, headers, body.as_bytes(), timeout)?;
        Ok((r.status, r.body_str()))
    }
}

/// A scripted LLM for tests: returns canned replies in order.
pub struct ScriptedLlm {
    replies: std::sync::Mutex<Vec<String>>,
}

impl ScriptedLlm {
    pub fn new(replies: Vec<&str>) -> Self {
        ScriptedLlm { replies: std::sync::Mutex::new(replies.into_iter().rev().map(String::from).collect()) }
    }
}

impl Llm for ScriptedLlm {
    fn name(&self) -> String {
        "scripted".into()
    }
    fn chat(&self, _: &str, _: &[(String, String)], _: f64) -> Result<String, String> {
        self.replies.lock().unwrap().pop().ok_or_else(|| "script exhausted".into())
    }
}

pub fn base64(input: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Extracts the first JSON object embedded in a model reply.
pub fn extract_json(reply: &str) -> Option<Value> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    if end <= start {
        return None;
    }
    Value::parse(&reply[start..=end]).ok()
}


#[cfg(test)]
mod tests {
    #[test]
    fn b64() {
        assert_eq!(super::base64(b"user:pass"), "dXNlcjpwYXNz");
        assert_eq!(super::base64(b"a"), "YQ==");
        assert_eq!(super::extract_json("sure: {\"say\":\"hi\"} ok").unwrap().str("say"), "hi");
    }
}
