/* Conversational Agents console — dependency-free single-page app. */
'use strict';

// ---------- helpers ----------
const $ = (s, r = document) => r.querySelector(s);
function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  if (attrs) for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'class') el.className = v;
    else if (k === 'value') el.value = v;
    else if (k === 'checked') el.checked = !!v;
    else if (k === 'html') el.innerHTML = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const k of kids.flat(Infinity)) if (k != null && k !== false) el.append(k.nodeType ? k : document.createTextNode(String(k)));
  return el;
}
const idOf = n => (n || '').split('/').pop();
const parentOf = n => n.split('/').slice(0, -2).join('/');
const clone = v => JSON.parse(JSON.stringify(v ?? null));
const esc = s => String(s ?? '');
function toast(msg, err) {
  const t = $('#toast'); t.textContent = msg; t.className = 'toast' + (err ? ' err' : '');
  clearTimeout(toast.t); toast.t = setTimeout(() => t.classList.add('hidden'), err ? 6000 : 2500);
}
async function call(method, path, body, query) {
  const qs = query ? '?' + new URLSearchParams(query) : '';
  const r = await fetch('/v3/' + path + qs, { method, headers: { 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
  const text = await r.text();
  const data = text ? JSON.parse(text) : {};
  if (!r.ok) throw new Error((data.error && data.error.message) || r.statusText);
  return data;
}
const api = {
  get: (p, q) => call('GET', p, undefined, q),
  post: (p, b, q) => call('POST', p, b ?? {}, q),
  patch: (p, b, mask) => call('PATCH', p, b, mask ? { updateMask: mask } : undefined),
  del: p => call('DELETE', p),
  list: async (p, key) => (await api.get(p))[key || idOf(p)] || [],
};
async function guard(fn, okMsg) {
  try { const r = await fn(); if (okMsg) toast(okMsg); return r; } catch (e) { toast(e.message, true); throw e; }
}
function modal(title, body, actions) {
  return new Promise(resolve => {
    const root = $('#modal-root');
    const close = v => { root.innerHTML = ''; resolve(v); };
    const back = h('div', { class: 'modal-back', onclick: e => { if (e.target === back) close(null); } },
      h('div', { class: 'modal' }, h('h2', null, title), body,
        h('div', { class: 'row', style: 'justify-content:flex-end;margin-top:18px' },
          h('button', { class: 'btn ghost', onclick: () => close(null) }, 'Cancel'),
          ...(actions || [['OK', 'primary', () => true]]).map(([label, cls, fn]) =>
            h('button', { class: 'btn ' + cls, onclick: async () => { const v = await fn(); if (v !== undefined && v !== false) close(v); } }, label)))));
    root.append(back);
    const first = back.querySelector('input,textarea,select'); if (first) first.focus();
  });
}
function field(label, input) { return h('div', null, h('label', { class: 'f' }, label), input); }
function download(name, text) {
  const a = h('a', { href: URL.createObjectURL(new Blob([text], { type: 'application/json' })), download: name }); a.click();
}
function pickFile() {
  return new Promise(res => { const i = h('input', { type: 'file', accept: '.json,application/json' }); i.onchange = () => { const f = i.files[0]; if (!f) return res(null); const rd = new FileReader(); rd.onload = () => res(rd.result); rd.readAsText(f); }; i.click(); });
}
// Picks an agent file (.json bundle or Google .zip/.blob export) and returns
// the API's agentContent value: an object for JSON, base64 for archives.
function pickAgentContent() {
  return new Promise(res => {
    const i = h('input', { type: 'file', accept: '.json,.zip,.blob,application/json,application/zip' });
    i.onchange = () => {
      const f = i.files[0]; if (!f) return res(null);
      const rd = new FileReader();
      rd.onload = () => {
        const bytes = new Uint8Array(rd.result);
        if (bytes[0] === 0x50 && bytes[1] === 0x4b) {
          let bin = ''; for (let k = 0; k < bytes.length; k += 0x8000) bin += String.fromCharCode.apply(null, bytes.subarray(k, k + 0x8000));
          return res(btoa(bin));
        }
        try { res(JSON.parse(new TextDecoder().decode(bytes))); } catch (e) { toast('Not a JSON file or agent ZIP export', true); res(null); }
      };
      rd.readAsArrayBuffer(f);
    };
    i.click();
  });
}
function downloadBase64(name, b64, type) {
  const bin = atob(b64); const u = new Uint8Array(bin.length); for (let k = 0; k < bin.length; k++) u[k] = bin.charCodeAt(k);
  const a = h('a', { href: URL.createObjectURL(new Blob([u], { type })), download: name }); a.click();
}
function tabs(names, render) {
  const bar = h('div', { class: 'tabs' }); const body = h('div');
  const show = n => { [...bar.children].forEach(b => b.classList.toggle('on', b.dataset.n === n)); body.innerHTML = ''; body.append(render(n)); tabs.last = n; };
  names.forEach(n => bar.append(h('button', { 'data-n': n, onclick: () => show(n) }, n)));
  show(names.includes(tabs.last) ? tabs.last : names[0]);
  return h('div', null, bar, body);
}
function jsonEditor(obj, onSave) {
  const ta = h('textarea', { class: 'code', spellcheck: 'false' }); ta.value = JSON.stringify(obj, null, 2);
  return h('div', null, ta, h('div', { class: 'row', style: 'margin-top:8px' },
    h('button', { class: 'btn primary', onclick: async () => { let v; try { v = JSON.parse(ta.value); } catch (e) { return toast('Invalid JSON: ' + e.message, true); } await onSave(v); } }, 'Save JSON')));
}
// text messages <-> lines
function getTexts(f) { const m = (f && f.messages || []).find(m => m.text); return m ? (m.text.text || []).join('\n') : ''; }
function setTexts(f, s) {
  f = f || {}; const lines = s.split('\n').map(x => x.trim()).filter(Boolean);
  const others = (f.messages || []).filter(m => !m.text);
  f.messages = lines.length ? [{ text: { text: lines } }, ...others] : others;
  return f;
}

// ---------- state & routing ----------
const S = { sim: { open: false, session: null, env: '', log: [] }, cache: {} };
function go(path) { location.hash = '#' + path; }
function routeParts() { return (location.hash.slice(1) || '/projects').split('/').filter(Boolean); }

window.addEventListener('hashchange', render);
window.addEventListener('load', () => {
  if (!location.hash && location.pathname !== '/' && location.pathname.startsWith('/projects')) location.hash = '#' + location.pathname;
  $('#toggle-sim').onclick = () => { S.sim.open = !S.sim.open; renderSim(); };
  render();
});

function crumbs(items) {
  const c = $('#crumbs'); c.innerHTML = '';
  items.forEach(([label, href], i) => { if (i) c.append(h('span', { class: 'sep' }, '›')); c.append(href ? h('a', { href: '#' + href }, label) : h('span', null, label)); });
}

async function render() {
  const p = routeParts();
  const main = $('#main'); main.innerHTML = ''; $('#sidenav').innerHTML = '';
  try {
    if (p[0] !== 'projects') return go('/projects');
    if (p.length === 1) return await projectsPage(main);
    if (p.length === 2) return go(`/projects/${p[1]}/locations/global/agents`);
    if (p.length <= 5) return await agentsPage(main, p[1], p[3] || 'global');
    const agent = p.slice(0, 6).join('/');
    S.agent = agent;
    await agentShell(agent, p.slice(6), main);
  } catch (e) { main.append(h('div', { class: 'empty' }, 'Error: ' + e.message)); }
  renderSim();
}

// ---------- projects & agents ----------
async function projectsPage(main) {
  crumbs([['Projects']]); S.agent = null;
  const projects = await api.list('projects');
  main.append(h('div', { class: 'row between' }, h('h1', null, 'Projects'),
    h('button', { class: 'btn primary', onclick: newProject }, '+ New project')));
  if (!projects.length) main.append(h('div', { class: 'empty' }, 'No projects yet. Create one to start building agents.'));
  main.append(h('div', { class: 'grid' }, projects.map(pr => h('div', { class: 'tile', onclick: () => go(`/${pr.name}/locations/global/agents`) },
    h('div', { class: 't' }, pr.displayName), h('div', { class: 's' }, 'ID: ' + idOf(pr.name)), h('div', { class: 's' }, 'Created ' + (pr.createTime || '').slice(0, 10))))));
}
async function newProject() {
  const dn = h('input', { placeholder: 'My Project' }); const id = h('input', { placeholder: 'my-project (optional)' });
  const r = await modal('New project', h('div', null, field('Project name', dn), field('Project ID', id)), [['Create', 'primary', () =>
    guard(() => api.post('projects', { displayName: dn.value || 'My Project', projectId: id.value.trim() || undefined }))]]);
  if (r) go(`/${r.name}/locations/global/agents`);
}
const LOCATIONS = ['global', 'us-central1', 'us-east1', 'us-west1', 'europe-west1', 'europe-west2', 'asia-northeast1', 'asia-south1', 'australia-southeast1'];
async function agentsPage(main, project, loc) {
  const proj = await api.get(`projects/${project}`);
  crumbs([['Projects', '/projects'], [proj.displayName], ['Agents']]); S.agent = null;
  const parent = `projects/${project}/locations/${loc}`;
  const agents = await api.list(parent + '/agents');
  const locSel = h('select', { class: 'inline', onchange: e => go(`/projects/${project}/locations/${e.target.value}/agents`) }, LOCATIONS.map(l => h('option', { value: l, selected: l === loc }, l)));
  main.append(h('div', { class: 'row between' }, h('h1', null, 'Agents'),
    h('div', { class: 'row' }, h('span', { class: 'muted' }, 'Location'), locSel,
      h('button', { class: 'btn', title: 'Import a .json bundle or a Google Conversational Agents export (.zip / .blob)', onclick: () => importAgent(parent) }, 'Import agent'),
      h('button', { class: 'btn primary', onclick: () => newAgent(parent) }, '+ Create agent'),
      h('button', { class: 'btn danger', onclick: async () => { if (confirm(`Delete project ${proj.displayName} and all its agents?`)) { await guard(() => api.del(`projects/${project}`), 'Project deleted'); go('/projects'); } } }, 'Delete project'))));
  if (!agents.length) main.append(h('div', { class: 'empty' }, `No agents in ${loc}. Create one or import an exported agent.`));
  main.append(h('div', { class: 'grid' }, agents.map(a => h('div', { class: 'tile', onclick: () => go('/' + a.name + '/flows') },
    h('div', { class: 't' }, a.displayName),
    h('div', { class: 's' }, (a.description || '').slice(0, 120)),
    h('div', { class: 's' }, `${a.defaultLanguageCode} · ${a.timeZone} · ${a.startPlaybook ? 'playbook start' : 'flow start'}`)))));
}
async function newAgent(parent) {
  const dn = h('input', { placeholder: 'My agent' }); const lang = h('input', { value: 'en' }); const tz = h('input', { value: Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC' });
  const kind = h('select', null, h('option', { value: 'flow' }, 'Flow-based (deterministic)'), h('option', { value: 'playbook' }, 'Playbook (generative)'));
  const r = await modal('Create agent', h('div', null, field('Display name', dn), field('Default language', lang), field('Time zone', tz), field('Start resource', kind)), [['Create', 'primary', async () => {
    const a = await guard(() => api.post(parent + '/agents', { displayName: dn.value || 'My agent', defaultLanguageCode: lang.value, timeZone: tz.value }));
    if (kind.value === 'playbook') {
      const pb = await api.post(a.name + '/playbooks', { displayName: 'Default Generative Playbook', goal: 'Help the user with their request.', instruction: { steps: [{ text: 'Greet the user and ask how you can help.' }] } });
      await api.patch(a.name, { startPlaybook: pb.name }, 'startPlaybook');
    }
    return a;
  }]]);
  if (r) go('/' + r.name + '/flows');
}
async function importAgent(parent) {
  const content = await pickAgentContent(); if (!content) return;
  const a = await guard(() => api.post(parent + '/agents:import', { agentContent: content }), 'Agent imported');
  go('/' + a.name + '/flows');
}

// ---------- agent shell ----------
const SECTIONS = [
  ['Build', [['flows', 'Flows'], ['playbooks', 'Playbooks'], ['tools', 'Tools'], ['intents', 'Intents'], ['entityTypes', 'Entity types'], ['transitionRouteGroups', 'Route groups'], ['webhooks', 'Webhooks'], ['generators', 'Generators']]],
  ['Test & deploy', [['testCases', 'Test cases'], ['environments', 'Environments'], ['validation', 'Validation']]],
  ['Monitor', [['conversations', 'Conversation history'], ['changelogs', 'Change history']]],
  ['Manage', [['settings', 'Agent settings']]],
];
async function agentShell(agent, rest, main) {
  const a = await api.get(agent); S.agentRes = a;
  const project = agent.split('/')[1];
  const section = rest[0] || 'flows';
  crumbs([['Projects', '/projects'], [project, `/projects/${project}/locations/${agent.split('/')[3]}/agents`], [a.displayName, `/${agent}/flows`], [SECTIONS.flatMap(s => s[1]).find(x => x[0] === section)?.[1] || section]]);
  const nav = $('#sidenav');
  for (const [group, items] of SECTIONS) {
    nav.append(h('div', { class: 'group' }, group));
    for (const [k, label] of items) nav.append(h('a', { href: `#/${agent}/${k}`, class: k === section ? 'active' : '' }, h('span', { class: 'lbl' }, label)));
  }
  const ctx = new Ctx(agent);
  const fn = {
    flows: flowsSection, intents: intentsSection, entityTypes: entitiesSection, webhooks: webhooksSection,
    transitionRouteGroups: groupsSection, generators: generatorsSection, playbooks: playbooksSection, tools: toolsSection,
    testCases: testsSection, environments: envSection, validation: validationSection, conversations: convSection,
    changelogs: changelogSection, settings: settingsSection,
  }[section];
  if (!fn) return main.append(h('div', { class: 'empty' }, 'Unknown section'));
  await fn(ctx, rest.slice(1), main);
}
class Ctx {
  constructor(agent) { this.agent = agent; this.c = {}; }
  async list(coll, parent) { const p = (parent || this.agent) + '/' + coll; if (!this.c[p]) this.c[p] = await api.list(p); return this.c[p]; }
  async pagesOf(flow) { return this.list('pages', flow); }
}
function listPage(main, title, items, cols, onOpen, onNew, extra) {
  main.append(h('div', { class: 'row between' }, h('h1', null, title), h('div', { class: 'row' }, extra || null, onNew ? h('button', { class: 'btn primary', onclick: onNew }, '+ Create') : null)));
  if (!items.length) return main.append(h('div', { class: 'empty' }, `No ${title.toLowerCase()} yet.`));
  main.append(h('table', null, h('tr', null, cols.map(c => h('th', null, c[0]))),
    items.map(it => h('tr', { class: 'click', onclick: () => onOpen(it) }, cols.map(c => h('td', null, c[1](it)))))));
}
async function createNamed(collPath, title, extraFields, build) {
  const dn = h('input', { placeholder: 'Display name' });
  const r = await modal('New ' + title, h('div', null, field('Display name', dn), extraFields || null), [['Create', 'primary', () => {
    if (!dn.value.trim()) return toast('Display name is required', true);
    return guard(() => api.post(collPath, Object.assign({ displayName: dn.value.trim() }, build ? build() : {})));
  }]]);
  return r;
}
function deleteBtn(name, after) {
  return h('button', { class: 'btn danger', onclick: async () => { if (!confirm('Delete this resource?')) return; await guard(() => api.del(name), 'Deleted'); go(after); } }, 'Delete');
}
function header(title, sub, ...buttons) {
  return h('div', { class: 'row between' }, h('div', null, h('h1', { style: 'margin-bottom:2px' }, title), sub ? h('div', { class: 'muted', style: 'margin-bottom:12px' }, sub) : null), h('div', { class: 'row' }, buttons));
}

// ---------- shared editors ----------
const SPECIAL = ['END_SESSION', 'END_FLOW', 'PREVIOUS_PAGE', 'CURRENT_PAGE', 'START_PAGE', 'END_FLOW_WITH_HUMAN_ESCALATION'];
async function targetOptions(ctx, flow) {
  const flows = await ctx.list('flows'); const pbs = await ctx.list('playbooks');
  const pages = flow ? await ctx.pagesOf(flow) : [];
  const opts = [['', '— none —']];
  if (flow) for (const s of SPECIAL) opts.push(['page:' + flow + '/pages/' + s, s]);
  else for (const s of SPECIAL) opts.push(['page:' + s, s]);
  for (const p of pages) opts.push(['page:' + p.name, 'Page: ' + p.displayName]);
  for (const f of flows) opts.push(['flow:' + f.name, 'Flow: ' + f.displayName]);
  for (const p of pbs) opts.push(['pb:' + p.name, 'Playbook: ' + p.displayName]);
  return opts;
}
function targetValue(r) { return r.targetPage ? 'page:' + r.targetPage : r.targetFlow ? 'flow:' + r.targetFlow : r.targetPlaybook ? 'pb:' + r.targetPlaybook : ''; }
function applyTarget(r, v) {
  delete r.targetPage; delete r.targetFlow; delete r.targetPlaybook;
  if (v.startsWith('page:')) r.targetPage = v.slice(5); else if (v.startsWith('flow:')) r.targetFlow = v.slice(5); else if (v.startsWith('pb:')) r.targetPlaybook = v.slice(3);
}
function sel(opts, value, onchange) {
  const s = h('select', { onchange: e => onchange(e.target.value) }, opts.map(([v, l]) => h('option', { value: v, selected: v === value }, l)));
  if (value && !opts.some(o => o[0] === value)) s.prepend(h('option', { value, selected: true }, idOf(value.replace(/^\w+:/, '')) + ' (unresolved)'));
  return s;
}
async function routesEditor(ctx, routes, flow, opts = {}) {
  const intents = await ctx.list('intents');
  const tOpts = await targetOptions(ctx, flow);
  const iOpts = [['', '— none —'], ...intents.map(i => [i.name, i.displayName])];
  const tbody = h('tbody');
  const draw = () => {
    tbody.innerHTML = '';
    routes.forEach((r, i) => {
      r.triggerFulfillment = r.triggerFulfillment || {};
      const ta = h('textarea', { rows: 2, placeholder: 'Agent says (one variant per line)', oninput: e => setTexts(r.triggerFulfillment, e.target.value) }); ta.value = getTexts(r.triggerFulfillment);
      tbody.append(h('tr', null,
        h('td', null, sel(iOpts, r.intent || '', v => { if (v) r.intent = v; else delete r.intent; })),
        h('td', null, h('input', { value: r.condition || '', placeholder: 'e.g. $page.params.status = "FINAL"', oninput: e => { r.condition = e.target.value; if (!r.condition) delete r.condition; } })),
        h('td', null, ta, h('input', { style: 'margin-top:4px', value: r.triggerFulfillment.webhook ? idOf(r.triggerFulfillment.webhook) : '', placeholder: 'webhook (optional)', title: 'Webhook id or display name', onchange: async e => { const w = (await ctx.list('webhooks')).find(x => x.displayName === e.target.value || idOf(x.name) === e.target.value); if (e.target.value && !w) toast('Unknown webhook', true); if (w) r.triggerFulfillment.webhook = w.name; else delete r.triggerFulfillment.webhook; } })),
        h('td', null, sel(tOpts, targetValue(r), v => applyTarget(r, v))),
        h('td', null, h('button', { class: 'btn small danger', onclick: () => { routes.splice(i, 1); draw(); } }, '✕'))));
    });
  };
  draw();
  return h('div', null, h('table', null, h('thead', null, h('tr', null, h('th', { style: 'width:18%' }, 'Intent'), h('th', { style: 'width:24%' }, 'Condition'), h('th', null, 'Fulfillment'), h('th', { style: 'width:20%' }, 'Transition'), h('th', null, ''))), tbody),
    h('button', { class: 'btn small', style: 'margin-top:6px', onclick: () => { routes.push({ condition: opts.defaultCondition || '' }); draw(); } }, '+ Add route'));
}
async function handlersEditor(ctx, handlers, flow) {
  const tOpts = await targetOptions(ctx, flow);
  const events = ['sys.no-match-default', 'sys.no-input-default', 'sys.no-match-1', 'sys.no-match-2', 'sys.no-match-3', 'sys.no-input-1', 'webhook.error', 'sys.long-utterance', 'flow.failed'];
  const tbody = h('tbody');
  const draw = () => {
    tbody.innerHTML = '';
    handlers.forEach((r, i) => {
      r.triggerFulfillment = r.triggerFulfillment || {};
      const ta = h('textarea', { rows: 2, oninput: e => setTexts(r.triggerFulfillment, e.target.value) }); ta.value = getTexts(r.triggerFulfillment);
      const dl = h('datalist', { id: 'ev' + i }, events.map(e => h('option', { value: e })));
      tbody.append(h('tr', null, h('td', null, h('input', { value: r.event || '', list: 'ev' + i, oninput: e => r.event = e.target.value }), dl),
        h('td', null, ta), h('td', null, sel(tOpts, targetValue(r), v => applyTarget(r, v))),
        h('td', null, h('button', { class: 'btn small danger', onclick: () => { handlers.splice(i, 1); draw(); } }, '✕'))));
    });
  };
  draw();
  return h('div', null, h('table', null, h('thead', null, h('tr', null, h('th', { style: 'width:22%' }, 'Event'), h('th', null, 'Fulfillment'), h('th', { style: 'width:22%' }, 'Transition'), h('th', null, ''))), tbody),
    h('button', { class: 'btn small', style: 'margin-top:6px', onclick: () => { handlers.push({ event: 'sys.no-match-default' }); draw(); } }, '+ Add event handler'));
}
async function groupsPicker(ctx, holder, flow) {
  const groups = [...(await ctx.list('transitionRouteGroups', flow)), ...(await ctx.list('transitionRouteGroups'))];
  holder.transitionRouteGroups = holder.transitionRouteGroups || [];
  if (!groups.length) return h('div', { class: 'muted' }, 'No route groups defined.');
  return h('div', { class: 'row' }, groups.map(g => h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: holder.transitionRouteGroups.includes(g.name), onchange: e => {
    holder.transitionRouteGroups = holder.transitionRouteGroups.filter(x => x !== g.name); if (e.target.checked) holder.transitionRouteGroups.push(g.name);
  } }), g.displayName)));
}
async function entityOptions(ctx) {
  const ets = await ctx.list('entityTypes');
  const sys = S.sysEntities || (S.sysEntities = (await api.get('systemEntities')).systemEntities);
  return [...ets.map(e => [e.name, '@' + e.displayName]), ...sys.map(s => ['projects/-/locations/-/agents/-/entityTypes/' + s.name.slice(1), s.name])];
}

// ---------- flows & pages ----------
async function flowsSection(ctx, rest, main) {
  if (rest[0] && rest[1] === 'pages' && rest[2]) return pageEditor(ctx, `${ctx.agent}/flows/${rest[0]}`, `${ctx.agent}/flows/${rest[0]}/pages/${rest[2]}`, main);
  if (rest[0]) return flowDetail(ctx, `${ctx.agent}/flows/${rest[0]}`, main);
  const flows = await ctx.list('flows');
  listPage(main, 'Flows', flows, [['Name', f => h('b', null, f.displayName)], ['Description', f => f.description || ''], ['Start', f => S.agentRes.startFlow === f.name ? h('span', { class: 'chip blue' }, 'start flow') : '']],
    f => go('/' + f.name), async () => { const r = await createNamed(ctx.agent + '/flows', 'flow'); if (r) go('/' + r.name); });
}
async function flowDetail(ctx, flowName, main) {
  const flow = await api.get(flowName);
  const pages = await ctx.pagesOf(flowName);
  main.append(header(flow.displayName, flow.description || 'Flow',
    h('button', { class: 'btn', onclick: async () => { const r = await createNamed(flowName + '/pages', 'page'); if (r) go('/' + r.name); } }, '+ Page'),
    h('button', { class: 'btn', onclick: async () => { await guard(() => api.post(flowName + ':train'), 'Flow trained'); } }, 'Train'),
    deleteBtn(flowName, `/${ctx.agent}/flows`)));
  main.append(tabs(['Graph', 'Start page', 'Pages', 'Route groups', 'Settings', 'Versions', 'JSON'], t => {
    const box = h('div');
    (async () => {
      if (t === 'Graph') box.append(await flowGraph(ctx, flow, pages));
      if (t === 'Pages') box.append(pages.length ? h('table', null, h('tr', null, h('th', null, 'Page'), h('th', null, 'Form parameters'), h('th', null, 'Routes')),
        pages.map(p => h('tr', { class: 'click', onclick: () => go('/' + p.name) }, h('td', null, h('b', null, p.displayName)), h('td', null, ((p.form || {}).parameters || []).map(x => h('span', { class: 'chip' }, x.displayName))), h('td', null, (p.transitionRoutes || []).length)))) : h('div', { class: 'empty' }, 'No pages yet.'));
      if (t === 'Start page') {
        const f = clone(flow); f.transitionRoutes = f.transitionRoutes || []; f.eventHandlers = f.eventHandlers || [];
        box.append(h('h2', null, 'Routes'), await routesEditor(ctx, f.transitionRoutes, flowName),
          h('h2', null, 'Route groups'), await groupsPicker(ctx, f, flowName),
          h('h2', null, 'Event handlers'), await handlersEditor(ctx, f.eventHandlers, flowName),
          h('div', { style: 'margin-top:14px' }, h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(flowName, { transitionRoutes: f.transitionRoutes, eventHandlers: f.eventHandlers, transitionRouteGroups: f.transitionRouteGroups }, 'transitionRoutes,eventHandlers,transitionRouteGroups'), 'Saved') }, 'Save')));
      }
      if (t === 'Route groups') {
        const gs = await ctx.list('transitionRouteGroups', flowName);
        box.append(h('div', { class: 'row', style: 'margin-bottom:8px' }, h('button', { class: 'btn', onclick: async () => { const r = await createNamed(flowName + '/transitionRouteGroups', 'route group', null, () => ({ transitionRoutes: [] })); if (r) go(`/${ctx.agent}/transitionRouteGroups/${encodeURIComponent(r.name)}`); } }, '+ Flow route group')));
        box.append(gs.length ? h('table', null, gs.map(g => h('tr', { class: 'click', onclick: () => go(`/${ctx.agent}/transitionRouteGroups/${encodeURIComponent(g.name)}`) }, h('td', null, g.displayName), h('td', null, (g.transitionRoutes || []).length + ' routes')))) : h('div', { class: 'empty' }, 'No flow-level route groups.'));
      }
      if (t === 'Settings') {
        const dn = h('input', { value: flow.displayName }); const desc = h('textarea', { rows: 2 }); desc.value = flow.description || '';
        const th = h('input', { type: 'number', step: '0.05', min: 0, max: 1, value: (flow.nluSettings || {}).classificationThreshold ?? 0.3 });
        box.append(h('div', { class: 'card' }, field('Display name', dn), field('Description', desc), field('NLU classification threshold', th),
          h('div', { style: 'margin-top:12px' }, h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(flowName, { displayName: dn.value, description: desc.value, nluSettings: Object.assign({}, flow.nluSettings, { classificationThreshold: +th.value }) }, 'displayName,description,nluSettings'), 'Saved') }, 'Save'))));
      }
      if (t === 'Versions') {
        const vs = await api.list(flowName + '/versions');
        box.append(h('div', { class: 'row', style: 'margin-bottom:8px' }, h('button', { class: 'btn primary', onclick: async () => { const r = await createNamed(flowName + '/versions', 'version'); if (r) render(); } }, '+ Create version')));
        box.append(vs.length ? h('table', null, h('tr', null, h('th', null, 'Version'), h('th', null, 'Created'), h('th', null, 'Description'), h('th', null, '')),
          vs.map(v => h('tr', null, h('td', null, v.displayName + ' ', h('span', { class: 'muted' }, '#' + idOf(v.name).slice(0, 8))), h('td', null, (v.createTime || '').replace('T', ' ').slice(0, 19)), h('td', null, v.description || ''),
            h('td', null, h('button', { class: 'btn small', onclick: async () => { if (confirm('Load this version into the draft flow? Unsaved draft changes are replaced.')) { await guard(() => api.post(v.name + ':load'), 'Version loaded'); render(); } } }, 'Load to draft'), ' ',
              h('button', { class: 'btn small danger', onclick: async () => { await guard(() => api.del(v.name), 'Deleted'); render(); } }, 'Delete'))))) : h('div', { class: 'empty' }, 'No versions. Versions snapshot the flow so environments can serve them.'));
      }
      if (t === 'JSON') box.append(jsonEditor(flow, v => guard(() => api.patch(flowName, v), 'Saved')));
    })();
    return box;
  }));
}
async function flowGraph(ctx, flow, pages) {
  const flows = await ctx.list('flows');
  const nodes = new Map(); const edges = [];
  const add = (id, label, kind, href) => { if (!nodes.has(id)) nodes.set(id, { id, label, kind, href }); return id; };
  add('START', 'Start Page', 'start', null);
  pages.forEach(p => add(p.name, p.displayName, 'page', '/' + p.name));
  const intents = await ctx.list('intents');
  const iname = n => (intents.find(i => i.name === n) || {}).displayName || idOf(n);
  const edgeFrom = (from, holder) => {
    for (const r of [...(holder.transitionRoutes || []), ...(holder.eventHandlers || [])]) {
      let to = null;
      if (r.targetPage) { const id = idOf(r.targetPage); to = SPECIAL.includes(id) ? add('S:' + id, id, 'special') : (nodes.has(r.targetPage) ? r.targetPage : add(r.targetPage, idOf(r.targetPage), 'special')); if (id === 'START_PAGE') to = 'START'; }
      else if (r.targetFlow) { const f = flows.find(x => x.name === r.targetFlow); to = add('F:' + r.targetFlow, '⇢ ' + (f ? f.displayName : idOf(r.targetFlow)), 'flowref', f ? '/' + f.name : null); }
      else if (r.targetPlaybook) to = add('P:' + r.targetPlaybook, '✦ playbook', 'flowref', null);
      if (to) edges.push({ from, to, label: r.intent ? iname(r.intent) : r.event ? r.event : (r.condition || '').slice(0, 26) });
    }
  };
  edgeFrom('START', flow); pages.forEach(p => edgeFrom(p.name, p));
  // layered layout by BFS
  const layer = new Map([['START', 0]]); const q = ['START'];
  while (q.length) { const n = q.shift(); for (const e of edges.filter(e => e.from === n)) if (!layer.has(e.to)) { layer.set(e.to, layer.get(n) + 1); q.push(e.to); } }
  let maxL = Math.max(0, ...layer.values());
  for (const n of nodes.keys()) if (!layer.has(n)) layer.set(n, ++maxL);
  const cols = {}; for (const [n, l] of layer) (cols[l] = cols[l] || []).push(n);
  const W = 170, H = 40, GX = 90, GY = 34; const pos = {};
  Object.entries(cols).forEach(([l, ns]) => ns.forEach((n, i) => pos[n] = { x: 20 + l * (W + GX), y: 20 + i * (H + GY) }));
  const width = 40 + (Math.max(...Object.keys(cols).map(Number)) + 1) * (W + GX);
  const height = 40 + Math.max(...Object.values(cols).map(c => c.length)) * (H + GY);
  const NS = 'http://www.w3.org/2000/svg';
  const s = (t, a, ...k) => { const e = document.createElementNS(NS, t); for (const [x, y] of Object.entries(a || {})) e.setAttribute(x, y); k.forEach(c => e.append(c)); return e; };
  const svg = s('svg', { width, height, viewBox: `0 0 ${width} ${height}` });
  svg.append(s('defs', {}, s('marker', { id: 'arr', viewBox: '0 0 10 10', refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' }, s('path', { d: 'M0,0 L10,5 L0,10 z', fill: 'currentColor', class: 'edge' }))));
  edges.forEach((e, i) => {
    const a = pos[e.from], b = pos[e.to]; if (!a || !b) return;
    const self = e.from === e.to;
    const x1 = a.x + W, y1 = a.y + H / 2, x2 = b.x, y2 = b.y + H / 2;
    const d = self ? `M${a.x + W - 30},${a.y} C${a.x + W},${a.y - 40} ${a.x + W + 30},${a.y} ${a.x + W},${a.y + 10}` : (x2 > x1 ? `M${x1},${y1} C${x1 + 45},${y1} ${x2 - 45},${y2} ${x2},${y2}` : `M${a.x + W / 2},${a.y + H} C${a.x + W / 2},${a.y + H + 60} ${b.x + W / 2},${b.y + H + 60} ${b.x + W / 2},${b.y + H}`);
    svg.append(s('path', { d, class: 'edge', 'marker-end': 'url(#arr)' }));
    const back = !self && x2 <= x1;
    const lx = self ? a.x + W : back ? (a.x + b.x + W) / 2 : (x1 + x2) / 2;
    const ly = self ? a.y - 22 : back ? Math.max(a.y, b.y) + H + 42 : (y1 + y2) / 2 - 4 + (i % 2) * 10;
    svg.append(s('text', { x: lx, y: ly, class: 'edgelabel', 'text-anchor': 'middle' }, document.createTextNode((e.label || '').slice(0, 30))));
  });
  for (const n of nodes.values()) {
    const p = pos[n.id];
    const g = s('g', { class: 'node ' + n.kind, transform: `translate(${p.x},${p.y})` }, s('rect', { width: W, height: H, rx: 8 }), s('text', { x: W / 2, y: H / 2 + 4, 'text-anchor': 'middle' }, document.createTextNode(n.label.length > 22 ? n.label.slice(0, 21) + '…' : n.label)));
    if (n.href) g.addEventListener('click', () => go(n.href));
    if (n.id === 'START') g.addEventListener('click', () => { tabs.last = 'Start page'; render(); });
    svg.append(g);
  }
  return h('div', null, h('div', { class: 'muted', style: 'margin-bottom:6px' }, 'Click a page to edit it. Edges show intent / event / condition routes.'), h('div', { class: 'graph' }, svg));
}
async function pageEditor(ctx, flowName, pageName, main) {
  const page = await api.get(pageName); const p = clone(page);
  p.form = p.form || { parameters: [] }; p.form.parameters = p.form.parameters || [];
  p.transitionRoutes = p.transitionRoutes || []; p.eventHandlers = p.eventHandlers || []; p.entryFulfillment = p.entryFulfillment || {};
  const flow = await api.get(flowName);
  main.append(header(page.displayName, h('span', null, 'Page in flow ', h('a', { href: '#/' + flowName }, flow.displayName)),
    h('button', { class: 'btn primary', onclick: save }, 'Save'), deleteBtn(pageName, '/' + flowName)));
  async function save() { await guard(() => api.patch(pageName, p), 'Page saved'); }
  const entOpts = await entityOptions(ctx);
  main.append(tabs(['Page', 'JSON'], t => {
    if (t === 'JSON') return jsonEditor(page, v => guard(() => api.patch(pageName, v), 'Saved'));
    const box = h('div');
    (async () => {
      const dn = h('input', { value: p.displayName, oninput: e => p.displayName = e.target.value });
      const entry = h('textarea', { rows: 3, placeholder: 'What the agent says when the page is entered (one variant per line). Use $session.params.x for parameters.', oninput: e => setTexts(p.entryFulfillment, e.target.value) }); entry.value = getTexts(p.entryFulfillment);
      const webhooks = await ctx.list('webhooks');
      const wh = sel([['', '— no webhook —'], ...webhooks.map(w => [w.name, w.displayName])], p.entryFulfillment.webhook || '', v => { if (v) p.entryFulfillment.webhook = v; else delete p.entryFulfillment.webhook; });
      const tag = h('input', { value: p.entryFulfillment.tag || '', placeholder: 'tag', oninput: e => p.entryFulfillment.tag = e.target.value });
      box.append(h('div', { class: 'card' }, field('Display name', dn), field('Entry fulfillment', entry), h('div', { class: 'row', style: 'margin-top:6px' }, h('div', { style: 'flex:1' }, wh), h('div', { style: 'flex:1' }, tag))));
      // form
      const ptb = h('tbody');
      const drawParams = () => {
        ptb.innerHTML = '';
        p.form.parameters.forEach((fp, i) => {
          fp.fillBehavior = fp.fillBehavior || {}; fp.fillBehavior.initialPromptFulfillment = fp.fillBehavior.initialPromptFulfillment || {};
          const prompt = h('textarea', { rows: 2, oninput: e => setTexts(fp.fillBehavior.initialPromptFulfillment, e.target.value) }); prompt.value = getTexts(fp.fillBehavior.initialPromptFulfillment);
          ptb.append(h('tr', null,
            h('td', null, h('input', { value: fp.displayName || '', oninput: e => fp.displayName = e.target.value })),
            h('td', null, sel(entOpts, fp.entityType || '', v => fp.entityType = v)),
            h('td', null, h('input', { type: 'checkbox', checked: fp.required, onchange: e => fp.required = e.target.checked })),
            h('td', null, h('input', { type: 'checkbox', checked: fp.isList, onchange: e => fp.isList = e.target.checked })),
            h('td', null, h('input', { type: 'checkbox', checked: fp.redact, onchange: e => fp.redact = e.target.checked })),
            h('td', null, prompt),
            h('td', null, h('button', { class: 'btn small danger', onclick: () => { p.form.parameters.splice(i, 1); drawParams(); } }, '✕'))));
        });
      };
      drawParams();
      box.append(h('h2', null, 'Parameters (form)'), h('div', { class: 'muted', style: 'margin-bottom:6px' }, 'Required parameters are prompted in order. When all are filled, $page.params.status = "FINAL".'),
        h('table', null, h('thead', null, h('tr', null, h('th', null, 'Name'), h('th', null, 'Entity type'), h('th', null, 'Required'), h('th', null, 'List'), h('th', null, 'Redact'), h('th', null, 'Initial prompt'), h('th', null, ''))), ptb),
        h('button', { class: 'btn small', style: 'margin-top:6px', onclick: () => { p.form.parameters.push({ displayName: 'param' + (p.form.parameters.length + 1), entityType: 'projects/-/locations/-/agents/-/entityTypes/sys.any', required: true }); drawParams(); } }, '+ Add parameter'));
      box.append(h('h2', null, 'Routes'), await routesEditor(ctx, p.transitionRoutes, flowName, { defaultCondition: '$page.params.status = "FINAL"' }));
      box.append(h('h2', null, 'Route groups'), await groupsPicker(ctx, p, flowName));
      box.append(h('h2', null, 'Event handlers'), await handlersEditor(ctx, p.eventHandlers, flowName));
      box.append(h('div', { style: 'margin-top:16px' }, h('button', { class: 'btn primary', onclick: save }, 'Save page')));
    })();
    return box;
  }));
}

// ---------- intents ----------
function partsToText(parts) { return (parts || []).map(p => p.parameterId ? `[${p.text}](${p.parameterId})` : p.text).join(''); }
function textToParts(s) {
  const parts = []; const re = /\[([^\]]+)\]\(([^)]+)\)/g; let last = 0, m;
  while ((m = re.exec(s))) { if (m.index > last) parts.push({ text: s.slice(last, m.index) }); parts.push({ text: m[1], parameterId: m[2] }); last = re.lastIndex; }
  if (last < s.length) parts.push({ text: s.slice(last) });
  return parts;
}
async function intentsSection(ctx, rest, main) {
  if (rest[0]) return intentEditor(ctx, `${ctx.agent}/intents/${rest[0]}`, main);
  const intents = await ctx.list('intents');
  listPage(main, 'Intents', intents, [['Name', i => h('b', null, i.displayName)], ['Training phrases', i => (i.trainingPhrases || []).length], ['Parameters', i => (i.parameters || []).map(p => h('span', { class: 'chip' }, p.id))], ['Labels', i => Object.keys(i.labels || {}).map(k => h('span', { class: 'chip blue' }, k))]],
    i => go('/' + i.name), async () => { const r = await createNamed(ctx.agent + '/intents', 'intent', null, () => ({ trainingPhrases: [], parameters: [] })); if (r) go('/' + r.name); });
}
async function intentEditor(ctx, name, main) {
  const it = await api.get(name); const x = clone(it); x.parameters = x.parameters || []; x.trainingPhrases = x.trainingPhrases || [];
  const save = () => guard(() => api.patch(name, x), 'Intent saved');
  main.append(header(it.displayName, 'Intent', h('button', { class: 'btn primary', onclick: save }, 'Save'), deleteBtn(name, `/${ctx.agent}/intents`)));
  const entOpts = await entityOptions(ctx);
  main.append(tabs(['Intent', 'JSON'], t => {
    if (t === 'JSON') return jsonEditor(it, v => guard(() => api.patch(name, v), 'Saved'));
    const dn = h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value });
    const desc = h('input', { value: x.description || '', oninput: e => x.description = e.target.value });
    const fallback = h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: x.isFallback, onchange: e => x.isFallback = e.target.checked }), 'Negative / fallback intent');
    const list = h('div', { class: 'stack' });
    const drawTp = () => {
      list.innerHTML = '';
      x.trainingPhrases.forEach((tp, i) => list.append(h('div', { class: 'row' }, h('input', { style: 'flex:1', value: partsToText(tp.parts), oninput: e => tp.parts = textToParts(e.target.value) }), h('button', { class: 'btn small danger', onclick: () => { x.trainingPhrases.splice(i, 1); drawTp(); } }, '✕'))));
    };
    drawTp();
    const add = h('input', { placeholder: 'Type a training phrase and press Enter. Annotate with [large](size).', onkeydown: e => { if (e.key === 'Enter' && e.target.value.trim()) { x.trainingPhrases.unshift({ parts: textToParts(e.target.value.trim()), repeatCount: 1 }); e.target.value = ''; drawTp(); } } });
    const ptb = h('tbody');
    const drawP = () => {
      ptb.innerHTML = '';
      x.parameters.forEach((p, i) => ptb.append(h('tr', null, h('td', null, h('input', { value: p.id, oninput: e => p.id = e.target.value })), h('td', null, sel(entOpts, p.entityType, v => p.entityType = v)),
        h('td', null, h('input', { type: 'checkbox', checked: p.isList, onchange: e => p.isList = e.target.checked })), h('td', null, h('input', { type: 'checkbox', checked: p.redact, onchange: e => p.redact = e.target.checked })),
        h('td', null, h('button', { class: 'btn small danger', onclick: () => { x.parameters.splice(i, 1); drawP(); } }, '✕')))));
    };
    drawP();
    return h('div', null, h('div', { class: 'card' }, field('Display name', dn), field('Description', desc), h('div', { style: 'margin-top:8px' }, fallback)),
      h('h2', null, `Training phrases (${x.trainingPhrases.length})`), add, h('div', { style: 'margin-top:8px' }, list),
      h('h2', null, 'Parameters'), h('table', null, h('thead', null, h('tr', null, h('th', null, 'ID'), h('th', null, 'Entity type'), h('th', null, 'List'), h('th', null, 'Redact'), h('th', null, ''))), ptb),
      h('button', { class: 'btn small', style: 'margin-top:6px', onclick: () => { x.parameters.push({ id: 'param', entityType: 'projects/-/locations/-/agents/-/entityTypes/sys.any' }); drawP(); } }, '+ Add parameter'),
      h('div', { style: 'margin-top:16px' }, h('button', { class: 'btn primary', onclick: save }, 'Save intent')));
  }));
}

