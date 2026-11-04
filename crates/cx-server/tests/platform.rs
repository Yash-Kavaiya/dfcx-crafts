//! End-to-end tests of the platform over real HTTP.

use cx_core::{obj, Value};
use cx_http::{request, Server};
use cx_server::api::Api;
use cx_server::store::Store;
use std::sync::Arc;
use std::time::Duration;

struct Client {
    base: String,
}

impl Client {
    fn start() -> Client {
        std::env::set_var("CXA_LOG", "0");
        let api = Arc::new(Api::new(Arc::new(Store::in_memory()), cx_engine::Runtime::default()));
        let server = Server::bind("127.0.0.1:0").unwrap();
        let addr = server.spawn(cx_server::handler(api, None));
        Client { base: format!("http://{}", addr) }
    }
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
        let b = body.map(|b| b.to_string()).unwrap_or_default();
        let r = request(method, &format!("{}{}", self.base, path), &[("Content-Type".into(), "application/json".into())], b.as_bytes(), Duration::from_secs(10)).unwrap();
        let text = r.body_str();
        (r.status, if text.trim_start().starts_with('{') { Value::parse(&text).unwrap() } else { Value::String(text) })
    }
    fn ok(&self, method: &str, path: &str, body: Option<&Value>) -> Value {
        let (s, v) = self.call(method, path, body);
        assert!(s < 300, "{} {} -> {} {}", method, path, s, v);
        v
    }
    fn say(&self, session: &str, text: &str) -> Value {
        self.ok("POST", &format!("/v3/{}:detectIntent", session), Some(&obj! {"queryInput" => obj!{"text" => obj!{"text" => text}, "languageCode" => "en"}}))
    }
}

fn texts(r: &Value) -> Vec<String> {
    cx_engine::response_texts(r)
}

