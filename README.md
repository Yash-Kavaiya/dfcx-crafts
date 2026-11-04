# dfcx-crafts — Conversational Agents platform in Rust

A self-hostable, **multi-tenant** platform that reproduces Google Cloud
**Conversational Agents** (Dialogflow CX) in Rust: a REST API shaped like the
`v3` API, a web console at `/projects`, a conversation runtime with flows,
pages, forms, route groups, webhooks, generators and playbooks, and a CLI.

The whole workspace is built on the Rust standard library only — **zero
third-party crates**. JSON, regular expressions, HTTP server and client, YAML
(for OpenAPI tools) and NLU are implemented in-tree, so it builds offline
with `cargo build`.

```
cargo build --release
./target/release/cxa serve --seed examples/pizza-agent.json --seed examples/travel-playbook-agent.json
# console: http://127.0.0.1:8080/projects     API: http://127.0.0.1:8080/v3/projects
```

## A platform, not one agent

Resources follow the same hierarchy as the Google console and API:

```
projects/{project}
  locations/{location}                     global, us-central1, europe-west1, …
    agents/{agent}
      flows/{flow}
        pages/{page}
        transitionRouteGroups/{group}
        versions/{version}
      intents/{intent}            entityTypes/{entityType}       webhooks/{webhook}
      transitionRouteGroups/{g}   generators/{generator}         tools/{tool}
      playbooks/{playbook}/examples/{example}
      testCases/{testCase}/results/{result}
      environments/{env}[/sessions/{s}]    sessions/{s}[/entityTypes/{e}]
      changelogs/{c}              conversations/{c}
    securitySettings/{s}
```

You can create any number of projects and agents. Each agent keeps its own
sessions, conversation history, change history, versions and environments,
and every agent can be exported, imported into another project, or restored.

## Features

| Area | What works |
|---|---|
| **Console** | Projects and agents across locations, flow graph, page/form/route editors, intents with inline `[annotation](param)`, entity types, webhooks, route groups, generators, playbooks and examples, tools, test cases with diffs and coverage, environments and flow versions, validation, conversation and change history, agent settings, import/export/restore, simulator (environments, events, execution trace, "save as test case"), dark mode |
| **Flows & pages** | Start page, flow-level and page-level routes, intent and condition routes, the CX handler scope and evaluation order, route groups (flow and agent level), the flow stack with `END_FLOW` returning to the calling page, `END_SESSION`, `PREVIOUS_PAGE`, `CURRENT_PAGE`, `START_PAGE`, `END_FLOW_WITH_HUMAN_ESCALATION` (adds `liveAgentHandoff`), loop protection |
| **Forms** | Required/optional and list parameters, default values, initial prompts, reprompt handlers (`sys.no-match-N`), filling several parameters from one utterance, intent parameters pre-filling forms, `$page.params.status = "FINAL"` |
| **Events** | `sys.no-match-default`, `sys.no-match-N`, `sys.no-input-*`, `webhook.error` (handled in the scope where it was raised), custom events through `queryInput.event` |
| **Fulfillment** | Text variants, custom payloads, `outputAudioText`, `conversationSuccess`, `liveAgentHandoff`, `endInteraction`; parameter presets; conditional responses (if / elif / else, nested); generators; webhooks |
| **Expressions** | `$session.params.x`, `$page.params.*`, `$intent.params.x.original/.resolved`, `$flow.x`, `$request.user-utterance`, `$request.generative.*`; operators `= != < > <= >= :`, `AND OR NOT`, `: *`; 32 `$sys.func.*` functions (`ADD`, `IF`, `CONCATENATE`, `COUNT`, `JOIN`, `SPLIT`, `GET_FIELD`, …) |
| **NLU** | IDF-weighted intent classifier with typo correction and stemming, annotated training phrases, open-slot (`@sys.any`) alignment, per-flow thresholds, Default Negative Intent; map, list and regexp entity types with fuzzy matching, synonyms and excluded phrases; session entity types (override or supplement) |
| **System entities** | `sys.number(-integer)`, `cardinal`, `ordinal`, `number-sequence`, `date` (relative and absolute), `time`, `email`, `url`, `phone-number`, `zip-code`, `percentage`, `unit-currency`, `color`, and open types (`any`, `person`, `given-name`, `last-name`, `geo-city`, `geo-country`, `address`, `location`) |
| **Webhooks** | Standard `WebhookRequest`/`WebhookResponse` (messages with APPEND/REPLACE, session and form parameters, `targetPage`/`targetFlow`, payloads), flexible webhooks (templated body and JSON-path parameter mapping), headers, basic auth, timeouts, disable switch |
| **Generative** | Playbooks with goal, nested instructions, examples, referenced tools, playbooks and flows; OpenAPI tools (YAML or JSON schema, path/query/body parameters, API-key auth); client-side function tools (`toolCall` / `toolCallResult`); generators. Uses any OpenAI-compatible endpoint; **without an LLM, playbooks replay their examples deterministically** |
| **Lifecycle** | Flow versions (snapshot, load, compare), environments that pin versions, continuous tests, test cases (`run`, `batchRun`, `calculateCoverage`), validation, change history, conversation history, export/restore |
| **Agent files** | Import Google Conversational Agents exports directly: `.zip` / `.blob` archives (JSON package) or unzipped folders, through the console, `agents:import` / `:restore` (base64 `agentContent`) and the CLI; export as a single JSON bundle or a JSON-package `.zip` (`dataFormat: JSON_PACKAGE`). Built on an in-tree DEFLATE decoder and ZIP reader/writer |
| **Integrations** | `<cx-messenger>` embeddable chat widget (`/messenger.js`) with Dialogflow-Messenger-style rich content (chips, buttons, info cards, images), environment selection and API-key support; a hosted demo page (`/messenger/demo?agent=…`); an Integrations page in the console with copy-ready snippets |
| **API** | Generic CRUD for every collection with `pageSize`/`pageToken`, `updateMask`, `filter`; Google-style errors; optional API key; CORS |