// ---------- entity types ----------
async function entitiesSection(ctx, rest, main) {
  if (rest[0]) return entityEditor(ctx, `${ctx.agent}/entityTypes/${rest[0]}`, main);
  const ets = await ctx.list('entityTypes');
  listPage(main, 'Entity types', ets, [['Name', e => h('b', null, '@' + e.displayName)], ['Kind', e => e.kind], ['Entities', e => (e.entities || []).slice(0, 6).map(x => h('span', { class: 'chip' }, x.value))], ['Fuzzy', e => e.enableFuzzyExtraction ? '✓' : '']],
    e => go('/' + e.name), async () => { const r = await createNamed(ctx.agent + '/entityTypes', 'entity type', null, () => ({ kind: 'KIND_MAP', entities: [] })); if (r) go('/' + r.name); });
}
async function entityEditor(ctx, name, main) {
  const et = await api.get(name); const x = clone(et); x.entities = x.entities || [];
  const save = () => guard(() => api.patch(name, x), 'Entity type saved');
  main.append(header('@' + et.displayName, 'Entity type', h('button', { class: 'btn primary', onclick: save }, 'Save'), deleteBtn(name, `/${ctx.agent}/entityTypes`)));
  main.append(tabs(['Entities', 'JSON'], t => {
    if (t === 'JSON') return jsonEditor(et, v => guard(() => api.patch(name, v), 'Saved'));
    const kind = sel([['KIND_MAP', 'Map (value + synonyms)'], ['KIND_LIST', 'List'], ['KIND_REGEXP', 'Regular expression']], x.kind, v => x.kind = v);
    const tb = h('tbody');
    const draw = () => {
      tb.innerHTML = '';
      x.entities.forEach((e, i) => tb.append(h('tr', null, h('td', null, h('input', { value: e.value, oninput: ev => e.value = ev.target.value })),
        h('td', null, h('input', { value: (e.synonyms || []).join(', '), placeholder: 'comma separated', oninput: ev => e.synonyms = ev.target.value.split(',').map(s => s.trim()).filter(Boolean) })),
        h('td', null, h('button', { class: 'btn small danger', onclick: () => { x.entities.splice(i, 1); draw(); } }, '✕')))));
    };
    draw();
    const excl = h('textarea', { rows: 2, placeholder: 'One excluded phrase per line', oninput: e => x.excludedPhrases = e.target.value.split('\n').map(s => s.trim()).filter(Boolean).map(value => ({ value })) });
    excl.value = (x.excludedPhrases || []).map(p => p.value).join('\n');
    return h('div', null, h('div', { class: 'card' }, field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })), field('Kind', kind),
      h('div', { class: 'row', style: 'margin-top:10px' }, h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: x.enableFuzzyExtraction, onchange: e => x.enableFuzzyExtraction = e.target.checked }), 'Fuzzy matching'),
        h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: x.redact, onchange: e => x.redact = e.target.checked }), 'Redact in logs'))),
      h('h2', null, 'Entities'), h('table', null, h('thead', null, h('tr', null, h('th', null, 'Value (or regex)'), h('th', null, 'Synonyms'), h('th', null, ''))), tb),
      h('button', { class: 'btn small', style: 'margin-top:6px', onclick: () => { x.entities.push({ value: '', synonyms: [] }); draw(); } }, '+ Add entity'),
      h('h2', null, 'Excluded phrases'), excl, h('div', { style: 'margin-top:16px' }, h('button', { class: 'btn primary', onclick: save }, 'Save')));
  }));
}