#[test]
fn multi_tenant_platform_lifecycle() {
    let c = Client::start();
    // console is served
    let (s, html) = c.call("GET", "/projects", None);
    assert_eq!(s, 200);
    assert!(html.as_str().unwrap().contains("Conversational Agents"));
    assert_eq!(c.call("GET", "/static/app.js", None).0, 200);

    // projects
    let p1 = c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "Retail Bots", "projectId" => "retail"}));
    assert_eq!(p1.str("name"), "projects/retail");
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "Bank"}));
    assert_eq!(c.ok("GET", "/v3/projects", None).arr("projects").len(), 2);
    assert_eq!(c.call("POST", "/v3/projects", Some(&obj! {"displayName" => "x", "projectId" => "retail"})).0, 409);
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "Bank", "projectId" => "bank-eu"}));

    // agents in different locations / projects
    let a1 = c.ok("POST", "/v3/projects/retail/locations/global/agents", Some(&obj! {"displayName" => "Support", "timeZone" => "UTC"}));
    let a1n = a1.str("name").to_string();
    c.ok("POST", "/v3/projects/retail/locations/us-central1/agents", Some(&obj! {"displayName" => "Sales"}));
    c.ok("POST", "/v3/projects/bank/locations/global/agents", Some(&obj! {"displayName" => "Support"}));
    assert_eq!(c.ok("GET", "/v3/projects/retail/locations/global/agents", None).arr("agents").len(), 1);
    assert_eq!(c.call("POST", "/v3/projects/retail/locations/global/agents", Some(&obj! {"displayName" => "Support"})).0, 409);
    assert_eq!(c.call("POST", "/v3/projects/nope/locations/global/agents", Some(&obj! {"displayName" => "X"})).0, 404);

    // a new agent has the default skeleton and greets
    let flows = c.ok("GET", &format!("/v3/{}/flows", a1n), None);
    assert_eq!(flows.arr("flows")[0].str("displayName"), "Default Start Flow");
    let r = c.say(&format!("{}/sessions/s1", a1n), "hello");
    assert_eq!(r.at("queryResult").at("intent").str("displayName"), "Default Welcome Intent");

    // build: intent + page + route through the API
    let flow = flows.arr("flows")[0].str("name").to_string();
    let intent = c.ok(
        "POST",
        &format!("/v3/{}/intents", a1n),
        Some(&obj! {"displayName" => "refund", "trainingPhrases" => cx_core::arr![
            obj!{"parts" => cx_core::arr![obj!{"text" => "I want a refund"}]},
            obj!{"parts" => cx_core::arr![obj!{"text" => "refund my order"}]},
            obj!{"parts" => cx_core::arr![obj!{"text" => "money back please"}]}]}),
    );
    let page = c.ok(
        "POST",
        &format!("/v3/{}/pages", flow),
        Some(&obj! {"displayName" => "Refund", "entryFulfillment" => obj!{"messages" => cx_core::arr![obj!{"text" => obj!{"text" => cx_core::arr!["Sure, let's get your refund started."]}}]}}),
    );
    let mut f = c.ok("GET", &format!("/v3/{}", flow), None);
    let mut routes = f.arr("transitionRoutes").to_vec();
    routes.push(obj! {"intent" => intent.str("name"), "targetPage" => page.str("name")});
    f.set("transitionRoutes", Value::Array(routes));
    c.ok("PATCH", &format!("/v3/{}?updateMask=transitionRoutes", flow), Some(&f));
    let r = c.say(&format!("{}/sessions/s2", a1n), "I'd like a refund for my order");
    assert_eq!(texts(&r), vec!["Sure, let's get your refund started."]);
    assert_eq!(r.at("queryResult").at("currentPage").str("displayName"), "Refund");

    // matchIntent does not change state
    let m = c.ok("POST", &format!("/v3/{}/sessions/s3:matchIntent", a1n), Some(&obj! {"queryInput" => obj!{"text" => obj!{"text" => "refund please"}}}));
    assert_eq!(m.arr("matches")[0].at("intent").str("displayName"), "refund");

    // versions + environments: snapshot, then change draft, environment keeps old behaviour
    let v = c.ok("POST", &format!("/v3/{}/versions", flow), Some(&obj! {"displayName" => "v1"}));
    let vname = v.at("response").str("name").to_string();
    let mut page_upd = c.ok("GET", &format!("/v3/{}", page.str("name")), None);
    page_upd.set("entryFulfillment", obj! {"messages" => cx_core::arr![obj!{"text" => obj!{"text" => cx_core::arr!["Refunds are handled by phone now."]}}]});
    c.ok("PATCH", &format!("/v3/{}", page.str("name")), Some(&page_upd));
    let env = c.ok("POST", &format!("/v3/{}/environments", a1n), Some(&obj! {"displayName" => "prod", "versionConfigs" => cx_core::arr![obj!{"version" => vname.clone()}]}));
    let r = c.say(&format!("{}/sessions/d1", a1n), "refund my order");
    assert_eq!(texts(&r), vec!["Refunds are handled by phone now."]);
    let r = c.say(&format!("{}/sessions/e1", env.str("name")), "refund my order");
    assert_eq!(texts(&r), vec!["Sure, let's get your refund started."]);
    // load version back into the draft
    c.ok("POST", &format!("/v3/{}:load", vname), Some(&obj! {}));
    let r = c.say(&format!("{}/sessions/d2", a1n), "refund my order");
    assert_eq!(texts(&r), vec!["Sure, let's get your refund started."]);

    // conversation history and change history
    let convs = c.ok("GET", &format!("/v3/{}/conversations", a1n), None);
    assert!(convs.arr("conversations").len() >= 4);
    let conv = c.ok("GET", &format!("/v3/{}/conversations/s2", a1n), None);
    assert_eq!(conv.arr("interactions").len(), 1);
    let cl = c.ok("GET", &format!("/v3/{}/changelogs", a1n), None);
    assert!(cl.arr("changelogs").iter().any(|x| x.str("action") == "Create" && x.str("displayName") == "refund"));

    // save a simulator conversation as a test case and run it
    let tc = c.ok("POST", &format!("/v3/{}:saveTestCase", a1n), Some(&obj! {"session" => format!("{}/sessions/d2", a1n), "displayName" => "refund path"}));
    let run = c.ok("POST", &format!("/v3/{}:run", tc.str("name")), Some(&obj! {}));
    assert_eq!(run.at("response").at("result").str("testResult"), "PASSED");
    let batch = c.ok("POST", &format!("/v3/{}/testCases:batchRun", a1n), Some(&obj! {}));
    assert_eq!(batch.at("response").arr("results").len(), 1);
    let cov = c.ok("POST", &format!("/v3/{}/testCases:calculateCoverage", a1n), Some(&obj! {"type" => "INTENT"}));
    assert!(cov.at("intentCoverage").f64_field("coverageScore").unwrap() > 0.0);

    // validation
    let val = c.ok("POST", &format!("/v3/{}:validate", a1n), None);
    assert_eq!(val.at("summary").f64_field("ERROR"), Some(0.0), "{}", val.to_pretty());
    c.ok("GET", &format!("/v3/{}/validationResult", a1n), None);

    // export, import into another project, restore
    let exp = c.ok("POST", &format!("/v3/{}:export", a1n), Some(&obj! {}));
    let content = exp.at("response").at("agentContent").clone();
    assert!(content.arr("intents").iter().any(|i| i.str("displayName") == "refund"));
    let copy = c.ok("POST", "/v3/projects/bank/locations/global/agents:import", Some(&obj! {"agentContent" => content.clone(), "agentId" => "support-copy"}));
    assert_eq!(copy.str("name"), "projects/bank/locations/global/agents/support-copy");
    let r = c.say("projects/bank/locations/global/agents/support-copy/sessions/x", "refund my order");
    assert_eq!(texts(&r), vec!["Sure, let's get your refund started."]);
    let pizza = Value::parse(include_str!("../../../examples/pizza-agent.json")).unwrap();
    c.ok("POST", "/v3/projects/bank/locations/global/agents/support-copy:restore", Some(&obj! {"agentContent" => pizza}));
    let r = c.say("projects/bank/locations/global/agents/support-copy/sessions/y", "hi");
    assert!(texts(&r)[0].starts_with("Welcome to Rusty Pizza"));
    let tcs = c.ok("POST", "/v3/projects/bank/locations/global/agents/support-copy/testCases:batchRun", Some(&obj! {}));
    for res in tcs.at("response").arr("results") {
        assert_eq!(res.str("testResult"), "PASSED", "{}", res.to_pretty());
    }

    // errors are Google-style
    let (s, e) = c.call("GET", "/v3/projects/retail/locations/global/agents/missing", None);
    assert_eq!(s, 404);
    assert_eq!(e.at("error").str("status"), "NOT_FOUND");

    // deleting a project removes its agents
    c.ok("DELETE", "/v3/projects/bank", None);
    assert_eq!(c.call("GET", "/v3/projects/bank/locations/global/agents/support-copy", None).0, 404);
}

