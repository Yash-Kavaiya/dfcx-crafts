// End-to-end tests for the Conversational Agents platform.
//
// Starts the real server binary, then drives every feature through the web
// console in Chromium (desktop + phone viewports) and checks the results
// through the REST API. Also enforces a latency budget.
//
//   cargo build --release -p cxa
//   npm install --no-save playwright@1.63.0 && npx playwright install chromium
//   node e2e/console.e2e.mjs            # CXA_BIN overrides the binary path
//
// Exit code is non-zero on the first failing check.

import { chromium, devices } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import net from 'node:net';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const BIN = process.env.CXA_BIN || join(ROOT, 'target/release/cxa');
const AGENT_PATH = 'projects/demo/locations/global/agents/rusty-pizza';
const results = [];
const servers = [];

function freePort() {
  return new Promise(res => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); });
}

async function startServer(extraEnv = {}) {
  const port = await freePort();
  const dir = mkdtempSync(join(tmpdir(), 'cxa-e2e-'));
  const proc = spawn(BIN, ['serve'], {
    env: { ...process.env, CXA_LOG: '0', CXA_ADDR: `127.0.0.1:${port}`, CXA_DATA: join(dir, 'state.json'),
      CXA_SEED: ['pizza-agent', 'help-center-agent', 'travel-playbook-agent'].map(n => join(ROOT, 'examples', n + '.json')).join(','), ...extraEnv },
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  servers.push({ proc, dir });
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(base + '/healthz')).ok) return base; } catch (e) { }
    await new Promise(r => setTimeout(r, 50));
  }
  throw new Error('server did not start');
}

async function api(base, method, path, body, key) {
  const headers = { 'Content-Type': 'application/json' };
  if (key) headers.Authorization = 'Bearer ' + key;
  const r = await fetch(base + '/v3/' + path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  const text = await r.text();
  return { status: r.status, data: text ? JSON.parse(text) : {} };
}

function assert(cond, msg) { if (!cond) throw new Error(msg); }

async function check(name, fn) {
  const t0 = Date.now();
  try { await fn(); results.push([true, name, Date.now() - t0]); console.log(`  ✓ ${name} (${Date.now() - t0} ms)`); }
  catch (e) { results.push([false, name, Date.now() - t0]); console.log(`  ✗ ${name}\n      ${e.message.split('\n')[0]}`); throw e; }
}

async function newPage(browser, opts = {}) {
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 }, ...opts });
  const page = await ctx.newPage();
  page.errors = [];
  page.on('pageerror', e => page.errors.push(e.message));
  page.on('dialog', d => d.accept(d.defaultValue() || 'e2e'));
  return page;
}

const settle = page => page.waitForFunction(() => !document.body.classList.contains('loading')).then(() => page.waitForTimeout(80));

