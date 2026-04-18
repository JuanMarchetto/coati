"use strict";

const { invoke } = window.__TAURI__.core;

function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
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
}

document.addEventListener("DOMContentLoaded", boot);