#[test]
fn session_entity_types_and_pagination() {
    let c = Client::start();
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "P", "projectId" => "p"}));
    let pizza = Value::parse(include_str!("../../../examples/pizza-agent.json")).unwrap();
    let a = c.ok("POST", "/v3/projects/p/locations/global/agents:import", Some(&obj! {"agentContent" => pizza, "agentId" => "pizza"}));
    let an = a.str("name").to_string();
    // add a session-scoped topping
    let sess = format!("{}/sessions/vip", an);
    c.ok(
        "POST",
        &format!("/v3/{}/entityTypes?id=topping", sess),
        Some(&obj! {"displayName" => "topping", "entityOverrideMode" => "ENTITY_OVERRIDE_MODE_SUPPLEMENT",
            "entities" => cx_core::arr![obj!{"value" => "truffle", "synonyms" => cx_core::arr!["truffle", "truffles"]}]}),
    );
    c.say(&sess, "order pizza");
    c.say(&sess, "large");
    let r = c.say(&sess, "truffles");
    assert_eq!(r.at("queryResult").at("parameters").at("topping"), &cx_core::arr!["truffle"]);
    // pagination
    let page1 = c.ok("GET", &format!("/v3/{}/intents?pageSize=3", an), None);
    assert_eq!(page1.arr("intents").len(), 3);
    let tok = page1.str("nextPageToken").to_string();
    let page2 = c.ok("GET", &format!("/v3/{}/intents?pageSize=3&pageToken={}", an, tok), None);
    assert!(!page2.arr("intents").is_empty());
}

#[test]
fn zip_agent_import_and_json_package_export() {
    let c = Client::start();
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "Z", "projectId" => "z"}));
    // a Google-style JSON-package export zip (built with Python's zipfile, deflated)
    let zip = include_bytes!("../../cx-core/src/testdata_agent.zip");
    let b64 = cx_core::base64::encode(zip);
    let a = c.ok("POST", "/v3/projects/z/locations/global/agents:import", Some(&obj! {"agentContent" => b64, "agentId" => "zipped"}));
    assert_eq!(a.str("displayName"), "Zipped");
    let r = c.say("projects/z/locations/global/agents/zipped/sessions/s", "hello there");
    assert_eq!(texts(&r), vec!["Hi from a zip!"]);
    // export as JSON package (base64 zip) and restore it into a second agent
    let exp = c.ok("POST", "/v3/projects/z/locations/global/agents/zipped:export", Some(&obj! {"dataFormat" => "JSON_PACKAGE"}));
    let content = exp.at("response").str("agentContent").to_string();
    assert!(cx_core::zip::is_zip(&cx_core::base64::decode(&content).unwrap()));
    c.ok("POST", "/v3/projects/z/locations/global/agents", Some(&obj! {"displayName" => "Target"}));
    let target = c.ok("GET", "/v3/projects/z/locations/global/agents", None).arr("agents").iter().find(|x| x.str("displayName") == "Target").unwrap().str("name").to_string();
    c.ok("POST", &format!("/v3/{}:restore", target), Some(&obj! {"agentContent" => content}));
    let r = c.say(&format!("{}/sessions/t", target), "hello there");
    assert_eq!(texts(&r), vec!["Hi from a zip!"]);
    // garbage is rejected cleanly
    let (s, e) = c.call("POST", "/v3/projects/z/locations/global/agents:import", Some(&obj! {"agentContent" => "UEsDBAo="}));
    assert_eq!(s, 400, "{}", e);
}