// ---------- webhooks ----------
async function webhooksSection(ctx, rest, main) {
  if (rest[0]) return webhookEditor(ctx, `${ctx.agent}/webhooks/${rest[0]}`, main);
  const ws = await ctx.list('webhooks');
  listPage(main, 'Webhooks', ws, [['Name', w => h('b', null, w.displayName)], ['URL', w => h('code', null, (w.genericWebService || {}).uri || '')], ['Type', w => (w.genericWebService || {}).webhookType || 'STANDARD'], ['Status', w => w.disabled ? h('span', { class: 'chip bad' }, 'disabled') : h('span', { class: 'chip ok' }, 'enabled')]],
    w => go('/' + w.name), async () => { const r = await createNamed(ctx.agent + '/webhooks', 'webhook', null, () => ({ timeout: '5s', genericWebService: { uri: 'http://localhost:8090/webhook' } })); if (r) go('/' + r.name); });
}
async function webhookEditor(ctx, name, main) {
  const w = await api.get(name); const x = clone(w); x.genericWebService = x.genericWebService || {};
  const g = x.genericWebService; const save = () => guard(() => api.patch(name, x), 'Webhook saved');
  main.append(header(w.displayName, 'Webhook', h('button', { class: 'btn primary', onclick: save }, 'Save'), deleteBtn(name, `/${ctx.agent}/webhooks`)));
  main.append(tabs(['Webhook', 'JSON'], t => {
    if (t === 'JSON') return jsonEditor(w, v => guard(() => api.patch(name, v), 'Saved'));
    const jsonField = (obj, key, ph) => { const ta = h('textarea', { rows: 3, class: '', placeholder: ph, onchange: e => { try { obj[key] = e.target.value.trim() ? JSON.parse(e.target.value) : undefined; } catch (err) { toast('Invalid JSON in ' + key, true); } } }); ta.value = obj[key] ? JSON.stringify(obj[key], null, 2) : ''; return ta; };
    return h('div', { class: 'card' },
      field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })),
      field('Webhook URL (plain http, or a local TLS proxy for https)', h('input', { value: g.uri || '', oninput: e => g.uri = e.target.value })),
      h('div', { class: 'row' }, h('div', { style: 'flex:1' }, field('Timeout', h('input', { value: x.timeout || '5s', oninput: e => x.timeout = e.target.value }))),
        h('div', { style: 'flex:1' }, field('Type', sel([['STANDARD', 'Standard (CX WebhookRequest)'], ['FLEXIBLE', 'Flexible (custom body)']], g.webhookType || 'STANDARD', v => g.webhookType = v))),
        h('div', { style: 'flex:1' }, field('HTTP method (flexible)', sel(['POST', 'GET', 'PUT', 'PATCH', 'DELETE'].map(m => [m, m]), g.httpMethod || 'POST', v => g.httpMethod = v)))),
      h('div', { class: 'row' }, h('div', { style: 'flex:1' }, field('Username', h('input', { value: g.username || '', oninput: e => g.username = e.target.value }))), h('div', { style: 'flex:1' }, field('Password', h('input', { type: 'password', value: g.password || '', oninput: e => g.password = e.target.value })))),
      field('Request headers (JSON object)', jsonField(g, 'requestHeaders', '{"X-Api-Key": "..."}')),
      field('Flexible request body template', h('textarea', { rows: 3, oninput: e => g.requestBody = e.target.value, value: g.requestBody || '' })),
      field('Flexible response parameter mapping (JSON: param -> $.json.path)', jsonField(g, 'parameterMapping', '{"order_id": "$.order.id"}')),
      h('label', { class: 'inline', style: 'margin-top:10px' }, h('input', { type: 'checkbox', checked: x.disabled, onchange: e => x.disabled = e.target.checked }), 'Disabled'));
  }));
}

