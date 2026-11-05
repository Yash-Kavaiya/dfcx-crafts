/* Conversational Agents console — dependency-free single-page app. */
'use strict';

// ---------- helpers ----------
const $ = (s, r = document) => r.querySelector(s);
// ---------- icons (inline SVG, stroke style, themed via currentColor) ----------
const ICONS = {
  menu: '<path d="M4 6h16M4 12h16M4 18h16"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  chevron: '<path d="m9 6 6 6-6 6"/>',
  flow: '<rect x="3" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="16" width="7" height="5" rx="1.5"/><path d="M6.5 8v3.5A2.5 2.5 0 0 0 9 14h6.5a2 2 0 0 1 2 2"/>',
  sparkles: '<path d="m12 3 1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z"/><path d="m19 14 .8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z"/>',
  tool: '<path d="M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6-2.4-.6-.6-2.4z"/>',
  database: '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5"/><path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>',
  intent: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M8 9h8M8 13h5"/>',
  tag: '<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/><circle cx="7.5" cy="7.5" r="1.5"/>',
  route: '<circle cx="6" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.5 6H15a3 3 0 0 1 0 6H9a3 3 0 0 0 0 6h6.5"/>',
  webhook: '<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>',
  wand: '<path d="m3 21 12-12"/><path d="M15 4V2M15 12v-2M11 6H9M21 6h-2M18.5 3.5 17 5M18.5 8.5 17 7"/>',
  flask: '<path d="M9 3h6M10 3v6l-5.5 9.5A2 2 0 0 0 6.2 21h11.6a2 2 0 0 0 1.7-2.5L14 9V3"/><path d="M7.5 15h9"/>',
  layers: '<path d="m12 2 10 5-10 5L2 7z"/><path d="m2 17 10 5 10-5"/><path d="m2 12 10 5 10-5"/>',
  shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="m9 12 2 2 4-4"/>',
  chart: '<path d="M3 3v18h18"/><path d="M8 16v-5M13 16V8M18 16v-9"/>',
  chats: '<path d="M14 9a2 2 0 0 1-2 2H6l-4 4V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2z"/><path d="M18 9h2a2 2 0 0 1 2 2v11l-4-4h-6a2 2 0 0 1-2-2v-1"/>',
  history: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l3 2"/>',
  plug: '<path d="M12 22v-5M9 8V2M15 8V2"/><path d="M18 8v4a6 6 0 0 1-12 0V8z"/>',
  sliders: '<path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6"/>',
  folder: '<path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>',
  bot: '<rect x="4" y="8" width="16" height="12" rx="3"/><path d="M12 8V4"/><circle cx="12" cy="3" r="1"/><path d="M9 13v1M15 13v1M9.5 17h5"/>',
  trash: '<path d="M3 6h18M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/>',
  play: '<path d="M7 4v16l13-8z"/>',
  send: '<path d="M22 2 11 13"/><path d="m22 2-7 20-4-9-9-4z"/>',
  upload: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>',
  monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
  key: '<circle cx="7.5" cy="15.5" r="5.5"/><path d="m21 2-9.6 9.6M15.5 7.5l3 3L22 7l-3-3"/>',
  logout: '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>',
  external: '<path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  copy: '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
  refresh: '<path d="M21 12a9 9 0 1 1-2.6-6.4L21 8"/><path d="M21 3v5h-5"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  alert: '<circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
  file: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>',
  code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/>',
  bolt: '<path d="M13 2 3 14h9l-1 8 10-12h-9z"/>',
  globe: '<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 0 20 15 15 0 0 1 0-20"/>',
  edit: '<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>',
  eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
};
function icon(name, size = 18, cls = '') {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24'); s.setAttribute('width', size); s.setAttribute('height', size);
  s.setAttribute('fill', 'none'); s.setAttribute('stroke', 'currentColor'); s.setAttribute('stroke-width', '1.8');
  s.setAttribute('stroke-linecap', 'round'); s.setAttribute('stroke-linejoin', 'round'); s.setAttribute('aria-hidden', 'true');
  s.setAttribute('class', 'ic ' + cls); s.innerHTML = ICONS[name] || ICONS.info;
  return s;
}
// Buttons get a matching icon automatically from their label.
const BUTTON_ICONS = {
  'Delete': 'trash', 'Save': 'check', 'Save JSON': 'check', 'Save page': 'check', 'Save intent': 'check', 'Run': 'play', 'Run all': 'play',
  'Run tests': 'play', 'Train': 'bolt', 'Import agent': 'upload', 'Export agent (JSON)': 'download', 'Export JSON package (.zip)': 'download',
  'Restore from file (.json / .zip)': 'upload', 'Search': 'search', 'Send': 'send', 'Reset': 'refresh', 'Event': 'bolt', 'Save as test': 'flask',
  'Coverage': 'chart', 'Copy': 'copy', 'Re-validate': 'refresh', 'Delete project': 'trash', 'Delete agent': 'trash', 'Import CSV': 'upload',
  'Load to draft': 'download', 'Sign in': 'key', 'Create': 'plus', 'Add': 'plus',
};
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
  const flat = kids.flat(Infinity).filter(k => k != null && k !== false);
  if (tag === 'button' && flat.length === 1 && typeof flat[0] === 'string') {
    const label = flat[0];
    if (label.startsWith('+ ')) { flat[0] = label.slice(2); flat.unshift(icon('plus', 16)); }
    else if (BUTTON_ICONS[label]) flat.unshift(icon(BUTTON_ICONS[label], 16));
  }
  for (const k of flat) el.append(k.nodeType ? k : document.createTextNode(String(k)));
  // Tables scroll horizontally inside their own container on small screens.
  if (tag === 'table') return h('div', { class: 'table-wrap' }, el);
  return el;
}
const idOf = n => (n || '').split('/').pop();
const parentOf = n => n.split('/').slice(0, -2).join('/');
const clone = v => JSON.parse(JSON.stringify(v ?? null));
const esc = s => String(s ?? '');
function toast(msg, err) {
  const t = $('#toast'); t.innerHTML = ''; t.append(icon(err ? 'alert' : 'check', 18), h('span', null, msg));
  t.className = 'toast' + (err ? ' err' : '');
  clearTimeout(toast.t); toast.t = setTimeout(() => t.classList.add('hidden'), err ? 6000 : 2500);
}
// API key handling: servers started with --api-key / CXA_API_KEY answer 401
// until the console sends the key (kept in this browser only).
const KEY = 'cxa-api-key';
const getKey = () => { try { return localStorage.getItem(KEY) || ''; } catch (e) { return S.memKey || ''; } };
const setKey = k => { try { k ? localStorage.setItem(KEY, k) : localStorage.removeItem(KEY); } catch (e) { S.memKey = k; } };
let keyPrompt = null;
function askForKey(message) {
  if (keyPrompt) return keyPrompt;
  const input = h('input', { type: 'password', placeholder: 'API key', autocomplete: 'current-password' });
  keyPrompt = modal('API key required', h('div', null, h('p', { class: 'muted', style: 'margin-top:0' }, message || 'This server requires an API key. It is stored only in this browser.'), input),
    [['Sign in', 'primary', () => input.value.trim() || toast('Enter the API key', true) && false]]).then(v => { keyPrompt = null; if (v) setKey(input.value.trim()); return !!v; });
  return keyPrompt;
}
let inflight = 0;
function busy(delta) { inflight = Math.max(0, inflight + delta); document.body.classList.toggle('loading', inflight > 0); }
async function call(method, path, body, query, attempt = 0) {
  busy(1);
  try { return await call1(method, path, body, query, attempt); } finally { busy(-1); }
}
async function call1(method, path, body, query, attempt) {
  const qs = query ? '?' + new URLSearchParams(query) : '';
  const headers = { 'Content-Type': 'application/json' };
  const key = getKey(); if (key) headers.Authorization = 'Bearer ' + key;
  const r = await fetch('/v3/' + path + qs, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  if (r.status === 401 && attempt < 3) {
    if (key) setKey('');
    if (await askForKey(key ? 'That API key was rejected. Enter a valid key.' : undefined)) return call(method, path, body, query, attempt + 1);
    throw new Error('API key required');
  }
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
// theme: auto (follows the OS) -> light -> dark
const THEMES = ['auto', 'light', 'dark'];
function currentTheme() { try { return localStorage.getItem('cxa-theme') || 'auto'; } catch (e) { return 'auto'; } }
function applyTheme(t) {
  if (t === 'auto') document.documentElement.removeAttribute('data-theme'); else document.documentElement.setAttribute('data-theme', t);
  try { localStorage.setItem('cxa-theme', t); } catch (e) { }
  const b = $('#theme-toggle'); b.innerHTML = ''; b.append(icon(t === 'dark' ? 'moon' : t === 'light' ? 'sun' : 'monitor', 20));
  b.title = 'Theme: ' + t + ' (click to change)';
}
function setNav(open) {
  document.body.classList.toggle('nav-open', open);
  $('#nav-toggle').setAttribute('aria-expanded', String(open));
}
window.addEventListener('load', () => {
  if (!location.hash && location.pathname !== '/' && location.pathname.startsWith('/projects')) location.hash = '#' + location.pathname;
  $('#nav-toggle').append(icon('menu', 22));
  $('#nav-toggle').onclick = () => setNav(!document.body.classList.contains('nav-open'));
  $('#scrim').onclick = () => { setNav(false); if (innerWidth <= 900 && S.sim.open) { S.sim.open = false; renderSim(); } };
  $('#toggle-sim').append(icon('chats', 18), h('span', { class: 'lbl' }, 'Test agent'));
  $('#toggle-sim').onclick = () => { S.sim.open = !S.sim.open; renderSim(); };
  applyTheme(currentTheme());
  $('#theme-toggle').onclick = () => applyTheme(THEMES[(THEMES.indexOf(currentTheme()) + 1) % THEMES.length]);
  document.addEventListener('keydown', e => { if (e.key === 'Escape') { setNav(false); const m = $('#modal-root .modal-back'); if (m) m.click(); } });
  fetch('/v3/serverInfo').then(r => r.json()).then(info => {
    S.serverInfo = info;
    if (info.apiKeyRequired) $('#top-actions').prepend(h('button', { class: 'iconbtn', title: 'Sign out (forget the API key stored in this browser)', 'aria-label': 'Sign out', onclick: () => { setKey(''); toast('Signed out'); render(); } }, icon('logout', 20)));
  }).catch(() => { });
  render();
});

function crumbs(items) {
  const c = $('#crumbs'); c.innerHTML = '';
  items.forEach(([label, href], i) => {
    if (i) c.append(h('span', { class: 'sep' }, icon('chevron', 14)));
    c.append(href ? h('a', { href: '#' + href }, label) : h('span', { class: i === items.length - 1 ? 'current' : '' }, label));
  });
}

async function render() {
  const p = routeParts();
  const main = $('#main'); main.innerHTML = ''; $('#sidenav').innerHTML = '';
  setNav(false); main.scrollTop = 0;
  try {
    if (p[0] !== 'projects') return go('/projects');
    if (p.length === 1) { S.agent = null; await projectsPage(main); }
    else if (p.length === 2) return go(`/projects/${p[1]}/locations/global/agents`);
    else if (p.length <= 5) { S.agent = null; await agentsPage(main, p[1], p[3] || 'global'); }
    else {
      const agent = p.slice(0, 6).join('/');
      S.agent = agent;
      await agentShell(agent, p.slice(6), main);
    }
  } catch (e) { main.append(emptyState('alert', 'Something went wrong', e.message)); }
  finally {
    document.body.classList.toggle('has-nav', !!$('#sidenav').firstChild);
    renderSim();
  }
}

// ---------- projects & agents ----------
function emptyState(ic, title, text, action) {
  return h('div', { class: 'empty' }, h('div', { class: 'empty-ic' }, icon(ic, 28)), h('div', { class: 'empty-t' }, title), text ? h('div', { class: 'muted' }, text) : null, action || null);
}
function avatar(name, ic) {
  const hue = [...(name || '?')].reduce((a, c) => (a * 31 + c.charCodeAt(0)) % 360, 7);
  return h('span', { class: 'avatar', style: `--hue:${hue}` }, ic ? icon(ic, 20) : (name || '?').trim().charAt(0).toUpperCase());
}
async function projectsPage(main) {
  crumbs([['Projects']]); S.agent = null;
  const projects = await api.list('projects');
  main.append(pageHead('folder', 'Projects', 'Each project holds agents across locations.', h('button', { class: 'btn primary', onclick: newProject }, '+ New project')));
  if (!projects.length) return main.append(emptyState('folder', 'No projects yet', 'Create a project to start building agents.', h('button', { class: 'btn primary', onclick: newProject }, '+ New project')));
  main.append(h('div', { class: 'grid' }, projects.map(pr => h('a', { class: 'tile', href: `#/${pr.name}/locations/global/agents` },
    h('div', { class: 'tile-top' }, avatar(pr.displayName), h('div', { class: 'tile-main' }, h('div', { class: 't' }, pr.displayName), h('div', { class: 's' }, idOf(pr.name))), icon('chevron', 18, 'tile-go')),
    h('div', { class: 'tile-meta' }, icon('history', 14), 'Created ' + ((pr.createTime || '').slice(0, 10) || '—'))))));
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
  const locSel = h('select', { class: 'inline', 'aria-label': 'Location', onchange: e => go(`/projects/${project}/locations/${e.target.value}/agents`) }, LOCATIONS.map(l => h('option', { value: l, selected: l === loc }, l)));
  main.append(pageHead('bot', 'Agents', h('span', { class: 'row tight' }, icon('globe', 14), 'Location', locSel),
    h('button', { class: 'btn', title: 'Import a .json bundle or a Google Conversational Agents export (.zip / .blob)', onclick: () => importAgent(parent) }, 'Import agent'),
    h('button', { class: 'btn primary', onclick: () => newAgent(parent) }, '+ Create agent'),
    h('button', { class: 'btn danger ghost', onclick: async () => { if (confirm(`Delete project ${proj.displayName} and all its agents?`)) { await guard(() => api.del(`projects/${project}`), 'Project deleted'); go('/projects'); } } }, 'Delete project')));
  if (!agents.length) return main.append(emptyState('bot', `No agents in ${loc}`, 'Create an agent, or import a .json bundle or a Google agent export (.zip).', h('button', { class: 'btn primary', onclick: () => newAgent(parent) }, '+ Create agent')));
  main.append(h('div', { class: 'grid' }, agents.map(a => h('a', { class: 'tile', href: '#/' + a.name + '/flows' },
    h('div', { class: 'tile-top' }, avatar(a.displayName, a.startPlaybook ? 'sparkles' : 'bot'), h('div', { class: 'tile-main' }, h('div', { class: 't' }, a.displayName), h('div', { class: 's' }, (a.description || 'No description').slice(0, 110))), icon('chevron', 18, 'tile-go')),
    h('div', { class: 'tile-meta' }, h('span', { class: 'chip' }, a.defaultLanguageCode), h('span', { class: 'chip' }, a.timeZone), h('span', { class: 'chip ' + (a.startPlaybook ? 'violet' : 'blue') }, a.startPlaybook ? 'Generative' : 'Flow-based'))))));
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
  ['Build', [['flows', 'Flows', 'flow'], ['playbooks', 'Playbooks', 'sparkles'], ['tools', 'Tools', 'tool'], ['dataStores', 'Data stores', 'database'], ['intents', 'Intents', 'intent'], ['entityTypes', 'Entity types', 'tag'], ['transitionRouteGroups', 'Route groups', 'route'], ['webhooks', 'Webhooks', 'webhook'], ['generators', 'Generators', 'wand']]],
  ['Test & deploy', [['testCases', 'Test cases', 'flask'], ['environments', 'Environments', 'layers'], ['validation', 'Validation', 'shield']]],
  ['Monitor', [['analytics', 'Analytics', 'chart'], ['conversations', 'Conversation history', 'chats'], ['changelogs', 'Change history', 'history']]],
  ['Manage', [['integrations', 'Integrations', 'plug'], ['settings', 'Agent settings', 'sliders']]],
];
async function agentShell(agent, rest, main) {
  const a = await api.get(agent); S.agentRes = a;
  const project = agent.split('/')[1];
  const section = rest[0] || 'flows';
  const sec = SECTIONS.flatMap(s => s[1]).find(x => x[0] === section);
  S.sectionIcon = sec ? sec[2] : 'file';
  crumbs([['Projects', '/projects'], [project, `/projects/${project}/locations/${agent.split('/')[3]}/agents`], [a.displayName, `/${agent}/flows`], [sec ? sec[1] : section]]);
  const nav = $('#sidenav');
  nav.append(h('a', { class: 'agent-card', href: `#/projects/${project}/locations/${agent.split('/')[3]}/agents`, title: 'All agents' },
    avatar(a.displayName, a.startPlaybook ? 'sparkles' : 'bot'), h('span', { class: 'agent-card-t' }, h('b', null, a.displayName), h('small', null, `${project} · ${agent.split('/')[3]}`))));
  for (const [group, items] of SECTIONS) {
    nav.append(h('div', { class: 'group' }, group));
    for (const [k, label, ic] of items) nav.append(h('a', { href: `#/${agent}/${k}`, class: k === section ? 'active' : '', 'aria-current': k === section ? 'page' : null }, icon(ic, 18), h('span', { class: 'lbl' }, label)));
  }
  const ctx = new Ctx(agent);
  const fn = {
    flows: flowsSection, intents: intentsSection, entityTypes: entitiesSection, webhooks: webhooksSection,
    transitionRouteGroups: groupsSection, generators: generatorsSection, playbooks: playbooksSection, tools: toolsSection,
    testCases: testsSection, environments: envSection, validation: validationSection, conversations: convSection,
    changelogs: changelogSection, settings: settingsSection, integrations: integrationsSection, dataStores: dataStoresSection, analytics: analyticsSection,
  }[section];
  if (!fn) return main.append(h('div', { class: 'empty' }, 'Unknown section'));
  await fn(ctx, rest.slice(1), main);
}
class Ctx {
  constructor(agent) { this.agent = agent; this.c = {}; }
  async list(coll, parent) { const p = (parent || this.agent) + '/' + coll; if (!this.c[p]) this.c[p] = await api.list(p); return this.c[p]; }
  async pagesOf(flow) { return this.list('pages', flow); }
}
function pageHead(ic, title, sub, ...buttons) {
  return h('div', { class: 'page-head' },
    h('div', { class: 'page-title' }, h('span', { class: 'page-ic' }, icon(ic, 22)), h('div', null, h('h1', null, title), sub ? h('div', { class: 'muted sub' }, sub) : null)),
    h('div', { class: 'row actions' }, buttons));
}
function listPage(main, title, items, cols, onOpen, onNew, extra) {
  main.append(pageHead(S.sectionIcon || 'file', title, items.length ? `${items.length} item${items.length === 1 ? '' : 's'}` : null, extra || null, onNew ? h('button', { class: 'btn primary', onclick: onNew }, '+ Create') : null));
  if (!items.length) return main.append(emptyState(S.sectionIcon || 'file', `No ${title.toLowerCase()} yet`, onNew ? 'Create the first one to get started.' : null, onNew ? h('button', { class: 'btn primary', onclick: onNew }, '+ Create') : null));
  main.append(h('table', { class: 'list' }, h('thead', null, h('tr', null, cols.map(c => h('th', null, c[0])), h('th', { class: 'go' }))),
    h('tbody', null, items.map(it => h('tr', { class: 'click', tabindex: '0', onclick: () => onOpen(it), onkeydown: e => { if (e.key === 'Enter') onOpen(it); } }, cols.map(c => h('td', { 'data-label': c[0] }, c[1](it))), h('td', { class: 'go' }, icon('chevron', 16)))))));
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
  return h('button', { class: 'btn danger ghost', onclick: async () => { if (!confirm('Delete this resource?')) return; await guard(() => api.del(name), 'Deleted'); go(after); } }, 'Delete');
}
function header(title, sub, ...buttons) {
  return pageHead(S.sectionIcon || 'file', title, sub, ...buttons);
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

// ---------- data stores & knowledge ----------
const locationOf = agent => parentOf(agent);
async function knowledgeEditor(ctx, holder, flow) {
  const stores = await api.list(locationOf(ctx.agent) + '/dataStores');
  const k = holder.knowledgeConnectorSettings = holder.knowledgeConnectorSettings || { enabled: false, dataStoreConnections: [] };
  k.dataStoreConnections = k.dataStoreConnections || []; k.triggerFulfillment = k.triggerFulfillment || {};
  const tOpts = await targetOptions(ctx, flow);
  const ta = h('textarea', { rows: 2, placeholder: 'Optional. Leave empty to reply with the best answer, or use $request.knowledge.answers[0]', oninput: e => setTexts(k.triggerFulfillment, e.target.value) }); ta.value = getTexts(k.triggerFulfillment);
  return h('div', { class: 'card' },
    h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: k.enabled, onchange: e => k.enabled = e.target.checked }), 'Answer unmatched questions from data stores'),
    h('div', { class: 'muted', style: 'margin:6px 0' }, 'Used when no intent route matches. ', h('a', { href: `#/${ctx.agent}/dataStores` }, 'Manage data stores')),
    stores.length ? h('div', { class: 'row' }, stores.map(d => h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: k.dataStoreConnections.some(c => c.dataStore === d.name), onchange: e => {
      k.dataStoreConnections = k.dataStoreConnections.filter(c => c.dataStore !== d.name);
      if (e.target.checked) k.dataStoreConnections.push({ dataStoreType: d.type || 'UNSTRUCTURED', dataStore: d.name });
    } }), d.displayName + ' (' + (d.type || 'UNSTRUCTURED').toLowerCase() + ')'))) : h('div', { class: 'muted' }, 'No data stores in this location yet.'),
    field('Fulfillment', ta), field('Transition after answering', sel(tOpts, targetValue(k), v => applyTarget(k, v))));
}
async function dataStoresSection(ctx, rest, main) {
  const loc = locationOf(ctx.agent);
  if (rest[0]) return dataStoreDetail(ctx, `${loc}/dataStores/${rest[0]}`, main);
  const stores = await api.list(loc + '/dataStores');
  const type = h('select', null, h('option', { value: 'UNSTRUCTURED' }, 'Documents (unstructured text)'), h('option', { value: 'FAQ' }, 'FAQ (question / answer)'));
  listPage(main, 'Data stores', stores, [['Name', d => h('b', null, d.displayName)], ['Type', d => d.type || 'UNSTRUCTURED'], ['Created', d => (d.createTime || '').slice(0, 10)]],
    d => go(`/${ctx.agent}/dataStores/${idOf(d.name)}`), async () => { const r = await createNamed(loc + '/dataStores', 'data store', field('Type', type), () => ({ type: type.value, createTime: new Date().toISOString() })); if (r) go(`/${ctx.agent}/dataStores/${idOf(r.name)}`); },
    h('span', { class: 'muted' }, 'Shared by all agents in ' + idOf(loc)));
}
async function dataStoreDetail(ctx, name, main) {
  const ds = await api.get(name); const docs = await api.list(name + '/documents');
  const faq = ds.type === 'FAQ';
  main.append(header(ds.displayName, (faq ? 'FAQ' : 'Document') + ' data store · ' + docs.length + ' documents', deleteBtn(name, `/${ctx.agent}/dataStores`)));
  // search tester
  const q = h('input', { placeholder: 'Try a question…' }); const out = h('div');
  const search = async () => { out.innerHTML = ''; if (!q.value.trim()) return; const r = await guard(() => api.post(name + ':search', { query: q.value, pageSize: 5 }));
    out.append(r.results.length ? h('table', null, r.results.map(x => h('tr', null, h('td', { style: 'width:70px' }, h('span', { class: 'chip ' + (x.confidence >= 0.35 ? 'ok' : 'warn') }, x.confidence)), h('td', null, h('b', null, x.faqQuestion || x.title), h('div', null, x.snippet), x.uri ? h('a', { href: x.uri, target: '_blank', rel: 'noopener' }, x.uri) : null)))) : h('div', { class: 'muted' }, 'No results.')); };
  q.onkeydown = e => { if (e.key === 'Enter') search(); };
  main.append(h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'Test search'), h('div', { class: 'row' }, h('div', { style: 'flex:1' }, q), h('button', { class: 'btn primary', onclick: search }, 'Search')), h('div', { class: 'muted', style: 'margin-top:6px' }, 'Answers with confidence ≥ 0.35 are used by knowledge connectors.'), out));
  // add documents
  const addDoc = async () => {
    const t = h('input'); const u = h('input', { placeholder: 'https://… (optional)' }); const c = h('textarea', { rows: 8 });
    const body = faq ? h('div', null, field('Question', t), field('Answer', c)) : h('div', null, field('Title', t), field('Source URL', u), field('Content (paragraphs are split into passages)', c));
    const r = await modal(faq ? 'Add FAQ entry' : 'Add document', body, [['Add', 'primary', () => {
      if (!t.value.trim() || !c.value.trim()) return toast('Both fields are required', true);
      const doc = faq ? { displayName: t.value, structData: { question: t.value, answer: c.value } } : { displayName: t.value, title: t.value, uri: u.value, content: c.value };
      return guard(() => api.post(name + '/documents:import', { documents: [doc] }), 'Added');
    }]]);
    if (r) render();
  };
  const importCsv = async () => {
    const ta = h('textarea', { rows: 10, placeholder: 'question,answer\nWhat are your hours?,9am-5pm Monday to Friday' });
    const r = await modal('Import FAQ CSV', h('div', null, h('p', { class: 'muted' }, 'Two columns: question, answer. A header row is optional; quote fields containing commas.'), ta), [['Import', 'primary', () => guard(() => api.post(name + '/documents:import', { faqCsv: ta.value }), 'Imported')]]);
    if (r) render();
  };
  main.append(h('div', { class: 'row between' }, h('h2', null, 'Documents'), h('div', { class: 'row' }, faq ? h('button', { class: 'btn', onclick: importCsv }, 'Import CSV') : null, h('button', { class: 'btn primary', onclick: addDoc }, faq ? '+ FAQ entry' : '+ Document'))));
  main.append(docs.length ? h('table', null, h('tr', null, h('th', null, faq ? 'Question' : 'Title'), h('th', null, faq ? 'Answer' : 'Content'), h('th', null, '')),
    docs.map(d => h('tr', null, h('td', { style: 'width:30%' }, h('b', null, (d.structData || {}).question || d.title || d.displayName), d.uri ? h('div', { class: 'muted' }, d.uri) : null),
      h('td', null, ((d.structData || {}).answer || d.content || '').slice(0, 260)),
      h('td', null, h('button', { class: 'btn small danger', onclick: async () => { await guard(() => api.del(d.name), 'Deleted'); render(); } }, '✕')))))
    : h('div', { class: 'empty' }, 'No documents yet.'));
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
          h('h2', null, 'Knowledge connector (flow level)'), await knowledgeEditor(ctx, f, flowName),
          h('div', { style: 'margin-top:14px' }, h('button', { class: 'btn primary', onclick: () => guard(() => api.patch(flowName, { transitionRoutes: f.transitionRoutes, eventHandlers: f.eventHandlers, transitionRouteGroups: f.transitionRouteGroups, knowledgeConnectorSettings: f.knowledgeConnectorSettings }, 'transitionRoutes,eventHandlers,transitionRouteGroups,knowledgeConnectorSettings'), 'Saved') }, 'Save')));
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
      box.append(h('h2', null, 'Knowledge connector'), await knowledgeEditor(ctx, p, flowName));
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
        field('Type', sel([['openapi', 'OpenAPI (server-side HTTP)'], ['function', 'Function (client-side)'], ['datastore', 'Data store (search)']], x.functionSpec ? 'function' : x.dataStoreSpec ? 'datastore' : 'openapi', v => {
          delete x.openApiSpec; delete x.functionSpec; delete x.dataStoreSpec;
          if (v === 'function') x.functionSpec = { inputSchema: { type: 'object' }, outputSchema: { type: 'object' } };
          else if (v === 'datastore') x.dataStoreSpec = { dataStoreConnections: [] };
          else x.openApiSpec = { textSchema: '' };
          guard(() => api.patch(name, x, 'openApiSpec,functionSpec,dataStoreSpec'), 'Tool type changed').then(render);
        })),
        x.dataStoreSpec ? h('div', null, h('label', { class: 'f' }, 'Data stores searched by this tool'), await (async () => { const stores = await api.list(locationOf(ctx.agent) + '/dataStores'); x.dataStoreSpec.dataStoreConnections = x.dataStoreSpec.dataStoreConnections || [];
          return stores.length ? h('div', { class: 'row' }, stores.map(d => h('label', { class: 'inline' }, h('input', { type: 'checkbox', checked: x.dataStoreSpec.dataStoreConnections.some(c => c.dataStore === d.name), onchange: e => { x.dataStoreSpec.dataStoreConnections = x.dataStoreSpec.dataStoreConnections.filter(c => c.dataStore !== d.name); if (e.target.checked) x.dataStoreSpec.dataStoreConnections.push({ dataStoreType: d.type || 'UNSTRUCTURED', dataStore: d.name }); } }), d.displayName))) : h('div', { class: 'muted' }, 'No data stores yet.'); })())
          : field(x.functionSpec ? 'Function schema (JSON)' : 'OpenAPI schema (YAML or JSON)', schema)), h('h2', null, 'JSON'), jsonEditor(t, v => guard(() => api.patch(name, v), 'Saved')));
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
// ---------- analytics ----------
const tip = (() => { let t; return { show(e, html) { if (!t) { t = h('div', { class: 'tip', role: 'tooltip' }); document.body.append(t); } t.innerHTML = html; t.style.display = 'block'; t.style.left = Math.min(e.clientX + 12, innerWidth - t.offsetWidth - 8) + 'px'; t.style.top = (e.clientY + 14) + 'px'; }, hide() { if (t) t.style.display = 'none'; } }; })();
const escHtml = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
function statTile(label, value, sub) { return h('div', { class: 'stat' }, h('div', { class: 'stat-l' }, label), h('div', { class: 'stat-v' }, value), sub ? h('div', { class: 'stat-s' }, sub) : null); }
const pct = x => (Math.round(x * 1000) / 10) + '%';
function hbars(rows, fmt) {
  if (!rows.length) return h('div', { class: 'muted' }, 'No data yet.');
  const max = Math.max(...rows.map(r => r.count), 1);
  return h('div', { class: 'hbars' }, rows.map(r => h('div', { class: 'hbar', onmousemove: e => tip.show(e, `<b>${escHtml(r.name)}</b><br>${r.count} turns${r.avgConfidence != null ? ' · avg confidence ' + r.avgConfidence : ''}`), onmouseleave: () => tip.hide() },
    h('div', { class: 'hbar-l', title: r.name }, r.name || '—'), h('div', { class: 'hbar-t' }, h('div', { class: 'hbar-f', style: `width:${Math.max(2, (r.count / max) * 100)}%` })), h('div', { class: 'hbar-v' }, fmt ? fmt(r) : r.count))));
}
function columnChart(series) {
  const max = Math.max(...series.map(d => d.turns), 0);
  const cols = h('div', { class: 'cols', role: 'img', 'aria-label': 'Turns per day' }, series.map(d => h('div', { class: 'colw', onmousemove: e => tip.show(e, `<b>${d.date}</b><br>${d.turns} turn${d.turns === 1 ? '' : 's'}`), onmouseleave: () => tip.hide() },
    d.turns ? h('div', { class: 'colb', style: `height:${(d.turns / max) * 100}%` }) : null)));
  const n = series.length; const mid = Math.floor(n / 2);
  const axis = h('div', { class: 'colx' }, h('span', null, series[0] ? series[0].date.slice(5) : ''), h('span', null, series[mid] ? series[mid].date.slice(5) : ''), h('span', null, series[n - 1] ? series[n - 1].date.slice(5) : ''));
  const table = h('details', null, h('summary', { class: 'muted' }, 'Show as table'), h('table', null, h('tr', null, h('th', null, 'Date'), h('th', null, 'Turns')), series.slice().reverse().map(d => h('tr', null, h('td', null, d.date), h('td', null, d.turns)))));
  return h('div', null, h('div', { class: 'colchart' }, h('div', { class: 'coly' }, max ? 'max ' + max : 'no turns yet'), cols, axis), table);
}
async function analyticsSection(ctx, rest, main) {
  S.an = S.an || { days: 30, env: '' };
  const envs = await ctx.list('environments');
  const q = { days: S.an.days }; if (S.an.env) q.environment = S.an.env;
  const a = await api.get(ctx.agent + '/analytics', q);
  const s = a.summary;
  main.append(header('Analytics', `Last ${a.days} days · ${S.an.env ? 'environment ' + idOf(S.an.env) : 'draft'} · based on conversation history`));
  main.append(h('div', { class: 'row filters', style: 'margin-bottom:12px' },
    h('span', { class: 'muted' }, 'Range'), sel([['7', 'Last 7 days'], ['30', 'Last 30 days'], ['90', 'Last 90 days']], String(S.an.days), v => { S.an.days = +v; render(); }),
    h('span', { class: 'muted' }, 'Environment'), sel([['', 'Draft'], ...envs.map(e => [e.name, e.displayName])], S.an.env, v => { S.an.env = v; render(); })));
  main.append(h('div', { class: 'stats' },
    statTile('Conversations', s.sessions), statTile('Turns', s.turns, s.avgTurnsPerSession + ' per conversation'),
    statTile('No-match rate', pct(s.noMatchRate), 'of text inputs'), statTile('Completed', s.endedSessions, 'reached end of session'),
    statTile('Live agent handoffs', s.liveAgentHandoffs), statTile('Webhook errors', pct(s.webhookErrorRate), s.webhookCalls + (s.webhookCalls === 1 ? ' call' : ' calls'))));
  main.append(h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'Turns per day'), columnChart(a.turnsPerDay)));
  main.append(h('div', { class: 'grid2' },
    h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'Match types'), hbars(a.matchTypes.sort((x, y) => y.count - x.count))),
    h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'Top intents'), hbars(a.topIntents, r => `${r.count} · ${Math.round(r.avgConfidence * 100)}% conf.`)),
    h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'Most visited pages'), hbars(a.topPages)),
    h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'Drop-off pages'), h('div', { class: 'muted', style: 'margin:-4px 0 8px' }, 'Where conversations that did not end were last seen'), hbars(a.dropOffPages))));
  const intents = await ctx.list('intents');
  const addTo = u => {
    const pick = sel([['', 'Add to intent…'], ...intents.map(i => [i.name, i.displayName])], '', async v => {
      if (!v) return;
      const it = await api.get(v);
      const phrases = [{ parts: [{ text: u.text }], repeatCount: 1 }, ...(it.trainingPhrases || [])];
      await guard(() => api.patch(v, { trainingPhrases: phrases }, 'trainingPhrases'), `Added to ${it.displayName}`);
      pick.value = '';
    });
    return pick;
  };
  const rows = a.recentUnmatched.map(u => h('tr', null,
    h('td', { style: 'width:170px' }, u.time.replace('T', ' ').slice(0, 19)),
    h('td', null, u.text),
    h('td', { style: 'width:260px' }, addTo(u))));
  main.append(h('div', { class: 'card' },
    h('h2', { style: 'margin-top:0' }, 'Recent unmatched user inputs'),
    h('div', { class: 'muted', style: 'margin:-4px 0 8px' }, 'Add them as training phrases to improve coverage.'),
    rows.length ? h('table', null, h('tr', null, h('th', null, 'Time'), h('th', null, 'User said'), h('th', null, '')), rows) : h('div', { class: 'muted' }, 'Nothing unmatched in this period.')));
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
  // security settings (location level)
  const secs = await api.list(parentOf(ctx.agent) + '/securitySettings');
  const secSel = sel([['', '— none —'], ...secs.map(x => [x.name, x.displayName])], x.securitySettings || '', v => { x.securitySettings = v || undefined; });
  main.append(h('h2', null, 'Security'), h('div', { class: 'card' },
    h('p', { class: 'muted', style: 'margin-top:0' }, 'Parameters marked Redact (on pages, intents or entity types) are masked in conversation history. Security settings add retention and purge rules.'),
    field('Security settings', secSel),
    h('button', { class: 'btn small', style: 'margin-top:8px', onclick: async () => {
      const dn = h('input', { placeholder: 'e.g. 30-day retention' }); const days = h('input', { type: 'number', min: 0, value: 30 });
      const purge = h('input', { type: 'checkbox' });
      const r = await modal('New security settings', h('div', null, field('Display name', dn), field('Retention window (days, 0 = keep)', days), h('label', { class: 'inline', style: 'margin-top:10px' }, purge, 'Do not store conversation history')),
        [['Create', 'primary', () => guard(() => api.post(parentOf(ctx.agent) + '/securitySettings', { displayName: dn.value || 'Security settings', retentionWindowDays: +days.value || undefined, purgeDataTypes: purge.checked ? ['DIALOGFLOW_HISTORY'] : [], redactionScope: 'REDACT_DISK_STORAGE' }))]]);
      if (r) { x.securitySettings = r.name; await guard(() => api.patch(ctx.agent, x), 'Security settings attached'); render(); }
    } }, '+ New security settings')));
  main.append(h('h2', null, 'Import / export'), h('div', { class: 'row' },
    h('button', { class: 'btn', onclick: async () => { const r = await api.get(ctx.agent + ':export'); download(slug(a.displayName) + '.agent.json', JSON.stringify(r.response.agentContent, null, 2)); } }, 'Export agent (JSON)'),
    h('button', { class: 'btn', onclick: async () => { const r = await api.post(ctx.agent + ':export', { dataFormat: 'JSON_PACKAGE' }); downloadBase64(slug(a.displayName) + '.zip', r.response.agentContent, 'application/zip'); } }, 'Export JSON package (.zip)'),
    h('button', { class: 'btn', onclick: async () => { const c = await pickAgentContent(); if (!c) return; if (!confirm('Restore replaces all flows, intents and other resources of this agent. Continue?')) return; await guard(() => api.post(ctx.agent + ':restore', { agentContent: c }), 'Agent restored'); render(); } }, 'Restore from file (.json / .zip)')));
  main.append(h('h2', null, 'Danger zone'), h('button', { class: 'btn danger', onclick: async () => { if (!confirm(`Delete agent "${a.displayName}"? This cannot be undone.`)) return; await guard(() => api.del(ctx.agent), 'Agent deleted'); go(`/${parentOf(ctx.agent)}/agents`); } }, 'Delete agent'));
}
async function integrationsSection(ctx, rest, main) {
  const envs = await ctx.list('environments');
  let env = '';
  main.append(header('Integrations', 'Connect this agent to websites and backends'));
  const snippet = h('pre'); const curl = h('pre');
  const demo = h('a', { target: '_blank', rel: 'noopener' }, 'Open demo page ↗');
  const draw = () => {
    const envAttr = env ? `\n  environment="${idOf(env)}"` : '';
    snippet.textContent = `<script src="${location.origin}/messenger.js" defer></script>\n<cx-messenger\n  agent="${ctx.agent}"${envAttr}\n  chat-title="${S.agentRes.displayName.replace(/"/g, '')}"\n  language-code="${S.agentRes.defaultLanguageCode || 'en'}">\n</cx-messenger>`;
    curl.textContent = `curl -X POST ${location.origin}/v3/${env || ctx.agent}/sessions/my-session:detectIntent \\\n  -H 'Content-Type: application/json' \\\n  -d '{"queryInput":{"text":{"text":"hi"},"languageCode":"${S.agentRes.defaultLanguageCode || 'en'}"}}'`;
    demo.href = `/messenger/demo?agent=${encodeURIComponent(ctx.agent)}&title=${encodeURIComponent(S.agentRes.displayName)}` + (env ? `&environment=${encodeURIComponent(env)}` : '');
  };
  const copy = el => h('button', { class: 'btn small', onclick: async () => { try { await navigator.clipboard.writeText(el.textContent); toast('Copied'); } catch (e) { toast('Copy failed — select the text manually', true); } } }, 'Copy');
  main.append(h('div', { class: 'card' }, h('div', { class: 'row between' }, h('h2', { style: 'margin:0' }, 'Web messenger'), h('div', { class: 'row' }, h('span', { class: 'muted' }, 'Environment'), sel([['', 'Draft'], ...envs.map(e => [e.name, e.displayName])], '', v => { env = v; draw(); }))),
    h('p', { class: 'muted' }, 'Paste this snippet into any web page to add a chat bubble. It supports text and rich content payloads (chips, buttons, info cards, images).'),
    S.serverInfo && S.serverInfo.apiKeyRequired && !S.serverInfo.publicSessions ? h('p', { class: 'chip warn' }, 'This server requires an API key for all calls. Start it with --public-sessions (CXA_PUBLIC_SESSIONS=1) so the widget can chat without exposing your key.') : null,
    snippet, h('div', { class: 'row' }, copy(snippet), demo)));
  main.append(h('div', { class: 'card' }, h('h2', { style: 'margin-top:0' }, 'REST API'), h('p', { class: 'muted' }, 'Call detectIntent from your backend. Session ids are any string you choose; state is kept per session.'), curl, copy(curl)));
  draw();
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
  document.body.classList.toggle('sim-open', !!(S.sim.open && S.agent));
  $('#toggle-sim').classList.toggle('hidden', !S.agent);
  if (!S.sim.open || !S.agent) return;
  if (S.sim.agent !== S.agent) { S.sim.agent = S.agent; S.sim.session = null; S.sim.log = []; S.sim.state = null; }
  if (!S.sim.session) S.sim.session = 'sim-' + Math.random().toString(36).slice(2, 12);
  sim.innerHTML = '';
  let envs = [];
  try { envs = await api.list(S.agent + '/environments'); } catch (e) { }
  const envSel = sel([['', 'Draft'], ...envs.map(e => [e.name, e.displayName])], S.sim.env, v => { S.sim.env = v; resetSim(); });
  const log = h('div', { class: 'sim-log', 'aria-live': 'polite' });
  const botRow = (...content) => h('div', { class: 'msg-row bot' }, h('span', { class: 'bot-av' }, icon(S.agentRes && S.agentRes.startPlaybook ? 'sparkles' : 'bot', 16)), h('div', { class: 'msg-stack' }, content));
  const drawLog = () => {
    log.innerHTML = '';
    if (!S.sim.log.length) log.append(h('div', { class: 'sim-empty' }, h('div', { class: 'empty-ic' }, icon('chats', 26)), h('b', null, 'Try your agent'), h('div', { class: 'muted' }, 'Type a message below. Every turn shows the matched intent, page and round-trip time.')));
    for (const e of S.sim.log) {
      if (e.user != null) log.append(h('div', { class: 'msg-row user' }, h('div', { class: 'bubble user' }, e.user)));
      if (e.pending) log.append(botRow(h('div', { class: 'bubble bot typing' }, h('i'), h('i'), h('i'))));
      if (e.resp) {
        const qr = e.resp.queryResult; const parts = [];
        for (const m of qr.responseMessages || []) {
          if (m.text) parts.push(h('div', { class: 'bubble bot' }, m.text.text.join(' ')));
          else if (m.payload && m.payload.richContent) parts.push(h('div', { class: 'chips-row' }, m.payload.richContent.flat().filter(x => x.type === 'chips').flatMap(x => x.options || []).map(o => h('button', { class: 'chip-btn', onclick: () => { input.value = o.text; send(); } }, o.text))));
          else if (m.payload) parts.push(h('div', { class: 'bubble bot' }, h('code', null, JSON.stringify(m.payload))));
          else if (m.toolCall) parts.push(h('div', { class: 'bubble sys' }, icon('tool', 14), ' client tool call: ' + idOf(m.toolCall.tool) + ' ' + JSON.stringify(m.toolCall.inputParameters || {})));
          else if (m.liveAgentHandoff) parts.push(h('div', { class: 'bubble sys' }, icon('external', 14), ' live agent handoff'));
          else if (m.endInteraction) parts.push(h('div', { class: 'bubble sys' }, 'conversation ended'));
        }
        parts.push(h('button', { class: 'meta', title: 'Show execution details', onclick: () => modal('Execution details', h('pre', null, JSON.stringify(qr.diagnosticInfo, null, 2)), [['Close', 'primary', () => true]]) },
          icon('info', 13), h('span', null, metaLine(qr)), e.ms != null ? h('span', { class: 'ms' }, e.ms + ' ms') : null));
        log.append(botRow(...parts));
      }
      if (e.err) log.append(h('div', { class: 'bubble sys err' }, icon('alert', 14), ' ' + e.err));
    }
    log.scrollTop = log.scrollHeight;
  };
  const input = h('input', { placeholder: 'Message your agent…', 'aria-label': 'Message', enterkeyhint: 'send', onkeydown: e => { if (e.key === 'Enter') send(); } });
  const state = h('details', { class: 'sim-state' });
  const drawState = () => {
    const qr = S.sim.state; state.innerHTML = '';
    const n = qr ? Object.keys(qr.parameters || {}).length : 0;
    state.append(h('summary', null, icon('code', 14), ` Session parameters (${n})`));
    state.append(qr ? h('pre', null, JSON.stringify(qr.parameters || {}, null, 2)) : h('div', { class: 'muted' }, 'Parameters appear after the first turn.'));
  };
  async function send(body) {
    const text = input.value; if (!body && !text.trim()) return;
    input.value = '';
    const base = S.sim.env ? S.sim.env : S.agent;
    const entry = { user: body ? null : text, pending: true }; S.sim.log.push(entry); drawLog();
    const t0 = performance.now();
    try {
      const resp = await api.post(`${base}/sessions/${S.sim.session}:detectIntent`, body || { queryInput: { text: { text }, languageCode: (S.agentRes || {}).defaultLanguageCode || 'en' } });
      entry.resp = resp; entry.ms = Math.round(performance.now() - t0); S.sim.state = resp.queryResult;
    } catch (e) { entry.err = e.message; }
    entry.pending = false;
    drawLog(); drawState(); input.focus();
  }
  function resetSim() { S.sim.session = null; S.sim.log = []; S.sim.state = null; renderSim(); }
  const evBtn = h('button', { class: 'iconbtn small', title: 'Send an event', 'aria-label': 'Send an event', onclick: async () => { const ev = prompt('Event name', 'sys.no-input-default'); if (ev) { S.sim.log.push({ user: '[event] ' + ev }); await send({ queryInput: { event: { event: ev } } }); S.sim.log.splice(S.sim.log.length - 2, 1); drawLog(); } } }, icon('bolt', 18));
  const saveBtn = h('button', { class: 'iconbtn small', title: 'Save conversation as a test case', 'aria-label': 'Save as test case', onclick: async () => { const n = prompt('Test case name', 'Simulator conversation'); if (!n) return; const r = await guard(() => api.post(S.agent + ':saveTestCase', { session: (S.sim.env || S.agent) + '/sessions/' + S.sim.session, displayName: n }), 'Saved as test case'); if (r) go('/' + r.name); } }, icon('flask', 18));
  sim.append(
    h('div', { class: 'sim-head' },
      h('div', { class: 'sim-title' }, avatar((S.agentRes || {}).displayName, (S.agentRes || {}).startPlaybook ? 'sparkles' : 'bot'), h('div', null, h('b', null, (S.agentRes || {}).displayName || 'Simulator'), h('small', { class: 'muted' }, 'Simulator'))),
      h('div', { class: 'row tight' }, evBtn, saveBtn,
        h('button', { class: 'iconbtn small', title: 'New session', 'aria-label': 'Reset conversation', onclick: resetSim }, icon('refresh', 18)),
        h('button', { class: 'iconbtn small', title: 'Close', 'aria-label': 'Close simulator', onclick: () => { S.sim.open = false; renderSim(); } }, icon('x', 18)))),
    h('div', { class: 'sim-env' }, icon('layers', 14), h('span', { class: 'muted' }, 'Environment'), envSel),
    log, state,
    h('div', { class: 'sim-input' }, input, h('button', { class: 'send-btn', 'aria-label': 'Send', onclick: () => send() }, icon('send', 18))));
  drawLog(); drawState(); input.focus();
}