#[test]
fn messenger_widget_is_served_and_demo_validates_input() {
    let c = Client::start();
    let (s, js) = c.call("GET", "/messenger.js", None);
    assert_eq!(s, 200);
    assert!(js.as_str().unwrap().contains("customElements.define('cx-messenger'"));
    let (s, page) = c.call("GET", "/messenger/demo?agent=projects/p/locations/global/agents/a&title=Hi", None);
    assert_eq!(s, 200);
    assert!(page.as_str().unwrap().contains(r#"<cx-messenger agent="projects/p/locations/global/agents/a""#));
    // attribute injection is rejected
    let (s, _) = c.call("GET", "/messenger/demo?agent=x%22%3E%3Cscript%3E", None);
    assert_eq!(s, 400);
}

#[test]
fn data_stores_and_knowledge_connectors_over_http() {
    let c = Client::start();
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "K", "projectId" => "k"}));
    let loc = "projects/k/locations/global";
    let ds = c.ok("POST", &format!("/v3/{}/dataStores", loc), Some(&obj! {"displayName" => "faq", "type" => "FAQ"}));
    let dsn = ds.str("name").to_string();
    let imp = c.ok("POST", &format!("/v3/{}/documents:import", dsn), Some(&obj! {"faqCsv" => "question,answer\nIs parking free?,\"Yes, parking is free for customers.\"\nDo you have wifi?,Free wifi is available in every store.\n"}));
    assert_eq!(imp.at("response").f64_field("importedCount"), Some(2.0));
    let s = c.ok("POST", &format!("/v3/{}:search", dsn), Some(&obj! {"query" => "is there wifi"}));
    assert_eq!(s.arr("results")[0].str("snippet"), "Free wifi is available in every store.");

    // a new agent whose start flow uses the data store
    let a = c.ok("POST", &format!("/v3/{}/agents", loc), Some(&obj! {"displayName" => "Store bot"}));
    let an = a.str("name").to_string();
    let flow = c.ok("GET", &format!("/v3/{}/flows", an), None).arr("flows")[0].str("name").to_string();
    c.ok(
        "PATCH",
        &format!("/v3/{}?updateMask=knowledgeConnectorSettings", flow),
        Some(&obj! {"knowledgeConnectorSettings" => obj!{"enabled" => true, "dataStoreConnections" => cx_core::arr![obj!{"dataStoreType" => "FAQ", "dataStore" => dsn.clone()}]}}),
    );
    let r = c.say(&format!("{}/sessions/s", an), "can I park for free?");
    assert_eq!(texts(&r), vec!["Yes, parking is free for customers."]);
    // export carries the referenced data store; importing elsewhere recreates it
    let exp = c.ok("POST", &format!("/v3/{}:export", an), Some(&obj! {}));
    let content = exp.at("response").at("agentContent").clone();
    assert_eq!(content.arr("dataStores").len(), 1);
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "K2", "projectId" => "k2"}));
    c.ok("POST", "/v3/projects/k2/locations/global/agents:import", Some(&obj! {"agentContent" => content, "agentId" => "copy"}));
    let r = c.say("projects/k2/locations/global/agents/copy/sessions/s", "do you have wifi?");
    assert_eq!(texts(&r), vec!["Free wifi is available in every store."]);
    assert_eq!(c.ok("GET", "/v3/projects/k2/locations/global/dataStores", None).arr("dataStores").len(), 1);
    // the help-center example imports with both stores and passes its tests
    let help = Value::parse(include_str!("../../../examples/help-center-agent.json")).unwrap();
    c.ok("POST", &format!("/v3/{}/agents:import", loc), Some(&obj! {"agentContent" => help, "agentId" => "help"}));
    let run = c.ok("POST", &format!("/v3/{}/agents/help/testCases:batchRun", loc), Some(&obj! {}));
    for res in run.at("response").arr("results") {
        assert_eq!(res.str("testResult"), "PASSED", "{}", res.to_pretty());
    }
}

