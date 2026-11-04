//! `cxa` — the Conversational Agents platform command-line tool.

use cx_core::{obj, Value};
use cx_engine::import::{export_cx_directory, import_cx_directory, normalize_bundle};
use cx_engine::{Agent, Runtime, SessionState};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

const HELP: &str = "cxa — Conversational Agents platform (Rust)

USAGE:
  cxa serve [--addr HOST:PORT] [--data FILE | --memory] [--api-key KEY] [--seed AGENT.json ...]
      Run the multi-tenant platform: REST API under /v3 and the web console at /projects.
  cxa chat AGENT [--say TEXT ...] [--debug]
      Talk to an agent bundle (.json) or an exported JSON-package directory in the terminal.
  cxa test AGENT
      Run the agent's test cases; exits non-zero on failure.
  cxa validate AGENT
      Validate an agent and print the findings.
  cxa convert AGENT --to bundle FILE | --to package DIR
      Convert between the single-file bundle and the JSON-package directory format.
  cxa version

ENVIRONMENT:
  CXA_LLM_URL / CXA_LLM_MODEL / CXA_LLM_API_KEY   OpenAI-compatible chat endpoint (plain http) used by
                                                  generators and playbooks; without it playbooks replay examples.
  CXA_LOG=0                                       silence request logging in `serve`.
";

fn runtime() -> Runtime {
    Runtime::new(Arc::from(cx_engine::llm::from_env()))
}

fn slug(s: &str) -> String {
    let s: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "agent".into()
    } else {
        s
    }
}

/// Loads a bundle file or JSON-package directory (not normalised).
fn load_raw(path: &str) -> Result<Value, String> {
    let p = Path::new(path);
    if p.is_dir() {
        return import_cx_directory(p);
    }
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", path, e))?;
    let v = Value::parse(&text).map_err(|e| format!("{}: {}", path, e))?;
    // accept an :export response or LRO wrapper too
    if let Some(c) = v.path("response.agentContent").or_else(|| v.get("agentContent")) {
        return Ok(c.clone());
    }
    Ok(v)
}