## Runtime semantics (summary)

For each text input:

1. Intents are classified. The best match above the threshold that is **in
   scope** wins. Scope is the current page's routes and route groups plus the
   current flow's routes and route groups.
2. Otherwise, if a form parameter is being requested, the parameter is filled
   from the utterance (an in-scope intent wins only at confidence ≥ 0.75).
3. Otherwise, condition-only routes are checked. Flow-level condition routes
   are only checked on the start page.
4. Otherwise the turn raises `sys.no-match-N` and then `sys.no-match-default`.
   Handlers are searched in this order: parameter reprompt handlers, then the
   page, then the flow.

Entering a page runs, in order:

1. Apply form default values.
2. Run the entry fulfillment.
3. Evaluate condition routes.
4. Prompt for the next required parameter.

Fulfillment runs, in order:

1. Parameter presets.
2. Generators.
3. Messages.
4. Conditional cases.
5. The webhook.

## CLI

```
cxa serve [--addr 127.0.0.1:8080] [--data data/cxa-state.json | --memory] [--api-key KEY] [--seed AGENT.json ...]
cxa chat AGENT [--say "hi" --say "..."] [--debug]      # REPL: :params :reset :event NAME :debug :quit
cxa test AGENT                                         # runs test cases, exit code reflects result
cxa validate AGENT
cxa convert AGENT --to bundle out.json | --to package out-dir/ | --to zip out.zip
```

`AGENT` is a single-file bundle (see `examples/`), a Conversational Agents
export archive (`.zip` / `.blob`), or an unzipped **JSON package** directory
(`agent.json`, `flows/`, `intents/`, `entityTypes/`, …).

## Embedding the web messenger

```html
<script src="http://127.0.0.1:8080/messenger.js" defer></script>
<cx-messenger agent="projects/demo/locations/global/agents/rusty-pizza"
              chat-title="Rusty Pizza" language-code="en"></cx-messenger>
```

