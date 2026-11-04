use cx_core::{obj, Value};
use cx_engine::import::normalize_bundle;
use cx_engine::llm::Transport;
use cx_engine::{response_texts, Agent, Runtime, SessionState};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const AGENT: &str = "projects/demo/locations/global/agents/pizza";

fn agent() -> Agent {
    let src = Value::parse(include_str!("../../../examples/pizza-agent.json")).unwrap();
    Agent::compile(&normalize_bundle(&src, AGENT)).unwrap()
}

/// Records webhook requests and answers like an order service.
struct MockOrders {
    seen: Mutex<Vec<Value>>,
    fail: bool,
}

impl Transport for MockOrders {
    fn send(&self, _m: &str, _url: &str, _h: &[(String, String)], body: &str, _t: Duration) -> Result<(u16, String), String> {
        let req = Value::parse(body).unwrap();
        self.seen.lock().unwrap().push(req.clone());
        if self.fail {
            return Err("connection refused".into());
        }
        let size = req.at("sessionInfo").at("parameters").str("size").to_string();
        Ok((
            200,
            obj! {
                "fulfillmentResponse" => obj!{"messages" => cx_core::arr![obj!{"text" => obj!{"text" => cx_core::arr![format!("Your {} pizza is ordered. Order id ORD-0042.", size)]}}]},
                "sessionInfo" => obj!{"parameters" => obj!{"order_id" => "ORD-0042"}},
            }
            .to_string(),
        ))
    }
}

fn say(rt: &Runtime, a: &Agent, st: &mut SessionState, text: &str) -> Value {
    rt.detect_intent(a, &format!("{}/sessions/s1", AGENT), st, &obj! {"queryInput" => obj!{"text" => obj!{"text" => text}}})
}

fn page(r: &Value) -> String {
    r.at("queryResult").at("currentPage").str("displayName").to_string()
}

#[test]
fn full_order_conversation_with_webhook() {
    let a = agent();
    let mock = Arc::new(MockOrders { seen: Mutex::new(vec![]), fail: false });
    let rt = Runtime { transport: mock.clone(), ..Default::default() };
    let mut st = SessionState::new();

    let r = say(&rt, &a, &mut st, "hello");
    assert_eq!(r.at("queryResult").at("intent").str("displayName"), "Default Welcome Intent");
    assert!(response_texts(&r)[0].starts_with("Welcome to Rusty Pizza"));

    let r = say(&rt, &a, &mut st, "can I order a pizza");
    assert_eq!(page(&r), "Collect Order");
    assert_eq!(response_texts(&r), vec!["What size would you like: small, medium or large?"]);

    // no-match on the size parameter uses the reprompt handler
    let r = say(&rt, &a, &mut st, "purple");
    assert_eq!(response_texts(&r), vec!["Sorry, which size? Small, medium or large?"]);
    assert_eq!(r.at("queryResult").at("match").str("matchType"), "NO_MATCH");

    // filling two parameters with one utterance (size + toppings)
    let r = say(&rt, &a, &mut st, "large with ham and pineapple");
    let texts = response_texts(&r);
    assert_eq!(page(&r), "Confirm", "{:?}", texts);
    assert_eq!(texts[0], "Got it: 1 large pizza with ham, pineapple. That's $15.");
    assert_eq!(texts[1], "Large is our most popular choice!");
    assert_eq!(texts[2], "Shall I place the order?");

    let r = say(&rt, &a, &mut st, "yes please");
    assert_eq!(response_texts(&r), vec!["Your large pizza is ordered. Order id ORD-0042."]);
    assert_eq!(r.at("queryResult").at("parameters").str("order_id"), "ORD-0042");
    // END_FLOW returns to the start flow
    assert_eq!(page(&r), "Start Page");
    assert_eq!(r.at("queryResult").at("currentFlow").str("displayName"), "Default Start Flow");
    let sent = mock.seen.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].at("fulfillmentInfo").str("tag"), "place-order");
    assert_eq!(sent[0].at("intentInfo").str("displayName"), "confirm.yes");
    drop(sent);

    // the conversation continues in the start flow
    let r = say(&rt, &a, &mut st, "what are your opening hours");
    assert_eq!(response_texts(&r), vec!["We're open every day from 11am to 11pm."]);
}

