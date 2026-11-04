//! Test cases: replay conversation turns and diff against expectations
//! (the `TestCase` / `TestCaseResult` resources).

use crate::agent::{find_in, Agent};
use crate::runtime::{response_texts, Runtime};
use crate::session::SessionState;
use cx_core::name::{id_of, START_PAGE};
use cx_core::{arr, obj, Value};

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn matches_ref(expected: &Value, actual: &Value) -> bool {
    let en = expected.str("name");
    let ed = expected.str("displayName");
    (!en.is_empty() && (en == actual.str("name") || id_of(en) == id_of(actual.str("name"))))
        || (!ed.is_empty() && ed == actual.str("displayName"))
}

fn subset(expected: &Value, actual: &Value) -> Vec<String> {
    let mut diffs = Vec::new();
    if let Some(m) = expected.as_object() {
        for (k, v) in m.iter() {
            let a = actual.path(k).cloned().or_else(|| actual.get(k).cloned()).unwrap_or_default();
            let equal = match (v, &a) {
                (Value::Number(x), Value::Number(y)) => (x - y).abs() < 1e-9,
                (Value::Object(_), Value::Object(_)) => subset(v, &a).is_empty(),
                _ => *v == a,
            };
            if !equal {
                diffs.push(format!("parameter '{}': expected {}, got {}", k, v, a));
            }
        }
    }
    diffs
}

/// Runs one test case and returns a `TestCaseResult`.
pub fn run_test_case(rt: &Runtime, agent: &Agent, tc: &Value, environment: &str) -> Value {
    let mut st = SessionState::new();
    let session = format!("{}/sessions/test-{}", agent.name, cx_core::uuid::new_v4());
    let cfg = tc.at("testConfig");
    if let Some(f) = agent.flow(cfg.str("flow")) {
        st.flow = f.str("name").to_string();
        st.page = START_PAGE.into();
    }
    if let Some((f, p)) = agent.page_any(cfg.str("page")) {
        st.flow = f.str("name").to_string();
        st.page = p.str("name").to_string();
    }
    let mut turns_out = Vec::new();
    let mut passed = true;
    for turn in tc.arr("testCaseConversationTurns") {
        let ui = turn.at("userInput");
        let mut qp = obj! {"disableWebhook" => !ui.get("isWebhookEnabled").and_then(Value::as_bool).unwrap_or(false)};
        if let Some(p) = ui.get("injectedParameters").filter(|p| p.is_object()) {
            qp.set("parameters", p.clone());
        }
        let req = obj! {"queryInput" => ui.at("input").clone(), "queryParams" => qp};
        let resp = rt.detect_intent(agent, &session, &mut st, &req);
        let qr = resp.at("queryResult");
        let expected = turn.at("virtualAgentOutput");
        let mut differences = Vec::new();
        if let Some(ei) = expected.get("triggeredIntent").filter(|v| v.is_object()) {
            let actual = qr.at("intent");
            if !matches_ref(ei, actual) {
                let exp_dn = find_in(agent.intents(), ei.str("name")).map(|i| i.str("displayName").to_string()).unwrap_or_else(|| ei.str("displayName").to_string());
                differences.push(obj! {"type" => "INTENT", "description" => format!("Expected intent '{}', got '{}'", exp_dn, actual.str("displayName"))});
            }
        }
        if let Some(ep) = expected.get("currentPage").filter(|v| v.is_object()) {
            let actual = qr.at("currentPage");
            if !matches_ref(ep, actual) {
                differences.push(obj! {"type" => "PAGE", "description" => format!("Expected page '{}', got '{}'", if ep.str("displayName").is_empty() { ep.str("name") } else { ep.str("displayName") }, actual.str("displayName"))});
            }
        }
        if let Some(sp) = expected.get("sessionParameters").filter(|v| v.is_object()) {
            for d in subset(sp, qr.at("parameters")) {
                differences.push(obj! {"type" => "PARAMETERS", "description" => d});
            }
        }
        let actual_texts = response_texts(&resp);
        let expected_texts: Vec<String> =
            expected.arr("textResponses").iter().flat_map(|t| t.arr("text").iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>()).collect();
        if !expected_texts.is_empty() {
            let a = norm(&actual_texts.join(" "));
            let e = norm(&expected_texts.join(" "));
            if a != e {
                differences.push(obj! {"type" => "UTTERANCE", "description" => format!("Expected agent response '{}', got '{}'", expected_texts.join(" "), actual_texts.join(" "))});
            }
        }
        if !differences.is_empty() {
            passed = false;
        }
        turns_out.push(obj! {
            "userInput" => ui.clone(),
            "virtualAgentOutput" => obj!{
                "sessionParameters" => qr.at("parameters").clone(),
                "triggeredIntent" => qr.at("intent").clone(),
                "currentPage" => qr.at("currentPage").clone(),
                "textResponses" => Value::Array(actual_texts.iter().map(|t| obj!{"text" => arr![t.clone()]}).collect()),
                "differences" => Value::Array(differences),
                "status" => obj!{"code" => 0},
            },
        });
    }
    obj! {
        "name" => format!("{}/results/{}", tc.str("name"), cx_core::uuid::new_v4()),
        "environment" => environment,
        "conversationTurns" => Value::Array(turns_out),
        "testResult" => if passed { "PASSED" } else { "FAILED" },
        "testTime" => cx_core::time::now_rfc3339(),
    }
}

/// Builds a test case from a live conversation transcript (the console's
/// "save as test case" feature). `turns` are (request, response) pairs.
pub fn test_case_from_transcript(display_name: &str, turns: &[(Value, Value)]) -> Value {
    let conv: Vec<Value> = turns
        .iter()
        .map(|(req, resp)| {
            let qr = resp.at("queryResult");
            let mut vao = obj! {
                "sessionParameters" => qr.at("parameters").clone(),
                "currentPage" => qr.at("currentPage").clone(),
                "textResponses" => Value::Array(response_texts(resp).into_iter().map(|t| obj!{"text" => arr![t]}).collect()),
            };
            if qr.get("intent").is_some() {
                vao.set("triggeredIntent", qr.at("intent").clone());
            }
            obj! {"userInput" => obj!{"input" => req.at("queryInput").clone(), "isWebhookEnabled" => true}, "virtualAgentOutput" => vao}
        })
        .collect();
    obj! {"displayName" => display_name, "tags" => arr![], "testCaseConversationTurns" => Value::Array(conv)}
}