Optional attributes: `environment`, `welcome-event`, `welcome-text`,
`api-key`, `api-base`, `primary-color` and `expanded`. The widget emits
`cx-response-received` events with each `detectIntent` response.

To use an LLM for playbooks and generators, point these variables at any
OpenAI-compatible endpoint (Ollama, vLLM, LiteLLM, llama.cpp, …):

| Variable | Meaning |
|---|---|
| `CXA_LLM_URL` | Endpoint URL, e.g. `http://localhost:11434/v1` |
| `CXA_LLM_MODEL` | Model name |
| `CXA_LLM_API_KEY` | API key, if the endpoint needs one |

## API examples

```bash
curl -X POST localhost:8080/v3/projects -d '{"displayName":"Retail","projectId":"retail"}'
curl -X POST localhost:8080/v3/projects/retail/locations/global/agents -d '{"displayName":"Support"}'
curl -X POST localhost:8080/v3/projects/retail/locations/global/agents:import \
     -d "{\"agentId\":\"pizza\",\"agentContent\":$(cat examples/pizza-agent.json)}"
curl -X POST localhost:8080/v3/projects/retail/locations/global/agents/pizza/sessions/s1:detectIntent \
     -d '{"queryInput":{"text":{"text":"I want a large pizza"},"languageCode":"en"}}'
```

Custom methods:

| Resource | Methods |
|---|---|
| sessions | `:detectIntent`, `:matchIntent`, `:fulfillIntent` (also under `environments/{e}/sessions`) |
| agents | `:export`, `:restore`, `:validate`, `GET …/validationResult`, `agents:import`, `:saveTestCase` |
| flows | `:train`, `:validate`, `:export` |
| flow versions | `:load`, `:compareVersions` |
| test cases | `testCases/{t}:run`, `testCases:batchRun`, `testCases:calculateCoverage`, `testCases:batchDelete` |
| environments | `:deployFlow`, `:runContinuousTest` |
| intents, entity types | `:batchDelete` |

Helper endpoints: `GET /v3/systemEntities`, `GET /v3/systemFunctions` and `GET /healthz`.

## Workspace layout

| Crate | Purpose |
|---|---|
| `cx-core` | JSON value, parser and serializer (order-preserving); RFC 3339 time; UUID v4; resource names; base64; DEFLATE + ZIP |
| `cx-regex` | Backtracking regex VM with bit-state memoisation (linear time), classes, groups, lazy quantifiers, `(?i)` |
| `cx-nlu` | Tokenizer, stemmer, system and custom entity extraction, intent classifier |
| `cx-http` | HTTP/1.1 server (keep-alive, chunked) and client |
| `cx-engine` | Agent compiler, runtime, expression language, playbooks and tools, YAML reader, importer and exporter, test runner, validator |
| `cx-server` | Persistent store, v3 REST API, embedded console (`assets/`) |
| `cxa` | CLI binary |

## How this was reverse-engineered

The live console (`conversational-agents.cloud.google.com`) needs a Google
login, and the build environment's network policy blocks Google hosts. The
`npx rea-agents` web-bundle capture was not possible for the same reason.

The platform was therefore rebuilt from the public resource model and
semantics of the Dialogflow CX / Conversational Agents `v3` API: resource
names, field names, request and response shapes, handler scope, form and event
behaviour, and the JSON-package export layout. Exported agents and v3-shaped
clients should feel familiar.

**Known gaps:**

- No speech (STT/TTS) or telephony.
- No data-store (RAG) tools.
- Plain HTTP only for outbound calls, because there is no TLS library. Put a
  local TLS-terminating proxy in front of HTTPS webhooks.
- The NLU is a fast heuristic classifier, not Google's ML model.

## Development

```
cargo test             # unit and integration tests (engine conversations, HTTP platform lifecycle)
cargo build --release
```