#[test]
fn route_group_cancel_ends_session_and_webhook_error_event() {
    let a = agent();
    let rt = Runtime { transport: Arc::new(MockOrders { seen: Mutex::new(vec![]), fail: true }), ..Default::default() };
    let mut st = SessionState::new();
    say(&rt, &a, &mut st, "I want a small pepperoni pizza please");
    assert_eq!(st.param("size"), &Value::from("small"));
    let r = say(&rt, &a, &mut st, "yes");
    // webhook down -> webhook.error handler in the flow
    assert_eq!(response_texts(&r), vec!["Our ordering system is down right now. Please try again later."]);
    assert!(st.ended);
    let msgs = r.at("queryResult").arr("responseMessages");
    assert!(msgs.iter().any(|m| m.get("endInteraction").is_some()));

    let mut st = SessionState::new();
    say(&rt, &a, &mut st, "order pizza");
    let r = say(&rt, &a, &mut st, "never mind");
    assert_eq!(response_texts(&r), vec!["Okay, I've cancelled that. Come back hungry!"]);
    assert!(st.ended);
}

#[test]
fn reprompt_escalation_sets_default_and_confirm_no_loops_back() {
    let a = agent();
    let rt = Runtime { transport: Arc::new(MockOrders { seen: Mutex::new(vec![]), fail: false }), ..Default::default() };
    let mut st = SessionState::new();
    say(&rt, &a, &mut st, "order pizza");
    say(&rt, &a, &mut st, "banana");
    let r = say(&rt, &a, &mut st, "banana");
    let t = response_texts(&r);
    assert_eq!(t[0], "Let's go with medium then.");
    assert_eq!(st.param("size"), &Value::from("medium"));
    assert_eq!(t.last().unwrap(), "Which toppings would you like?");
    say(&rt, &a, &mut st, "olives");
    let r = say(&rt, &a, &mut st, "no");
    let t = response_texts(&r);
    assert_eq!(t[0], "No problem, let's start over.");
    assert_eq!(page(&r), "Collect Order");
    assert!(st.param("size").is_null());
}

#[test]
fn test_cases_pass_and_agent_validates() {
    let a = agent();
    let rt = Runtime::default();
    for tc in a.test_cases() {
        let res = cx_engine::testing::run_test_case(&rt, &a, tc, "draft");
        assert_eq!(res.str("testResult"), "PASSED", "{}: {}", tc.str("displayName"), res.to_pretty());
    }
    let v = cx_engine::validate::validate(&a);
    assert_eq!(v.at("summary").f64_field("ERROR"), Some(0.0), "{}", v.to_pretty());
}

#[test]
fn events_direct_intents_and_no_input() {
    let a = agent();
    let rt = Runtime::default();
    let mut st = SessionState::new();
    let r = rt.detect_intent(&a, "s", &mut st, &obj! {"queryInput" => obj!{"text" => obj!{"text" => "  "}}});
    assert_eq!(response_texts(&r), vec!["Are you still there?"]);
    assert_eq!(r.at("queryResult").at("match").str("matchType"), "NO_INPUT");
    let r = rt.detect_intent(&a, "s", &mut st, &obj! {"queryInput" => obj!{"intent" => obj!{"intent" => "store.hours"}}});
    assert_eq!(response_texts(&r), vec!["We're open every day from 11am to 11pm."]);
    let r = say(&rt, &a, &mut st, "what's the weather like");
    assert_eq!(r.at("queryResult").at("match").str("matchType"), "NO_MATCH");
    assert!(response_texts(&r)[0].starts_with("Sorry, I didn't catch that."));
}