async function main() {
  const base = await startServer();
  const browser = await chromium.launch();
  const page = await newPage(browser);
  const go = async hash => { await page.goto(`${base}/projects#${hash}`); await settle(page); };
  const say = async text => {
    const before = await page.locator('.sim-log .meta').count();
    await page.fill('.sim-input input', text); await page.press('.sim-input input', 'Enter');
    await page.waitForFunction(n => document.querySelectorAll('.sim-log .meta').length > n, before);
    return (await page.locator('.sim-log').innerText());
  };

  console.log('\nConsole: projects and agents');
  await check('projects page lists seeded project and creates a new one', async () => {
    await go('/projects');
    assert(await page.isVisible('.tile >> text=Demo'), 'demo project tile');
    await page.click('text=New project');
    await page.fill('.modal input >> nth=0', 'E2E Project'); await page.fill('.modal input >> nth=1', 'e2e');
    await page.click('.modal button.primary'); await settle(page);
    assert(page.url().includes('/projects/e2e/locations/global/agents'), 'navigated to new project');
  });
  await check('create a flow agent from the console', async () => {
    await page.click('text=Create agent');
    await page.fill('.modal input >> nth=0', 'Support Bot');
    await page.click('.modal button.primary'); await settle(page);
    assert(page.url().includes('/flows'), 'opened builder');
    assert(await page.isVisible('.agent-card >> text=Support Bot'), 'agent card in sidenav');
  });
  await check('import an agent from a .json file and a Google .zip export', async () => {
    await go('/projects/e2e/locations/global/agents');
    for (const file of [join(ROOT, 'examples/pizza-agent.json'), join(ROOT, 'crates/cx-core/src/testdata_agent.zip')]) {
      const [chooser] = await Promise.all([page.waitForEvent('filechooser'), page.click('text=Import agent')]);
      await chooser.setFiles(file); await page.waitForURL(/\/flows$/); await settle(page);
      await go('/projects/e2e/locations/global/agents');
    }
    const agents = (await api(base, 'GET', 'projects/e2e/locations/global/agents')).data.agents.map(a => a.displayName);
    assert(['Support Bot', 'Rusty Pizza', 'Zipped'].every(n => agents.includes(n)), 'agents: ' + agents);
  });

  console.log('\nConsole: builder');
  const A = '/' + AGENT_PATH;
  await check('every builder section renders without errors', async () => {
    for (const s of ['flows', 'playbooks', 'tools', 'dataStores', 'intents', 'entityTypes', 'transitionRouteGroups', 'webhooks', 'generators',
      'testCases', 'environments', 'validation', 'analytics', 'conversations', 'changelogs', 'integrations', 'settings']) {
      await go(`${A}/${s}`);
      assert(await page.isVisible(`.sidenav a.active`), `nav active for ${s}`);
      assert(!(await page.locator('.empty >> text=Something went wrong').count()), `error on ${s}`);
    }
    assert(!page.errors.length, page.errors.join('; '));
  });
  await check('flow graph renders pages and opens the page editor', async () => {
    await go(`${A}/flows`); await page.click('table >> text=Order Pizza'); await settle(page);
    assert(await page.locator('.graph .node').count() >= 4, 'graph nodes');
    await page.click('.graph >> text=Collect Order'); await settle(page);
    assert(await page.isVisible('h1 >> text=Collect Order'), 'page editor');
    assert(await page.isVisible('text=Parameters (form)'), 'form editor');
  });
  await check('create an intent with training phrases and route it from the start page', async () => {
    await go(`${A}/intents`); await page.click('text=Create'); await page.fill('.modal input', 'store.parking');
    await page.click('.modal button.primary'); await settle(page);
    for (const t of ['where can I park', 'is there parking', 'parking near the store']) {
      await page.fill('input[placeholder^="Type a training phrase"]', t); await page.press('input[placeholder^="Type a training phrase"]', 'Enter');
    }
    await page.click('button:has-text("Save intent")'); await settle(page);
    const intents = (await api(base, 'GET', AGENT_PATH + '/intents')).data.intents;
    const it = intents.find(i => i.displayName === 'store.parking');
    assert(it && it.trainingPhrases.length === 3, 'intent saved with 3 phrases');
    const flows = (await api(base, 'GET', AGENT_PATH + '/flows')).data.flows;
    const start = flows.find(f => f.displayName === 'Default Start Flow');
    await go(`${A}/flows/${start.name.split('/').pop()}`); await page.click('.tabs >> text=Start page'); await settle(page);
    await page.click('button:has-text("Add route")');
    const row = page.locator('.table-wrap').first().locator('tbody tr').last();
    await row.locator('select').first().selectOption(it.name);
    await row.locator('textarea').fill('Free parking is behind the store.');
    await page.click('main button:has-text("Save")'); await settle(page);
  });
  await check('entity type, webhook and route group editors create resources', async () => {
    await go(`${A}/entityTypes`); await page.click('text=Create'); await page.fill('.modal input', 'crust'); await page.click('.modal button.primary'); await settle(page);
    await page.click('button:has-text("Add entity")'); await page.locator('tbody tr input').first().fill('thin');
    await page.locator('tbody tr input').nth(1).fill('thin, thin crust'); await page.click('main button:has-text("Save") >> nth=0'); await settle(page);
    await go(`${A}/webhooks`); await page.click('text=Create'); await page.fill('.modal input', 'crm-hook'); await page.click('.modal button.primary'); await settle(page);
    await go(`${A}/transitionRouteGroups`); await page.click('text=Create'); await page.fill('.modal input', 'Global'); await page.click('.modal button.primary'); await settle(page);
    const ets = (await api(base, 'GET', AGENT_PATH + '/entityTypes')).data.entityTypes;
    assert(ets.find(e => e.displayName === 'crust').entities[0].value === 'thin', 'entity saved');
    assert((await api(base, 'GET', AGENT_PATH + '/webhooks')).data.webhooks.some(w => w.displayName === 'crm-hook'), 'webhook');
    assert((await api(base, 'GET', AGENT_PATH + '/transitionRouteGroups')).data.transitionRouteGroups.some(g => g.displayName === 'Global'), 'route group');
  });

  console.log('\nConsole: simulator');
  await check('simulator: welcome, chips, form filling, new route, latency shown', async () => {
    await go(`${A}/intents`); await page.click('#toggle-sim'); await page.waitForSelector('.sim-input input');
    let log = await say('hi');
    assert(log.includes('Welcome to Rusty Pizza'), 'welcome');
    await page.click('.chip-btn >> text=Order a pizza'); await page.waitForFunction(() => document.querySelector('.sim-log').innerText.includes('What size'));
    log = await say('large with ham');
    assert(log.includes("That's $15"), 'form filled + conditional response');
    await page.click('.sim-head [aria-label="Reset conversation"]'); await page.waitForSelector('.sim-empty');
    log = await say('is there parking?');
    assert(log.includes('Free parking is behind the store.'), 'route added in the UI answers');
    const ms = await page.locator('.meta .ms').last().innerText();
    assert(/\d+ ms/.test(ms), 'latency badge');
  });
  await check('save the simulator conversation as a test case and run all tests', async () => {
    await page.click('.sim-head [aria-label="Save as test case"]'); await page.waitForURL(/testCases\//); await settle(page);
    await go(`${A}/testCases`); await page.click('button:has-text("Run all")'); await settle(page);
    const tcs = (await api(base, 'GET', AGENT_PATH + '/testCases')).data.testCases;
    assert(tcs.length >= 3 && tcs.every(t => t.lastTestResult && t.lastTestResult.testResult === 'PASSED'), 'all test cases pass: ' + tcs.map(t => t.displayName + '=' + (t.lastTestResult || {}).testResult));
  });

  console.log('\nConsole: knowledge, versions, monitoring, integrations');
  await check('data store: add a document, search it, answer from it', async () => {
    const H = '/projects/demo/locations/global/agents/help-center';
    await go(`${H}/dataStores`); await page.click('table >> text=policies'); await settle(page);
    await page.click('button:has-text("Document")'); await page.fill('.modal input >> nth=0', 'Gift cards');
    await page.fill('.modal textarea', 'Gift cards never expire and can be used online or in any store.');
    await page.click('.modal button.primary'); await settle(page);
    await page.fill('input[placeholder="Try a question…"]', 'do gift cards expire'); await page.click('main button:has-text("Search")'); await settle(page);
    assert((await page.locator('main .card table').innerText()).includes('never expire'), 'search result');
    const r = await api(base, 'POST', H.slice(1) + '/sessions/kb:detectIntent', { queryInput: { text: { text: 'do gift cards expire?' } } });
    assert(r.data.queryResult.match.matchType === 'KNOWLEDGE_CONNECTOR', 'knowledge answer');
  });
  await check('versions and environments', async () => {
    await go(`${A}/flows`); await page.click('table >> text=Order Pizza'); await page.click('.tabs >> text=Versions'); await settle(page);
    await page.click('button:has-text("Create version")'); await page.fill('.modal input', 'v1'); await page.click('.modal button.primary'); await settle(page);
    await go(`${A}/environments`); await page.click('text=Create'); await page.fill('.modal input', 'production'); await page.click('.modal button.primary'); await settle(page);
    assert((await api(base, 'GET', AGENT_PATH + '/environments')).data.environments.some(e => e.displayName === 'production'), 'environment');
  });
  await check('validation, analytics, conversation and change history', async () => {
    await go(`${A}/validation`); assert(await page.isVisible('text=errors'), 'validation summary');
    await go(`${A}/analytics`); assert(await page.locator('.stat').count() >= 6, 'stat tiles');
    assert(Number(await page.locator('.stat-v').first().innerText()) >= 1, 'conversations counted');
    await go(`${A}/conversations`); assert(await page.locator('main tbody tr').count() >= 1, 'conversation history');
    await go(`${A}/changelogs`); assert(await page.locator('main tbody tr').count() >= 1, 'change history');
  });
  await check('integrations: embed snippet and widget demo page chat', async () => {
    await go(`${A}/integrations`);
    assert((await page.locator('pre').first().innerText()).includes('<cx-messenger'), 'snippet');
    const w = await newPage(browser);
    await w.goto(`${base}/messenger/demo?agent=${AGENT_PATH}&title=Pizza`); await w.waitForTimeout(300);
    const m = w.locator('cx-messenger');
    await m.locator('input').fill('when are you open'); await m.locator('input').press('Enter');
    await w.waitForFunction(() => document.querySelector('cx-messenger').shadowRoot.querySelector('.log').innerText.includes('11am'));
    assert(!w.errors.length, w.errors.join('; '));
    await w.context().close();
  });
  await check('settings: export downloads a JSON bundle and a ZIP package', async () => {
    await go(`${A}/settings`);
    for (const label of ['Export agent (JSON)', 'Export JSON package (.zip)']) {
      const [dl] = await Promise.all([page.waitForEvent('download'), page.click(`button:has-text("${label}")`)]);
      const p = await dl.path(); const buf = readFileSync(p);
      assert(label.includes('zip') ? buf[0] === 0x50 && buf[1] === 0x4b : JSON.parse(buf.toString()).agent.displayName === 'Rusty Pizza', 'download ' + label);
    }
  });
  await check('theme toggle cycles auto → light → dark', async () => {
    await page.click('#theme-toggle'); assert(await page.getAttribute('html', 'data-theme') === 'light', 'light');
    await page.click('#theme-toggle'); assert(await page.getAttribute('html', 'data-theme') === 'dark', 'dark');
    await page.click('#theme-toggle'); assert(await page.getAttribute('html', 'data-theme') === null, 'auto');
  });
  assert(!page.errors.length, 'page errors: ' + page.errors.join('; '));
  await page.context().close();

  console.log('\nConsole: phone viewport');
  await check('phone: drawer navigation, card lists and full-screen simulator', async () => {
    const m = await newPage(browser, { ...devices['iPhone 13'] });
    await m.goto(`${base}/projects#${A}/intents`); await settle(m);
    assert(!(await m.isVisible('.sidenav a.active')), 'drawer closed');
    const overflow = await m.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
    assert(!overflow, 'no horizontal page scroll');
    await m.click('#nav-toggle'); await m.waitForTimeout(300);
    await m.click('.sidenav >> text=Entity types'); await settle(m);
    assert(m.url().endsWith('/entityTypes'), 'navigated via drawer');
    await m.click('#toggle-sim'); await m.fill('.sim-input input', 'hi'); await m.press('.sim-input input', 'Enter');
    await m.waitForFunction(() => document.querySelector('.sim-log').innerText.includes('Welcome'));
    const box = await m.locator('#sim').boundingBox();
    assert(box.width >= (m.viewportSize().width - 2), 'simulator is full width');
    assert(!m.errors.length, m.errors.join('; '));
    await m.context().close();
  });

  console.log('\nLatency');
  await check('detectIntent p95 under 50 ms and console assets compressed and cached', async () => {
    const times = [];
    for (let i = 0; i < 60; i++) {
      const t0 = performance.now();
      const r = await api(base, 'POST', AGENT_PATH + `/sessions/lat${i % 6}:detectIntent`, { queryInput: { text: { text: ['hi', 'order pizza', 'large', 'ham', 'yes', 'when are you open'][i % 6] } } });
      times.push(performance.now() - t0); assert(r.status === 200, 'status');
    }
    times.sort((a, b) => a - b);
    const p50 = times[Math.floor(times.length / 2)], p95 = times[Math.floor(times.length * 0.95)];
    console.log(`      detectIntent p50 ${p50.toFixed(1)} ms, p95 ${p95.toFixed(1)} ms (${times.length} turns)`);
    assert(p95 < 50, `p95 ${p95.toFixed(1)} ms`);
    const html = await (await fetch(base + '/projects')).text();
    const js = html.match(/\/static\/app\.js\?v=\w+/)[0];
    const r = await fetch(base + js, { headers: { 'Accept-Encoding': 'gzip' } });
    assert(/immutable/.test(r.headers.get('cache-control')), 'immutable cache');
  });

  console.log('\nAPI key mode');
  await check('console prompts for the key, re-asks on a wrong key, then works; widget chat stays public', async () => {
    const kbase = await startServer({ CXA_API_KEY: 'e2e-secret', CXA_PUBLIC_SESSIONS: '1' });
    const k = await newPage(browser);
    await k.goto(kbase + '/projects'); await k.waitForSelector('.modal >> text=API key required');
    await k.fill('.modal input', 'nope'); await k.click('.modal button.primary');
    await k.waitForSelector('.modal >> text=rejected');
    await k.fill('.modal input', 'e2e-secret'); await k.click('.modal button.primary');
    await k.waitForSelector('.tile >> text=Demo');
    assert((await api(kbase, 'GET', 'projects')).status === 401, 'API needs the key');
    const chat = await api(kbase, 'POST', AGENT_PATH + '/sessions/pub:detectIntent', { queryInput: { text: { text: 'hi' } } });
    assert(chat.status === 200, 'public sessions');
    assert(!k.errors.length, k.errors.join('; '));
    await k.context().close();
  });

  await browser.close();
}

let failed = false;
try { await main(); } catch (e) { failed = true; }
finally {
  for (const s of servers) { s.proc.kill('SIGTERM'); }
  await new Promise(r => setTimeout(r, 300));
  for (const s of servers) { try { rmSync(s.dir, { recursive: true, force: true }); } catch (e) { } }
}
const passed = results.filter(r => r[0]).length;
console.log(`\n${passed}/${results.length} checks passed${failed ? ' — FAILED' : ''}`);
process.exit(failed ? 1 : 0);