// ---------- generic small editors ----------
async function genericSection(ctx, rest, main, coll, title, cols, newBody, detail) {
  if (rest[0]) { const n = decodeURIComponent(rest[0]); return detail(ctx, n.includes('/') ? n : `${ctx.agent}/${coll}/${n}`, main); }
  const items = await ctx.list(coll);
  listPage(main, title, items, cols, it => go(`/${ctx.agent}/${coll}/${idOf(it.name)}`), async () => { const r = await createNamed(ctx.agent + '/' + coll, title.replace(/s$/, '').toLowerCase(), null, newBody); if (r) go(`/${ctx.agent}/${coll}/${idOf(r.name)}`); });
}
async function groupsSection(ctx, rest, main) {
  return genericSection(ctx, rest, main, 'transitionRouteGroups', 'Route groups', [['Name', g => h('b', null, g.displayName)], ['Routes', g => (g.transitionRoutes || []).length]], () => ({ transitionRoutes: [] }), async (ctx, name, main) => {
    const g = await api.get(name); const x = clone(g); x.transitionRoutes = x.transitionRoutes || [];
    const flow = name.includes('/flows/') ? name.split('/transitionRouteGroups/')[0] : null;
    main.append(header(g.displayName, flow ? 'Flow-level route group' : 'Agent-level route group', h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(name, x), 'Saved') }, 'Save'), deleteBtn(name, `/${ctx.agent}/transitionRouteGroups`)));
    main.append(field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })), h('h2', null, 'Routes'), await routesEditor(ctx, x.transitionRoutes, flow), h('h2', null, 'JSON'), jsonEditor(g, v => guard(() => api.patch(name, v), 'Saved')));
  });
}
async function generatorsSection(ctx, rest, main) {
  return genericSection(ctx, rest, main, 'generators', 'Generators', [['Name', g => h('b', null, g.displayName)], ['Prompt', g => ((g.promptText || {}).text || '').slice(0, 90)]], () => ({ promptText: { text: 'Summarize: $text' }, placeholders: [{ id: 'text', name: 'text' }] }), async (ctx, name, main) => {
    const g = await api.get(name); const x = clone(g); x.promptText = x.promptText || {};
    main.append(header(g.displayName, 'Generator — LLM prompt used from fulfillment', h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(name, x), 'Saved') }, 'Save'), deleteBtn(name, `/${ctx.agent}/generators`)));
    const ta = h('textarea', { rows: 8, oninput: e => { x.promptText.text = e.target.value; x.placeholders = [...new Set((e.target.value.match(/\$[A-Za-z_][\w-]*/g) || []).map(s => s.slice(1)))].map(n => ({ id: n, name: n })); } }); ta.value = x.promptText.text || '';
    main.append(h('div', { class: 'card' }, field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })), field('Prompt ($placeholders are filled from fulfillment inputParameters)', ta),
      field('Temperature', h('input', { type: 'number', step: '0.1', value: (x.modelParameter || {}).temperature ?? 0.3, oninput: e => x.modelParameter = Object.assign({}, x.modelParameter, { temperature: +e.target.value }) }))),
      h('h2', null, 'JSON'), jsonEditor(g, v => guard(() => api.patch(name, v), 'Saved')));
  });
}
function stepsToText(steps, d = 0) { return (steps || []).map(s => '  '.repeat(d) + '- ' + s.text + (s.steps && s.steps.length ? '\n' + stepsToText(s.steps, d + 1) : '')).join('\n'); }
function textToSteps(t) {
  const root = { steps: [] }; const stack = [{ d: -1, n: root }];
  for (const line of t.split('\n')) {
    if (!line.trim()) continue;
    const d = Math.floor((line.length - line.trimStart().length) / 2); const text = line.trim().replace(/^[-*]\s*/, '');
    while (stack.length > 1 && stack[stack.length - 1].d >= d) stack.pop();
    const node = { text, steps: [] }; stack[stack.length - 1].n.steps.push(node); stack.push({ d, n: node });
  }
  const clean = s => s.map(x => x.steps.length ? { text: x.text, steps: clean(x.steps) } : { text: x.text });
  return clean(root.steps);
}
async function playbooksSection(ctx, rest, main) {
  return genericSection(ctx, rest, main, 'playbooks', 'Playbooks', [['Name', p => h('b', null, p.displayName)], ['Goal', p => (p.goal || '').slice(0, 100)], ['Start', p => S.agentRes.startPlaybook === p.name ? h('span', { class: 'chip blue' }, 'start playbook') : '']],
    () => ({ goal: 'Describe the goal of this playbook.', instruction: { steps: [{ text: 'Greet the user.' }] } }), async (ctx, name, main) => {
      const p = await api.get(name); const x = clone(p); x.instruction = x.instruction || { steps: [] };
      const tools = await ctx.list('tools'); const flows = await ctx.list('flows'); const pbs = await ctx.list('playbooks');
      main.append(header(p.displayName, 'Playbook', h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(name, x), 'Saved') }, 'Save'), deleteBtn(name, `/${ctx.agent}/playbooks`)));
      const instr = h('textarea', { rows: 8, oninput: e => x.instruction.steps = textToSteps(e.target.value) }); instr.value = stepsToText(x.instruction.steps);
      const refs = (key, items) => h('div', { class: 'row' }, items.length ? items.map(t => h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: (x[key] || []).includes(t.name), onchange: e => { x[key] = (x[key] || []).filter(n => n !== t.name); if (e.target.checked) x[key].push(t.name); } }), t.displayName)) : h('span', { class: 'muted' }, 'none available'));
      main.append(tabs(['Playbook', 'Examples', 'JSON'], t => {
        if (t === 'JSON') return jsonEditor(p, v => guard(() => api.patch(name, v), 'Saved'));
        if (t === 'Examples') {
          const box = h('div');
          (async () => {
            const exs = await api.list(name + '/examples');
            box.append(h('div', { class: 'muted', style: 'margin-bottom:8px' }, 'Examples teach the playbook. Without an LLM configured, the runtime replays the closest example deterministically.'),
              h('button', { class: 'btn', onclick: async () => { await guard(() => api.post(name + '/examples', { displayName: 'Example ' + (exs.length + 1), actions: [{ userUtterance: { text: 'Hi' } }, { agentUtterance: { text: 'Hello! How can I help?' } }], conversationState: 'OUTPUT_STATE_OK' }), 'Example added'); render(); } }, '+ Add example'));
            for (const ex of exs) box.append(h('div', { class: 'card' }, h('div', { class: 'row between' }, h('b', null, ex.displayName), h('button', { class: 'btn small danger', onclick: async () => { await guard(() => api.del(ex.name), 'Deleted'); render(); } }, 'Delete')),
              ...(ex.actions || []).map(a => a.userUtterance ? h('div', null, h('span', { class: 'chip blue' }, 'user'), a.userUtterance.text) : a.agentUtterance ? h('div', null, h('span', { class: 'chip' }, 'agent'), a.agentUtterance.text) : h('div', null, h('span', { class: 'chip warn' }, Object.keys(a)[0]), h('code', null, JSON.stringify(Object.values(a)[0])))),
              h('details', null, h('summary', { class: 'muted' }, 'Edit JSON'), jsonEditor(ex, v => guard(() => api.patch(ex.name, v), 'Saved')))));
          })();
          return box;
        }
        return h('div', null, h('div', { class: 'card' }, field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })),
          field('Goal', h('textarea', { rows: 2, oninput: e => x.goal = e.target.value, value: x.goal || '' })),
          field('Instructions (indent with two spaces for sub-steps; reference ${TOOL:name} or ${PLAYBOOK:name})', instr)),
          h('h2', null, 'Tools'), refs('referencedTools', tools), h('h2', null, 'Playbooks it can hand off to'), refs('referencedPlaybooks', pbs.filter(b => b.name !== name)), h('h2', null, 'Flows it can hand off to'), refs('referencedFlows', flows));
      }));
    });
}
async function toolsSection(ctx, rest, main) {
  return genericSection(ctx, rest, main, 'tools', 'Tools', [['Name', t => h('b', null, t.displayName)], ['Type', t => t.openApiSpec ? 'OpenAPI' : t.functionSpec ? 'Function' : t.dataStoreSpec ? 'Data store' : '—'], ['Description', t => (t.description || '').slice(0, 100)]],
    () => ({ description: 'Describe what the tool does.', openApiSpec: { textSchema: 'openapi: 3.0.0\ninfo:\n  title: My API\n  version: "1.0"\nservers:\n  - url: http://localhost:9000\npaths:\n  /items/{id}:\n    get:\n      operationId: getItem\n      parameters:\n        - name: id\n          in: path\n          required: true\n          schema: { type: string }\n' } }), async (ctx, name, main) => {
      const t = await api.get(name); const x = clone(t);
      main.append(header(t.displayName, 'Tool', h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(name, x), 'Saved') }, 'Save'), deleteBtn(name, `/${ctx.agent}/tools`)));
      const schema = h('textarea', { class: 'code', style: 'min-height:260px', oninput: e => { if (x.openApiSpec) x.openApiSpec.textSchema = e.target.value; else { try { x.functionSpec = JSON.parse(e.target.value); } catch (er) { } } } });
      schema.value = x.openApiSpec ? x.openApiSpec.textSchema || '' : JSON.stringify(x.functionSpec || { inputSchema: { type: 'object', properties: {} }, outputSchema: { type: 'object' } }, null, 2);
      main.append(h('div', { class: 'card' }, field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })), field('Description', h('textarea', { rows: 2, value: x.description || '', oninput: e => x.description = e.target.value })),
        field('Type', sel([['openapi', 'OpenAPI (server-side HTTP)'], ['function', 'Function (client-side)']], x.functionSpec ? 'function' : 'openapi', v => { if (v === 'function') { delete x.openApiSpec; x.functionSpec = { inputSchema: { type: 'object' }, outputSchema: { type: 'object' } }; } else { delete x.functionSpec; x.openApiSpec = { textSchema: '' }; } render(); })),
        field(x.functionSpec ? 'Function schema (JSON)' : 'OpenAPI schema (YAML or JSON)', schema)), h('h2', null, 'JSON'), jsonEditor(t, v => guard(() => api.patch(name, v), 'Saved')));
    });
}

