"use strict";
const { invoke } = window.__TAURI__.core;

async function load() {
  const s = await invoke("get_settings");
  document.getElementById("hotkey").value = s.hotkey;
  document.getElementById("theme").value = s.theme;
  document.getElementById("window_width").value = s.window_width;
  document.getElementById("window_height").value = s.window_height;
}

document.getElementById("settings-form").onsubmit = async (ev) => {
  ev.preventDefault();
  const settings = {
    hotkey: document.getElementById("hotkey").value,
    theme: document.getElementById("theme").value,
    window_width: parseInt(document.getElementById("window_width").value, 10),
    window_height: parseInt(document.getElementById("window_height").value, 10),
  };
  await invoke("set_settings", { settings });
  alert("Saved. Restart Coati to apply hotkey changes.");
};

document.addEventListener("DOMContentLoaded", load);
