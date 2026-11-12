/* ==========================================================================
   SupportOS+ Design Lab — "BEFORE" replica behaviour
   Recreates the CURRENT interaction feel, including the documented
   misfeatures, so the contrast with the reskin is visible:
     - conversation click triggers the thread card to visibly re-sync
       (stand-in for the 5-6 synchronous with_read queries on the UI thread,
       inbox.rs:526/551/574/624/676)
     - composer drafts die when you switch conversations (inbox.rs:373-390)
     - thread read through the 300px keyhole (inbox.rs:2306-2314)
     - AI badge identical to Agent badge (inbox.rs:2871 vs 2875)
     - assignee nowhere in the list (inbox.rs:121), raw UTC stamps
       (compact_time, inbox.rs:2797)
   ========================================================================== */
(() => {
  "use strict";
  const LAB = window.LAB;
  const $ = (sel) => document.querySelector(sel);

  const state = {
    theme: "light",
    status: "all",
    query: "",
    selected: null,
    composerMode: "reply",
    draft: "",
    page: "inbox",
  };
  const PAGES_LIST = LAB.NAV_GROUPS.flatMap(([, ks]) => ks);

  /* ----- icons ------------------------------------------------------------ */
  $("#sb-search-ic").innerHTML = LAB.ICONS.search;
  document.querySelector(".ic-search").innerHTML = LAB.ICONS.search;
  document.querySelector(".ic-search2").innerHTML = LAB.ICONS.search;
  document.querySelector(".ic-bell").innerHTML = LAB.ICONS.bell;
  document.querySelectorAll(".ic-caret").forEach((el) => (el.innerHTML = LAB.ICONS.chevronDown));
  $("#theme-ic").innerHTML = LAB.ICONS.moon;

  /* ----- sidebar nav (all 23 pages, hash router) ---------------------------- */
  const nav = $("#sb-nav");
  function renderNav(active) {
    nav.innerHTML = LAB.NAV_GROUPS.map(([group, keys]) =>
      `<div class="nav-group-label">${group}</div>` + keys.map((k) =>
        `<button class="nav-item${k === active ? " active" : ""}" data-page="${k}" type="button">${LAB.ICONS[k]}<span>${LAB.NAV_LABELS[k]}</span></button>`
      ).join("")
    ).join("");
  }
  nav.addEventListener("click", (e) => {
    const b = e.target.closest(".nav-item");
    if (b) go(b.dataset.page);
  });
  document.querySelectorAll("[data-goto]").forEach((b) =>
    b.addEventListener("click", () => go(b.dataset.goto)));
  function go(page) {
    if (("#" + page) === location.hash) route();
    else location.hash = "#" + page;
  }
  function route() {
    let p = (location.hash || "#inbox").slice(1);
    if (!PAGES_LIST.includes(p)) p = "inbox";
    state.page = p;
    renderNav(p);
    const isInbox = p === "inbox";
    const ip = document.getElementById("inbox-page");
    const pr = document.getElementById("page-root");
    if (ip) ip.hidden = !isInbox;
    if (pr) {
      pr.hidden = isInbox;
      if (!isInbox) window.LAB_PAGES.render(p, pr);
    }
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

  /* ----- theme toggle ------------------------------------------------------ */
  $("#theme-toggle").addEventListener("click", () => {
    state.theme = state.theme === "light" ? "dark" : "light";
    document.body.dataset.theme = state.theme;
    $("#theme-ic").innerHTML = state.theme === "light" ? LAB.ICONS.moon : LAB.ICONS.sun;
    $("#theme-label").textContent = state.theme === "light" ? "switch to dark theme" : "switch to light theme";
  });

  /* ----- status tabs -------------------------------------------------------- */
  const tabs = ["all", "active", "pending", "closed"];
  const tabLabels = { all: "All", active: "Active", pending: "Pending", closed: "Closed" };
  function renderTabs() {
    $("#seg-tabs").innerHTML = tabs
      .map((t) => `<button class="seg-tab${state.status === t ? " active" : ""}" role="tab" data-tab="${t}" type="button">${tabLabels[t]}</button>`)
      .join("");
  }
  $("#seg-tabs").addEventListener("click", (e) => {
    const b = e.target.closest("[data-tab]");
    if (!b) return;
    state.status = b.dataset.tab;
    state.selected = null;
    renderTabs();
    renderList();
    renderDetail();
  });

  $("#reset-filters").addEventListener("click", () => {
    state.status = "all";
    state.query = "";
    $("#q").value = "";
    $("#tag").value = "";
    state.selected = null;
    renderTabs();
    renderList();
    renderDetail();
  });
  $("#q").addEventListener("input", () => {
    state.query = $("#q").value.trim().toLowerCase();
    renderList();
  });

  /* ----- conversation list (66px rows, badge + meta + preview + UTC stamp) -- */
  function statusTone(status) {
    if (status === "active") return ["info", "Active"];
    if (status === "pending") return ["warning", "Pending"];
    return ["neutral", "Closed"];
  }
  function filtered() {
    return LAB.conversations.filter((c) => {
      if (state.status !== "all" && c.status !== state.status) return false;
      if (state.query && !(c.subject + " " + c.customer).toLowerCase().includes(state.query)) return false;
      return true;
    });
  }
  function renderList() {
    const rows = filtered();
    const n = rows.length;
    $("#page-sub").textContent = `${n} of ${LAB.conversations.length} conversations`;
    $("#list").innerHTML = rows
      .map((c) => {
        const [tone, label] = statusTone(c.status);
        const sel = state.selected === c.id ? " selected" : "";
        return `<div class="row${sel}" data-id="${c.id}" role="listitem" tabindex="0">
          <div class="r1"><span class="subject">#${c.number}&nbsp; ${c.subject}</span><span class="badge ${tone}">${label}</span></div>
          <div class="r2">${c.customer} · ${c.mailbox}</div>
          <div class="r3"><span class="preview">${c.preview}</span><span class="time">${c.updatedAt}</span></div>
        </div>`;
      })
      .join("");
  }
  $("#list").addEventListener("click", (e) => {
    const row = e.target.closest(".row");
    if (row) select(Number(row.dataset.id));
  });
  $("#list").addEventListener("keydown", (e) => {
    const row = e.target.closest(".row");
    if (row && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      select(Number(row.dataset.id));
    }
  });

  /* ----- selection: the "sync burst" + draft loss --------------------------- */
  function select(id) {
    if (state.selected !== id) {
      state.draft = ""; // drafts are not persisted (inbox.rs:373-390)
      state.composerMode = "reply";
    }
    state.selected = id;
    renderList();
    renderDetail(true);
  }

  /* ----- detail pane --------------------------------------------------------- */
  function badge(tone, text) {
    return `<span class="badge ${tone}">${text}</span>`;
  }
  function entryHTML(m) {
    // Faithful mapping of thread_entry_ui (inbox.rs:2869-2878):
    // Agent, AI, Chat, Phone all share the same Info-blue badge tone.
    let tone = "neutral", label = "Other";
    if (m.kind === "customer") [tone, label] = ["success", "Customer"];
    else if (m.kind === "agent") [tone, label] = ["info", "Agent"];
    else if (m.kind === "note") [tone, label] = ["warning", "Note"];
    else if (m.kind === "ai") [tone, label] = ["info", "AI"]; // same tone as Agent
    const quoted = (m.quoted || [])
      .map((q) => `<div class="t-quote">${q.from} — ${q.date}\n${q.body}</div>`)
      .join("");
    return `<div class="t-entry">
      <div class="t-head">${badge(tone, label)}<span class="t-actor">${m.author}</span><span class="t-time">${m.ts}</span></div>
      <div class="t-body">${m.body.map((p) => escapeHTML(p)).join("\n\n")}</div>
      ${quoted}
    </div><div class="t-divider"></div>`;
  }
  function escapeHTML(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function copilotHTML(cp) {
    if (!cp) return "";
    const urgencyTone = cp.urgency === "high" ? "danger" : cp.urgency === "medium" ? "warning" : "neutral";
    const sentTone = cp.sentiment === "negative" ? "warning" : cp.sentiment === "positive" ? "success" : "neutral";
    const sims = cp.similar
      .map(
        (s) => `<div class="sim-row">
          <div class="sim-head">#${s.number} ${s.subject} ${badge("info", s.score + "%")}</div>
          <div class="sim-res">resolved with: ${s.resolution}</div>
          <div class="sim-why">why: ${s.why}</div>
        </div>`
      )
      .join("");
    return `<div class="card">
      <div class="card-h">Copilot <span class="card-sub">local · no configuration needed</span></div>
      <div class="card-b">
        <div class="chip-row">
          ${badge("info", cp.intent)}
          ${badge(urgencyTone, "urgency: " + cp.urgency)}
          ${badge(sentTone, "sentiment: " + cp.sentiment)}
          ${cp.escalation ? badge("danger", "escalation risk") : ""}
        </div>
        <div class="field-label">What the customer needs</div>
        ${cp.summary.map((s) => `<div class="bullet">• ${s}</div>`).join("")}
        <div class="field-label">Open questions</div>
        ${cp.questions.map((q) => `<div class="q">? ${q}</div>`).join("")}
        <div class="hint">History: ${cp.history}</div>
        <div class="field-label">Similar past issues</div>
        ${sims}
      </div>
    </div>`;
  }

  function detailHTML(c) {
    const triage = c.triage
      .map((t) => {
        let tone = "info";
        if (t.label === "urgency" && (t.value === "high" || t.value === "critical")) tone = "danger";
        else if ((t.label === "urgency" && t.value === "moderate") || (t.label === "sentiment" && t.value === "negative")) tone = "warning";
        else if (t.label === "escalation signal") tone = "danger";
        const conf = t.conf || "rules";
        const text = conf === "heuristic" ? `${t.label}: ${t.value}` : `${t.label}: ${t.value} · ${conf}`;
        return badge(tone, text);
      })
      .join("");
    const brief = c.briefing.length
      ? `<div class="card">
          <div class="card-h">Know before you reply <span class="card-sub">durable customer facts extracted from past conversations</span></div>
          <div class="card-b">
            <div class="chip-row">${LAB.ICONS.sparkles} ${badge("info", "AI")}</div>
            ${c.briefing.map((f) => `<div class="bullet">· ${f.key}: ${f.value}</div>`).join("")}
          </div>
        </div>`
      : "";
    const statuses = ["active", "pending", "closed", "spam"]
      .map((s) => `<button class="chip${c.status === s ? " on" : ""}" data-status="${s}" type="button">${s}</button>`)
      .join("");
    const mode = state.composerMode;
    const composer = `<div class="card composer">
      <div class="card-h">${mode === "reply" ? "Reply to customer" : "Internal note"}</div>
      <div class="card-b">
        <div class="chip-row">
          <button class="chip${mode === "reply" ? " on" : ""}" data-mode="reply" type="button">${LAB.ICONS.send} Reply</button>
          <button class="chip${mode === "note" ? " on" : ""}" data-mode="note" type="button">${LAB.ICONS.pencil} Internal note</button>
        </div>
        <textarea id="composer-body" placeholder="${mode === "reply"
            ? "Type your reply to the customer… (Ctrl+Enter sends)"
            : "Type an internal note (visible only to your team) — @ to mention (Ctrl+Enter sends)"}">${escapeHTML(state.draft)}</textarea>
        <div class="composer-foot">
          <div class="chip-row" style="margin:0">
            <button class="chip" type="button" title="run the 9 deterministic coaching checks — advisory only">${LAB.ICONS.sparkles} Review draft</button>
          </div>
          <div style="display:flex;align-items:center;gap:10px">
            <span class="send-hint">Ctrl+Enter sends</span>
            <button class="send-btn" id="send" type="button">${LAB.ICONS.send} Send</button>
          </div>
        </div>
        <div id="composer-status"></div>
      </div>
    </div>`;

    return `
      <div class="detail-hint">${c.customer} &lt;${c.email}&gt; · mailbox: ${c.mailbox} · priority: ${c.priority}</div>
      <div class="triage-row">${triage}</div>
      ${brief}
      <div class="card">
        <div class="card-b">
          <div class="chip-row"><span class="field-inline">status:</span>${statuses}</div>
          <div class="chip-row"><span class="field-inline">assign:</span>
            <button class="combo" type="button" style="min-width:200px;height:26px">Unassigned ${"" /* combobox, faithful to the single assign dropdown */}
            <span class="ic-caret"></span></button>
          </div>
        </div>
      </div>
      ${copilotHTML(c.copilot)}
      <div class="card">
        <div class="card-h">Thread</div>
        <div class="card-b"><div class="thread-scroll" id="thread"></div></div>
      </div>
      ${composer}`;
  }

  function renderDetail(withSyncFlash) {
    const pane = $("#detail");
    const c = LAB.conversations.find((x) => x.id === state.selected);
    if (!c) {
      pane.innerHTML = `<div class="empty-state">${LAB.ICONS.image}<div class="t">Select a conversation</div><div class="s">Pick a conversation from the list to read the thread and reply.</div></div>`;
      return;
    }
    pane.innerHTML = detailHTML(c);

    // The thread region re-syncs on every selection — a visible stand-in for
    // the synchronous query burst the real app runs on the UI thread.
    const thread = pane.querySelector("#thread");
    if (withSyncFlash) {
      thread.innerHTML = `<div class="syncing">Syncing thread…</div>`;
      setTimeout(() => {
        if (state.selected === c.id) thread.innerHTML = c.messages.map(entryHTML).join("");
      }, 220);
    } else {
      thread.innerHTML = c.messages.map(entryHTML).join("");
    }

    // composer behaviour
    const body = pane.querySelector("#composer-body");
    body.addEventListener("input", () => (state.draft = body.value));
    body.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) pane.querySelector("#send").click();
    });
    pane.querySelectorAll("[data-mode]").forEach((b) =>
      b.addEventListener("click", () => {
        state.composerMode = b.dataset.mode;
        renderDetail(false);
      })
    );
    pane.querySelectorAll("[data-status]").forEach((b) =>
      b.addEventListener("click", () => {
        c.status = b.dataset.status;
        renderList();
        renderDetail(false);
      })
    );
    const send = pane.querySelector("#send");
    send.addEventListener("click", () => {
      const statusEl = pane.querySelector("#composer-status");
      if (!state.draft.trim()) {
        statusEl.innerHTML = `<div class="status-err">Nothing to send — the composer is empty.</div>`;
        return;
      }
      statusEl.innerHTML = `<div class="status-ok">${state.composerMode === "reply" ? "Reply queued as a durable outbound job — delivered to Help Scout." : "Internal note saved."}</div>`;
      state.draft = "";
      setTimeout(() => {
        if (state.selected === c.id && statusEl.isConnected) statusEl.innerHTML = "";
      }, 3200);
    });
  }

  function flashStatus(msg, isErr) {
    const pane = $("#detail");
    let el = pane.querySelector("#composer-status");
    if (!el) return;
    el.innerHTML = `<div class="${isErr ? "status-err" : "status-ok"}">${msg}</div>`;
    setTimeout(() => {
      if (el.isConnected) el.innerHTML = "";
    }, 2600);
  }

  renderTabs();
  renderList();
  renderDetail();
  route();
})();