// ---------- tests ----------
function resultChip(r) { if (!r || !r.testResult) return h('span', { class: 'chip' }, 'not run'); return h('span', { class: 'chip ' + (r.testResult === 'PASSED' ? 'ok' : 'bad') }, r.testResult.toLowerCase()); }
async function testsSection(ctx, rest, main) {
  if (rest[0]) return testDetail(ctx, `${ctx.agent}/testCases/${rest[0]}`, main);
  const tcs = await ctx.list('testCases');
  const runAll = h('button', { class: 'btn', onclick: async () => { const r = await guard(() => api.post(ctx.agent + '/testCases:batchRun', {})); const res = r.response.results; toast(`${res.filter(x => x.testResult === 'PASSED').length}/${res.length} passed`); render(); } }, 'Run all');
  const cov = h('button', { class: 'btn', onclick: async () => { const r = await api.post(ctx.agent + '/testCases:calculateCoverage', { type: 'INTENT' }); const c = r.intentCoverage; modal('Intent coverage', h('div', null, h('p', null, `Coverage score: ${Math.round(c.coverageScore * 100)}%`), h('table', null, c.intents.map(i => h('tr', null, h('td', null, i.displayName), h('td', null, i.covered ? '✓' : '—')))))); } }, 'Coverage');
  listPage(main, 'Test cases', tcs, [['Name', t => h('b', null, t.displayName)], ['Tags', t => (t.tags || []).map(x => h('span', { class: 'chip' }, x))], ['Turns', t => (t.testCaseConversationTurns || []).length], ['Last result', t => resultChip(t.lastTestResult)], ['Last run', t => ((t.lastTestResult || {}).testTime || '').replace('T', ' ').slice(0, 19)]],
    t => go('/' + t.name), async () => { const r = await createNamed(ctx.agent + '/testCases', 'test case', null, () => ({ testCaseConversationTurns: [] })); if (r) go('/' + r.name); }, h('span', null, cov, ' ', runAll));
}
async function testDetail(ctx, name, main) {
  const tc = await api.get(name);
  main.append(header(tc.displayName, 'Test case', h('button', { class: 'btn primary', onclick: async () => { await guard(() => api.post(name + ':run', {}), 'Test run complete'); render(); } }, 'Run'), deleteBtn(name, `/${ctx.agent}/testCases`)));
  const r = tc.lastTestResult;
  main.append(tabs(['Conversation', 'JSON'], t => {
    if (t === 'JSON') return jsonEditor(tc, v => guard(() => api.patch(name, v), 'Saved'));
    const turns = tc.testCaseConversationTurns || [];
    return h('div', null, h('p', null, 'Last result: ', resultChip(r)), h('table', null, h('tr', null, h('th', null, '#'), h('th', null, 'User'), h('th', null, 'Expected'), h('th', null, 'Actual / differences')),
      turns.map((turn, i) => {
        const act = r && r.conversationTurns && r.conversationTurns[i] ? r.conversationTurns[i].virtualAgentOutput : null;
        const exp = turn.virtualAgentOutput || {};
        const inp = turn.userInput.input; const said = inp.text ? inp.text.text : inp.event ? 'event: ' + inp.event.event : inp.intent ? 'intent: ' + idOf(inp.intent.intent) : JSON.stringify(inp);
        return h('tr', null, h('td', null, i + 1), h('td', null, said),
          h('td', null, exp.triggeredIntent ? h('div', null, 'Intent: ', exp.triggeredIntent.displayName || idOf(exp.triggeredIntent.name)) : null, exp.currentPage ? h('div', null, 'Page: ', exp.currentPage.displayName || idOf(exp.currentPage.name)) : null,
            (exp.textResponses || []).map(x => h('div', { class: 'muted' }, '“' + (x.text || []).join(' ') + '”')), exp.sessionParameters ? h('code', null, JSON.stringify(exp.sessionParameters)) : null),
          h('td', null, act ? [((act.differences || []).length ? act.differences.map(d => h('div', { class: 'sev-ERROR' }, d.type + ': ' + d.description)) : h('span', { class: 'chip ok' }, 'match')),
            h('div', { class: 'muted' }, (act.textResponses || []).map(x => (x.text || []).join(' ')).join(' '))] : h('span', { class: 'muted' }, '—')));
      })));
  }));
}
async function envSection(ctx, rest, main) {
  return genericSection(ctx, rest, main, 'environments', 'Environments', [['Name', e => h('b', null, e.displayName)], ['Flow versions', e => (e.versionConfigs || []).map(v => h('span', { class: 'chip' }, idOf(parentOf(v.version)).slice(0, 8) + ' v' + idOf(v.version).slice(0, 6)))], ['Updated', e => (e.updateTime || '').slice(0, 19).replace('T', ' ')]],
    () => ({ versionConfigs: [] }), async (ctx, name, main) => {
      const e = await api.get(name); const x = clone(e); x.versionConfigs = x.versionConfigs || [];
      main.append(header(e.displayName, 'Environment — serves pinned flow versions; unpinned flows use the draft', h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(name, x), 'Saved') }, 'Save'),
        h('button', { class: 'btn', onclick: async () => { const r = await guard(() => api.post(name + ':runContinuousTest', {})); toast('Continuous test: ' + r.response.continuousTestResult.result); } }, 'Run tests'), deleteBtn(name, `/${ctx.agent}/environments`)));
      const flows = await ctx.list('flows'); const rows = [];
      for (const f of flows) {
        const vs = await api.list(f.name + '/versions'); const cur = (x.versionConfigs.find(v => v.version.startsWith(f.name + '/')) || {}).version || '';
        rows.push(h('tr', null, h('td', null, f.displayName), h('td', null, sel([['', 'Draft (latest)'], ...vs.map(v => [v.name, v.displayName + ' (' + (v.createTime || '').slice(0, 10) + ')'])], cur, val => {
          x.versionConfigs = x.versionConfigs.filter(v => !v.version.startsWith(f.name + '/')); if (val) x.versionConfigs.push({ version: val });
        }))));
      }
      main.append(h('div', { class: 'card' }, field('Display name', h('input', { value: x.displayName, oninput: ev => x.displayName = ev.target.value })), field('Description', h('input', { value: x.description || '', oninput: ev => x.description = ev.target.value }))),
        h('h2', null, 'Flow versions'), h('table', null, h('tr', null, h('th', null, 'Flow'), h('th', null, 'Version')), rows));
    });
}
async function validationSection(ctx, rest, main) {
  main.append(header('Validation', 'Checks routes, references, conditions, training phrases and entities', h('button', { class: 'btn primary', onclick: () => render() }, 'Re-validate')));
  const r = await api.post(ctx.agent + ':validate');
  const s = r.summary || {};
  main.append(h('div', { class: 'row', style: 'margin-bottom:10px' }, h('span', { class: 'chip bad' }, `${s.ERROR || 0} errors`), h('span', { class: 'chip warn' }, `${s.WARNING || 0} warnings`), h('span', { class: 'chip' }, `${s.INFO || 0} info`)));
  const table = (msgs) => msgs.length ? h('table', null, msgs.map(m => h('tr', null, h('td', { class: 'sev-' + m.severity, style: 'width:90px' }, m.severity), h('td', { style: 'width:170px' }, m.resourceType), h('td', null, m.detail, h('div', { class: 'muted' }, (m.resourceNames || []).map(x => x.name).join(', ')))))) : h('div', { class: 'muted' }, 'No issues.');
  main.append(h('h2', null, 'Agent'), table(r.agentValidationMessages || []));
  for (const f of r.flowValidationResults || []) main.append(h('h2', null, 'Flow: ' + f.flowDisplayName), table(f.validationMessages || []));
}
async function convSection(ctx, rest, main) {
  if (rest[0]) {
    const c = await api.get(`${ctx.agent}/conversations/${rest[0]}`);
    main.append(header('Conversation ' + rest[0].slice(0, 12), `${c.turnCount} turns · started ${(c.startTime || '').replace('T', ' ').slice(0, 19)}`, deleteBtn(c.name, `/${ctx.agent}/conversations`)));
    const log = h('div', { class: 'card', style: 'display:flex;flex-direction:column;gap:8px' });
    for (const it of c.interactions || []) {
      const qi = it.request.queryInput || {}; const qr = it.response.queryResult || {};
      log.append(h('div', { class: 'bubble user' }, qi.text ? qi.text.text : qi.event ? '[event] ' + qi.event.event : qi.intent ? '[intent] ' + idOf(qi.intent.intent) : '[input]'));
      for (const m of qr.responseMessages || []) if (m.text) log.append(h('div', { class: 'bubble bot' }, m.text.text.join(' ')));
      log.append(h('div', { class: 'meta' }, metaLine(qr)));
    }
    return main.append(log);
  }
  const r = await api.get(ctx.agent + '/conversations');
  listPage(main, 'Conversation history', r.conversations || [], [['Session', c => h('code', null, idOf(c.session).slice(0, 18))], ['Started', c => (c.startTime || '').replace('T', ' ').slice(0, 19)], ['Turns', c => c.turnCount], ['Intents', c => [...new Set((c.intents || []).map(i => i.displayName))].slice(0, 4).map(i => h('span', { class: 'chip' }, i))], ['Ended', c => c.ended ? '✓' : '']],
    c => go(`/${ctx.agent}/conversations/${idOf(c.name)}`));
}
async function changelogSection(ctx, rest, main) {
  const cls = await ctx.list('changelogs');
  listPage(main, 'Change history', cls.slice(0, 300), [['Time', c => (c.createTime || '').replace('T', ' ').slice(0, 19)], ['User', c => c.userEmail], ['Action', c => c.action], ['Type', c => c.type], ['Resource', c => c.displayName]], () => { });
}
async function settingsSection(ctx, rest, main) {
  const a = await api.get(ctx.agent); const x = clone(a);
  const flows = await ctx.list('flows'); const pbs = await ctx.list('playbooks');
  x.nluSettings = x.nluSettings || {};
  main.append(header('Agent settings', a.name, h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(ctx.agent, x), 'Settings saved') }, 'Save')));
  const startVal = x.startPlaybook ? 'pb:' + x.startPlaybook : 'flow:' + (x.startFlow || '');
  main.append(h('div', { class: 'card' },
    field('Display name', h('input', { value: x.displayName, oninput: e => x.displayName = e.target.value })),
    field('Description', h('textarea', { rows: 2, value: x.description || '', oninput: e => x.description = e.target.value })),
    h('div', { class: 'row' }, h('div', { style: 'flex:1' }, field('Default language', h('input', { value: x.defaultLanguageCode, oninput: e => x.defaultLanguageCode = e.target.value }))), h('div', { style: 'flex:1' }, field('Time zone', h('input', { value: x.timeZone, oninput: e => x.timeZone = e.target.value })))),
    field('Start resource', sel([...flows.map(f => ['flow:' + f.name, 'Flow: ' + f.displayName]), ...pbs.map(p => ['pb:' + p.name, 'Playbook: ' + p.displayName])], startVal, v => { if (v.startsWith('pb:')) { x.startPlaybook = v.slice(3); delete x.startFlow; } else { x.startFlow = v.slice(5); delete x.startPlaybook; } })),
    field('NLU classification threshold', h('input', { type: 'number', step: '0.05', min: 0, max: 1, value: x.nluSettings.classificationThreshold ?? 0.3, oninput: e => x.nluSettings.classificationThreshold = +e.target.value })),
    h('label', { class: 'inline', style: 'margin-top:10px' }, h('input', { type: 'checkbox', checked: ((x.advancedSettings || {}).loggingSettings || {}).enableInteractionLogging !== false, onchange: e => { x.advancedSettings = x.advancedSettings || {}; x.advancedSettings.loggingSettings = Object.assign({}, x.advancedSettings.loggingSettings, { enableInteractionLogging: e.target.checked }); } }), 'Log conversation history')));
  main.append(h('h2', null, 'Import / export'), h('div', { class: 'row' },
    h('button', { class: 'btn', onclick: async () => { const r = await api.get(ctx.agent + ':export'); download(slug(a.displayName) + '.agent.json', JSON.stringify(r.response.agentContent, null, 2)); } }, 'Export agent (JSON)'),
    h('button', { class: 'btn', onclick: async () => { const r = await api.post(ctx.agent + ':export', { dataFormat: 'JSON_PACKAGE' }); downloadBase64(slug(a.displayName) + '.zip', r.response.agentContent, 'application/zip'); } }, 'Export JSON package (.zip)'),
    h('button', { class: 'btn', onclick: async () => { const c = await pickAgentContent(); if (!c) return; if (!confirm('Restore replaces all flows, intents and other resources of this agent. Continue?')) return; await guard(() => api.post(ctx.agent + ':restore', { agentContent: c }), 'Agent restored'); render(); } }, 'Restore from file (.json / .zip)')));
  main.append(h('h2', null, 'Danger zone'), h('button', { class: 'btn danger', onclick: async () => { if (!confirm(`Delete agent "${a.displayName}"? This cannot be undone.`)) return; await guard(() => api.del(ctx.agent), 'Agent deleted'); go(`/${parentOf(ctx.agent)}/agents`); } }, 'Delete agent'));
}
function slug(s) { return (s || 'agent').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, ''); }

