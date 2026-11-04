use cx_core::{obj, Value};
use cx_engine::import::normalize_bundle;
use cx_engine::llm::ScriptedLlm;
use cx_engine::{response_texts, Agent, Runtime, SessionState};
use std::sync::Arc;

fn agent() -> Agent {
    let src = Value::parse(include_str!("../../../examples/help-center-agent.json")).unwrap();
    Agent::compile(&normalize_bundle(&src, "projects/demo/locations/global/agents/help")).unwrap()
}

fn say(rt: &Runtime, a: &Agent, st: &mut SessionState, text: &str) -> Value {
    rt.detect_intent(a, "s", st, &obj! {"queryInput" => obj!{"text" => obj!{"text" => text}}})
}

#[test]
fn knowledge_connector_answers_from_faq_and_documents() {
    let a = agent();
    let rt = Runtime::default();
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "how can I reset my password?");
    assert_eq!(response_texts(&r), vec!["Use 'Forgot password' on the sign-in page and follow the link we email you."]);
    let qr = r.at("queryResult");
    assert_eq!(qr.at("match").str("matchType"), "KNOWLEDGE_CONNECTOR");
    assert_eq!(qr.at("knowledgeAnswers").arr("answers")[0].str("faqQuestion"), "How do I reset my password?");

    let r = say(&rt, &a, &mut st, "how long do refunds take after I return an item?");
    let t = response_texts(&r);
    assert!(t[0].contains("Refunds are issued"), "{:?}", t);
    let src = &r.at("queryResult").at("knowledgeAnswers").arr("answers")[0].at("source");
    assert_eq!(src.str("uri"), "https://example.com/policies/returns");

    // intents still win over knowledge
    let r = say(&rt, &a, &mut st, "I want to talk to a human");
    assert_eq!(response_texts(&r), vec!["Connecting you to a support specialist."]);
}

#[test]
fn unanswerable_questions_fall_through_to_no_match_and_tests_pass() {
    let a = agent();
    let rt = Runtime::default();
    for tc in a.test_cases() {
        let res = cx_engine::testing::run_test_case(&rt, &a, tc, "draft");
        assert_eq!(res.str("testResult"), "PASSED", "{}", res.to_pretty());
    }
    let v = cx_engine::validate::validate(&a);
    assert_eq!(v.at("summary").f64_field("ERROR"), Some(0.0), "{}", v.to_pretty());
}

#[test]
fn grounded_generation_and_data_store_tool() {
    // Grounded answer via the LLM for unstructured content.
    let a = agent();
    let rt = Runtime { llm: Arc::new(ScriptedLlm::new(vec!["Electronics have a two-year warranty for defects."])), ..Default::default() };
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "what does the warranty cover for electronics?");
    assert_eq!(response_texts(&r), vec!["Electronics have a two-year warranty for defects."]);
    assert_eq!(r.at("queryResult").at("knowledgeAnswers").arr("answers")[0].at("generative"), &Value::Bool(true));

    // A playbook using a data-store tool.
    let bundle = Value::parse(
        r#"{
        "agent": {"displayName": "KB bot", "startPlaybook": "Helper"},
        "dataStores": [{"displayName": "kb", "documents": [{"displayName": "d", "title": "Parking", "content": "Free parking is available in the garage on level two."}]}],
        "tools": [{"displayName": "kb-search", "description": "Search the KB", "dataStoreSpec": {"dataStoreConnections": [{"dataStore": "kb"}]}}],
        "playbooks": [{"displayName": "Helper", "goal": "Answer questions", "referencedTools": ["kb-search"]}]
    }"#,
    )
    .unwrap();
    let a = Agent::compile(&normalize_bundle(&bundle, "projects/demo/locations/global/agents/kb")).unwrap();
    let rt = Runtime {
        llm: Arc::new(ScriptedLlm::new(vec![
            r#"{"tool": "kb-search", "input": {"query": "where can I park"}}"#,
            r#"{"say": "Parking is free on level two of the garage."}"#,
        ])),
        ..Default::default()
    };
    let mut st = SessionState::new();
    let r = say(&rt, &a, &mut st, "is there parking?");
    assert_eq!(response_texts(&r), vec!["Parking is free on level two of the garage."]);
    let hist = st.playbook.as_ref().unwrap().history.iter().map(|h| h.to_string()).collect::<String>();
    assert!(hist.contains("Free parking is available"), "{}", hist);
}
