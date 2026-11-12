/* ==========================================================================
   SupportOS+ Design Lab — DS-4 conformed app shell (design-lab-v4)
   --------------------------------------------------------------------------
   This file is SHELL ONLY: sidebar nav, hash routing, theme toggle, the
   per-page notes drawer, and the global inbox keyboard shortcuts (j/k, /,
   Ctrl+Enter). Every page — Inbox included — renders through the DS-4
   engine in ds.js: page = PAGE_DATA (pages-ds.js) + one archetype call
   (DS.renderPage). There is no page markup and no page CSS here.
   ========================================================================== */
(() => {
  "use strict";
  const LAB = window.LAB;
  const DS = window.DS;
  const $ = (sel) => document.querySelector(sel);

  const state = {
    theme: document.body.dataset.theme || "light",
    page: "inbox",
    notesOn: false,
    notesPage: "inbox",
  };
  const PAGES_LIST = LAB.NAV_GROUPS.flatMap(([, ks]) => ks);
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  /* ----- sidebar nav (NAV_GROUPS verbatim from data.js) ---------------------- */
  function renderNav(active) {
    $("#sb-nav").innerHTML = LAB.NAV_GROUPS.map(([group, keys]) => {
      const items = keys
        .map((k) => `<button class="nav-item${k === active ? " active" : ""}" data-page="${k}" type="button">${LAB.ICONS[k]}<span>${LAB.NAV_LABELS[k]}</span></button>`)
        .join("");
      return `<div class="nav-group">${group}</div>${items}`;
    }).join("");
  }
  $("#sb-nav").addEventListener("click", (e) => {
    const b = e.target.closest(".nav-item");
    if (b) go(b.dataset.page);
  });
  document.querySelectorAll("[data-goto]").forEach((b) =>
    b.addEventListener("click", () => go(b.dataset.goto)));
  function go(page) {
    if (("#" + page) === location.hash) route();
    else location.hash = "#" + page;
  }

  /* ----- routing — every page through the DS engine --------------------------- */
  function route() {
    let p = (location.hash || "#inbox").slice(1);
    if (!PAGES_LIST.includes(p)) p = "inbox";
    state.page = p;
    renderNav(p);
    const isInbox = p === "inbox";
    const ip = document.getElementById("inbox-page");
    const pr = document.getElementById("page-root");
    if (ip) ip.hidden = !isInbox;
    if (pr) pr.hidden = isInbox;
    DS.renderPage(p, isInbox ? ip : pr);
    if (state.notesPage !== p) closeNotes();
    if (window.parent !== window) {
      try { window.parent.postMessage({ type: "lab-nav", page: p }, "*"); } catch (_) {}
    }
  }
  window.addEventListener("hashchange", route);
  window.addEventListener("message", (e) => {
    const d = e.data;
    if (d && d.type === "lab-nav" && d.page && d.page !== state.page) {
      if (location.hash !== "#" + d.page) location.hash = "#" + d.page;
      else route();
    }
  });

  /* ----- theme ----------------------------------------------------------------- */
  $("#theme-toggle").addEventListener("click", () => {
    state.theme = state.theme === "light" ? "dark" : "light";
    document.body.dataset.theme = state.theme;
    $("#theme-ic").innerHTML = state.theme === "light" ? LAB.ICONS.moon : LAB.ICONS.sun;
    $("#theme-label").textContent = state.theme === "light" ? "switch to dark theme" : "switch to light theme";
  });

  /* ----- notes drawer (per page; same receipt style as the Inbox notes) -------- */
  function renderNotes(page) {
    const isInbox = page === "inbox";
    const notes = isInbox ? LAB.NOTES : (LAB.PAGE_NOTES[page] || []);
    const title = isInbox ? "Design notes — what changed and why" : `Reskin notes — ${LAB.NAV_LABELS[page] || page}`;
    const sub = isInbox ? "Every pin cites the forensic receipt it fixes." : "Same receipt style as the Inbox notes: FIX / WAS / file:line.";
    const tt = document.querySelector(".nd-title");
    if (tt) tt.textContent = title;
    const ts = document.querySelector(".nd-sub");
    if (ts) ts.textContent = sub;
    const countEl = document.getElementById("notes-count");
    if (countEl) countEl.textContent = notes.length;
    $("#nd-list").innerHTML = notes.map((n, i) =>
      `<div class="nd-item" id="note-${isInbox ? n.n : "p" + i}">
        <span class="nd-pin">${isInbox ? n.n : i + 1}</span>
        <div>
          <div class="nd-title2">${esc(n.title)}</div>
          <div class="nd-fix">${esc(n.fix)}</div>
          <div class="nd-was"><b>WAS</b> — ${esc(n.was)}</div>
          <span class="nd-receipt">${esc(n.receipt)}</span>
        </div>
      </div>`).join("");
  }
  function openNotes(page) {
    state.notesPage = page;
    renderNotes(page);
    state.notesOn = true;
    document.body.classList.add("notes-on");
    const t = document.getElementById("notes-toggle");
    if (t) t.setAttribute("aria-pressed", "true");
  }
  function closeNotes() {
    state.notesOn = false;
    state.notesPage = state.page;
    document.body.classList.remove("notes-on");
    const t = document.getElementById("notes-toggle");
    if (t) t.setAttribute("aria-pressed", "false");
  }
  /* delegation — the engine re-renders page DOM, so bind once on document */
  document.addEventListener("click", (e) => {
    if (e.target.closest(".notes-toggle")) {
      if (state.notesOn) closeNotes();
      else openNotes(state.page === "inbox" ? "inbox" : state.page);
    }
  });
  window.addEventListener("lab-open-notes", (e) => openNotes(e.detail.page));
  $("#notes-close").addEventListener("click", closeNotes);
  document.addEventListener("keydown", (e) => { if (e.key === "Escape") { DS.panel.close(); closeNotes(); } });

  /* ----- global inbox keyboard shortcuts (engine exposes the state API) -------- */
  document.addEventListener("keydown", (e) => {
    if (state.page !== "inbox") return;
    const tag = (e.target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea") return;
    if (e.key === "/") {
      e.preventDefault();
      const f = document.getElementById("conv-filter");
      if (f) f.focus();
      return;
    }
    if (e.key === "j" || e.key === "k") {
      const rows = DS.inboxVisible();
      if (!rows.length) return;
      const sel = DS.inboxState.selected;
      const idx = rows.findIndex((c) => c.id === sel);
      const dir = e.key === "j" ? 1 : -1;
      const next = rows[Math.min(rows.length - 1, Math.max(0, (idx === -1 ? 0 : idx) + dir))];
      DS.inboxOpen(next.id);
      const el = document.querySelector(`.ds-conv-row[data-id="${next.id}"]`);
      if (el) el.scrollIntoView({ block: "nearest" });
    }
  });

  /* ----- boot ------------------------------------------------------------------ */
  const ti = document.getElementById("theme-ic");
  if (ti) ti.innerHTML = state.theme === "light" ? LAB.ICONS.moon : LAB.ICONS.sun;
  const sbi = document.getElementById("sb-search-ic");
  if (sbi) sbi.innerHTML = LAB.ICONS.search || LAB.ICONS.inbox;
  const tbi = document.querySelector(".ic-search");
  if (tbi) tbi.innerHTML = LAB.ICONS.search || LAB.ICONS.inbox;
  const bbi = document.querySelector(".ic-bell");
  if (bbi) bbi.innerHTML = LAB.ICONS.bell;
  const nx = document.getElementById("notes-x");
  if (nx) nx.innerHTML = LAB.ICONS.x;
  renderNotes("inbox");
  route();
})();