// ---------- simulator ----------
function metaLine(qr) {
  const bits = [];
  if (qr.match) bits.push(qr.match.matchType === 'INTENT' ? `${(qr.intent || {}).displayName} (${qr.intentDetectionConfidence})` : qr.match.matchType);
  if (qr.currentPlaybook) bits.push('playbook: ' + qr.currentPlaybook.displayName);
  else if (qr.currentPage) bits.push((qr.currentFlow ? qr.currentFlow.displayName + ' › ' : '') + qr.currentPage.displayName);
  if (qr.webhookStatuses) bits.push('webhook: ' + qr.webhookStatuses.map(w => w.code === 0 ? 'ok' : 'error').join(','));
  return bits.join(' · ');
}
async function renderSim() {
  const sim = $('#sim');
  sim.classList.toggle('hidden', !S.sim.open || !S.agent);
  $('#toggle-sim').classList.toggle('hidden', !S.agent);
  if (!S.sim.open || !S.agent) return;
  if (S.sim.agent !== S.agent) { S.sim.agent = S.agent; S.sim.session = null; S.sim.log = []; S.sim.state = null; }
  if (!S.sim.session) S.sim.session = 'sim-' + Math.random().toString(36).slice(2, 12);
  sim.innerHTML = '';
  let envs = [];
  try { envs = await api.list(S.agent + '/environments'); } catch (e) { }
  const envSel = sel([['', 'Draft'], ...envs.map(e => [e.name, e.displayName])], S.sim.env, v => { S.sim.env = v; resetSim(); });
  const log = h('div', { class: 'sim-log' });
  const drawLog = () => {
    log.innerHTML = '';
    if (!S.sim.log.length) log.append(h('div', { class: 'bubble sys' }, 'Say something to start a conversation.'));
    for (const e of S.sim.log) {
      if (e.user != null) log.append(h('div', { class: 'bubble user' }, e.user));
      if (e.resp) {
        const qr = e.resp.queryResult;
        for (const m of qr.responseMessages || []) {
          if (m.text) log.append(h('div', { class: 'bubble bot' }, m.text.text.join(' ')));
          else if (m.payload) log.append(h('div', { class: 'bubble bot' }, h('code', null, JSON.stringify(m.payload))));
          else if (m.toolCall) log.append(h('div', { class: 'bubble sys' }, '⚙ client tool call: ' + idOf(m.toolCall.tool) + ' ' + JSON.stringify(m.toolCall.inputParameters || {})));
          else if (m.liveAgentHandoff) log.append(h('div', { class: 'bubble sys' }, '→ live agent handoff'));
          else if (m.endInteraction) log.append(h('div', { class: 'bubble sys' }, '— conversation ended —'));
        }
        log.append(h('div', { class: 'meta', title: 'Show execution details', onclick: () => modal('Execution details', h('pre', null, JSON.stringify(qr.diagnosticInfo, null, 2)), [['Close', 'primary', () => true]]) }, metaLine(qr) + '  ⓘ'));
      }
      if (e.err) log.append(h('div', { class: 'bubble sys' }, '⚠ ' + e.err));
    }
    log.scrollTop = log.scrollHeight;
  };
  const input = h('input', { placeholder: 'Talk to your agent', onkeydown: e => { if (e.key === 'Enter') send(); } });
  const state = h('div', { class: 'sim-state' });
  const drawState = () => {
    const qr = S.sim.state; state.innerHTML = '';
    if (!qr) return state.append(h('span', { class: 'muted' }, 'Session parameters appear here.'));
    state.append(h('b', null, 'Parameters'), h('pre', null, JSON.stringify(qr.parameters || {}, null, 2)));
  };
  async function send(body) {
    const text = input.value; if (!body && !text.trim()) return;
    input.value = '';
    const base = S.sim.env ? S.sim.env : S.agent;
    const entry = { user: body ? null : text }; S.sim.log.push(entry); drawLog();
    try {
      const resp = await api.post(`${base}/sessions/${S.sim.session}:detectIntent`, body || { queryInput: { text: { text }, languageCode: (S.agentRes || {}).defaultLanguageCode || 'en' } });
      entry.resp = resp; S.sim.state = resp.queryResult;
    } catch (e) { entry.err = e.message; }
    drawLog(); drawState(); input.focus();
  }
  function resetSim() { S.sim.session = null; S.sim.log = []; S.sim.state = null; renderSim(); }
  const evBtn = h('button', { class: 'btn small', onclick: async () => { const ev = prompt('Event name', 'sys.no-input-default'); if (ev) { S.sim.log.push({ user: '[event] ' + ev }); await send({ queryInput: { event: { event: ev } } }); S.sim.log.splice(S.sim.log.length - 2, 1); drawLog(); } } }, 'Event');
  const saveBtn = h('button', { class: 'btn small', onclick: async () => { const n = prompt('Test case name', 'Simulator conversation'); if (!n) return; const r = await guard(() => api.post(S.agent + ':saveTestCase', { session: (S.sim.env || S.agent) + '/sessions/' + S.sim.session, displayName: n }), 'Saved as test case'); if (r) go('/' + r.name); } }, 'Save as test');
  sim.append(h('div', { class: 'sim-head' }, h('div', { class: 'row between' }, h('b', null, 'Simulator'), h('div', { class: 'row' }, evBtn, saveBtn, h('button', { class: 'btn small', onclick: resetSim }, 'Reset'), h('button', { class: 'btn small ghost', onclick: () => { S.sim.open = false; renderSim(); } }, '✕'))),
    h('div', { class: 'row', style: 'margin-top:6px' }, h('span', { class: 'muted' }, 'Environment'), envSel)), log, state, h('div', { class: 'sim-input' }, input, h('button', { class: 'btn primary', onclick: () => send() }, 'Send')));
  drawLog(); drawState(); input.focus();
}
