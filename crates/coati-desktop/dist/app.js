"use strict";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let conversationId = null;
let currentAssistantEl = null;
let currentAssistantText = "";

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
}

function addMessage(role, content) {
  const msgs = document.getElementById("messages");
  const m = el("div", "msg " + role, content);
  msgs.appendChild(m);
  msgs.scrollTop = msgs.scrollHeight;
  return m;
}

function looksLikeProposal(text) {
  const m = text.match(/^\s*PROPOSE:\s*(.+)$/m);
  return m ? m[1].trim() : null;
}

function renderProposal(full) {
  const cmd = looksLikeProposal(full);
  if (!cmd) return null;
  const isSudo = /^\s*sudo(\s|$)/.test(cmd);
  const box = el("div", "proposal" + (isSudo ? " sudo" : ""));
  box.appendChild(el("div", null, "Proposed command:"));
  const code = el("code");
  code.textContent = cmd;
  box.appendChild(code);
  const actions = el("div", "proposal-actions");
  const no = el("button", "btn-no", isSudo ? "No (default)" : "No");
  const yes = el("button", "btn-yes", isSudo ? "Yes, run as sudo" : "Yes, run");
  actions.appendChild(no);
  actions.appendChild(yes);
  box.appendChild(actions);
  no.onclick = () => { box.remove(); };
  yes.onclick = async () => {
    no.disabled = true;
    yes.disabled = true;
    try {
      const res = await invoke("run_proposal", { command: cmd, confirmed: true });
      const out = el("pre");
      out.textContent = `exit ${res.exit_code}\n${res.stdout}${res.stderr}`;
      box.appendChild(out);
    } catch (e) {
      const err = el("pre");
      err.textContent = String(e);
      box.appendChild(err);
    }
  };
  return box;
}

async function send() {
  const input = document.getElementById("input");
  const q = input.value.trim();
  if (!q) return;
  input.value = "";

  if (!conversationId) {
    conversationId = await invoke("create_conversation", { title: q.slice(0, 40) });
    await refreshConversations();
  }

  addMessage("user", q);
  currentAssistantEl = addMessage("assistant", "");
  currentAssistantText = "";

  try {
    await invoke("send_stream", { question: q, conversationId });
  } catch (e) {
    currentAssistantEl.textContent = "Error: " + String(e);
  }
}

async function boot() {
  const models = await invoke("list_models").catch(() => []);
  const sel = document.getElementById("model");
  clear(sel);
  for (const m of models) {
    const opt = document.createElement("option");
    opt.value = m.name;
    opt.textContent = m.name;
    sel.appendChild(opt);
  }

  document.getElementById("send").onclick = send;
  document.getElementById("input").addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      send();
    }
  });

  document.getElementById("new-conv").onclick = newConversation;
  await refreshConversations();

  await listen("coati://chunk", (ev) => {
    if (!currentAssistantEl) return;
    currentAssistantText += ev.payload;
    currentAssistantEl.textContent = currentAssistantText;
    const msgs = document.getElementById("messages");
    msgs.scrollTop = msgs.scrollHeight;
  });

  await listen("coati://end", (ev) => {
    if (!currentAssistantEl) return;
    const full = ev.payload || currentAssistantText;
    const proposal = renderProposal(full);
    if (proposal) {
      currentAssistantEl.after(proposal);
    }
    currentAssistantEl = null;
    currentAssistantText = "";
  });

  await listen("coati://error", (ev) => {
    if (currentAssistantEl) {
      currentAssistantEl.textContent = "Error: " + ev.payload;
      currentAssistantEl = null;
    }
  });
}

async function refreshConversations() {
  const list = document.getElementById("conv-list");
  clear(list);
  const rows = await invoke("list_conversations");
  for (const c of rows) {
    const li = el("li", null, c.title);
    if (c.id === conversationId) li.classList.add("active");
    li.onclick = () => loadConversation(c.id);
    list.appendChild(li);
  }
}

async function loadConversation(id) {
  const msgs = document.getElementById("messages");
  clear(msgs);
  conversationId = id;
  const rows = await invoke("load_conversation", { id });
  for (const r of rows) {
    addMessage(r.role, r.content);
  }
  refreshConversations();
}

function newConversation() {
  conversationId = null;
  clear(document.getElementById("messages"));
  refreshConversations();
}

document.addEventListener("DOMContentLoaded", boot);

(() => {
  const banner = document.getElementById("rec-banner");
  const input = document.getElementById("input");
  const sendBtn = document.getElementById("send");

  if (!window.__TAURI__ || !banner) return;
  const { listen } = window.__TAURI__.event;

  function setBanner(text) {
    while (banner.firstChild) banner.removeChild(banner.firstChild);
    const dot = document.createElement("span");
    dot.className = "dot";
    banner.appendChild(dot);
    banner.appendChild(document.createTextNode(" " + text));
    banner.hidden = false;
  }

  listen("voice://recording", () => {
    setBanner("Listening\u2026 release F9 to send");
    if (input) input.disabled = true;
  });

  listen("voice://transcribing", () => {
    setBanner("Transcribing\u2026");
  });

  listen("voice://idle", () => {
    banner.hidden = true;
    if (input) input.disabled = false;
  });

  listen("voice://final", (event) => {
    const text = (event.payload && event.payload.text) || "";
    if (!text.trim()) return;
    if (input) {
      input.disabled = false;
      input.value = text;
      if (sendBtn) sendBtn.click();
    }
  });
})();