fn load_agent(path: &str) -> Result<Agent, String> {
    let raw = load_raw(path)?;
    let name = format!("projects/local/locations/global/agents/{}", slug(raw.at("agent").str("displayName")));
    Agent::compile(&normalize_bundle(&raw, &name))
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn arg_values(args: &[String], flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            if let Some(v) = args.get(i + 1) {
                out.push(v.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(1)..];
    let result = match cmd {
        "serve" => serve(rest),
        "chat" => chat(rest),
        "test" => test(rest),
        "validate" => validate(rest),
        "convert" => convert(rest),
        "version" | "--version" | "-V" => {
            println!("cxa {}", env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        _ => {
            print!("{}", HELP);
            Ok(cmd == "help" || cmd == "--help" || cmd == "-h")
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn serve(args: &[String]) -> Result<bool, String> {
    let addr = arg_value(args, "--addr").unwrap_or_else(|| "127.0.0.1:8080".into());
    let data_file = if args.iter().any(|a| a == "--memory") {
        None
    } else {
        Some(PathBuf::from(arg_value(args, "--data").unwrap_or_else(|| "data/cxa-state.json".into())))
    };
    let seeds = arg_values(args, "--seed");
    if !seeds.is_empty() {
        // seed agents into project "demo" if not present
        let store = cx_server::store::Store::open(data_file.clone())?;
        {
            let mut d = store.data.lock().unwrap();
            if d.get("projects/demo").is_none() {
                d.put("projects/demo", obj! {"name" => "projects/demo", "displayName" => "Demo", "createTime" => cx_core::time::now_rfc3339()});
            }
            for s in &seeds {
                let raw = load_raw(s)?;
                let name = format!("projects/demo/locations/global/agents/{}", slug(raw.at("agent").str("displayName")));
                if d.get(&name).is_none() {
                    let mut n = normalize_bundle(&raw, &name);
                    let mut a = n.at("agent").clone();
                    a.set("createTime", cx_core::time::now_rfc3339());
                    n.set("agent", a);
                    cx_server::store::restore(&mut d, &name, &n);
                    eprintln!("seeded agent {}", name);
                }
            }
            store.persist(&d);
        }
        if data_file.is_none() {
            return Err("--seed with --memory is not supported; use --data".into());
        }
    }
    cx_server::run(cx_server::Config { addr, data_file, runtime: runtime(), api_key: arg_value(args, "--api-key") })?;
    Ok(true)
}

fn print_turn(resp: &Value, debug: bool) {
    let qr = resp.at("queryResult");
    for m in qr.arr("responseMessages") {
        if let Some(t) = m.get("text") {
            println!("agent> {}", t.arr("text").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "));
        } else if let Some(p) = m.get("payload") {
            println!("agent> [payload] {}", p);
        } else if m.get("endInteraction").is_some() {
            println!("       — conversation ended —");
        } else if let Some(tc) = m.get("toolCall") {
            println!("agent> [tool call] {} {}", tc.str("tool"), tc.at("inputParameters"));
        } else if m.get("liveAgentHandoff").is_some() {
            println!("agent> [live agent handoff]");
        }
    }
    let mt = qr.at("match").str("matchType");
    let mut info = mt.to_string();
    if let Some(i) = qr.get("intent") {
        info = format!("{} ({})", i.str("displayName"), qr.at("intentDetectionConfidence"));
    }
    let loc = if let Some(pb) = qr.get("currentPlaybook") {
        format!("playbook {}", pb.str("displayName"))
    } else {
        format!("{} › {}", qr.at("currentFlow").str("displayName"), qr.at("currentPage").str("displayName"))
    };
    println!("       [{} · {}]", info, loc);
    if debug {
        println!("{}", qr.at("diagnosticInfo").to_pretty());
    }
}

fn chat(args: &[String]) -> Result<bool, String> {
    let path = args.iter().find(|a| !a.starts_with("--") && !arg_values(args, "--say").contains(a)).ok_or("usage: cxa chat AGENT")?;
    let agent = load_agent(path)?;
    let rt = runtime();
    let mut st = SessionState::new();
    let session = format!("{}/sessions/cli-{}", agent.name, &cx_core::uuid::new_v4()[..8]);
    let debug = args.iter().any(|a| a == "--debug");
    let says = arg_values(args, "--say");
    let send = |st: &mut SessionState, text: &str| {
        let req = if let Some(ev) = text.strip_prefix(":event ") {
            obj! {"queryInput" => obj!{"event" => obj!{"event" => ev.trim()}}}
        } else {
            obj! {"queryInput" => obj!{"text" => obj!{"text" => text}}}
        };
        rt.detect_intent(&agent, &session, st, &req)
    };
    if !says.is_empty() {
        for s in says {
            println!("user > {}", s);
            print_turn(&send(&mut st, &s), debug);
        }
        return Ok(true);
    }
    println!("Chatting with '{}' (llm: {}). Commands: :params :reset :event NAME :debug :quit", agent.display_name(), rt.llm.name());
    let stdin = std::io::stdin();
    let mut debug = debug;
    loop {
        print!("user > ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let line = line.trim_end_matches(['\n', '\r']);
        match line.trim() {
            ":quit" | ":q" | ":exit" => break,
            ":reset" => {
                st = SessionState::new();
                println!("       (new session)");
            }
            ":params" => println!("{}", st.params.to_pretty()),
            ":debug" => {
                debug = !debug;
                println!("       debug {}", if debug { "on" } else { "off" });
            }
            _ => print_turn(&send(&mut st, line), debug),
        }
    }
    Ok(true)
}

fn test(args: &[String]) -> Result<bool, String> {
    let path = args.first().ok_or("usage: cxa test AGENT")?;
    let agent = load_agent(path)?;
    let rt = runtime();
    let mut failed = 0;
    let tcs = agent.test_cases();
    if tcs.is_empty() {
        println!("no test cases in {}", path);
        return Ok(true);
    }
    for tc in tcs {
        let r = cx_engine::testing::run_test_case(&rt, &agent, tc, "draft");
        let ok = r.str("testResult") == "PASSED";
        println!("{} {}", if ok { "PASS" } else { "FAIL" }, tc.str("displayName"));
        if !ok {
            failed += 1;
            for (i, t) in r.arr("conversationTurns").iter().enumerate() {
                for d in t.at("virtualAgentOutput").arr("differences") {
                    println!("     turn {}: {} {}", i + 1, d.str("type"), d.str("description"));
                }
            }
        }
    }
    println!("\n{} passed, {} failed", tcs.len() - failed, failed);
    Ok(failed == 0)
}

fn validate(args: &[String]) -> Result<bool, String> {
    let path = args.first().ok_or("usage: cxa validate AGENT")?;
    let agent = load_agent(path)?;
    let r = cx_engine::validate::validate(&agent);
    let print = |scope: &str, msgs: &[Value]| {
        for m in msgs {
            let names: Vec<&str> = m.arr("resourceNames").iter().map(|n| n.str("name")).collect();
            println!("{:<8} {:<24} {} [{}] {}", m.str("severity"), m.str("resourceType"), scope, names.join(", "), m.str("detail"));
        }
    };
    print("agent", r.arr("agentValidationMessages"));
    for f in r.arr("flowValidationResults") {
        print(&format!("flow '{}'", f.str("flowDisplayName")), f.arr("validationMessages"));
    }
    let s = r.at("summary");
    println!("\n{} errors, {} warnings, {} info", s["ERROR"].to_display(), s["WARNING"].to_display(), s["INFO"].to_display());
    Ok(s.f64_field("ERROR").unwrap_or(0.0) == 0.0)
}

fn convert(args: &[String]) -> Result<bool, String> {
    let path = args.first().ok_or("usage: cxa convert AGENT --to bundle FILE|package DIR")?;
    let to = arg_value(args, "--to").ok_or("missing --to bundle|package")?;
    let out = args.iter().skip_while(|a| *a != "--to").nth(2).ok_or("missing output path")?;
    let raw = load_raw(path)?;
    let name = format!("projects/local/locations/global/agents/{}", slug(raw.at("agent").str("displayName")));
    let b = normalize_bundle(&raw, &name);
    match to.as_str() {
        "bundle" => std::fs::write(out, b.to_pretty()).map_err(|e| e.to_string())?,
        "package" => export_cx_directory(&b, Path::new(out))?,
        other => return Err(format!("unknown target format '{}'", other)),
    }
    println!("wrote {}", out);
    Ok(true)
}
