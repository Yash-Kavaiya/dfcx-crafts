/*
 * <cx-messenger> — embeddable chat widget for agents served by this platform.
 *
 *   <script src="https://YOUR-HOST/messenger.js" defer></script>
 *   <cx-messenger agent="projects/p/locations/global/agents/a" chat-title="Support"></cx-messenger>
 *
 * Attributes: agent (required), chat-title, environment (env id or full name),
 * language-code, welcome-event, welcome-text, api-key, api-base, expanded,
 * primary-color.
 * Renders text, and `richContent` payloads in the Dialogflow Messenger format
 * (chips, button, info, description, image, divider, list).
 */
(function () {
  'use strict';
  if (customElements.get('cx-messenger')) return;
  const SCRIPT_BASE = (() => {
    const s = document.currentScript;
    try { return s ? new URL(s.src).origin : location.origin; } catch (e) { return location.origin; }
  })();

  const CSS = `
  :host { all: initial; font: 14px/1.4 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; --p: var(--cx-primary, #1a73e8); }
  .fab { position: fixed; right: 20px; bottom: 20px; width: 56px; height: 56px; border-radius: 50%; border: none; background: var(--p); color: #fff; cursor: pointer; box-shadow: 0 3px 10px rgba(0,0,0,.25); display: grid; place-items: center; z-index: 2147483000; }
  .fab svg { width: 26px; height: 26px; fill: #fff; }
  .win { position: fixed; right: 20px; bottom: 88px; width: min(370px, calc(100vw - 32px)); height: min(560px, calc(100vh - 120px)); background: #fff; color: #202124; border-radius: 12px; box-shadow: 0 6px 24px rgba(0,0,0,.25); display: flex; flex-direction: column; overflow: hidden; z-index: 2147483000; }
  .win[hidden] { display: none; }
  .head { background: var(--p); color: #fff; padding: 12px 14px; display: flex; align-items: center; justify-content: space-between; font-weight: 600; }
  .head button { background: none; border: none; color: #fff; font-size: 18px; cursor: pointer; }
  .log { flex: 1; overflow-y: auto; padding: 12px; display: flex; flex-direction: column; gap: 8px; background: #f8f9fa; }
  .m { max-width: 82%; padding: 8px 12px; border-radius: 14px; white-space: pre-wrap; word-wrap: break-word; }
  .u { align-self: flex-end; background: var(--p); color: #fff; border-bottom-right-radius: 4px; }
  .b { align-self: flex-start; background: #fff; border: 1px solid #e0e0e0; border-bottom-left-radius: 4px; }
  .sys { align-self: center; color: #5f6368; font-size: 12px; }
  .card { align-self: flex-start; background: #fff; border: 1px solid #e0e0e0; border-radius: 10px; max-width: 86%; overflow: hidden; }
  .card .t { font-weight: 600; padding: 8px 12px 2px; } .card .s { color: #5f6368; padding: 0 12px 8px; }
  .card img { display: block; max-width: 100%; }
  .card hr { border: none; border-top: 1px solid #e0e0e0; margin: 0; }
  .chips { align-self: flex-start; display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { border: 1px solid var(--p); color: var(--p); background: #fff; border-radius: 16px; padding: 5px 12px; cursor: pointer; font: inherit; }
  .chip:hover { background: #e8f0fe; }
  .btn { display: block; margin: 0 12px 10px; text-align: center; background: var(--p); color: #fff; border-radius: 6px; padding: 7px; text-decoration: none; cursor: pointer; border: none; font: inherit; width: calc(100% - 24px); }
  .in { display: flex; gap: 6px; padding: 10px; border-top: 1px solid #e0e0e0; background: #fff; }
  .in input { flex: 1; border: 1px solid #dadce0; border-radius: 18px; padding: 8px 12px; font: inherit; outline: none; }
  .in input:focus { border-color: var(--p); }
  .in button { border: none; background: var(--p); color: #fff; border-radius: 18px; padding: 0 14px; cursor: pointer; font: inherit; }
  .typing { align-self: flex-start; color: #5f6368; font-size: 12px; }
  @media (prefers-color-scheme: dark) {
    .win { background: #202124; color: #e8eaed; } .log { background: #171717; }
    .b, .card, .chip, .in { background: #2a2a2a; border-color: #444; color: #e8eaed; }
    .in input { background: #171717; color: #e8eaed; border-color: #444; }
  }`;

  class CxMessenger extends HTMLElement {
    constructor() {
      super();
      this.root = this.attachShadow({ mode: 'open' });
      this.started = false;
    }
    connectedCallback() {
      const pc = this.getAttribute('primary-color');
      if (pc) this.style.setProperty('--cx-primary', pc);
      this.root.innerHTML = `<style>${CSS}</style>
        <button class="fab" part="launcher" aria-label="Open chat"><svg viewBox="0 0 24 24"><path d="M4 4h16v11H8l-4 4z"/></svg></button>
        <div class="win" hidden role="dialog" aria-label="Chat">
          <div class="head"><span class="title"></span><button class="close" aria-label="Close chat">✕</button></div>
          <div class="log" aria-live="polite"></div>
          <form class="in"><input placeholder="Ask something…" aria-label="Message"><button>Send</button></form>
        </div>`;
      this.$ = s => this.root.querySelector(s);
      this.$('.title').textContent = this.getAttribute('chat-title') || 'Chat';
      this.$('.fab').onclick = () => this.toggle();
      this.$('.close').onclick = () => this.toggle(false);
      this.$('form').onsubmit = e => { e.preventDefault(); const v = this.$('input').value.trim(); if (v) { this.$('input').value = ''; this.sendText(v); } };
      if (this.hasAttribute('expanded')) this.toggle(true);
    }
    get sessionId() {
      const key = 'cx-messenger:' + this.getAttribute('agent');
      let id = null;
      try { id = sessionStorage.getItem(key); } catch (e) { /* storage blocked */ }
      if (!id) { id = 'web-' + Math.random().toString(36).slice(2, 12); try { sessionStorage.setItem(key, id); } catch (e) { } }
      return id;
    }
    endpoint() {
      const base = (this.getAttribute('api-base') || SCRIPT_BASE).replace(/\/$/, '');
      const agent = this.getAttribute('agent');
      const env = this.getAttribute('environment');
      const scope = env ? (env.includes('/') ? env : `${agent}/environments/${env}`) : agent;
      return `${base}/v3/${scope}/sessions/${this.sessionId}:detectIntent`;
    }
    toggle(open) {
      const w = this.$('.win');
      const show = open === undefined ? w.hidden : open;
      w.hidden = !show;
      if (show) {
        this.$('input').focus();
        if (!this.started) {
          this.started = true;
          const ev = this.getAttribute('welcome-event'); const wt = this.getAttribute('welcome-text');
          if (ev) this.query({ event: { event: ev } }, null);
          else if (wt) this.query({ text: { text: wt } }, null);
        }
      }
      this.dispatchEvent(new CustomEvent(show ? 'cx-messenger-opened' : 'cx-messenger-closed', { bubbles: true, composed: true }));
    }
    add(cls, text) {
      const d = document.createElement('div'); d.className = cls; d.textContent = text;
      this.$('.log').append(d); this.$('.log').scrollTop = 1e9; return d;
    }
    sendText(text) { this.add('m u', text); this.query({ text: { text } }, text); }
    async query(input, text) {
      input.languageCode = this.getAttribute('language-code') || 'en';
      const typing = this.add('typing', '…');
      const headers = { 'Content-Type': 'application/json' };
      const key = this.getAttribute('api-key'); if (key) headers.Authorization = 'Bearer ' + key;
      try {
        const r = await fetch(this.endpoint(), { method: 'POST', headers, body: JSON.stringify({ queryInput: input }) });
        const data = await r.json();
        typing.remove();
        if (!r.ok) throw new Error((data.error && data.error.message) || r.statusText);
        this.render(data.queryResult || {});
        this.dispatchEvent(new CustomEvent('cx-response-received', { detail: data, bubbles: true, composed: true }));
      } catch (e) {
        typing.remove(); this.add('sys', 'Something went wrong: ' + e.message);
      }
    }
    render(qr) {
      for (const m of qr.responseMessages || []) {
        if (m.text) this.add('m b', (m.text.text || []).join(' '));
        else if (m.payload && m.payload.richContent) this.rich(m.payload.richContent);
        else if (m.liveAgentHandoff) this.add('sys', 'Connecting you to an agent…');
        else if (m.endInteraction) { this.add('sys', 'Conversation ended'); this.started = false; try { sessionStorage.removeItem('cx-messenger:' + this.getAttribute('agent')); } catch (e) { } }
      }
    }
    rich(rc) {
      for (const group of rc) {
        const card = document.createElement('div'); card.className = 'card'; let used = false;
        for (const el of group) {
          if (el.type === 'chips') {
            const c = document.createElement('div'); c.className = 'chips';
            for (const o of el.options || []) { const b = document.createElement('button'); b.className = 'chip'; b.textContent = o.text; b.onclick = () => o.link ? window.open(o.link, '_blank', 'noopener') : this.sendText(o.text); c.append(b); }
            this.$('.log').append(c);
          } else if (el.type === 'info' || el.type === 'description') {
            const t = document.createElement('div'); t.className = 't'; t.textContent = el.title || '';
            const s = document.createElement('div'); s.className = 's'; s.textContent = Array.isArray(el.text) ? el.text.join('\n') : (el.subtitle || el.text || '');
            if (el.actionLink) { t.style.cursor = 'pointer'; t.onclick = () => window.open(el.actionLink, '_blank', 'noopener'); }
            card.append(t, s); used = true;
          } else if (el.type === 'image') {
            const i = document.createElement('img'); i.src = (el.rawUrl || ''); i.alt = el.accessibilityText || ''; card.append(i); used = true;
          } else if (el.type === 'button') {
            const b = document.createElement('button'); b.className = 'btn'; b.textContent = el.text || 'Open';
            b.onclick = () => el.link ? window.open(el.link, '_blank', 'noopener') : (el.event ? this.query({ event: { event: el.event.event || el.event } }, null) : null);
            card.append(b); used = true;
          } else if (el.type === 'divider') { card.append(document.createElement('hr')); used = true; }
          else if (el.type === 'list') {
            const t = document.createElement('div'); t.className = 't'; t.textContent = el.title || ''; t.style.cursor = 'pointer';
            t.onclick = () => this.sendText(el.title || ''); card.append(t); used = true;
          }
        }
        if (used) this.$('.log').append(card);
      }
      this.$('.log').scrollTop = 1e9;
    }
  }
  customElements.define('cx-messenger', CxMessenger);
})();
