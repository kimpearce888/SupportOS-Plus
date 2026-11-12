/* ==========================================================================
   SupportOS+ Design Lab — "BEFORE" replicas for the 22 non-Inbox pages
   --------------------------------------------------------------------------
   Every string is copied from the forensic specs (.extract/*.md, each cites
   file:line). Quirks are replicated on purpose (raw UTC stamps, the
   Load/Pressure swap, the seven-zero outreach report, the empty Freshness
   tab, lowercase buttons, "—" placeholders). Presentation-only.
   ========================================================================== */
(() => {
  "use strict";
  const LAB = window.LAB;
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  /* ============ shared builders (faithful egui look) ====================== */
  const badge = (tone, text) => `<span class="badge ${tone}">${esc(text)}</span>`;
  function statusTone(s) {
    const map = { active: ["info", "Active"], pending: ["warning", "Pending"], closed: ["neutral", "Closed"], spam: ["danger", "Spam"], open: ["info", "Open"], waiting: ["warning", "Waiting"], resolved: ["success", "Resolved"], published: ["success", "Published"], draft: ["neutral", "Draft"] };
    return map[s] || ["neutral", "Other"];
  }
  function hdr(root, id, actionsHTML) {
    const m = LAB.PAGE_META[id];
    root.innerHTML = `<div class="page-head"><div><h1>${esc(m.title)}</h1>${m.subtitle ? `<div class="page-sub">${esc(m.subtitle)}</div>` : ""}</div><div class="page-actions">${actionsHTML || ""}</div></div>`;
  }
  function banner(text) { return `<div class="hint-banner">${esc(text)}</div>`; }
  function card(title, sub, body) {
    return `<div class="card">${title ? `<div class="card-h">${esc(title)}${sub ? `<span class="card-sub">${esc(sub)}</span>` : ""}</div>` : ""}<div class="card-b">${body}</div></div>`;
  }
  function statGrid(items) {
    return `<div class="stat-grid">${items.map((s) => `<div class="stat"><div class="l">${esc(s.label)}</div><div class="v">${esc(s.value)}</div>${s.hint ? `<div class="h">${esc(s.hint)}</div>` : ""}</div>`).join("")}</div>`;
  }
  function tbl(headers, rowsHTML) {
    return `<div class="tbl-wrap"><table class="tbl"><thead><tr>${headers.map((h) => `<th>${esc(h)}</th>`).join("")}</tr></thead><tbody>${rowsHTML}</tbody></table></div>`;
  }
  const td = (c, cls) => `<td${cls ? ` class="${cls}"` : ""}>${c}</td>`;
  const tdn = (c) => `<td class="num">${esc(c)}</td>`;
  const tds = (c) => `<td class="strong">${esc(c)}</td>`;
  const tdm = (c) => `<td class="mono muted2">${esc(c)}</td>`;
  function chips(items, activeIdx, act) {
    return `<div class="chip-row">${items.map((c, i) => `<button class="chip${i === activeIdx ? " on" : ""}" data-act="${act}" data-i="${i}" type="button">${esc(c)}</button>`).join("")}</div>`;
  }
  function emptyState(icon, title, sub, slim) {
    return `<div class="empty-state${slim ? " slim" : ""}">${icon}<div class="t">${esc(title)}</div><div class="s">${esc(sub)}</div></div>`;
  }
  const hint = (t) => `<div class="hint">${esc(t)}</div>`;
  const errLine = (t) => `<div class="status-line err">error: ${esc(t)}</div>`;
  const okLine = (t) => `<div class="status-line ok">${esc(t)}</div>`;
  const outlineBtn = (label, act, i) => `<button class="outline-btn" data-act="${act}"${i != null ? ` data-i="${i}"` : ""} type="button">${esc(label)}</button>`;
  const dangerBtn = (label, act, i) => `<button class="danger-btn" data-act="${act}"${i != null ? ` data-i="${i}"` : ""} type="button">${esc(label)}</button>`;
  const primaryBtn = (label, act, i) => `<button class="primary-btn" data-act="${act}"${i != null ? ` data-i="${i}"` : ""} type="button">${esc(label)}</button>`;
  const textBtn = (label, act) => `<button class="text-btn" data-act="${act}" type="button">${esc(label)}</button>`;
  const formRow = (label, control) => `<div class="fg-label">${esc(label)}</div><div class="fg-ctl">${control}</div>`;
  const inputPH = (ph, w) => `<input class="text-input"${w ? ` style="width:${w}px"` : ""} placeholder="${esc(ph)}" aria-label="${esc(ph)}">`;
  const tabbar = (tabs, active, act) => `<div class="seg-tabs tabbar">${tabs.map((t, i) => `<button class="seg-tab${i === active ? " active" : ""}" data-act="${act}" data-i="${i}" role="tab" type="button">${esc(t)}</button>`).join("")}</div>`;
  function lineChart(points, annotation) {
    if (points.length < 2) return `<div class="chart-empty">(not enough data to plot)</div>`;
    const w = 900, h = 180, pad = 12, max = Math.max(...points, 1);
    const px = (i) => pad + (i * (w - 2 * pad)) / (points.length - 1);
    const py = (v) => h - 26 - (v / max) * (h - 52);
    const poly = points.map((p, i) => `${px(i).toFixed(1)},${py(p).toFixed(1)}`).join(" ");
    const area = `${pad},${h - 26} ${poly} ${w - pad},${h - 26}`;
    const dots = points.map((p, i) => `<circle cx="${px(i).toFixed(1)}" cy="${py(p).toFixed(1)}" r="3"/>`).join("");
    return `<div class="chartbox"><svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" role="img" aria-label="${esc(annotation)}">
      <line x1="${pad}" y1="40" x2="${w - pad}" y2="40" class="grid"/><line x1="${pad}" y1="90" x2="${w - pad}" y2="90" class="grid"/><line x1="${pad}" y1="140" x2="${w - pad}" y2="140" class="grid"/>
      <polygon points="${area}" class="area"/><polyline points="${poly}" class="line"/>${dots}</svg>
      <div class="chart-note">${esc(annotation)}</div></div>`;
  }
  function barChart(items, annotation) {
    const max = Math.max(...items.map(([, v]) => v), 1);
    return `<div class="bars">${items.map(([label, v]) => `<div class="bar-row"><span class="bar-label">${esc(label)}</span><div class="bar-track"><i style="width:${Math.max(4, (v / max) * 100)}%"></i></div><span class="bar-val">${esc(v)}</span></div>`).join("")}</div>${annotation ? `<div class="chart-note">${esc(annotation)}</div>` : ""}`;
  }
  function countBars(rows) {
    const max = Math.max(...rows.map((r) => r[1]), 1);
    return rows.map((r) => `<div class="bar-row slim"><span class="bar-label w120">${esc(r[0])}</span><div class="bar-track"><i style="width:${Math.max(3, (r[1] / max) * 100)}%"></i></div><span class="bar-val mono">${esc(r[1])}</span></div>`).join("");
  }
  const kv = (label, value, mono) => `<div class="kv-row">${esc(label)} <span class="${mono === false ? "" : "mono"}">${esc(value)}</span></div>`;
  function selList(items, selectedIdx, act) {
    return `<div class="sel-list">${items.map((it, i) => `<div class="sel-row${i === selectedIdx ? " selected" : ""}" data-act="${act}" data-i="${i}" role="listitem" tabindex="0">${it}</div>`).join("")}</div>`;
  }
  function toggleTheme() {
    const next = document.body.dataset.theme === "light" ? "dark" : "light";
    document.body.dataset.theme = next;
    const ic = document.querySelector("#theme-ic"), lb = document.querySelector("#theme-label");
    if (ic) ic.innerHTML = next === "light" ? LAB.ICONS.moon : LAB.ICONS.sun;
    if (lb) lb.textContent = next === "light" ? "switch to dark theme" : "switch to light theme";
  }
  function syncNote(btn) { // shared "Sync now" behavior: weak note, then nothing (page re-reads anyway)
    let n = btn.parentElement.querySelector(".sync-note");
    if (!n) { n = document.createElement("span"); n.className = "sync-note"; btn.parentElement.appendChild(n); }
    n.textContent = "syncing…";
    setTimeout(() => { n.textContent = "up to date — nothing new from Help Scout"; }, 1200);
  }

  const S = {}; // per-page ui state
  const R = {}; // per-page renderers

  /* ============ Dashboard ================================================= */
  R.dashboard = (root) => {
      hdr(root, "dashboard", `${outlineBtn("Sync now", "sync")}<span class="sync-note"></span>`);
      const d = LAB.PAGES.dashboard;
      let html = card(null, null,
        `<div class="scope-row"><span class="field-inline">Range</span>${["7 days", "30 days", "90 days", "1 year"].map((r, i) => `<button class="chip${i === 1 ? " on" : ""}" type="button">${r}</button>`).join("")}</div>
         <div class="scope-row"><span class="field-inline">Channel</span>${["All channels", "Email", "Chat (Beacon)"].map((r, i) => `<button class="chip${i === 0 ? " on" : ""}" type="button">${r}</button>`).join("")}</div>
         <div class="scope-row"><span class="field-inline">Mailboxes</span><button class="chip on" type="button">All</button><button class="chip" type="button">Support</button><button class="chip" type="button">Billing</button></div>`);
      html += statGrid(d.metrics);
      html += card(d.chart.title, d.chart.hint, lineChart(d.chart.points, `${d.chart.series} · ${d.chart.points[0]} → ${d.chart.points[d.chart.points.length - 1]}`));
      html += `<div class="two-col">${d.breakdowns.slice(0, 2).map((b) => card(b.title, null, countBars(b.rows))).join("")}</div>`;
      html += `<div class="two-col">${d.breakdowns.slice(2, 4).map((b) => card(b.title, b.note || null, countBars(b.rows))).join("")}</div>`;
      html += `<div class="two-col">${card(d.breakdowns[4].title, null, countBars(d.breakdowns[4].rows))}${card(d.channelMetrics.title, null, tbl(d.channelMetrics.headers, d.channelMetrics.rows.map((r) => tr(r))))}</div>`;
      html += card(d.mailboxComparison.title, null, tbl(d.mailboxComparison.headers, d.mailboxComparison.rows.map((r) => `<tr>${tds(r[0])}${r.slice(1).map((c, i) => (i === 5 || i === 6 ? tdn(c) : tdn(c))).join("")}</tr>`).join("")));
      html += card("Issue radar", d.radar.hint, d.radar.alerts.map((a) => `<div class="radar-row">${badge(a.sev === "high" ? "danger" : "warning", a.sev)}<span>${esc(a.text)}</span></div>`).join(""));
      root.insertAdjacentHTML("beforeend", html);
  }
  function tr(r) { return `<tr>${r.map((c, i) => (i === 0 ? tds(c) : tdn(c))).join("")}</tr>`; }

  /* ============ Live chats ================================================ */
  R.livechatsInit = () => { S["live-chats"] = { view: "open", sel: "chat-1" }; };
  R["live-chats"] = (root) => {
    if (!S["live-chats"]) R.livechatsInit();
    const st = S["live-chats"], d = LAB.PAGES["live-chats"];
    hdr(root, "live-chats", `<span class="counter">${esc(d.counter)}</span>`);
    let html = chips(["open chats", "show ended chats"], st.view === "open" ? 0 : 1, "lc-view");
    const list = d.chats.filter((c) => (st.view === "open" ? c.state === "open" : c.state === "ended"));
    if (!list.length) {
      html += emptyState(LAB.ICONS.send, st.view === "open" ? "No live chats yet" : "No ended chats", "Chats appear here the moment the beacon.chat.created webhook lands — connect the webhook relay (Settings) and they will arrive on their own.");
    } else {
      const chat = d.chats.find((c) => c.id === st.sel) || list[0];
      const rows = list.map((c) => `<div class="sel-row2${c.id === chat.id ? " selected" : ""}" data-act="lc-sel" data-id="${c.id}" role="listitem" tabindex="0"><div class="lr1">${esc(c.name || "visitor")} · ${esc(c.last)}</div><div class="lr2">${esc(c.preview)}</div></div>`).join("");
      const msgs = chat.messages.length
        ? chat.messages.map((m) => `<div class="chat-msg">${esc(m.who)} — ${esc(m.ts)}\n${esc(m.text)}</div>`).join("")
        : hint("no messages stored for this chat yet");
      const sendHint = chat.state === "ended"
        ? errLine(chat.endedAs ? `this chat ended — it became conversation #${chat.endedAs}` : "this chat ended")
        : !chat.assignee
          ? hint("no agent is assigned — sending accepts this chat automatically")
          : "";
      const sendBox = chat.state === "open"
        ? `<div class="chat-send">${sendHint}<textarea class="text-area" placeholder="reply to the visitor…" aria-label="reply to the visitor"></textarea><div class="chat-send-foot"><span class="hint-inline">the message id is generated here — a retry can never double-send</span><button class="primary-btn" type="button">send</button></div></div>`
        : "";
      html += `<div class="two-col wide-left">
        <div><div class="section-title">Chats</div><div class="chat-scroll">${rows}</div></div>
        <div>${card(`Chat with ${chat.name || "a visitor"}`, `state: ${chat.state} · mailbox #${chat.mailbox} · last event ${chat.last}`,
          `<div class="chat-scroll inner">${msgs}</div><div class="badge-row">${badge(chat.state === "open" ? "success" : "neutral", chat.state)}</div>${sendBox}`)}</div>
      </div>`;
    }
    root.insertAdjacentHTML("beforeend", html + `<div class="t-divider"></div>`);
  };

  /* ============ Operations Center ========================================= */
  R.operationsInit = () => { S.operations = { tab: 0, mailbox: 0 }; };
  R.operations = (root) => {
    if (!S.operations) R.operationsInit();
    const st = S.operations, d = LAB.PAGES.operations;
    hdr(root, "operations");
    let html = tabbar(["Tiles", "Workload", "Library"], st.tab, "op-tab");
    if (st.tab === 0) {
      html += hint(d.generated);
      const groups = ["Critical", "Warning", "Info"];
      for (const g of groups) {
        const tiles = d.tiles.filter((t) => t.sev === g);
        if (!tiles.length) continue;
        html += `<div class="section-title">${g}</div><div class="tile-wrap">${tiles.map((t) => `<div class="tile ${t.sev.toLowerCase()}${t.drill ? " drill" : ""}"${t.drill ? ` data-act="op-drill" title="Active conversations with no agent assigned."` : ""}><span class="cnt">${t.count}</span><span class="lbl">${esc(t.label)}${t.drill ? `<span class="drill-line">open in inbox →</span>` : ""}</span></div>`).join("")}</div>`;
      }
      html += hint("(tiles marked with an arrow drill into the inbox)");
    } else if (st.tab === 1) {
      html += card("Team Workload", `generated 2026-10-27T09:59:59.123Z · unassigned work 4`,
        tbl(d.workload.headers, d.workload.rows.map((r) => `<tr>${tds(r.agent)}${tdn(r.open)}${tdn(r.waiting)}${tdn(r.capacity)}${tdn(r.load)}${tdn(r.pressure)}<td>${outlineBtn("+", "noop")}${outlineBtn("−", "noop")}</td></tr>`).join("")));
      html += card("Suggested Assignees", "Agents with the lowest load are suggested for new assignments. Use the +/− buttons to adjust individual capacity.",
        d.workload.suggested.map((s) => `<div class="bullet">· ${esc(s)}</div>`).join(""));
    } else {
      html += `<div class="chip-row">${d.mailboxes.map((m, i) => `<button class="chip${i === st.mailbox ? " on" : ""}" data-act="op-mailbox" data-i="${i}" type="button">${esc(m)}</button>`).join("")}</div>`;
      html += `<div class="section-title">Saved replies ${primaryBtn("new saved reply", "noop")}</div>`;
      html += card(null, null, LAB.SAVED_REPLIES.map((r) => `<div class="lib-row"><span class="lib-label">${esc(r.title)} — ${esc(r.body.slice(0, 72))}…</span>${outlineBtn("edit", "noop")}${dangerBtn("delete", "noop")}</div>`).join(""))
        + hint("no saved replies in this mailbox yet — the sync mirrors the platform's list");
      html += `<div class="section-title">Workflows</div>`;
      html += card(null, null,
        `<div class="tool-row">${formRow("conversation #:", inputPH("e.g. 778", 110))}</div>` +
        hint("the run resolves the number against the local mirror; the platform enforces same-inbox and manual-only (a 400 lands in the jobs table)") +
        d.workflows.map((w) => `<div class="lib-row"><span class="lib-label">${badge(w.status === "active" ? "success" : "neutral", w.status)} ${esc(w.name)} · ${esc(w.kind)}</span>${w.kind === "manual" ? outlineBtn("run", "noop") : ""}</div>`).join(""));
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Customers ================================================= */
  R.customersInit = () => { S.customers = { mode: "search" }; };
  R.customers = (root) => {
    if (!S.customers) R.customersInit();
    const st = S.customers, d = LAB.PAGES.customers;
    hdr(root, "customers", `${outlineBtn("Sync now", "sync")}<span class="sync-note"></span>`);
    if (st.mode === "search") {
      let html = `<div class="tool-row">${inputPH("Search customers by name or email…", 280)}${primaryBtn("search", "cu-search")}</div>`;
      html += card("4 customers (of 4)", null,
        tbl(["name", "email", "conversations", "last contact", ""], d.results.map((r, i) =>
          `<tr>${tds(r.name)}${td(r.email)}${tdn(r.conversations)}<td class="muted2">${esc(r.last)}</td><td>${outlineBtn("open", "cu-open", i)}</td></tr>`).join("")));
      root.insertAdjacentHTML("beforeend", html);
    } else {
      const p = d.profile;
      let html = outlineBtn("← back to search", "cu-back");
      html += `<div class="section-title big">${esc(p.name)}</div><div class="hint">${esc(p.email)} · ${esc(p.phone)} · ${esc(p.company)} · last activity ${esc(p.lastActivity)}</div>`;
      const health = Object.entries(p.health).map(([k, v]) => formRow(k, `<span class="mono">${v}</span>`)).join("");
      const mixMax = Math.max(...p.eventMix.map((e) => e[1]));
      html += `<div class="two-col">
        <div>${card(null, null, `<div class="section-title">Support health</div><div class="form-grid">${health}</div><div class="t-divider"></div><div class="section-title">Event mix</div>${p.eventMix.map(([k, v]) => `<div class="prog"><div class="prog-bar" style="width:${(v / mixMax) * 100}%"><span>${k}: ${v}</span></div></div>`).join("")}`)}</div>
        <div>${card(null, null, `<div class="section-title">${LAB.ICONS.sparkles} Interaction profile ${badge("info", "AI")} ${badge("success", p.stage)}</div>${Object.entries(p.profile).map(([k, v]) => `<div class="kv-row">${esc(k)}: ${esc(v)}</div>`).join("")}<div class="t-divider"></div><div class="field-label">Support approach</div>${Object.entries(p.recommendation).map(([k, v]) => `<div class="kv-row">${esc(k)}: ${esc(v)}</div>`).join("")}<div class="hint">${esc(p.provenance)}</div>`)}</div>
      </div>`;
      html += card("Timeline", null, `<div class="scroll-220">${p.timeline.map(([ts, kind, n, sum]) => `<div class="tl-row"><span class="tl-ts">${esc(ts)}</span><span class="tl-kind">${esc(kind)}</span><span class="mono muted2">#${n}</span><span>${esc(sum)}</span></div>`).join("")}</div>`);
      html += card("Support milestones", "tracked SLA breaches and high-friction flags from the intelligence sweep", p.milestones.map(([ts, et, det]) => `<div class="tl-row"><span class="tl-ts">${esc(ts)}</span><span class="tl-kind">${esc(et)}</span><span>${esc(det)}</span></div>`).join("") || hint("(no milestones recorded)"));
      html += card("Conversations", null, tbl(["#", "subject", "status", "updated"], p.conversations.map(([n, subj, status, upd]) => { const [tone, label] = statusTone(status); return `<tr>${tdn(n)}${td(subj)}${td(badge(tone, label))}<td class="muted2">${esc(upd)}</td></tr>`; }).join("")));
      root.insertAdjacentHTML("beforeend", html);
    }
  };

  /* ============ Organizations ============================================= */
  R.organizationsInit = () => { S.organizations = { sel: 0 }; };
  R.organizations = (root) => {
    if (!S.organizations) R.organizationsInit();
    const st = S.organizations, d = LAB.PAGES.organizations;
    hdr(root, "organizations", `${outlineBtn("Sync now", "sync")}<span class="sync-note"></span>`);
    const org = d.orgs[st.sel];
    const left = `<div class="section-title">${d.orgs.length} organization(s)</div>` +
      d.orgs.map((o, i) => `<button class="chip full${i === st.sel ? " on" : ""}" data-act="org-sel" data-i="${i}" type="button">${esc(o.name)} · ${o.customers} customers · ${o.conversations} conversations</button>`).join("") +
      card("New organization", null, `${formRow("", inputPH("Name"))}${formRow("", `<input class="text-input" placeholder="acme.com, acme.io" title="comma-separated domains">`)}${primaryBtn("create organization", "noop")}`);
    const right = org
      ? `<div class="section-title big">${esc(org.name)}</div><div class="mono muted2">${esc(org.domains)}</div><div class="t-divider"></div><div class="hint">${org.members.length} member(s)</div>` +
        org.members.map(([n, e, jt]) => `<div class="bullet">${esc(n)} · ${esc(e)}${jt ? ` (${esc(jt)})` : ""}</div>`).join("")
      : emptyState(LAB.ICONS.organizations, "Select an organization", "Its members, domains, and conversation counts appear here.");
    root.insertAdjacentHTML("beforeend", `<div class="tool-row">${inputPH("Filter by name…", 260)}${outlineBtn("filter", "noop")}</div><div class="t-divider"></div><div class="two-col"><div class="col-scroll">${left}</div><div class="col-scroll">${right}</div></div>`);
  };

  /* ============ Knowledge ================================================== */
  R.knowledgeInit = () => { S.knowledge = { tab: 0 }; };
  R.knowledge = (root) => {
    if (!S.knowledge) R.knowledgeInit();
    const st = S.knowledge, d = LAB.PAGES.knowledge;
    hdr(root, "knowledge", outlineBtn("Sync Help Center now", "sync"));
    let html = banner(LAB.PAGE_META.knowledge.banner);
    html += tabbar(["Documents", "Sources", "Freshness", "Help center", "Article drafts"], st.tab, "kn-tab");
    if (st.tab === 0) {
      html += hint("Local knowledge documents used by search and AI answers");
      html += card(null, null, tbl(["title", "source", "visibility", "chunks", "actions"], d.documents.map(([t, s, v, c]) =>
        `<tr>${tds(t)}${tdm(s)}${td(`<span class="mono">${esc(v)}</span>`)}${tdn(c)}<td>${outlineBtn("reviewed", "noop")}${outlineBtn("verify", "noop")}${dangerBtn("delete", "noop")}</td></tr>`).join("")));
    } else if (st.tab === 1) {
      html += card(null, null, tbl(["name", "kind", "visibility", "documents", "created"], d.sources.map(([n, k, v, dc, cr]) =>
        `<tr>${tds(n)}${td(k)}${td(v)}${tdn(dc)}${tdm(cr)}</tr>`).join("")));
    } else if (st.tab === 2) {
      // QUIRK: only the hint ever renders — report["flags"] never exists (knowledge.rs:1249 vs knowledge_store.rs:516-530)
      html += hint(d.freshnessHint);
    } else if (st.tab === 3) {
      html += statGrid(d.docsStats) + `<div class="t-divider"></div>`;
      html += `<div class="tool-row"><span class="field-inline">collection:</span><button class="combo" type="button">All</button><span class="field-inline">status:</span><button class="combo" type="button">Any</button>${outlineBtn("apply", "noop")}</div>`;
      html += `<div class="tool-row">${inputPH("Search help center…", 320)}${primaryBtn("search", "noop")}</div>`;
      html += card("Knowledge gaps (tracked)", "zero-hit searches recorded as gaps; stale count from the tracked freshness rows",
        `<div class="chip-row">${d.gaps.map((g) => badge("warning", g)).join("")}${badge("warning", d.stale)}</div>`);
      html += card(null, null, tbl(["article", "collection", "status", "views", ""], d.articles.map(([a, c, s, v]) => {
        const tone = s === "published" ? "success" : s === "internal" ? "info" : "neutral";
        return `<tr>${tds(a)}${td(c)}${td(badge(tone, s))}${tdn(v)}<td>${outlineBtn("read", "noop")}</td></tr>`;
      }).join("")));
    } else {
      html += hint("AI drafts knowledge articles from real resolutions; a human review is the only path to publishing. Approved drafts publish to Help Scout through the Docs API.");
      html += card("Draft an article from a gap", null, `<div class="tool-row"><button class="combo" type="button">Select a gap…</button>${primaryBtn("Draft article", "noop")}</div>`);
      html += card(null, null, d.drafts.map((dr) => `<div class="lib-row"><span class="lib-label">${dr.id} — <b>${esc(dr.title)}</b> <span class="${dr.status === "approved" ? "link" : "muted2"}">${esc(dr.status)}</span>${dr.publish ? ` <span class="muted2">${esc(dr.publish)}</span>` : ""}</span>${outlineBtn("Open", "noop")}</div>`).join(""));
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Reports ==================================================== */
  R.reportsInit = () => { S.reports = { tab: 0, range: 1 }; };
  R.reports = (root) => {
    if (!S.reports) R.reportsInit();
    const st = S.reports, d = LAB.PAGES.reports;
    hdr(root, "reports", `<span class="chip-row tight">${d.ranges.map((r, i) => `<button class="chip${i === st.range ? " on" : ""}" data-act="rep-range" data-i="${i}" type="button">${esc(r)}</button>`).join("")}</span>${outlineBtn("Sync now", "sync")}<span class="sync-note"></span>`);
    let html = tabbar(d.tabs, st.tab, "rep-tab");
    const t = d.tabs[st.tab];
    if (t === "Overview") {
      html += statGrid(d.overview) + hint(d.ratingsNote);
      html += card(d.chart.title, null, lineChart(d.chart.points, d.chart.annotation));
    } else if (t === "SLA") {
      html += card("First response avg (business minutes)", null, barChart(d.sla.bars, "first response avg · max 55"));
      html += card(null, null, tbl(d.sla.headers, d.sla.rows.map((r) => `<tr>${tds(r[0])}${r.slice(1).map((c) => tdn(c)).join("")}</tr>`).join("")));
      html += card("Persisted breach tracking", "recorded breach events; the thresholds feed the coarse tracker for mailboxes without business-hours config",
        `${badge("info", "9 recorded breach event(s)")}<div class="section-title">Tracking thresholds (hours)</div><div class="chip-row"><button class="chip on" type="button">Support</button><button class="chip" type="button">Billing</button></div><div class="form-grid">${formRow("first response hours:", inputPH("", 120))}${formRow("resolution hours:", inputPH("", 120))}</div><div class="tool-row">${primaryBtn("Save thresholds", "noop")}</div>`);
    } else if (t === "Why contacting" || t === "Top questions") {
      const rows = t === "Why contacting" ? d.whyContacting : d.topQuestions;
      html += card(t.toLowerCase(), null, rows.map(([q, c]) => `<div class="list-row"><span>${esc(q)}</span>${badge("neutral", c)}</div>`).join(""));
    } else if (t === "Quality") {
      html += card("Post-resolution QA", "deterministic checks over closed conversations (AI layer per conversation in the inbox)",
        `<div class="form-grid">${formRow("Closed conversations", `<span class="mono">${d.quality.closed}</span>`)}${formRow("QA rows computed", `<span class="mono">${d.quality.rows}</span>`)}${formRow("Conversations with the AI layer", `<span class="mono">${d.quality.aiLayer}</span>`)}</div><div class="hint">Deterministic checks: back-and-forth, repeated information, messages after close, handoffs, first-response and resolution minutes — per conversation in the inbox detail.</div>`);
      html += card("Recent closed conversations", "the 25 most recent closures with their QA rows",
        d.quality.recent.map((r) => `<div class="list-row"><span class="mono">#${r.id}</span><span class="chip-row tight">${r.badges.map((b) => badge(b === "AI review" ? "success" : "neutral", b)).join("")}</span></div>`).join(""));
    } else if (t === "Issue radar") {
      html += card("Issue radar", "AI-derived alerts from local conversation data",
        d.radar.map((a) => `<div class="radar-row">${badge("warning", a.sev)}<span>${esc(a.text)}</span></div>`).join(""));
    } else if (t === "Digest") {
      html += hint("AI-written weekly summaries over SQL-computed facts — volume, topics, sentiment, anomalies, workload, and what changed. Archived and searchable.");
      html += `<div class="tool-row">${primaryBtn("Generate digest now", "noop")}${inputPH("search digests…", 220)}</div>`;
      html += `<div class="chip-row">${d.digest.chips.map(([ts, s]) => `<button class="chip on" type="button">${esc(ts)} — ${esc(s)}</button>`).join("")}</div>`;
      const dg = d.digest.detail;
      html += card(dg.title, "AI-generated narrative — numbers are SQL facts, not model output",
        `<p>${esc(dg.narrative)}</p><div class="section-title">Anomalies</div>${dg.anomalies.map((a) => `<div class="bullet warn">• ${esc(a)}</div>`).join("")}<div class="section-title">What changed</div>${dg.changed.map((c) => `<div class="bullet">• ${esc(c)}</div>`).join("")}`);
    } else if (t === "Happiness") {
      html += statGrid(d.happiness.stats) + hint(d.happiness.note);
      html += card(null, null, tbl(["Rating", "Conversation", "Customer", "Comment"], d.happiness.rows.map(([r, n, cu, cm]) =>
        `<tr><td class="${r === "Great" ? "link" : r === "Not good" ? "err" : ""}">${esc(r)}</td>${tdn(n)}${td(cu)}${td(cm)}</tr>`).join("")));
    } else if (t === "Help Scout") {
      html += hint(d.helpScout.banner);
      html += `<div class="tool-row"><button class="combo" type="button">Company — Company overall</button><button class="default-btn" type="button">Fetch from Help Scout</button></div>`;
      html += emptyState(LAB.ICONS.reports, "no reports fetched yet", "Pick a report and fetch it — results cache here with their range.");
    } else if (t === "Builder") {
      html += card(null, null, `<div class="section-title">Report definition</div>
        <div class="tool-row"><span class="field-inline">metric:</span><button class="combo" type="button">Conversations</button><span class="field-inline">grouped by:</span><button class="combo" type="button">Mailbox</button><label class="check"><input type="checkbox"> compare with previous period</label><span class="field-inline">sort:</span><button class="chip on" type="button">metric ↓</button><button class="chip" type="button">label A→Z</button></div>
        <div class="tool-row">${formRow("from (YYYY-MM-DD):", inputPH("", 110))}${formRow("to:", inputPH("", 110))}${formRow("limit:", inputPH("50", 60))}</div>
        <div class="section-title">Filters (blank = unfiltered)</div>
        <div class="form-grid four">${["channel:", "statuses (csv):", "tags any (csv):", "tags none (csv):", "min priority:", "attribute key:", "attribute value:", "state key:", "mailbox ids (csv):", "assignee ids (csv):"].map((f) => formRow(f, inputPH("", 180))).join("")}</div>
        <div class="tool-row">${primaryBtn("Run report", "noop")}${formRow("save as:", inputPH("name", 160))}${outlineBtn("Save", "noop")}</div>`);
      html += card("Saved reports", "load or delete a saved configuration", d.builder.saved.map((s) => `<div class="lib-row"><span class="lib-label mono">${esc(s.name)}</span><span class="muted2">${esc(s.created)}</span>${outlineBtn("load", "noop")}${outlineBtn("delete", "noop")}</div>`).join(""));
      html += card(d.builder.result.title, d.builder.result.sub + `  ·  ${badge("info", "local")}`,
        tbl(d.builder.result.headers, d.builder.result.rows.map((r) => `<tr>${tds(r[0])}${tdn(r[1])}</tr>`).join("")));
    } else {
      const ins = d.insights;
      html += card("Health facts", "the last 30 days, over the whole mirror", statGrid(ins.health));
      html += card("Post-resolution QA effectiveness", "avg QA score and the score distribution over every stored checks",
        `${statGrid([{ label: "avg QA score", value: ins.qa.avg }, { label: "checks stored", value: ins.qa.checks }])}<div class="chip-row">${ins.qa.dist.map(([b, tone]) => badge(tone, b)).join("")}</div>`);
      html += card("Customer effort (friction)", "scores persist per conversation; crossing the threshold records a customer milestone",
        `${badge("warning", "6 high-friction conversation(s)")}<div class="tool-row">${outlineBtn("Recompute (last 100 closed)", "noop")}</div>` +
        tbl(ins.friction.headers, ins.friction.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? tdn(c) : td(c))).join("")}</tr>`).join("")));
      html += card("Graph nodes", "custom nodes over the support graph; neighbors come from the live human edges",
        `<div class="form-grid">${formRow("kind:", `<button class="combo" type="button">customer</button>`)}${formRow("label:", inputPH("", 180))}${formRow("entity id:", inputPH("", 180))}${formRow("properties (JSON):", inputPH("", 180))}</div><div class="tool-row">${primaryBtn("Add node", "noop")}</div>` +
        tbl(ins.graph.headers, ins.graph.rows.map((r) => `<tr>${td(r[0])}${td(r[1])}${tdm(r[2])}${tdm(r[3])}<td>${outlineBtn("inspect", "noop")}</td></tr>`).join("")));
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Issue Radar ================================================ */
  R.radarInit = () => { S.radar = { tab: 0 }; };
  R["issue-radar"] = (root) => {
    if (!S.radar) R.radarInit();
    const st = S.radar, d = LAB.PAGES["issue-radar"];
    hdr(root, "issue-radar", `${outlineBtn("Cluster now", "noop")}${outlineBtn("Reload clusters", "noop")}${outlineBtn("Detect gaps now", "noop")}`);
    let html = tabbar(["Alerts", "Clusters", "Gaps", "Known issues"], st.tab, "ir-tab");
    if (st.tab === 0) {
      html += card("3 radar alert(s)", "AI-derived from local conversation data", d.alerts.map((a) =>
        `<div class="radar-row">${badge(a.sev === "critical" ? "danger" : "warning", a.sev)}<span>${esc(a.text)}</span></div>`).join(""));
      html += card(d.sla.title, null, d.sla.rows.map(([text, cls]) => `<div class="radar-row ${cls}"><span class="small">${esc(text)}</span></div>`).join(""));
    } else if (st.tab === 1) {
      html += `<div class="section-title">2 cluster(s)</div>`;
      html += d.clusters.map((c) => card(c.title, null, `${c.trend ? badge(["rising", "new"].includes(c.trend) ? "warning" : "neutral", c.trend) : ""}<div class="hint">${esc(c.hint)}</div>${c.summary ? `<div class="hint">${esc(c.summary)}</div>` : ""}`)).join("");
    } else if (st.tab === 2) {
      html += `<div class="chip-row">${d.gapBadges.map(([b, tone]) => badge(tone, b)).join("")}</div>`;
      html += d.gapKinds.map((k) => card(k.kind, null, k.cands.map((c) =>
        `<div class="gap-cand"><div class="gap-head">${badge("warning", `${c.count}×`)}<b>${esc(c.query)}</b>${badge(c.status === "approved" ? "success" : c.status === "rejected" ? "neutral" : "info", c.status)}</div>
        <div class="hint">${esc(c.explain)}</div><div class="hint">examples: ${c.examples.map((e) => badge("neutral", e)).join(" ")}</div>
        ${c.status === "open" ? `<div class="tool-row">${primaryBtn("approve", "noop")}${textBtn("reject", "noop")}</div>` : ""}</div>`).join(""))).join("");
    } else {
      html += `<div class="tool-row">${primaryBtn("New known issue", "noop")}</div>`;
      html += `<div class="section-title">2 known issue(s)</div>`;
      html += d.knownIssues.map((k) => card(k.title, null, `${badge("neutral", k.status)}<span class="mono muted2 small"> ${k.convs} conversations</span><div class="hint">${esc(k.symptoms)}</div><div>workaround: ${esc(k.workaround)}</div>`)).join("");
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Incidents =================================================== */
  R.incidentsInit = () => { S.incidents = { sel: 0 }; };
  R.incidents = (root) => {
    if (!S.incidents) R.incidentsInit();
    const st = S.incidents, d = LAB.PAGES.incidents;
    hdr(root, "incidents", primaryBtn("New Incident", "noop"));
    const sevTone = (s) => (s === "sev1" ? "danger" : s === "sev2" ? "warning" : "neutral");
    const staTone = (s) => (s === "resolved" ? "success" : ["investigating", "monitoring"].includes(s) ? "warning" : ["identified", "fix_in_progress"].includes(s) ? "info" : "neutral");
    let html = card(null, null, `<div class="tool-row no-margin"><span class="field-inline">status:</span><button class="combo" type="button">Any</button><span class="field-inline">severity:</span><button class="combo" type="button">Any</button><label class="check"><input type="checkbox"> open only</label>${inputPH("search incidents…", 180)}${outlineBtn("apply", "noop")}</div>`);
    const det = d.detail;
    html += `<div class="two-col">
      <div class="col-scroll">${selList(d.list.map((i) => `<div class="inc-chip">${esc(i.code)} ${esc(i.title)}</div><div class="inc-badges">${badge(sevTone(i.sev), i.sev)}${badge(staTone(i.status), i.status)}<span class="muted2 small">${i.convs} conversations</span></div>`), st.sel, "in-sel")}
        <div class="hint-inline">${LAB.ICONS.incidents} No incidents match — adjust the filters or create a new incident.</div></div>
      <div class="col-scroll">
        <div class="section-title big">${esc(det.code)} · ${esc(det.title)}</div>
        <div class="inc-badges">${badge(staTone(det.status), det.status)}${badge(sevTone(det.sev), det.sev)}<span class="field-inline">started:</span><span class="mono">${esc(det.started)}</span></div>
        <div class="hint">owner: ${esc(det.owner)}</div>
        <div class="field-label">description</div><div>${esc(det.description)}</div>
        <div class="field-label">customer-safe</div><div>${esc(det.customerSafe)}</div>
        ${card(`Linked conversations (${det.linked.length})`, null, det.linked.map(([n, s]) => `<div class="bullet">#${n} · ${esc(s)}</div>`).join("") + `<div class="tool-row">${formRow("link conversation #:", inputPH("id", 140))}${primaryBtn("link", "noop")}</div>`)}
        ${card(`Affected customers (${det.customers.length})`, null, det.customers.map((c) => `<div class="bullet">${esc(c)}</div>`).join(""))}
        ${card(`Notes (${det.notes.length})`, null, det.notes.map(([ts, body]) => `<div class="note-card"><div class="muted2 small">${esc(ts)}</div>${esc(body)}</div>`).join("") + `<textarea class="text-area" placeholder="write an incident note…"></textarea><div class="tool-row">${primaryBtn("add note", "noop")}</div>`)}
        ${card(`Releases (${det.releases.length})`, null, det.releases.map(([v, s]) => `<div class="bullet">${esc(v)} · ${esc(s)}</div>`).join(""))}
      </div></div>`;
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Custom Objects ============================================== */
  R.coInit = () => { S.co = { sel: 0 }; };
  R["custom-objects"] = (root) => {
    if (!S.co) R.coInit();
    const st = S.co, d = LAB.PAGES["custom-objects"];
    hdr(root, "custom-objects", primaryBtn("New Type", "noop"));
    const t = d.types[st.sel];
    const left = `<div class="section-title">Types</div>` + d.types.map((ty, i) =>
      `<div class="lib-row"><button class="chip${i === st.sel ? " on" : ""}" data-act="co-sel" data-i="${i}" type="button">${esc(ty.name)} (${ty.count})</button>${dangerBtn("delete", "noop")}</div>${ty.desc ? `<div class="hint">${esc(ty.desc)}</div>` : ""}`).join("");
    const right = t.count === 0
      ? emptyState(LAB.ICONS["custom-objects"], "No objects", "Create one below or adjust the search.")
      : `<div class="section-title">${esc(t.name)} objects (${t.count})</div><div class="tool-row">${inputPH("search objects…", 220)}${primaryBtn("search", "noop")}</div>` +
        d.objects.map((o) => card(null, null, `<div class="obj-head"><b>${esc(o.title)}</b>${dangerBtn("delete", "noop")}</div>${o.props.map(([k, v]) => `<div class="mono small">${esc(k)}: ${typeof v === "string" ? esc(v) : esc(JSON.stringify(v))}</div>`).join("")}`)).join("") +
        card("New object", null, `${formRow("", inputPH("object title"))}${d.fieldTypes.slice(0, 2).map((f) => formRow(`${f}:`, inputPH("", 180))).join("")}${primaryBtn("create object", "noop")}`);
    root.insertAdjacentHTML("beforeend", `<div class="two-col"><div class="col-scroll">${left}</div><div class="col-scroll">${right}</div></div>`);
  };

  /* ============ Automation ==================================================== */
  R.automationInit = () => { S.automation = { open: false }; };
  R.automation = (root) => {
    if (!S.automation) R.automationInit();
    const st = S.automation, d = LAB.PAGES.automation;
    hdr(root, "automation", primaryBtn("New Rule", "aut-open"));
    let html = "";
    if (st.open) {
      html += card(null, null, `<div class="section-title">New Rule</div><div class="hint warn">${LAB.ICONS.incidents} enabled rules act on conversations automatically — actions that need review park in the approvals queue</div>
        <div class="form-grid">${formRow("name:", inputPH("rule name", 220))}${formRow("trigger:", chips(d.triggers, 0, "noop"))}${formRow("from → to:", `${inputPH("from status", 100)}<span class="field-inline">→</span>${inputPH("to status", 100)}`)}${formRow("action:", chips(d.actions, 1, "noop"))}${formRow("tag:", inputPH("tag", 180))}</div>
        <div class="tool-row">${primaryBtn("Create Rule", "aut-close")}</div>`);
    }
    html += card("Rules", `${d.rules.rows.length} rule(s)`, tbl(d.rules.headers, d.rules.rows.map((r) =>
      `<tr>${tds(r.name)}${td(r.when)}${td(`${esc(r.then)}${r.approval ? ` <span class="warn-text">${LAB.ICONS.incidents} (needs approval)</span>` : ""}`)}<td><button class="chip${r.enabled ? " on" : ""}" data-act="aut-toggle" type="button">${r.enabled ? "enabled" : "disabled"}</button>${dangerBtn("delete", "noop")}</td></tr>`).join("")));
    html += card("Pending approvals", `${d.approvals.length} pending approval(s)`,
      d.approvals.map((a) => `<div class="note-card"><div>${esc(a.text)}</div><div class="tool-row">${primaryBtn("approve", "noop")}${dangerBtn("reject", "noop")}</div></div>`).join("") +
      `<div class="approve-empty">${emptyState(LAB.ICONS.shell, "No approvals waiting", "Actions that need review will park here.", true)}</div>`);
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ AI Center ====================================================== */
  R.aiInit = () => { S.ai = { tab: 0 }; };
  R["ai-center"] = (root) => {
    if (!S.ai) R.aiInit();
    const st = S.ai, d = LAB.PAGES["ai-center"];
    hdr(root, "ai-center");
    let html = tabbar(d.tabs, st.tab, "ai-tab");
    const t = d.tabs[st.tab];
    if (t === "Health") {
      const h = d.health;
      html += card("LM Studio settings", null,
        `<div class="form-grid">${h.lmstudio.map(([label, val]) => { const [kind, v] = val.split("|"); return formRow(label, kind === "badge-success" ? badge("success", v) : `<span class="mono">${esc(v)}</span>`); }).join("")}</div>
        <div class="hint">Start LM Studio locally (default http://127.0.0.1:1234) and configure the models in Settings → LM Studio. Attributes stay local and never sync back.</div>
        <div class="hint ok-text">connection: connected · 3 model(s)</div>${primaryBtn("test connection", "noop")}`);
      html += card("AI provider record (ai_settings)", "the pre-hub fallback record the chat pipeline and embedding resolution consult when the Settings → LM Studio keys are unset",
        `<div class="tool-row"><span class="field-inline">provider:</span>${h.providerRecord.chips.map((c, i) => `<button class="chip${i === 1 ? " on" : ""}" type="button">${esc(c)}</button>`).join("")}${badge("success", h.providerRecord.ready)}</div>
        <div class="form-grid">${formRow("chat model:", inputPH("(empty — unset)", 260))}${formRow("embedding model:", inputPH("(empty — embedding columns stay unchanged)", 260))}${formRow("embedding dim:", inputPH("e.g. 768", 260))}${formRow("base url:", inputPH("http://127.0.0.1:1234", 260))}</div>
        <div class="tool-row">${primaryBtn("Save provider record", "noop")}</div><div class="hint mono">${esc(h.providerRecord.stored)}</div>`);
      html += card("Provider hub — endpoints", "OpenAI-compatible endpoints: local LM Studio, optional Ollama, cloud BYOK",
        h.endpoints.map((e) => `<div class="ep-block"><div class="ep-head">${badge(e.on ? "success" : "neutral", e.on ? "on" : "off")}<b>${esc(e.name)}</b><span class="mono muted2 small">${esc(e.url)}</span>${e.keySet ? badge("info", "key set") : ""}${e.builtin ? badge("neutral", "built-in") : ""}</div>
        <div class="tool-row">${outlineBtn("set key", "noop")}${outlineBtn("probe", "noop")}${e.builtin ? "" : dangerBtn("remove", "noop")}</div>
        ${e.probe ? `<div class="hint ok-text">${esc(e.probe)}</div>` : `<div class="hint err-text">offline — connection refused (12 ms)</div>`}</div>`).join("") +
        `<details class="collapse"><summary>add endpoint (cloud BYOK — off by default per-feature)</summary><div class="form-grid">${formRow("name:", inputPH("", 180))}${formRow("base url:", inputPH("", 280))}${formRow("api key:", `<input class="text-input" type="password" style="width:280px">`)}${formRow("$ / mtok prompt:", inputPH("", 120))}${formRow("$ / mtok completion:", inputPH("", 120))}</div>${primaryBtn("add endpoint", "noop")}</details>`);
      html += card("Model-role routing", "every inference goes through one of five roles (blueprint Table 6)",
        tbl(["role", "endpoint", "model", ""], h.routing.map(([role, note], i) =>
          `<tr>${tds(role)}${td(`<button class="combo" type="button" title="${esc(note)}">${i === 0 ? "(unset)" : "0"}</button>`)}${td(`<input class="text-input" style="width:180px" value="${i === 0 ? "" : "qwen2.5-7b-instruct"}">`)}${td(`<span class="hint-inline">${esc(note)}</span>`)}</tr>`).join("")));
      html += card("Features & kill switches", "one click each; the global switch dominates every feature",
        `<div class="feat-grid">${h.features.map((f) => `<label class="check"><input type="checkbox" ${!["ai_note", "draft"].includes(f) ? "checked" : ""}> <span class="mono">${esc(f)}</span></label>`).join("")}</div>
        <div class="tool-row">${outlineBtn("global kill switch: AI is ON — click to disable all AI", "noop")}</div>`);
      html += card("Cost ledger — last 7 days", "tokens, latency and estimated spend per feature per day, from ai_runs",
        tbl(h.ledger.headers, h.ledger.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? tdm(c) : i < 3 ? td(`<span class="mono">${esc(c)}</span>`) : tdn(c))).join("")}</tr>`).join("")));
    } else if (t === "Analytics") {
      html += card(null, null, `<div class="section-title">${LAB.ICONS.sparkles} Analytics ${badge("info", "AI")}</div><div class="form-grid">${d.analytics.map(([k, v]) => formRow(k, `<span class="mono">${esc(v)}</span>`)).join("")}</div>${hint("(no failure patterns)")}`);
    } else if (t === "Attributes") {
      html += card(null, null, `<div class="section-title">${LAB.ICONS.sparkles} Catalog ${badge("info", "AI")}</div><div class="hint">A missing attribute reads as unknown — it is never fabricated. Attributes are local-only and never written to Help Scout.</div>
        ${d.attributes.map((a) => `<div class="attr-card"><div class="attr-head"><b>${esc(a.label)}</b><span class="mono muted2">${esc(a.key)}</span><span class="muted2 small">${a.known} known / ${a.unknown} unknown</span></div>${a.values.map(([v, c, s]) => `<div class="mono small">  ${esc(v)}: ${c} (${s}%)</div>`).join("")}</div>`).join("")}`);
    } else if (t === "Copilot") {
      html += card(null, null, `<div class="section-title">Sessions ${outlineBtn("New chat", "noop")}</div><div class="chip-row">${d.copilot.sessions.map(([s, c], i) => `<button class="chip${i === 0 ? " on" : ""}" type="button">${esc(s)} (${c})</button>`).join("")}</div><div class="t-divider"></div><div class="section-title">Chat</div>
        <div class="hint">Ask anything answerable from the local mirror — the agent runs 22 read-only tools (searches, lookups, clusters, gaps) and cites what it used.</div>
        <div class="copilot-scroll">${d.copilot.chat.map((m) => `<div class="cop-row">${badge(m.role === "assistant" ? "info" : "neutral", m.role)}<span class="small">${esc(m.text)}</span>${m.cited ? `<div class="hint">cited: ${m.cited.map((c) => badge("success", c)).join(" ")}</div>` : ""}</div>`).join("")}</div>
        <div class="tool-row">${inputPH("Ask the copilot…", 320)}${primaryBtn("send", "noop")}</div>`);
    } else if (t === "Usage") {
      html += `<div class="section-title">Per-feature usage — last 30 days</div>`;
      html += card(null, null, tbl(d.usage.headers, d.usage.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? tdm(c) : typeof c === "number" ? tdn(c) : td(`<span class="mono">${esc(c)}</span>`))).join("")}</tr>`).join("")));
      html += card("Draft feedback (edit rate)", null, `<div class="chip-row">${d.usage.feedback.map(([b, tone]) => badge(tone, b)).join("")}</div><div class="hint">Accept, edit-distance and reject come from the composer send path and the draft panel — real rows, zero invention.</div>`);
      html += card("Golden-set evals (9 seeded cases)", null, `${outlineBtn("Run golden set now", "noop")}<div class="tool-row">${badge("success", d.usage.golden.badge)}<span class="mono small">${esc(d.usage.golden.backend)}</span></div><div class="hint">${esc(d.usage.golden.run)}</div>`);
    } else {
      html += card(null, null, `<div class="section-title">${LAB.ICONS.sparkles} Recent AI runs ${badge("info", "AI")}</div>` +
        tbl(d.jobs.headers, d.jobs.rows.map(([id, type, status, conv, model, created]) => {
          const tone = ["completed", "succeeded"].includes(status) ? "success" : status === "failed" ? "danger" : "neutral";
          return `<tr>${tdn(id)}${td(`<span class="mono">${esc(type)}</span>`)}${td(badge(tone, status))}${tdm(conv)}${tdm(model)}${tdm(created)}</tr>`;
        }).join("")));
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Outreach ========================================================= */
  R.outreachInit = () => { S.outreach = { tab: 0, sel: 0 }; };
  R.outreach = (root) => {
    if (!S.outreach) R.outreachInit();
    const st = S.outreach, d = LAB.PAGES.outreach;
    hdr(root, "outreach");
    let html = tabbar(["Campaigns", "Segments", "Do not contact"], st.tab, "ou-tab");
    const tone = (s) => (["queued", "running"].includes(s) ? "info" : s === "paused" ? "warning" : s === "completed" ? "success" : s === "cancelled" ? "danger" : "neutral");
    if (st.tab === 0) {
      html += primaryBtn("New Campaign", "noop") + `<div class="t-divider"></div>`;
      html += d.campaigns.map((c, i) => card(null, null,
        `<div class="ep-head"><button class="chip${i === st.sel ? " on" : ""}" data-act="ou-sel" data-i="${i}" type="button">${esc(c.name)}</button>${badge(tone(c.status), c.status)}</div>
         <div class="hint">${esc(c.hint)}</div>
         <div class="tool-row">${primaryBtn("queue", "noop")}${outlineBtn("pause", "noop")}${outlineBtn("resume", "noop")}${dangerBtn("cancel remaining", "noop")}${outlineBtn("retry failed", "noop")}</div>`)).join("");
      if (st.sel != null) {
        html += card("Campaign report", null,
          `<div class="form-grid">${d.report.map(([k, v]) => formRow(k, `<span class="mono">${v}</span>`)).join("")}</div><div class="hint warn-text">${esc(d.reportNote)}</div>`);
      }
    } else if (st.tab === 1) {
      html += primaryBtn("New Segment", "noop") + `<div class="t-divider"></div>`;
      html += d.segments.map(([name, criteria]) => card(null, null, `<div class="lib-row"><span class="lib-label"><b>${esc(name)}</b></span>${dangerBtn("delete", "noop")}</div><div class="mono small muted2">${esc(criteria)}</div>`)).join("");
    } else {
      html += card("Do not contact", "Customers on this list are excluded from every campaign.",
        `<div class="form-grid">${formRow("customer id:", inputPH("customer id", 140))}${formRow("reason:", inputPH("why (optional)", 220))}</div>${primaryBtn("add", "noop")}<div class="t-divider"></div>` +
        d.dnc.map(([id, reason, ts]) => `<div class="lib-row"><span class="mono">${esc(id)}</span><span class="muted2 small">${esc(reason)}</span><span class="muted2 small">${esc(ts)}</span>${dangerBtn("remove", "noop")}</div>`).join(""));
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Notifications ====================================================== */
  R.notificationsInit = () => { S.notifications = { tab: 0, read: {} }; };
  R.notifications = (root) => {
    if (!S.notifications) R.notificationsInit();
    const st = S.notifications, d = LAB.PAGES.notifications;
    hdr(root, "notifications", textBtn("mark all read", "nt-markall"));
    let html = tabbar(["All", "Mentions", "Unread"], st.tab, "nt-tab");
    let rows = d.rows;
    if (st.tab === 1) rows = rows.filter((r) => r.type.includes("mention"));
    if (st.tab === 2) rows = rows.filter((r) => !st.read[r.type + r.ts] && r.unread);
    html += rows.length
      ? rows.map((r) => { const read = st.read[r.type + r.ts] !== undefined ? st.read[r.type + r.ts] : !r.unread; return `<div class="nt-card"><div class="nt-line">${badge(r.sev === "error" ? "danger" : "info", `${read ? " " : "●"} ${r.type}`)}<span class="mono muted2 small">${esc(r.ts)}</span><button class="text-btn" data-act="nt-toggle" data-k="${esc(r.type + r.ts)}" type="button">${read ? "unread" : "mark read"}</button></div><div class="nt-title">${esc(r.title)}</div>${r.body ? `<div class="hint">${esc(r.body)}</div>` : ""}</div>`; }).join("")
      : emptyState(LAB.ICONS.bell, "(no notifications)", "Nothing here for this tab yet.");
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Search =============================================================== */
  R.searchInit = () => { S.search = { q: "", ran: false }; };
  R.search = (root) => {
    if (!S.search) R.searchInit();
    const st = S.search, d = LAB.PAGES.search;
    hdr(root, "search");
    let html = `<div class="tool-row"><input class="text-input" id="search-q" style="width:320px" placeholder="Search…" value="${esc(st.q)}">${primaryBtn("search", "se-run")}${textBtn("clear", "se-clear")}</div>`;
    html += `<div class="tool-row"><span class="field-inline">scope:</span>${["All", "Tickets", "Knowledge", "Customers"].map((c, i) => `<button class="chip${i === 0 ? " on" : ""}" type="button">${c}</button>`).join("")}</div><div class="t-divider"></div>`;
    if (!st.ran) {
      html += emptyState(LAB.ICONS.search, "Search across your workspace", "Type a query and press Enter to search conversations, knowledge, and customers.");
    } else {
      html += hint(`4 hits for "${st.q || "export"}"`);
      html += hint(d.modeNote);
      html += d.hits.map((h) => `<div class="hit-card"><div class="hit-line">${badge("neutral", h.scope)}<span class="mono">${esc(h.title)}</span>${h.subtitle ? `<span class="muted2 small">${esc(h.subtitle)}</span>` : ""}</div><div class="chip-row tight">${h.why.map((w) => badge(w === "semantic match" ? "info" : "neutral", w)).join("")}</div>${h.snippet ? `<div class="hint">${esc(h.snippet)}</div>` : ""}</div>`).join("");
    }
    root.insertAdjacentHTML("beforeend", html);
    const q = root.querySelector("#search-q");
    if (q) q.addEventListener("keydown", (e) => { if (e.key === "Enter") { st.q = q.value; st.ran = true; window.LAB_PAGES.render("search", root); } });
  };

  /* ============ Support Graph ========================================================== */
  R.graphInit = () => { S.graph = { sel: 1 }; };
  R["support-graph"] = (root) => {
    if (!S.graph) R.graphInit();
    const st = S.graph, d = LAB.PAGES["support-graph"];
    hdr(root, "support-graph");
    let html = `<div class="tool-row">${inputPH("Search the graph…", 320)}${primaryBtn("search", "noop")}</div><div class="t-divider"></div>`;
    html += `<div class="section-title">${d.results.length} node(s)</div>`;
    html += `<div class="graph-results">${d.results.map((n, i) => `<div class="node-chip${i === st.sel ? " selected" : ""}" data-act="sg-sel" data-i="${i}" role="listitem" tabindex="0"><button class="chip" type="button">${esc(n.kind)}: ${esc(n.label)}</button>${n.sub ? `<div class="hint">   ${esc(n.sub)}</div>` : ""}</div>`).join("")}</div><div class="t-divider"></div>`;
    const nb = d.neighborhood;
    html += card("Neighborhood", null, `<div class="graph-center"><b>${esc(nb.center.kind)} #${nb.center.id}</b><div class="hint">${esc(nb.center.label)}</div></div>` +
      nb.edges.map((e) => `<div class="edge-line">${esc(e)}</div>`).join(""));
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Connectors ================================================================ */
  R.connectors = (root) => {
    const d = LAB.PAGES.connectors;
    hdr(root, "connectors", primaryBtn("New Connector", "noop"));
    let html = okLine("status: refresh started");
    html += `<div class="t-divider"></div>`;
    html += card(null, null, tbl(d.headers, d.rows.map((r, i) =>
      `<tr><td><button class="chip${i === 0 ? " on" : ""}" type="button">${esc(r.name)}</button></td>${tdm(r.kind)}${td(badge(r.health === "ok" ? "success" : r.health === "stale" ? "warning" : r.health === "error" ? "danger" : "neutral", r.health))}${td(r.refresh)}${tdm(r.last)}<td>${outlineBtn("rows", "noop")}${outlineBtn("refresh", "noop")}${dangerBtn("delete", "noop")}</td></tr>`).join("")));
    html += card(d.rowsViewer.title, null, `<div class="tool-row">${inputPH("search rows…", 220)}${primaryBtn("search", "noop")}</div><div class="mono small">${d.rowsViewer.rows.map((r) => `<div class="edge-line">${esc(r)}</div>`).join("")}</div>`);
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Diagnostics =================================================================== */
  R.diagnostics = (root) => {
    const d = LAB.PAGES.diagnostics;
    hdr(root, "diagnostics");
    let html = card("Event log", d.eventLog.sub,
      `<div class="tool-row">${["all", "errors", "warnings", "info"].map((c, i) => `<button class="chip${i === 0 ? " on" : ""}" type="button">${c}</button>`).join("")}${outlineBtn("refresh", "noop")}${dangerBtn("clear log", "noop")}</div>` +
      `<div class="scroll-360">${d.eventLog.rows.map((e) => `<div class="ev-row lv-${e.level}"><div class="ev-line">${badge(e.level === "error" ? "danger" : e.level === "warn" ? "warning" : "info", e.level)}<span class="mono muted2 small">${esc(e.ts)}</span>${badge("neutral", e.service)}<span class="small">${esc(e.msg)}</span></div>${e.ctx ? `<div class="ev-ctx mono">${esc(e.ctx)}</div>` : ""}</div>`).join("")}</div>`);
    html += card("Audit trail", d.audit.sub,
      tbl(d.audit.headers, d.audit.rows.map(([when, actor, action, conv, ai, detail]) =>
        `<tr>${tdm(when)}${td(badge("neutral", actor))}${td(`<span class="small">${esc(action)}</span>`)}${tdm(conv)}${td(ai ? badge("info", "ai") : badge("neutral", "—"))}</tr>`).join("")) +
      `<div class="scroll-140">${d.audit.rows.filter((r) => r[5]).map(([id, , , , , detail]) => `<div class="mono small muted2">${esc(detail)}</div>`).join("")}</div>`);
    html += card("Sync reports", "The outcome of every recent sync run (newest first).",
      tbl(d.syncReports.headers, d.syncReports.rows.map(([id, kind, state, rec, err, started]) =>
        `<tr>${tdn(id)}${td(`<span class="mono">${kind}</span>`)}${td(badge(state === "LIVE" ? "success" : state === "ERROR" ? "danger" : "warning", state))}${tdn(rec)}${tdn(err)}${tdm(started)}</tr>`).join("")));
    html += card("Outbound deliveries", d.outbound.sub,
      tbl(d.outbound.headers, d.outbound.rows.map(([id, kind, at, err]) => `<tr>${tdn(id)}${td(`<span class="mono">${kind}</span>`)}${tdm(at)}${td(`<span class="small muted2">${esc(err)}</span>`)}</tr>`).join("")));
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Sync Health ======================================================================== */
  R["sync-health"] = (root) => {
    const d = LAB.PAGES["sync-health"];
    hdr(root, "sync-health");
    let html = hint("progress: conversations · resource 3/22 · 1204 record(s) so far");
    html += card("Engine", null,
      `<div class="form-grid">${formRow("state:", `<span class="mono">${d.engine.state}</span>`)}${formRow("last success:", `<span class="mono">${esc(d.engine.last)}</span>`)}</div><div class="hint">${esc(d.webhook)}</div><div class="hint">${esc(d.health)}</div>`);
    html += `<div class="tool-row">${primaryBtn("Run initial sync", "noop")}${outlineBtn("Run incremental sync", "noop")}${outlineBtn("Run reconcile", "noop")}${dangerBtn("Cancel running sync", "noop")}</div>`;
    html += card("Checkpoints", null, tbl(d.checkpoints.headers, d.checkpoints.rows.map(([r, s, p, f, rt, ls]) =>
      `<tr>${td(`<span class="mono">${r}</span>`)}${td(s)}${tdn(p)}${tdn(f)}${tdn(rt)}${td(`<span class="mono muted2">${esc(ls)}</span>`)}</tr>`).join("")));
    html += card("Recent runs", null, tbl(d.runs.headers, d.runs.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? tdn(c) : typeof c === "number" ? tdn(c) : i >= 3 ? td(`<span class="mono muted2">${esc(c)}</span>`) : td(`<span class="mono">${esc(c)}</span>`))).join("")}</tr>`).join("")));
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Settings ============================================================================== */
  R.settingsInit = () => { S.settings = { tab: 0 }; };
  R.settings = (root) => {
    if (!S.settings) R.settingsInit();
    const st = S.settings, d = LAB.PAGES.settings;
    hdr(root, "settings", `<span class="chip-row tight">${d.tabs.map((t, i) => `<button class="chip${i === st.tab ? " on" : ""}" data-act="se-tab" data-i="${i}" type="button">${esc(t)}</button>`).join("")}</span>`);
    let html = "";
    if (st.tab === 0) {
      html += card("Synchronization", null, `<div class="form-grid">
        ${formRow("max quiet sync interval (minutes):", `<button class="combo" type="button">1</button>`)}
        ${formRow("api concurrency:", `<button class="combo" type="button">2</button>`)}
        ${formRow("log level:", `<button class="combo" type="button">info</button>`)}
        ${formRow("agent language:", inputPH("", 220))}
        ${formRow("status after reply:", `<button class="combo" type="button">pending (await customer)</button>`)}</div>`);
      html += card("AI behaviour", "safe defaults — review before enabling write actions",
        `<div class="feat-grid">${d.aiBehaviour.map(([label, on]) => `<label class="check"><input type="checkbox" ${on ? "checked" : ""}> ${esc(label)}</label>`).join("")}</div>`);
      html += card("About & updates", null, `<div>Running version: v1.8.0</div><div class="tool-row">${outlineBtn("check for updates", "noop")}</div><div class="hint">The check reads the public GitHub release feed once, on click. The app never sends telemetry.</div>`);
      html += primaryBtn("save settings", "noop");
    } else if (st.tab === 1) {
      html += hint("loopback listener answering on :3001 (webhook ingress + OAuth callback)");
      html += card("Help Scout connection", "Connect with client credentials (recommended — no browser, no redirect URL) or the advanced browser flow",
        `<div class="form-grid">${formRow("credentials stored:", badge("success", d.connection.badge))}${formRow("redirect uri:", `<span class="mono small">${esc(d.connection.redirect)}</span>`)}</div><div class="tool-row">${dangerBtn("disconnect", "noop")}${primaryBtn("connect with client credentials (recommended)", "noop")}</div>`);
      html += card("Connection Doctor", "five checks: reach the API → sign in → read the user → list mailboxes → rate-limit budget",
        `${outlineBtn("run the checks", "noop")}<div class="doctor-grid">${d.doctor.map(([m, detail]) => { const cls = m.startsWith("✓") ? "ok" : m.startsWith("!") ? "warn" : "err"; return `<div class="doctor-row ${cls}"><b>${esc(m)}</b><span class="small"> — ${esc(detail)}</span></div>`; }).join("")}</div>`);
      html += card("Help Scout credentials", "Stored encrypted on this machine. Save them and the app connects immediately — the Connection Doctor verifies the chain and the first sync starts by itself. Environment variables override these values.",
        `<div class="form-grid">${formRow("client id:", inputPH("Help Scout custom app — client ID", 320))}${formRow("client secret:", `<input class="text-input" type="password" style="width:320px" placeholder="leave blank to keep the stored value">`)}${formRow("webhook secret:", `<input class="text-input" type="password" style="width:320px" placeholder="verifies incoming Help Scout webhooks">`)}${formRow("docs api key:", `<input class="text-input" type="password" style="width:320px" placeholder="optional — leave blank to keep the stored value">`)}</div>
        <div class="tool-row"><label class="check"><input type="checkbox"> show secrets</label>${primaryBtn("save credentials", "noop")}${dangerBtn("clear stored", "noop")}</div>
        <div class="form-grid">${d.sources.map(([k, v]) => formRow(k, badge(v === "not set" ? "neutral" : "success", v))).join("")}</div>`);
      html += card("Webhook relay & health", "the listener stays loopback-only by design; a user-run relay bridges the NAT, and the health check re-registers the hook when the platform silently drops it",
        `<div class="form-grid">${formRow("public relay URL:", inputPH("https://your-relay → forwards to this app's loopback listener", 320))}</div><div class="tool-row">${outlineBtn("save relay URL", "noop")}${primaryBtn("check now", "noop")}</div>
        ${hint("relay options: cloudflared tunnel --url http://127.0.0.1:PORT · ngrok http 127.0.0.1:PORT · ssh -N -R 80:127.0.0.1:PORT serveo.net — the registered URL must forward to the loopback listener")}
        ${hint("prove the chain from outside: curl -X POST <relay>/webhook -d '{}' must answer 401 (signature check) — anything else means the relay is not reaching the app")}
        ${hint("one honest limit: the API never reveals a hook's stored secret, so rotating the local webhook secret needs a re-register — the check does that for you when it finds the hook stale")}
        <div class="form-grid">${formRow("last check:", `${badge("success", "healthy")} <span class="muted2 small">· 2026-10-27T04:30:00Z</span>`)}</div>`);
      html += card("Workspace data", "Deletes every conversation, customer, thread, rating, knowledge document, incident, report and AI record — all synced content (and anything left over from the seeded content that shipped before 1.2.0) — and arms a fresh first sync. Keeps: credentials, settings, business hours, automation rules, saved views and replies, custom object types.",
        dangerBtn("delete all workspace data", "noop"));
    } else if (st.tab === 2) {
      html += card("LM Studio (local AI gateway)", "Test the connection from the AI Center page.",
        `<div class="form-grid">${formRow("base url:", inputPH("http://localhost:1234/v1", 320))}${formRow("chat model:", inputPH("", 320))}${formRow("embedding model:", inputPH("", 320))}</div>${primaryBtn("save", "noop")}`);
    } else if (st.tab === 3) {
      html += card("Vector index", "Dense vectors live in SQLite next to the application database (decision D4: the chunk count is far below the 100k gate). The index follows the qdrant_enabled setting; searches fall back to FTS5 when disabled.",
        `<div class="form-grid">${formRow("store:", `<span class="mono small">${esc(d.vectorStore)}</span>`)}</div><label class="check"><input type="checkbox" checked> vector index enabled</label><div class="hint">Dense vectors are embedded and searched while enabled; searches fall back to FTS5 when disabled. Applies on the next start — the workers open the store at boot.</div>`);
    } else if (st.tab === 4) {
      html += card("Business hours per mailbox", null,
        d.businessHours.map((b) => `<div class="lib-row"><span class="lib-label"><b>${esc(b.name)}</b> ${badge(b.configured ? "success" : "neutral", b.configured ? "configured" : "not configured")}${b.hint ? `<span class="muted2 small"> ${esc(b.hint)}</span>` : ""}</span>${outlineBtn("edit", "noop")}</div>`).join(""));
    } else {
      html += card("Backups & export", null,
        `<div class="tool-row">${primaryBtn("create backup", "noop")}${outlineBtn("export JSON", "noop")}${outlineBtn("export conversations CSV", "noop")}</div>` +
        d.backups.map(([f, size, ts]) => `<div class="lib-row"><span class="mono small">${esc(f)}</span><span class="muted2 small">${esc(size)}</span><span class="muted2 small">${esc(ts)}</span>${outlineBtn("restore", "noop")}</div>`).join(""));
    }
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Onboarding ================================================================================= */
  R.onboarding = (root) => {
    const d = LAB.PAGES.onboarding;
    hdr(root, "onboarding");
    let html = card("Setup status", null, `<div class="form-grid">${d.rows.map(([k, v]) => formRow(k, `<span class="mono">${esc(v)}</span>`)).join("")}</div>`);
    html += okLine("status: Initial sync started. Watch Sync Health or Diagnostics for progress and errors.");
    html += `<div class="tool-row">${primaryBtn("Run initial sync", "noop")}${outlineBtn("Mark complete", "noop")}</div>`;
    html += d.hints.map((h) => hint(h)).join("");
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ Shell ========================================================================================= */
  R.shell = (root) => {
    const d = LAB.PAGES.shell;
    hdr(root, "shell", `<button class="outline-btn" data-act="sh-theme" type="button">switch to ${document.body.dataset.theme === "light" ? "dark" : "light"}</button>`);
    let html = card("Runtime", null, `<div class="form-grid">${d.runtime.map(([k, v]) => formRow(k + "", `<span class="mono">${esc(v)}</span>`)).join("")}</div>`);
    html += hint(d.hint);
    root.insertAdjacentHTML("beforeend", html);
  };

  /* ============ actions dispatcher ============================================================================= */
  const ACT = {
    sync: (el) => syncNote(el),
    "lc-view": (el) => { S["live-chats"].view = el.dataset.i === "0" ? "open" : "ended"; R["live-chats"](rootOf(el)); },
    "lc-sel": (el) => { S["live-chats"].sel = el.dataset.id; R["live-chats"](rootOf(el)); },
    "op-tab": (el) => { S.operations.tab = Number(el.dataset.i); R.operations(rootOf(el)); },
    "op-mailbox": (el) => { S.operations.mailbox = Number(el.dataset.i); R.operations(rootOf(el)); },
    "op-drill": () => { location.hash = "#inbox"; },
    "cu-search": (el, root) => { /* already showing results — re-render is a no-op */ },
    "cu-open": (el, root) => { S.customers.mode = "profile"; R.customers(root); },
    "cu-back": () => { S.customers.mode = "search"; R.customers(document.getElementById("page-root")); },
    "kn-tab": (el) => { S.knowledge.tab = Number(el.dataset.i); R.knowledge(rootOf(el)); },
    "rep-tab": (el) => { S.reports.tab = Number(el.dataset.i); R.reports(rootOf(el)); },
    "rep-range": (el) => { S.reports.range = Number(el.dataset.i); R.reports(rootOf(el)); },
    "ir-tab": (el) => { S.radar.tab = Number(el.dataset.i); R["issue-radar"](rootOf(el)); },
    "in-sel": (el) => { S.incidents.sel = Number(el.dataset.i); R.incidents(rootOf(el)); },
    "co-sel": (el) => { S.co.sel = Number(el.dataset.i); R["custom-objects"](rootOf(el)); },
    "aut-open": (el, root) => { S.automation.open = true; R.automation(root); },
    "aut-close": (el, root) => { S.automation.open = false; R.automation(root); },
    "aut-toggle": (el, root) => { const i = Number(el.closest("tr").rowIndex) - 1; LAB.PAGES.automation.rules.rows[i].enabled = !LAB.PAGES.automation.rules.rows[i].enabled; R.automation(root); },
    "ai-tab": (el) => { S.ai.tab = Number(el.dataset.i); R["ai-center"](rootOf(el)); },
    "ou-tab": (el) => { S.outreach.tab = Number(el.dataset.i); R.outreach(rootOf(el)); },
    "ou-sel": (el) => { S.outreach.sel = Number(el.dataset.i); R.outreach(rootOf(el)); },
    "nt-tab": (el) => { S.notifications.tab = Number(el.dataset.i); R.notifications(rootOf(el)); },
    "nt-toggle": (el, root) => { S.notifications.read[el.dataset.k] = !S.notifications.read[el.dataset.k]; R.notifications(root); },
    "nt-markall": (el, root) => { LAB.PAGES.notifications.rows.forEach((r) => { S.notifications.read[r.type + r.ts] = true; }); R.notifications(root); },
    "se-run": (el, root) => { const q = root.querySelector("#search-q"); S.search.q = q ? q.value : ""; S.search.ran = true; R.search(root); },
    "se-clear": (el, root) => { S.search.q = ""; S.search.ran = false; R.search(root); },
    "sg-sel": (el) => { S.graph.sel = Number(el.dataset.i); R["support-graph"](rootOf(el)); },
    "se-tab": (el) => { S.settings.tab = Number(el.dataset.i); R.settings(rootOf(el)); },
    "sh-theme": () => toggleTheme(),
    noop: () => {},
  };
  function rootOf(el) { return el.closest("#page-root"); }

  window.LAB_PAGES = {
    render(id, root) {
      root.dataset.page = id;
      const fn = R[id] || R.shell;
      fn(root);
      root.onclick = (e) => {
        const el = e.target.closest("[data-act]");
        if (el && ACT[el.dataset.act]) ACT[el.dataset.act](el, root, e);
      };
    },
  };
})();