#[test]
fn redaction_and_security_settings() {
    let c = Client::start();
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "R", "projectId" => "r"}));
    let bundle = Value::parse(
        r#"{"agent": {"displayName": "Bank"},
        "entityTypes": [{"displayName": "pin", "kind": "KIND_REGEXP", "redact": true, "entities": [{"value": "\\d{4}", "synonyms": ["\\d{4}"]}]}],
        "flows": [{"displayName": "Default Start Flow",
          "transitionRoutes": [{"condition": "true", "targetPage": "Verify"}],
          "pages": [{"displayName": "Verify", "form": {"parameters": [
              {"displayName": "pin", "entityType": "@pin", "required": true, "fillBehavior": {"initialPromptFulfillment": {"messages": [{"text": {"text": ["Your PIN?"]}}]}}},
              {"displayName": "email", "entityType": "@sys.email", "required": true, "redact": true, "fillBehavior": {"initialPromptFulfillment": {"messages": [{"text": {"text": ["Email?"]}}]}}}]},
            "transitionRoutes": [{"condition": "$page.params.status = \"FINAL\"", "triggerFulfillment": {"messages": [{"text": {"text": ["Verified $session.params.email with PIN $session.params.pin."]}}]}, "targetPage": "END_SESSION"}]}]}]}"#,
    )
    .unwrap();
    c.ok("POST", "/v3/projects/r/locations/global/agents:import", Some(&obj! {"agentContent" => bundle, "agentId" => "bank"}));
    let s = "projects/r/locations/global/agents/bank/sessions/x";
    c.say(s, "hello");
    c.say(s, "it is 9876");
    let r = c.say(s, "ada@example.com");
    // the caller still receives real values
    assert_eq!(texts(&r), vec!["Verified ada@example.com with PIN 9876."]);
    // ...but conversation history is redacted
    let conv = c.ok("GET", "/v3/projects/r/locations/global/agents/bank/conversations/x", None).to_string();
    assert!(!conv.contains("9876") && !conv.contains("ada@example.com"), "{}", conv);
    assert!(conv.contains("[redacted]"));
    // security settings that purge history stop logging entirely
    let sec = c.ok("POST", "/v3/projects/r/locations/global/securitySettings", Some(&obj! {"displayName" => "no-history", "purgeDataTypes" => cx_core::arr!["DIALOGFLOW_HISTORY"]}));
    c.ok("PATCH", "/v3/projects/r/locations/global/agents/bank?updateMask=securitySettings", Some(&obj! {"securitySettings" => sec.str("name")}));
    c.say("projects/r/locations/global/agents/bank/sessions/y", "hello");
    assert_eq!(c.call("GET", "/v3/projects/r/locations/global/agents/bank/conversations/y", None).0, 404);
}

#[test]
fn analytics_aggregates_conversations() {
    let c = Client::start();
    c.ok("POST", "/v3/projects", Some(&obj! {"displayName" => "A", "projectId" => "a"}));
    let pizza = Value::parse(include_str!("../../../examples/pizza-agent.json")).unwrap();
    c.ok("POST", "/v3/projects/a/locations/global/agents:import", Some(&obj! {"agentContent" => pizza, "agentId" => "p"}));
    let base = "projects/a/locations/global/agents/p";
    c.say(&format!("{}/sessions/1", base), "hi");
    c.say(&format!("{}/sessions/1", base), "what's the weather like on mars");
    c.say(&format!("{}/sessions/2", base), "order pizza");
    c.say(&format!("{}/sessions/2", base), "cancel");
    let a = c.ok("GET", &format!("/v3/{}/analytics?days=7", base), None);
    let s = a.at("summary");
    assert_eq!(s.f64_field("sessions"), Some(2.0));
    assert_eq!(s.f64_field("turns"), Some(4.0));
    assert_eq!(s.f64_field("noMatchRate"), Some(0.25));
    assert_eq!(s.f64_field("endedSessions"), Some(1.0));
    assert_eq!(a.arr("recentUnmatched")[0].str("text"), "what's the weather like on mars");
    assert!(a.arr("topIntents").iter().any(|i| i.str("name") == "order.pizza"));
    assert_eq!(a.arr("turnsPerDay").len(), 7);
    assert_eq!(a.arr("turnsPerDay")[6].f64_field("turns"), Some(4.0));
    assert_eq!(a.arr("dropOffPages")[0].str("name"), "Default Start Flow › Start Page");
    assert_eq!(c.call("GET", "/v3/projects/a/locations/global/agents/missing/analytics", None).0, 404);
}
