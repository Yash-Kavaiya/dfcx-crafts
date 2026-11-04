use cx_core::{obj, Value};
use cx_engine::import::normalize_bundle;
use cx_engine::llm::{ScriptedLlm, Transport};
use cx_engine::{response_texts, Agent, Runtime, SessionState};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn agent() -> Agent {
    let src = Value::parse(include_str!("../../../examples/travel-playbook-agent.json")).unwrap();
    Agent::compile(&normalize_bundle(&src, "projects/demo/locations/global/agents/travel")).unwrap()
}

fn say(rt: &Runtime, a: &Agent, st: &mut SessionState, text: &str) -> Value {
    rt.detect_intent(a, "projects/demo/locations/global/agents/travel/sessions/t", st, &obj! {"queryInput" => obj!{"text" => obj!{"text" => text}}})
}

#[test]
fn offline_example_replay_multi_turn_with_output() {
    let a = agent();
    let rt = Runtime::default();
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "I'd like to fly to Lisbon");
    assert_eq!(response_texts(&r), vec!["Lovely choice! What date would you like to travel to Lisbon?"]);
    assert_eq!(r.at("queryResult").at("currentPlaybook").str("displayName"), "Concierge");
    let r = say(&rt, &a, &mut st, "next friday");
    assert_eq!(response_texts(&r), vec!["The cheapest flight to Lisbon next Friday is 89 EUR. Shall I hold it for you?"]);
    let r = say(&rt, &a, &mut st, "yes, hold it");
    assert_eq!(response_texts(&r), vec!["Done — your seat is on hold for 24 hours. Have a great trip!"]);
    assert_eq!(st.param("destination"), &Value::from("Lisbon"));
    assert!(st.ended, "start playbook finished and there is no start flow");
}

#[test]
fn playbook_hands_off_to_flow() {
    let a = agent();
    let rt = Runtime::default();
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "can I talk to a real person");
    let msgs = r.at("queryResult").arr("responseMessages");
    assert_eq!(response_texts(&r), vec!["Connecting you to a travel specialist now."]);
    assert!(msgs.iter().any(|m| m.get("liveAgentHandoff").is_some()));
}

struct FlightsApi(Mutex<Vec<String>>);
impl Transport for FlightsApi {
    fn send(&self, method: &str, url: &str, _h: &[(String, String)], _b: &str, _t: Duration) -> Result<(u16, String), String> {
        self.0.lock().unwrap().push(format!("{} {}", method, url));
        Ok((200, r#"{"flights":[{"to":"Rome","price":120},{"to":"Rome","price":75}]}"#.into()))
    }
}

#[test]
fn llm_loop_calls_openapi_tool_then_answers_and_ends() {
    let a = agent();
    let calls = Arc::new(FlightsApi(Mutex::new(vec![])));
    let llm = ScriptedLlm::new(vec![
        r#"{"tool": "flights", "action": "searchFlights", "input": {"destination": "Rome", "date": "2025-05-01"}}"#,
        r#"Here you go: {"say": "The cheapest flight to Rome is 75 EUR."}"#,
        r#"{"end": true, "say": "Enjoy Rome!", "output": {"destination": "Rome"}}"#,
    ]);
    let rt = Runtime { llm: Arc::new(llm), transport: calls.clone(), ..Default::default() };
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "flights to Rome on May 1st?");
    assert_eq!(response_texts(&r), vec!["The cheapest flight to Rome is 75 EUR."]);
    let seen = calls.0.lock().unwrap().clone();
    assert_eq!(seen, vec!["GET http://127.0.0.1:9099/api/flights?destination=Rome&date=2025-05-01"]);
    let r = say(&rt, &a, &mut st, "thanks, that's all");
    assert_eq!(response_texts(&r), vec!["Enjoy Rome!"]);
    assert_eq!(st.param("destination"), &Value::from("Rome"));
    // history kept the tool result for the model
    let diag = r.at("queryResult").at("diagnosticInfo").to_string();
    assert!(diag.contains("Playbook"));
}

#[test]
fn generator_fills_output_parameter() {
    let bundle = Value::parse(
        r#"{
        "agent": {"displayName": "Gen"},
        "generators": [{"displayName": "summarizer", "promptText": {"text": "Summarize for $name: $topic"}}],
        "flows": [{"displayName": "Default Start Flow", "transitionRoutes": [{
            "condition": "true",
            "triggerFulfillment": {
              "generators": [{"generator": "summarizer", "inputParameters": {"$name": "Ada", "$topic": "$request.user-utterance"}, "outputParameter": "$request.generative.summary"}],
              "messages": [{"text": {"text": ["Summary: $request.generative.summary"]}}]
            }}]}]
    }"#,
    )
    .unwrap();
    let a = Agent::compile(&normalize_bundle(&bundle, "projects/x/locations/global/agents/gen")).unwrap();
    let rt = Runtime { llm: Arc::new(ScriptedLlm::new(vec!["Short and sweet."])), ..Default::default() };
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "rust is a systems language");
    assert_eq!(response_texts(&r), vec!["Summary: Short and sweet."]);
}
