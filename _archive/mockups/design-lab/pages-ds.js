/* ==========================================================================
   SupportOS+ Design Lab — DS-4 page registry (design-lab-v4)
   --------------------------------------------------------------------------
   Every NAV_GROUPS page (23 PageKeys) is a PAGE_DATA object registered via
   DS.definePage and rendered by exactly one DS archetype (ds.js):
     DS-A-01 list (toolbar → DataTable → row opens DetailPanel DS-C-17)
     DS-A-02 dashboard / DS-A-04 settings (stack: header + tabs + blocks)
     DS-A-03 detail record (entity header + two-col; also DetailPanel content)
     DS-A-05 inbox workspace (views rail → list → reader → copilot)
     DS-A-06 console (header + hero + tabs + blocks)
   Rules honoured here (mismatch-register.md): A-1 DetailPanel flows, A-2 one
   FilterBar per page, A-3 status→color only through DS.TONE, A-4 one detail
   header, A-5 ≤2 inline row actions + destructive = danger + confirm,
   A-6 counts in header counter pills, A-7 DS-C-14 empty states with
   why + next step, A-8 sync = button + status line, B-1 fixed info-text
   roles, B-2 one-off composites folded into Card + kv/chip rows, C-1/C-2
   receipts and mock labels kept verbatim.
   ========================================================================== */
(() => {
  "use strict";
  const DS = window.DS;
  const LAB = window.LAB;
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const M = (id) => LAB.PAGE_META[id] || {};
  const ic = (k) => LAB.ICONS[k] || DS.ICONS[k] || "";
  const mono = (s) => `<span class="ds-mono">${esc(s)}</span>`;
  const muted = (s) => `<span class="ds-muted2">${esc(s)}</span>`;
  const smallMuted = (s) => `<span class="ds-small ds-muted2">${esc(s)}</span>`;
  const divider = `<div class="ds-t-divider"></div>`;
  const kv = (pairs) => pairs.map(([k, v]) => DS.kvRow(k, v)).join("");
  /* raw severity/status tokens → DS.TONE keys (A-3: colors resolve only here) */
  const SEV = { sev1: "critical", sev2: "high", sev3: "medium", critical: "critical", warning: "high", info: "medium" };
  const INC = { resolved: "resolved", investigating: "investigating", monitoring: "monitoring", identified: "identified", fix_in_progress: "fix_in_progress" };
  const CAMP = { queued: "queued", running: "running", paused: "paused", completed: "resolved", cancelled: "cancelled" };
  const SYNC = { LIVE: "live", ERROR: "error", NEW: "draft", CATCHING_UP: "catching_up" };
  /* A-5: destructive = danger + two-step arm/confirm (no dialog component in DS v1.0) */
  const armed = {};
  const confirmBtn = (key, label, act) => (armed[key] ? DS.btn.danger("confirm delete?", act, key) : DS.btn.danger(label, act, key));
  const recordHead = (name, meta, badges, actions, avatarName) =>
    `<div class="ds-record-head">${avatarName === null ? "" : DS.avatar(avatarName || name)}<div class="ds-record-id"><div class="ds-record-name">${esc(name)}</div>${meta ? `<div class="ds-record-meta">${meta}</div>` : ""}</div><div class="ds-record-badges">${badges || ""}</div><div class="ds-record-actions">${actions || ""}</div></div>`;

  /* ============================ Inbox (DS-A-05 pilot) ===================== */
  DS.definePage("inbox", { archetype: "inbox" });

  /* ============================ Dashboard (DS-A-02) ======================= */
  DS.definePage("dashboard", {
    archetype: "stack",
    meta: () => ({ title: M("dashboard").title, subtitle: M("dashboard").subtitle, actions: DS.syncControl() }),
    blocks: () => {
      const d = LAB.PAGES.dashboard;
      const bmax = (rows) => Math.max(...rows.map((r) => r[1]), 1);
      const brk = (b) => DS.card({ title: b.title, sub: b.note || null, body: b.rows.map((r) => DS.barRow(r[0], r[1], bmax(b.rows), true)).join("") });
      const scope = (label, opts, on) =>
        `<div class="ds-tool-row"><span class="ds-field-inline">${esc(label)}</span>${opts.map((o, i) => DS.chip(o, i === on)).join("")}</div>`;
      return [
        DS.card({ body: scope("Range", ["7 days", "30 days", "90 days", "1 year"], 1) + scope("Channel", ["All channels", "Email", "Chat (Beacon)"], 0) + scope("Mailboxes", ["All", "Support", "Billing"], 0) }),
        DS.statGrid(d.metrics),
        DS.card({ title: d.chart.title, sub: d.chart.hint, body: DS.lineChart(d.chart.points, `${d.chart.series} · ${d.chart.points[0]} → ${d.chart.points[d.chart.points.length - 1]}`) }),
        DS.twoCol(brk(d.breakdowns[0]), brk(d.breakdowns[1])),
        DS.twoCol(brk(d.breakdowns[2]), brk(d.breakdowns[3])),
        DS.twoCol(brk(d.breakdowns[4]), DS.card({ title: d.channelMetrics.title, body: DS.dataTable(d.channelMetrics.headers, d.channelMetrics.rows.map((r) => `<tr>${DS.tds(r[0])}${r.slice(1).map((c) => DS.tdn(c)).join("")}</tr>`).join("")) })),
        DS.card({ title: d.mailboxComparison.title, body: DS.dataTable(d.mailboxComparison.headers, d.mailboxComparison.rows.map((r) => `<tr>${DS.tds(r[0])}${r.slice(1).map((c) => DS.tdn(c)).join("")}</tr>`).join("")) }),
        DS.card({ title: "Issue radar", sub: d.radar.hint, body: d.radar.alerts.map((a) => `<div class="ds-row">${DS.aiBadge(a.sev)}<span>${esc(a.text)}</span></div>`).join("") }),
        DS.quirk("Data-layer quirk kept honest: 'By channel' ignores the channel chip (dashboard.rs:337-345) — presentation cannot fix SQL, so it is labeled here instead of hidden."),
      ];
    },
  });

  /* ============================ Live chats ================================ */
  DS.definePage("live-chats", {
    archetype: "stack",
    state: { view: "open", sel: "chat-1" },
    meta: (st) => ({ title: M("live-chats").title, subtitle: M("live-chats").subtitle, counter: LAB.PAGES["live-chats"].counter }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES["live-chats"];
      const chips = DS.toolbar(`<span class="ds-field-inline">view</span>${DS.chip("open chats", st.view === "open", "lc-view", 0)}${DS.chip("show ended chats", st.view !== "open", "lc-view", 1)}`);
      const list = d.chats.filter((c) => (st.view === "open" ? c.state === "open" : c.state === "ended"));
      if (!list.length) {
        return [chips, DS.emptyState(ic("send"), st.view === "open" ? "No live chats yet" : "No ended chats",
          "Chats appear here the moment the beacon.chat.created webhook lands — connect the webhook relay (Settings) and they will arrive on their own.",
          false, DS.btn.outline("Open Settings", "lc-goto-settings"))];
      }
      const chat = d.chats.find((c) => c.id === st.sel) || list[0];
      const rows = list.map((c) => DS.selRow(`${DS.dot(c.state === "open" ? "ok" : "closed")} ${esc(c.name || "visitor")} · ${esc(c.last)}`, esc(c.preview), { selected: c.id === chat.id, act: "lc-sel", id: c.id })).join("");
      const msgs = chat.messages.length
        ? chat.messages.map((m) => DS.messageCard({ role: m.who === "customer" ? "customer" : "agent", name: m.who, time: m.ts, body: `<p>${esc(m.text)}</p>` })).join("")
        : DS.hint("no messages stored for this chat yet");
      const sendHint = chat.state === "ended"
        ? DS.callout(chat.endedAs ? `this chat ended — it became conversation #${chat.endedAs}` : "this chat ended", "err")
        : !chat.assignee ? DS.callout("no agent is assigned — sending accepts this chat automatically") : "";
      const sendBox = chat.state === "open"
        ? `${sendHint}${DS.textarea("reply to the visitor…", 2)}<div class="ds-tool-row split"><span class="ds-hint">the message id is generated here — a retry can never double-send</span>${DS.btn.primary("send", "lc-send")}</div>`
        : `<div class="ds-tool-row">${DS.statusBadge(chat.state === "open" ? "resolved" : "closed", chat.state)}</div>`;
      return [chips, DS.twoCol(
        `<div>${DS.sectionTitle("Chats")}<div class="ds-scroll-360 ds-sel-list">${rows}</div></div>`,
        DS.card({ title: `Chat with ${chat.name || "a visitor"}`, sub: `state: ${chat.state} · mailbox #${chat.mailbox} · last event ${chat.last}`,
          body: `<div class="ds-scroll-360">${msgs}</div><div class="ds-tool-row">${DS.statusBadge(chat.state === "open" ? "ok" : "closed", chat.state)}</div>${sendBox}` }), true)];
    },
    act: {
      "lc-view": (el, ctx) => { ctx.state.view = el.dataset.i === "0" ? "open" : "ended"; ctx.rerender(); },
      "lc-sel": (el, ctx) => { ctx.state.sel = el.dataset.id; ctx.rerender(); },
      "lc-send": () => DS.toast("Message queued — the id is generated at send time, so a retry can never double-send (mock)."),
      "lc-goto-settings": () => { location.hash = "#settings"; },
    },
  });

  /* ============================ Operations (DS-A-06) ====================== */
  DS.definePage("operations", {
    archetype: "console",
    state: { tab: 0, mailbox: 0 },
    meta: () => ({ title: M("operations").title, subtitle: M("operations").subtitle }),
    tabs: (st) => ({ items: ["Tiles", "Workload", "Library"], active: st.tab }),
    blocks: (st) => {
      const d = LAB.PAGES.operations;
      if (st.tab === 0) {
        const out = [DS.hint(d.generated)];
        for (const g of ["Critical", "Warning", "Info"]) {
          const tiles = d.tiles.filter((t) => t.sev === g);
          if (!tiles.length) continue;
          out.push(DS.sectionTitle(g) + `<div class="ds-tile-wrap band-${g.toLowerCase()}">` +
            tiles.map((t) => DS.metricTile({ cnt: t.count, lbl: t.label, level: t.sev === "Critical" ? "crit" : t.sev === "Warning" ? "warning" : "info", drill: t.drill ? "op-drill" : null })).join("") + `</div>`);
        }
        out.push(DS.hint("(tiles marked with an arrow drill into the inbox)"));
        return out;
      }
      if (st.tab === 1) {
        return [
          DS.card({ title: "Team Workload", sub: "generated 2026-10-27T09:59:59.123Z · unassigned work 4", body: DS.dataTable(
            ["Agent", "Open", "Waiting", "Capacity", "Load", "Pressure", "Actions"],
            d.workload.rows.map((r) => `<tr>${DS.tds(r.agent)}${DS.tdn(r.open)}${DS.tdn(r.waiting)}${DS.tdn(r.capacity)}${DS.tdn(r.load)}<td>${DS.barCell(parseFloat(r.pressure) * 20, r.pressure)}</td><td>${DS.btn.outline("+")}${DS.btn.outline("−")}</td></tr>`).join("")) }),
          DS.quirk("Reskin fix (presentation): the current build swaps the Load/Pressure columns — % under 'Load', weighted number under 'Pressure' (operations.rs:425-426). Labels here match the values."),
          DS.card({ title: "Suggested Assignees", sub: "Agents with the lowest load are suggested for new assignments. Use the +/− buttons to adjust individual capacity.",
            body: d.workload.suggested.map((s) => DS.row(muted(s))).join("") }),
        ];
      }
      return [
        DS.toolbar(d.mailboxes.map((m, i) => DS.chip(m, i === st.mailbox, "op-mailbox", i)).join("")),
        DS.row(`<span class="ds-strong">Saved replies</span>`, DS.btn.primary("new saved reply")),
        DS.card({ body: LAB.SAVED_REPLIES.map((r, i) => DS.row(`${esc(r.title)} — ${esc(r.body.slice(0, 72))}…`, `${DS.btn.outline("edit")}${DS.btn.danger("delete", "op-del", i)}`)).join("") + DS.hint("no saved replies in this mailbox yet — the sync mirrors the platform's list") }),
        DS.quirk("Reskin companion (R3): these saved replies are also insertable from the Inbox composer — already prototyped in the Inbox After view (operations.rs:746)."),
        DS.sectionTitle("Workflows"),
        DS.card({ body: DS.formGrid([["conversation #:", DS.input("e.g. 778", 110)]]) +
          DS.hint("the run resolves the number against the local mirror; the platform enforces same-inbox and manual-only (a 400 lands in the jobs table)") +
          d.workflows.map((w) => DS.row(`${DS.badge(w.status === "active" ? "success" : "neutral", w.status)} ${esc(w.name)} · ${esc(w.kind)}`, w.kind === "manual" ? DS.btn.outline("run") : "")).join("") }),
      ];
    },
    act: { "op-drill": () => { location.hash = "#inbox"; }, "op-mailbox": (el, ctx) => { ctx.state.mailbox = Number(el.dataset.i); ctx.rerender(); } },
  });

  /* ============================ Customers (DS-A-01) ======================= */
  DS.definePage("customers", {
    archetype: "list",
    meta: () => ({ title: M("customers").title, subtitle: M("customers").subtitle, counter: `${LAB.PAGES.customers.results.length} customers`, actions: DS.syncControl() }),
    list: () => {
      const d = LAB.PAGES.customers;
      return {
        toolbar: { left: DS.input("Search customers by name or email…", 280), apply: "search" },
        headers: ["name", "email", "conversations", "last contact", ""],
        rows: d.results.map((r, i) => `<tr>${DS.tds(r.name)}<td>${esc(r.email)}</td>${DS.tdn(r.conversations)}<td class="ds-muted2">${esc(r.last)}</td><td>${DS.btn.outline("open", "open-detail", i)}</td></tr>`).join(""),
      };
    },
    detail: (i) => {
      const p = LAB.PAGES.customers.profile;
      const mixMax = Math.max(...p.eventMix.map((e) => e[1]), 1);
      return {
        title: p.name,
        body: recordHead(p.name, `${esc(p.email)} · ${esc(p.phone)} · ${esc(p.company)} · last activity ${esc(p.lastActivity)}`, DS.statusBadge("active"), "") +
          DS.twoCol(
            DS.card({ title: "Support health", body: DS.formGrid(Object.entries(p.health).map(([k, v]) => [k, mono(v)])) + divider + DS.sectionTitle("Event mix") +
              p.eventMix.map(([k, v]) => DS.barRow(k, v, mixMax, true)).join("") }) +
            DS.card({ title: "Support milestones", sub: "tracked SLA breaches and high-friction flags from the intelligence sweep",
              body: p.milestones.map(([ts, et, det]) => DS.tlRow(ts, et, esc(det))).join("") || DS.emptyState(null, "No milestones recorded", "SLA breaches and high-friction flags appear here once the intelligence sweep records them.", true) }),
            DS.card({ body: `<div class="ds-tool-row">${DS.aiBadge("Interaction profile")}${DS.badge("success", p.stage)}</div>` +
              kv(Object.entries(p.profile)) + divider + DS.fieldLabel("Support approach") + kv(Object.entries(p.recommendation)) + DS.hint(p.provenance) }) +
            DS.card({ title: "Conversations", body: DS.dataTable(["#", "subject", "status", "updated"],
              p.conversations.map(([n, subj, status, upd]) => `<tr>${DS.tdn(n)}<td>${esc(subj)}</td><td>${DS.statusBadge(status)}</td><td class="ds-muted2">${esc(upd)}</td></tr>`).join("")) }),
            false) +
          DS.card({ title: "Timeline", body: `<div class="ds-scroll-220">` + p.timeline.map(([ts, kind, n, sum]) => DS.tlRow(ts, kind, `<span class="ds-mono ds-muted2">#${n}</span> ${esc(sum)}`)).join("") + `</div>` }),
      };
    },
  });

  /* ============================ Organizations (DS-A-01) =================== */
  DS.definePage("organizations", {
    archetype: "list",
    meta: () => ({ title: M("organizations").title, subtitle: M("organizations").subtitle, counter: `${LAB.PAGES.organizations.orgs.length} organizations`, actions: DS.syncControl() }),
    list: () => {
      const d = LAB.PAGES.organizations;
      return {
        toolbar: { left: DS.input("Filter by name…", 260), apply: "filter" },
        headers: ["organization", "domains", "customers", "conversations", ""],
        rows: d.orgs.map((o, i) => `<tr>${DS.tds(o.name)}<td class="ds-mono ds-muted2">${esc(o.domains)}</td>${DS.tdn(o.customers)}${DS.tdn(o.conversations)}<td>${DS.btn.outline("open", "open-detail", i)}</td></tr>`).join(""),
      };
    },
    detail: (i) => {
      const o = LAB.PAGES.organizations.orgs[i];
      if (!o) return { title: "Organization", body: DS.emptyState(ic("organizations"), "Select an organization", "Its members, domains, and conversation counts appear here.", true) };
      return {
        title: o.name,
        body: recordHead(o.name, mono(o.domains), DS.badge("neutral", `${o.customers} customers`), "") +
          DS.card({ title: "Members", sub: `${o.members.length} member(s)`, body: o.members.map(([n, e, jt]) => DS.row(`${esc(n)} · ${esc(e)}${jt ? ` (${esc(jt)})` : ""}`)).join("") }) +
          DS.hint("Members come from the mirror's customer→organization edges; the live human edges feed the support graph."),
      };
    },
    blocks: () => [
      DS.card({ title: "New organization", body: DS.formGrid([["name", DS.input("Name")], ["domains", `<input class="ds-text-input" placeholder="acme.com, acme.io" title="comma-separated domains" aria-label="domains">`]]) + DS.btn.primary("create organization") }),
    ],
  });

  /* ============================ Knowledge ================================= */
  DS.definePage("knowledge", {
    archetype: "stack",
    state: { tab: 0, arm: {} },
    meta: () => ({ title: M("knowledge").title, subtitle: M("knowledge").subtitle, actions: DS.syncControl("Sync Help Center now") }),
    tabs: (st) => ({ items: ["Documents", "Sources", "Freshness", "Help center", "Article drafts"], active: st.tab }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES.knowledge;
      const out = [DS.banner(M("knowledge").banner || "")];
      if (st.tab === 0) {
        out.push(DS.hint("Local knowledge documents used by search and AI answers"));
        out.push(DS.card({ body: DS.dataTable(["title", "source", "visibility", "chunks", "actions"],
          d.documents.map(([t, s, v, c], i) => `<tr>${DS.tds(t)}<td class="ds-mono ds-muted2">${esc(s)}</td><td>${mono(v)}</td>${DS.tdn(c)}<td>${DS.btn.outline("mark reviewed", "kn-review")}${DS.btn.danger("delete", "kn-del", i)}</td></tr>`).join("")) }));
        out.push(DS.quirk("A-5 (DS-0): the reviewed/verify/delete triple collapsed to two inline actions — review marks the document reviewed; delete is danger + confirm."));
      } else if (st.tab === 1) {
        out.push(DS.card({ body: DS.dataTable(["name", "kind", "visibility", "documents", "created"],
          d.sources.map(([n, k, v, dc, cr]) => `<tr>${DS.tds(n)}<td>${esc(k)}</td><td>${esc(v)}</td>${DS.tdn(dc)}<td class="ds-mono ds-muted2">${esc(cr)}</td></tr>`).join("")) }));
      } else if (st.tab === 2) {
        out.push(DS.hint(d.freshnessHint));
        out.push(DS.emptyState(ic("knowledge"), "No freshness data",
          "The current build reads report[\"flags\"], which freshness_report() never emits — the four flag cards can never render (knowledge.rs:1249 vs knowledge_store.rs:516-530). Next step: re-enable the cards once the data layer emits flags."));
      } else if (st.tab === 3) {
        out.push(DS.statGrid(d.docsStats));
        out.push(DS.toolbar(`${DS.combo("collection: All")}${DS.combo("status: Any")}${DS.input("Search help center…", 320)}`, DS.btn.primary("apply")));
        out.push(DS.card({ title: "Knowledge gaps (tracked)", sub: "zero-hit searches recorded as gaps; stale count from the tracked freshness rows",
          body: `<div class="ds-chip-row">${d.gaps.map((g) => DS.badge("warning", g)).join("")}${DS.badge("warning", d.stale)}</div>` }));
        out.push(DS.card({ body: DS.dataTable(["article", "collection", "status", "views", ""],
          d.articles.map(([a, c, s, v]) => `<tr>${DS.tds(a)}<td>${esc(c)}</td><td>${DS.statusBadge(s)}</td>${DS.tdn(v)}<td>${DS.btn.outline("read")}</td></tr>`).join("")) }));
      } else {
        out.push(DS.hint("AI drafts knowledge articles from real resolutions; a human review is the only path to publishing. Approved drafts publish to Help Scout through the Docs API."));
        out.push(DS.card({ title: "Draft an article from a gap", body: DS.toolbar(DS.combo("Select a gap…"), DS.btn.primary("Draft article")) }));
        out.push(DS.card({ body: d.drafts.map((dr) => DS.row(
          `${esc(dr.id)} — <span class="ds-strong">${esc(dr.title)}</span> ${dr.status === "approved" ? DS.badge("success", dr.status) : smallMuted(dr.status)}${dr.publish ? ` ${smallMuted(dr.publish)}` : ""}`,
          DS.btn.outline("Open"))).join("") }));
      }
      return out;
    },
    act: {
      "kn-review": () => DS.toast("Document marked reviewed — verify runs with the next freshness pass (mock)."),
      "kn-del": (el, ctx) => {
        const k = el.dataset.i;
        if (ctx.state.arm[k]) { delete ctx.state.arm[k]; DS.toast("Delete is mocked in the design lab — no data was harmed."); }
        else ctx.state.arm[k] = true;
        ctx.rerender();
      },
    },
  });

  /* ============================ Reports =================================== */
  DS.definePage("reports", {
    archetype: "stack",
    state: { tab: 0, range: 1 },
    meta: (st) => ({ title: M("reports").title, subtitle: M("reports").subtitle,
      actions: `<span class="ds-chip-row">${LAB.PAGES.reports.ranges.map((r, i) => DS.chip(r, i === st.range, "rep-range", i)).join("")}</span>${DS.syncControl()}` }),
    tabs: (st) => {
      const d = LAB.PAGES.reports;
      return { groups: [
        { label: "Volume & quality", tabs: d.tabs.slice(0, 5), from: 0, active: st.tab < 5 ? st.tab : -1 },
        { label: "AI & digests", tabs: d.tabs.slice(5, 8), from: 5, active: st.tab >= 5 && st.tab < 8 ? st.tab - 5 : -1 },
        { label: "Platform & custom", tabs: d.tabs.slice(8, 11), from: 8, active: st.tab >= 8 ? st.tab - 8 : -1 },
      ] };
    },
    blocks: (st) => {
      const d = LAB.PAGES.reports;
      const t = d.tabs[st.tab];
      const tbl2 = (headers, rows) => DS.dataTable(headers, rows.map((r) => `<tr>${DS.tds(r[0])}${r.slice(1).map((c) => DS.tdn(c)).join("")}</tr>`).join(""));
      if (t === "Overview") return [DS.statGrid(d.overview), DS.hint(d.ratingsNote), DS.card({ title: d.chart.title, body: DS.lineChart(d.chart.points, d.chart.annotation) })];
      if (t === "SLA") return [
        DS.card({ title: "First response avg (business minutes)", body: d.sla.bars.map(([l, v]) => DS.barRow(l, v, Math.max(...d.sla.bars.map(([, x]) => x)), true)).join("") + DS.hint("first response avg · max 55") }),
        DS.card({ body: tbl2(d.sla.headers, d.sla.rows) }),
        DS.card({ title: "Persisted breach tracking", sub: "recorded breach events; the thresholds feed the coarse tracker for mailboxes without business-hours config",
          body: `${DS.badge("info", "9 recorded breach event(s)")}${divider}${DS.sectionTitle("Tracking thresholds (hours)")}<div class="ds-chip-row">${DS.chip("Support", true)}${DS.chip("Billing")}</div>${DS.formGrid([["first response hours:", DS.input("", 120)], ["resolution hours:", DS.input("", 120)]])}${DS.btn.primary("Save thresholds")}` }),
      ];
      if (t === "Why contacting" || t === "Top questions") {
        const rows = t === "Why contacting" ? d.whyContacting : d.topQuestions;
        return [DS.card({ title: t.toLowerCase(), body: rows.map(([q, c]) => DS.row(esc(q), DS.badge("neutral", c))).join("") })];
      }
      if (t === "Quality") return [
        DS.card({ title: "Post-resolution QA", sub: "deterministic checks over closed conversations (AI layer per conversation in the inbox)",
          body: DS.formGrid([["Closed conversations", mono(d.quality.closed)], ["QA rows computed", mono(d.quality.rows)], ["Conversations with the AI layer", mono(d.quality.aiLayer)]]) +
            DS.hint("Deterministic checks: back-and-forth, repeated information, messages after close, handoffs, first-response and resolution minutes — per conversation in the inbox detail.") }),
        DS.card({ title: "Recent closed conversations", sub: "the 25 most recent closures with their QA rows",
          body: d.quality.recent.map((r) => DS.row(mono(`#${r.id}`), `<span class="ds-chip-row">${r.badges.map((b) => DS.badge(b === "AI review" ? "success" : "neutral", b)).join("")}</span>`)).join("") }),
      ];
      if (t === "Issue radar") return [
        DS.card({ title: "Issue radar", sub: "AI-derived alerts from local conversation data",
          body: d.radar.map((a) => `<div class="ds-row">${DS.aiBadge(a.sev)}<span>${esc(a.text)}</span></div>`).join("") }),
        DS.quirk("Reskin honesty: this tab's badge is always 'medium'/Warning in the current build because the engine never emits 'high' (reports.rs:1997-2001). The raw severity token is shown here."),
      ];
      if (t === "Digest") {
        const dg = d.digest.detail;
        return [
          DS.hint("AI-written weekly summaries over SQL-computed facts — volume, topics, sentiment, anomalies, workload, and what changed. Archived and searchable."),
          DS.toolbar(`${DS.btn.primary("Generate digest now")}${DS.input("search digests…", 220)}`),
          `<div class="ds-chip-row">${d.digest.chips.map(([ts, s]) => DS.chip(`${ts} — ${s}`, true)).join("")}</div>`,
          DS.card({ title: dg.title, sub: "AI-generated narrative — numbers are SQL facts, not model output",
            body: `<p>${esc(dg.narrative)}</p>${DS.sectionTitle("Anomalies")}${dg.anomalies.map((a) => `<div class="ds-row">${DS.badge("warning", "anomaly")}<span>${esc(a)}</span></div>`).join("")}${DS.sectionTitle("What changed")}${dg.changed.map((c) => DS.row(esc(c))).join("")}` }),
        ];
      }
      if (t === "Happiness") return [
        DS.statGrid(d.happiness.stats), DS.hint(d.happiness.note),
        DS.card({ body: DS.dataTable(["Rating", "Conversation", "Customer", "Comment"],
          d.happiness.rows.map(([r, n, cu, cm]) => `<tr><td>${r === "Great" ? `<span class="ds-link">${esc(r)}</span>` : r === "Not good" ? DS.badge("danger", r) : esc(r)}</td>${DS.tdn(n)}<td>${esc(cu)}</td><td>${esc(cm)}</td></tr>`).join("")) }),
      ];
      if (t === "Help Scout") return [
        DS.hint(d.helpScout.banner),
        DS.toolbar(DS.combo("Company — Company overall"), DS.btn.outline("Fetch from Help Scout")),
        DS.emptyState(ic("reports"), "no reports fetched yet", "Pick a report and fetch it — results cache here with their range."),
      ];
      if (t === "Builder") return [
        DS.card({ body: DS.sectionTitle("Report definition") +
          DS.toolbar(`${DS.fieldLabel("metric:")} ${DS.combo("Conversations")}${DS.fieldLabel("grouped by:")} ${DS.combo("Mailbox")}<label class="ds-check"><input type="checkbox"> compare with previous period</label>${DS.fieldLabel("sort:")} ${DS.chip("metric ↓", true)}${DS.chip("label A→Z")}`) +
          DS.formGrid([["from (YYYY-MM-DD):", DS.input("", 110)], ["to:", DS.input("", 110)], ["limit:", DS.input("50", 60)]]) +
          DS.sectionTitle("Filters (blank = unfiltered)") +
          DS.formGrid(["channel:", "statuses (csv):", "tags any (csv):", "tags none (csv):", "min priority:", "attribute key:", "attribute value:", "state key:", "mailbox ids (csv):", "assignee ids (csv):"].map((f) => [f, DS.input("", 180)]), true) +
          DS.toolbar(`${DS.btn.primary("Run report")}${DS.fieldLabel("save as:")} ${DS.input("name", 160)}${DS.btn.outline("Save")}`) }),
        DS.card({ title: "Saved reports", sub: "load or delete a saved configuration",
          body: d.builder.saved.map((s, i) => DS.row(mono(s.name), `${smallMuted(s.created)}${DS.btn.outline("load")}${DS.btn.danger("delete", "rep-del", i)}`)).join("") }),
        DS.card({ title: d.builder.result.title, sub: `${d.builder.result.sub}  ·  local`, body: tbl2(d.builder.result.headers, d.builder.result.rows) }),
      ];
      const ins = d.insights;
      return [
        DS.card({ title: "Health facts", sub: "the last 30 days, over the whole mirror", body: DS.statGrid(ins.health) }),
        DS.card({ title: "Post-resolution QA effectiveness", sub: "avg QA score and the score distribution over every stored checks",
          body: DS.statGrid([{ label: "avg QA score", value: ins.qa.avg }, { label: "checks stored", value: ins.qa.checks }]) + `<div class="ds-chip-row">${ins.qa.dist.map(([b, tone]) => DS.badge(tone, b)).join("")}</div>` }),
        DS.card({ title: "Customer effort (friction)", sub: "scores persist per conversation; crossing the threshold records a customer milestone",
          body: `${DS.badge("warning", "6 high-friction conversation(s)")}${DS.toolbar(DS.btn.outline("Recompute (last 100 closed)"))}` +
            DS.dataTable(ins.friction.headers, ins.friction.rows.map((r) => `<tr>${r.map((c, i2) => (i2 === 0 ? DS.tdn(c) : `<td>${esc(c)}</td>`)).join("")}</tr>`).join("")) }),
        DS.card({ title: "Graph nodes", sub: "custom nodes over the support graph; neighbors come from the live human edges",
          body: DS.formGrid([["kind:", DS.combo("customer")], ["label:", DS.input("", 180)], ["entity id:", DS.input("", 180)], ["properties (JSON):", DS.input("", 180)]]) + DS.btn.primary("Add node") +
            DS.dataTable(ins.graph.headers, ins.graph.rows.map((r) => `<tr><td>${esc(r[0])}</td><td>${esc(r[1])}</td><td class="ds-mono ds-muted2">${esc(r[2])}</td><td class="ds-mono ds-muted2">${esc(r[3])}</td><td>${DS.btn.outline("inspect")}</td></tr>`).join("")) }),
      ];
    },
    act: { "rep-range": (el, ctx) => { ctx.state.range = Number(el.dataset.i); ctx.rerender(); }, "rep-del": (el, ctx) => { const k = "r" + el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Delete is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); } },
  });

  /* ============================ Issue Radar =============================== */
  DS.definePage("issue-radar", {
    archetype: "stack",
    state: { tab: 0, arm: {} },
    meta: () => ({ title: M("issue-radar").title, subtitle: M("issue-radar").subtitle,
      actions: `${DS.btn.outline("Cluster now")}${DS.btn.outline("Reload clusters")}${DS.btn.outline("Detect gaps now")}` }),
    tabs: (st) => ({ items: ["Alerts", "Clusters", "Gaps", "Known issues"], active: st.tab }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES["issue-radar"];
      if (st.tab === 0) return [
        DS.card({ title: `${d.alerts.length} radar alert(s)`, sub: "AI-derived from local conversation data",
          body: d.alerts.map((a) => `<div class="ds-row">${DS.badge(DS.toneOf(SEV[a.sev] || "medium"), a.sev)}<span>${esc(a.text)}</span></div>`).join("") }),
        DS.quirk("Raw severity tokens (critical/warning/info) — the engine's own strings; info keeps the Warning tone exactly as the current build (issue_radar.rs:703-723)."),
        DS.card({ title: d.sla.title, body: d.sla.rows.map(([text, cls]) => DS.callout(text, cls === "err" ? "err" : cls === "warn" ? "warn" : null)).join("") }),
      ];
      if (st.tab === 1) return [DS.row(`<span class="ds-strong">${d.clusters.length} cluster(s)</span>`)].concat(
        d.clusters.map((c) => DS.card({ title: c.title, body: `${c.trend ? DS.badge(["rising", "new"].includes(c.trend) ? "warning" : "neutral", c.trend) : ""}${DS.hint(c.hint)}${c.summary ? DS.hint(c.summary) : ""}` })));
      if (st.tab === 2) {
        const out = [`<div class="ds-chip-row">${d.gapBadges.map(([b, tone]) => DS.badge(tone, b)).join("")}</div>`];
        for (const k of d.gapKinds) {
          out.push(DS.card({ title: k.kind, body: k.cands.map((c) => `${divider}<div class="ds-tool-row">${DS.badge("warning", `${c.count}×`)}<span class="ds-strong">${esc(c.query)}</span>${DS.badge(c.status === "approved" ? "success" : c.status === "rejected" ? "neutral" : "info", c.status)}</div>
            ${DS.hint(c.explain)}${DS.hint(`examples: ${c.examples.join(" · ")}`)}
            ${c.status === "open" ? `<div class="ds-tool-row split">${DS.btn.outline("approve", "ir-approve")}<span>${confirmBtn(c.query, "reject", "ir-reject")}</span></div>` : ""}`).join("") }));
        }
        return out;
      }
      return [
        DS.toolbar(DS.btn.primary("New known issue")),
        DS.row(`<span class="ds-strong">${d.knownIssues.length} known issue(s)</span>`),
      ].concat(d.knownIssues.map((k) => DS.card({ title: k.title,
        body: `${DS.badge("neutral", k.status)} ${smallMuted(`${k.convs} conversations`)}${DS.hint(k.symptoms)}${DS.row(`workaround: ${esc(k.workaround)}`)}` })));
    },
    act: {
      "ir-approve": () => DS.toast("Gap candidate approved — it becomes a knowledge gap row (mock)."),
      "ir-reject": (el, ctx) => { const k = el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Candidate rejected (mock)."); } else armed[k] = true; ctx.rerender(); },
    },
  });

  /* ============================ Incidents (DS-A-01) ======================= */
  DS.definePage("incidents", {
    archetype: "list",
    meta: () => ({ title: M("incidents").title, subtitle: M("incidents").subtitle, counter: `${LAB.PAGES.incidents.list.length} incidents`, actions: DS.btn.primary("New Incident") }),
    list: () => {
      const d = LAB.PAGES.incidents;
      return {
        toolbar: { left: `${DS.combo("status: Any")}${DS.combo("severity: Any")}<label class="ds-check"><input type="checkbox"> open only</label>${DS.input("search incidents…", 180)}`, apply: "apply" },
        headers: ["code", "title", "severity", "status", "conversations", ""],
        rows: d.list.map((it, i) => `<tr>${DS.tds(it.code)}<td>${esc(it.title)}</td><td>${DS.statusBadge(SEV[it.sev] || "medium", it.sev)}</td><td>${DS.statusBadge(INC[it.status] || "closed", it.status)}</td>${DS.tdn(it.convs)}<td>${DS.btn.outline("open", "open-detail", i)}</td></tr>`).join(""),
      };
    },
    detail: (i) => {
      const d = LAB.PAGES.incidents;
      const it = d.list[i] || d.detail;
      const det = d.detail;
      return {
        title: `${it.code} · ${it.title}`,
        body: recordHead(`${it.code} · ${it.title}`, `started: ${mono(det.started)} · owner: ${esc(det.owner)}`,
          `${DS.statusBadge(INC[det.status] || "closed", it.status)}${DS.statusBadge(SEV[it.sev] || "medium", it.sev)}`, "", null) +
          DS.card({ title: "description", body: `<p>${esc(det.description)}</p>` }) +
          DS.card({ title: "customer-safe", body: `<p>${esc(det.customerSafe)}</p>` }) +
          DS.twoCol(
            DS.card({ title: `Linked conversations (${det.linked.length})`, body: det.linked.map(([n, s]) => DS.row(mono(`#${n}`), esc(s))).join("") +
              DS.formGrid([["link conversation #:", DS.input("id", 140)]]) + DS.btn.primary("link") }) +
            DS.card({ title: `Releases (${det.releases.length})`, body: det.releases.map(([v, s]) => DS.row(`${esc(v)} · ${esc(s)}`)).join("") }),
            DS.card({ title: `Affected customers (${det.customers.length})`, body: det.customers.map((c) => DS.row(esc(c))).join("") }) +
            DS.card({ title: `Notes (${det.notes.length})`, body: det.notes.map(([ts, body]) => DS.card({ body: `${smallMuted(ts)}<div>${esc(body)}</div>` })).join("") +
              DS.textarea("write an incident note…", 2) + DS.btn.primary("add note") }),
            false),
      };
    },
  });

  /* ============================ Custom Objects ============================ */
  DS.definePage("custom-objects", {
    archetype: "stack",
    state: { sel: 0, arm: {} },
    meta: (st) => ({ title: M("custom-objects").title, subtitle: M("custom-objects").subtitle, counter: `${LAB.PAGES["custom-objects"].types.length} types`, actions: DS.btn.primary("New Type") }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES["custom-objects"];
      const t = d.types[st.sel];
      const left = DS.sectionTitle("Types") + d.types.map((ty, i) =>
        DS.row(`<button class="ds-chip${i === st.sel ? " on" : ""}" data-ds-act="co-sel" data-i="${i}" type="button">${esc(ty.name)} (${ty.count})</button>`, DS.btn.danger("delete", "co-del", i)) + (ty.desc ? DS.hint(ty.desc) : "")).join("");
      const right = t.count === 0
        ? DS.emptyState(ic("custom-objects"), "No objects", "Create one below or adjust the search — objects store per-customer properties used by the graph and AI answers.")
        : `<div class="ds-tool-row">${DS.sectionTitle(`${t.name} objects`)}<span class="ds-counter">${t.count}</span></div>` +
          DS.toolbar(`${DS.input("search objects…", 220)}`, DS.btn.primary("search")) +
          d.objects.map((o) => DS.card({ body: `<div class="ds-tool-row"><span class="ds-strong">${esc(o.title)}</span>${DS.btn.danger("delete", "obj-del", esc(o.title))}</div>` +
            kv(o.props.map(([k, v]) => [k, mono(typeof v === "string" ? v : JSON.stringify(v))])) })).join("") +
          DS.card({ title: "New object", body: DS.formGrid([["title", DS.input("object title")]].concat(d.fieldTypes.slice(0, 2).map((f) => [`${f}:`, DS.input("", 180)]))) + DS.btn.primary("create object") });
      return [DS.twoCol(`<div class="ds-scroll-360">${left}</div>`, `<div class="ds-scroll-360">${right}</div>`)];
    },
    act: {
      "co-sel": (el, ctx) => { ctx.state.sel = Number(el.dataset.i); ctx.rerender(); },
      "co-del": (el, ctx) => { const k = "t" + el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Delete is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); },
      "obj-del": (el, ctx) => { const k = "o" + el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Delete is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); },
    },
  });

  /* ============================ Automation ================================ */
  DS.definePage("automation", {
    archetype: "stack",
    state: { open: false, arm: {} },
    meta: () => ({ title: M("automation").title, subtitle: M("automation").subtitle, actions: DS.btn.primary("New Rule", "aut-open") }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES.automation;
      const out = [];
      if (st.open) {
        out.push(DS.card({ body: DS.sectionTitle("New Rule") +
          DS.callout("enabled rules act on conversations automatically — actions that need review park in the approvals queue", "warn") +
          DS.formGrid([
            ["name:", DS.input("rule name", 220)],
            ["trigger:", d.triggers.map((tg) => DS.chip(tg, tg === d.triggers[0])).join("")],
            ["from → to:", `${DS.input("from status", 100)} <span class="ds-field-inline">→</span> ${DS.input("to status", 100)}`],
            ["action:", d.actions.map((a) => DS.chip(a, a === d.actions[1])).join("")],
            ["tag:", DS.input("tag", 180)],
          ]) + DS.btn.primary("Create Rule", "aut-close") }));
      }
      out.push(DS.card({ title: "Rules", sub: `${d.rules.rows.length} rule(s)`, body: DS.dataTable(d.rules.headers, d.rules.rows.map((r, i) =>
        `<tr>${DS.tds(r.name)}<td>${DS.badge("neutral", r.when)}</td><td>${DS.badge("neutral", r.then)}${r.approval ? ` ${DS.badge("warning", "needs approval")}` : ""}</td><td>${DS.chip(r.enabled ? "enabled" : "disabled", r.enabled, "aut-toggle", i)} ${confirmBtn("ru" + i, "delete", "aut-del")}</td></tr>`).join("")) }));
      out.push(DS.card({ title: "Pending approvals", sub: `${d.approvals.length} pending approval(s)`, body:
        d.approvals.map((a) => {
          const raw = a.text.slice(a.text.indexOf("·") + 2);
          const human = raw.includes("assign") ? "assign to user #506" : raw;
          const who = a.text.slice(0, a.text.indexOf("·"));
          return `${divider}<div class="ds-tool-row"><span class="ds-strong">${esc(who)}</span>${DS.badge("neutral", human)}</div>
            <details class="ds-collapse"><summary>raw proposed action</summary><div class="ds-mono ds-small ds-muted2">${esc(raw)}</div></details>
            <div class="ds-tool-row split">${DS.btn.outline("approve", "aut-approve")}<span>${DS.btn.danger("reject", "aut-reject")}</span></div>`;
        }).join("") + (d.approvals.length ? "" : DS.emptyState(ic("shell"), "No approvals waiting", "Actions that need review will park here.", true)) }));
      out.push(DS.quirk("Reskin fix (presentation): approval rows led with the raw proposed_action_json (automation.rs:492-501). Human-readable action first, raw JSON collapsible."));
      return out;
    },
    act: {
      "aut-open": (el, ctx) => { ctx.state.open = true; ctx.rerender(); },
      "aut-close": (el, ctx) => { ctx.state.open = false; DS.toast("Rule created (mock) — it would join the rules table after the sync."); ctx.rerender(); },
      "aut-toggle": (el, ctx) => { const i = Number(el.dataset.i); LAB.PAGES.automation.rules.rows[i].enabled = !LAB.PAGES.automation.rules.rows[i].enabled; ctx.rerender(); },
      "aut-del": (el, ctx) => { const k = el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Delete is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); },
      "aut-approve": () => DS.toast("Approval granted — the action runs through the jobs table (mock)."),
      "aut-reject": () => DS.toast("Approval rejected (mock)."),
    },
  });

  /* ============================ AI Center (DS-A-06) ======================= */
  DS.definePage("ai-center", {
    archetype: "console",
    state: { tab: 0 },
    meta: () => ({ title: M("ai-center").title, subtitle: M("ai-center").subtitle }),
    tabs: (st) => ({ items: LAB.PAGES["ai-center"].tabs, active: st.tab }),
    blocks: (st) => {
      const d = LAB.PAGES["ai-center"];
      const t = d.tabs[st.tab];
      if (t === "Health") {
        const h = d.health;
        return [
          DS.card({ title: "LM Studio settings", body: DS.formGrid(h.lmstudio.map(([label, val]) => {
            const [kind, v] = val.split("|");
            return [label, kind === "badge-success" ? DS.badge("success", v) : mono(v)];
          })) + DS.hint("Start LM Studio locally (default http://127.0.0.1:1234) and configure the models in Settings → LM Studio. Attributes stay local and never sync back.") +
            `<span class="ds-status-line ok">connection: connected · 3 model(s)</span>${DS.btn.primary("test connection")}` }),
          DS.card({ title: "AI provider record (ai_settings)", sub: "the pre-hub fallback record the chat pipeline and embedding resolution consult when the Settings → LM Studio keys are unset",
            body: DS.toolbar(`<span class="ds-field-inline">provider:</span>${h.providerRecord.chips.map((c, i) => DS.chip(c, i === 1)).join("")}`, DS.badge("success", h.providerRecord.ready)) +
            DS.formGrid([["chat model:", DS.input("(empty — unset)", 260)], ["embedding model:", DS.input("(empty — embedding columns stay unchanged)", 260)], ["embedding dim:", DS.input("e.g. 768", 260)], ["base url:", DS.input("http://127.0.0.1:1234", 260)]]) +
            DS.btn.primary("Save provider record") + `<div class="ds-hint ds-mono">${esc(h.providerRecord.stored)}</div>` }),
          DS.card({ title: "Provider hub — endpoints", sub: "OpenAI-compatible endpoints: local LM Studio, optional Ollama, cloud BYOK",
            body: h.endpoints.map((e) => `${divider}<div class="ds-tool-row">${DS.badge(e.on ? "success" : "neutral", e.on ? "on" : "off")}<span class="ds-strong">${esc(e.name)}</span>${smallMuted(e.url)}${e.keySet ? DS.badge("info", "key set") : ""}${e.builtin ? DS.badge("neutral", "built-in") : ""}</div>
              <div class="ds-tool-row">${DS.btn.outline("set key")}${DS.btn.outline("probe")}${e.builtin ? "" : DS.btn.danger("remove", "ai-ep-del", esc(e.name))}</div>
              ${e.probe ? `<span class="ds-status-line ok">${esc(e.probe)}</span>` : `<span class="ds-status-line err">offline — connection refused (12 ms)</span>`}`).join("") +
            `<details class="ds-collapse"><summary>add endpoint (cloud BYOK — off by default per-feature)</summary>${DS.formGrid([["name:", DS.input("", 180)], ["base url:", DS.input("", 280)], ["api key:", `<input class="ds-text-input" type="password" placeholder="sk-…" aria-label="api key">`], ["$ / mtok prompt:", DS.input("", 120)], ["$ / mtok completion:", DS.input("", 120)]])}${DS.btn.primary("add endpoint")}</details>` }),
          DS.card({ title: "Model-role routing", sub: "every inference goes through one of five roles (blueprint Table 6)",
            body: DS.dataTable(["role", "endpoint", "model", ""], h.routing.map(([role, note], i) =>
              `<tr>${DS.tds(role)}<td><button class="ds-combo" type="button" title="${esc(note)}"><span>${i === 0 ? "(unset)" : "LM Studio — http://127.0.0.1:1234"}</span>${DS.ICONS.chevronD}</button></td><td><input class="ds-text-input" value="${i === 0 ? "" : "qwen2.5-7b-instruct"}" aria-label="model for ${esc(role)}"></td><td class="ds-muted2 ds-small">${esc(note)}</td></tr>`).join("")) }),
          DS.quirk("Reskin fix (presentation): the routing combo showed its index number ('0', '1'), not the endpoint name (ai_center.rs:1444-1448). Names now."),
          DS.card({ title: "Features & kill switches", sub: "one click each; the global switch dominates every feature",
            body: `<div class="ds-form-grid four">${h.features.map((f) => `<label class="ds-check"><input type="checkbox" ${!["ai_note", "draft"].includes(f) ? "checked" : ""}> <span class="ds-mono">${esc(f)}</span></label>`).join("")}</div>${DS.toolbar(DS.btn.outline("global kill switch: AI is ON — click to disable all AI"))}` }),
          DS.card({ title: "Cost ledger — last 7 days", sub: "tokens, latency and estimated spend per feature per day, from ai_runs",
            body: DS.dataTable(h.ledger.headers, h.ledger.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? `<td class="ds-mono ds-muted2">${esc(c)}</td>` : i < 3 ? `<td>${mono(c)}</td>` : DS.tdn(c))).join("")}</tr>`).join("")) }),
        ];
      }
      if (t === "Analytics") return [
        DS.card({ body: `<div class="ds-tool-row">${DS.aiBadge("Analytics")}</div>` + DS.formGrid(d.analytics.map(([k, v]) => [k, mono(v)])) + DS.hint("(no failure patterns)") }),
      ];
      if (t === "Attributes") return [
        DS.card({ body: `<div class="ds-tool-row">${DS.aiBadge("Catalog")}</div>${DS.hint("A missing attribute reads as unknown — it is never fabricated. Attributes are local-only and never written to Help Scout.")}` +
          d.attributes.map((a) => `${divider}<div class="ds-tool-row"><span class="ds-strong">${esc(a.label)}</span>${smallMuted(a.key)}${smallMuted(`${a.known} known / ${a.unknown} unknown`)}</div>` +
            a.values.map(([v, c, s]) => `<div class="ds-mono ds-small">${esc(v)}: ${c} (${s}%)</div>`).join("")).join("") }),
      ];
      if (t === "Copilot") return [
        DS.card({ body: DS.row(`<span class="ds-strong">Sessions</span>`, DS.btn.outline("New chat")) +
          `<div class="ds-chip-row">${d.copilot.sessions.map(([s, c], i) => DS.chip(`${s} (${c})`, i === 0)).join("")}</div>${divider}${DS.sectionTitle("Chat")}` +
          DS.hint("Ask anything answerable from the local mirror — the agent runs 22 read-only tools (searches, lookups, clusters, gaps) and cites what it used.") +
          `<div class="ds-scroll-360">` + d.copilot.chat.map((m) => `<div class="ds-cop-msg ${esc(m.role)}"><span class="ds-who">${m.role === "assistant" ? DS.aiBadge("Copilot") : DS.avatar("You", "sm")}</span><div class="ds-cop-bubble">${esc(m.text)}${m.cited ? `<div class="ds-cited">cited: ${m.cited.map((c) => DS.badge("success", c)).join(" ")}</div>` : ""}</div></div>`).join("") + `</div>` +
          DS.toolbar(DS.input("Ask the copilot…", 320), DS.btn.primary("send")) }),
      ];
      if (t === "Usage") return [
        DS.sectionTitle("Per-feature usage — last 30 days"),
        DS.card({ body: DS.dataTable(d.usage.headers, d.usage.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? `<td class="ds-mono ds-muted2">${esc(c)}</td>` : typeof c === "number" ? DS.tdn(c) : `<td>${mono(c)}</td>`)).join("")}</tr>`).join("")) }),
        DS.card({ title: "Draft feedback (edit rate)", body: `<div class="ds-chip-row">${d.usage.feedback.map(([b, tone]) => DS.badge(tone, b)).join("")}</div>${DS.hint("Accept, edit-distance and reject come from the composer send path and the draft panel — real rows, zero invention.")}` }),
        DS.card({ title: "Golden-set evals (9 seeded cases)", body: `${DS.btn.outline("Run golden set now")}<div class="ds-tool-row">${DS.badge("success", d.usage.golden.badge)}${smallMuted(d.usage.golden.backend)}</div>${DS.hint(d.usage.golden.run)}` }),
      ];
      return [
        DS.card({ body: `<div class="ds-tool-row">${DS.aiBadge("Recent AI runs")}</div>` + DS.dataTable(d.jobs.headers, d.jobs.rows.map(([id, type, status, conv, model, created]) => {
          const key = ["completed", "succeeded"].includes(status) ? "ok" : status === "failed" ? "error" : "syncing";
          return `<tr>${DS.tdn(id)}<td>${mono(type)}</td><td>${DS.statusBadge(key, status)}</td><td class="ds-mono ds-muted2">${esc(conv)}</td><td class="ds-mono ds-muted2">${esc(model)}</td><td class="ds-mono ds-muted2">${esc(created)}</td></tr>`;
        }).join("")) }),
      ];
    },
    act: { "ai-ep-del": (el, ctx) => { const k = "ep" + el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Remove is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); } },
  });

  /* ============================ Outreach ================================== */
  DS.definePage("outreach", {
    archetype: "stack",
    state: { tab: 0, sel: 0, arm: {} },
    meta: () => ({ title: M("outreach").title, subtitle: M("outreach").subtitle }),
    tabs: (st) => ({ items: ["Campaigns", "Segments", "Do not contact"], active: st.tab }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES.outreach;
      if (st.tab === 0) {
        const out = [DS.toolbar(DS.btn.primary("New Campaign"))];
        out.push(d.campaigns.map((c, i) => DS.card({ body: `<div class="ds-tool-row">${DS.chip(c.name, i === st.sel, "ou-sel", i)}${DS.statusBadge(CAMP[c.status] || "closed", c.status)}</div>
          ${DS.hint(c.hint)}
          <div class="ds-tool-row split"><span class="ds-chip-row">${DS.btn.primary("queue")}${DS.btn.outline("pause")}${DS.btn.outline("resume")}</span><span class="ds-chip-row">${DS.btn.danger("cancel remaining", "ou-cancel")}${DS.btn.outline("retry failed")}</span></div>` })).join(""));
        if (st.sel != null) out.push(DS.card({ title: "Campaign report", body: DS.formGrid(d.report.map(([k, v]) => [k, mono(v)])) + DS.callout(d.reportNote, "warn") }));
        return out;
      }
      if (st.tab === 1) {
        return [DS.toolbar(DS.btn.primary("New Segment"))].concat(
          d.segments.map(([name, criteria], i) => DS.card({ body: DS.row(`<span class="ds-strong">${esc(name)}</span>`, DS.btn.danger("delete", "ou-seg-del", i)) + `<div class="ds-mono ds-small ds-muted2">${esc(criteria)}</div>` })));
      }
      return [
        DS.card({ title: "Do not contact", sub: "Customers on this list are excluded from every campaign.",
          body: DS.formGrid([["customer id:", DS.input("customer id", 140)], ["reason:", DS.input("why (optional)", 220)]]) + DS.btn.primary("add") + divider +
            d.dnc.map(([id, reason, ts], i) => DS.row(mono(id), `${smallMuted(reason)}${smallMuted(ts)}${DS.btn.danger("remove", "ou-dnc-del", i)}`)).join("") }),
      ];
    },
    act: {
      "ou-sel": (el, ctx) => { ctx.state.sel = Number(el.dataset.i); ctx.rerender(); },
      "ou-cancel": () => DS.toast("Cancel remaining is mocked in the design lab."),
      "ou-seg-del": (el, ctx) => { const k = "s" + el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Delete is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); },
      "ou-dnc-del": (el, ctx) => { const k = "d" + el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Remove is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); },
    },
  });

  /* ============================ Notifications ============================= */
  DS.definePage("notifications", {
    archetype: "stack",
    state: { tab: 0, read: {} },
    meta: () => ({ title: M("notifications").title, subtitle: M("notifications").subtitle, actions: DS.btn.text("mark all read", "nt-markall") }),
    tabs: (st) => ({ items: ["All", "Mentions", "Unread"], active: st.tab }),
    blocks: (st) => {
      const d = LAB.PAGES.notifications;
      const relmap = { "2026-10-27T05:01:41.123Z": "2h ago", "2026-10-27T04:12:09.001Z": "3h ago", "2026-10-26T18:40:22.500Z": "Yesterday", "2026-10-26T05:02:00.000Z": "Yesterday", "2026-10-25T14:20:11.300Z": "2d ago", "2026-10-25T09:12:40.100Z": "2d ago" };
      let rows = d.rows;
      if (st.tab === 1) rows = rows.filter((r) => r.type.includes("mention"));
      if (st.tab === 2) rows = rows.filter((r) => !(st.read[r.type + r.ts] !== undefined ? st.read[r.type + r.ts] : !r.unread));
      if (!rows.length) return [DS.emptyState(ic("bell"), "No notifications", "Nothing here for this tab yet — new alerts arrive with the sync.")];
      return rows.map((r) => {
        const read = st.read[r.type + r.ts] !== undefined ? st.read[r.type + r.ts] : !r.unread;
        return DS.card({ body:
          DS.toolbar(`${r.unread && !read ? `<span class="ds-udot"></span>` : ""}${DS.badge(r.sev === "error" ? "danger" : "info", r.type)}<span class="ds-small ds-muted2" title="${esc(r.ts)}">${esc(relmap[r.ts] || r.ts)}</span>`, DS.btn.text(read ? "unread" : "mark read", "nt-toggle", esc(r.type + r.ts)), true) +
          `<div class="ds-strong">${esc(r.title)}</div>${r.body ? DS.hint(r.body) : ""}` });
      }).concat([DS.quirk("Reskin fix (presentation): unread was a '● ' character inside the badge text (notifications.rs:197-208) — now a real state dot; raw UTC survives in the hover title.")]);
    },
    act: {
      "nt-toggle": (el, ctx) => { ctx.state.read[el.dataset.k] = !ctx.state.read[el.dataset.k]; ctx.rerender(); },
      "nt-markall": (el, ctx) => { LAB.PAGES.notifications.rows.forEach((r) => { ctx.state.read[r.type + r.ts] = true; }); ctx.rerender(); },
    },
  });

  /* ============================ Search ==================================== */
  DS.definePage("search", {
    archetype: "stack",
    state: { q: "", ran: false },
    meta: () => ({ title: M("search").title, subtitle: M("search").subtitle }),
    blocks: (st) => {
      const d = LAB.PAGES.search;
      const out = [
        DS.toolbar(DS.input("Search…", 320, "se-q", st.q), `${DS.btn.primary("search", "se-run")}${DS.btn.text("clear", "se-clear")}`),
        DS.toolbar(`<span class="ds-field-inline">scope:</span>${["All", "Tickets", "Knowledge", "Customers"].map((c, i) => DS.chip(c, i === 0)).join("")}`),
        divider,
      ];
      if (!st.ran) {
        out.push(DS.emptyState(ic("search") || DS.ICONS.inbox, "Search across your workspace", "Type a query and press Enter to search conversations, knowledge, and customers.", false, DS.btn.primary(`Try "export"`, "se-demo")));
      } else {
        out.push(DS.hint(`4 hits for "${st.q || "export"}"`));
        out.push(DS.hint(d.modeNote));
        out.push(d.hits.map((h) => DS.card({ body: DS.toolbar(`${DS.badge("neutral", h.scope)}<span class="ds-mono ds-strong">${esc(h.title)}</span>${h.subtitle ? smallMuted(h.subtitle) : ""}`) +
          `<div class="ds-chip-row">${h.why.map((w) => DS.badge(w === "semantic match" ? "info" : "neutral", w)).join("")}</div>${h.snippet ? DS.hint(h.snippet) : ""}` })).join(""));
      }
      return out;
    },
    after: (root, ctx) => {
      const q = root.querySelector("#se-q");
      if (q) q.addEventListener("keydown", (e) => { if (e.key === "Enter") { ctx.state.q = q.value; ctx.state.ran = true; ctx.rerender(); } });
    },
    act: {
      "se-run": (el, ctx) => { const q = ctx.root.querySelector("#se-q"); ctx.state.q = q ? q.value : ""; ctx.state.ran = true; ctx.rerender(); },
      "se-clear": (el, ctx) => { ctx.state.q = ""; ctx.state.ran = false; ctx.rerender(); },
      "se-demo": (el, ctx) => { ctx.state.q = "export"; ctx.state.ran = true; ctx.rerender(); },
    },
  });

  /* ============================ Support Graph ============================= */
  DS.definePage("support-graph", {
    archetype: "stack",
    state: { sel: 1 },
    meta: (st) => ({ title: M("support-graph").title, subtitle: M("support-graph").subtitle, counter: `${LAB.PAGES["support-graph"].results.length} nodes` }),
    blocks: (st) => {
      const d = LAB.PAGES["support-graph"];
      const nb = d.neighborhood;
      return [
        DS.toolbar(`${DS.input("Search the graph…", 320)}`, DS.btn.primary("search")),
        divider,
        `<div class="ds-sel-list">` + d.results.map((n, i) => DS.selRow(`${DS.badge("neutral", n.kind)}: ${esc(n.label)}`, n.sub ? esc(n.sub) : "", { selected: i === st.sel, act: "sg-sel", i })).join("") + `</div>`,
        divider,
        DS.card({ title: "Neighborhood", body: `<div class="ds-tool-row"><span class="ds-strong">${esc(nb.center.kind)} #${nb.center.id}</span></div>${DS.hint(nb.center.label)}` +
          nb.edges.map((e) => { const m = e.match(/^(\S+)\s+\(([^)]+)\)\s+(.*)$/); return m ? DS.row(`<span class="ds-strong">${esc(m[1])}</span> ${esc(m[3])}`, `<span class="ds-mono ds-small ds-muted2">${esc(m[2])}</span>`) : DS.row(esc(e)); }).join("") }),
      ];
    },
    act: { "sg-sel": (el, ctx) => { ctx.state.sel = Number(el.dataset.i); ctx.rerender(); } },
  });

  /* ============================ Connectors ================================ */
  DS.definePage("connectors", {
    archetype: "stack",
    state: { arm: {} },
    meta: () => ({ title: M("connectors").title, subtitle: M("connectors").subtitle, counter: `${LAB.PAGES.connectors.rows.length} connectors`, actions: DS.btn.primary("New Connector") }),
    blocks: (st, ctx) => {
      const d = LAB.PAGES.connectors;
      return [
        `<span class="ds-status-line ok">status: refresh started</span>`,
        divider,
        DS.card({ body: DS.dataTable(d.headers, d.rows.map((r, i) =>
          `<tr><td>${DS.chip(r.name, i === 0, "cn-sel", i)}</td><td class="ds-mono ds-muted2">${esc(r.kind)}</td><td>${DS.dot(r.health)} ${DS.statusBadge(r.health)}</td><td>${esc(r.refresh)}</td><td class="ds-mono ds-muted2">${esc(r.last)}</td><td>${DS.btn.outline("refresh")}${DS.btn.danger("delete", "cn-del", i)}</td></tr>`).join("")) }),
        DS.card({ title: d.rowsViewer.title, body: DS.toolbar(`${DS.input("search rows…", 220)}`, DS.btn.primary("search")) +
          `<div class="ds-scroll-220">${d.rowsViewer.rows.map((r) => `<div class="ds-mono ds-small">${esc(r)}</div>`).join("")}</div>` }),
        DS.quirk("A-5 (DS-0): the rows/refresh/delete triple collapsed to refresh + delete (danger + confirm); the connector chip opens the rows viewer below."),
      ];
    },
    act: {
      "cn-sel": () => DS.toast("Connector selected — the rows viewer below shows its mirror rows (mock)."),
      "cn-del": (el, ctx) => { const k = el.dataset.i; if (armed[k]) { delete armed[k]; DS.toast("Delete is mocked in the design lab."); } else armed[k] = true; ctx.rerender(); },
    },
  });

  /* ============================ Diagnostics (DS-A-06) ===================== */
  DS.definePage("diagnostics", {
    archetype: "console",
    meta: () => ({ title: M("diagnostics").title, subtitle: M("diagnostics").subtitle }),
    blocks: () => {
      const d = LAB.PAGES.diagnostics;
      return [
        DS.card({ title: "Event log", sub: d.eventLog.sub,
          body: DS.toolbar(`${["all", "errors", "warnings", "info"].map((c, i) => DS.chip(c, i === 0)).join("")}`, `${DS.btn.outline("refresh")}${DS.btn.danger("clear log", "dg-clear")}`) +
            `<div class="ds-scroll-360">${d.eventLog.rows.map((e) => DS.evRow(e.ts, e.level === "warn" ? "warn" : e.level, `${DS.badge("neutral", e.service)} <span class="ds-small">${esc(e.msg)}</span>`, e.ctx || "")).join("")}</div>` }),
        DS.card({ title: "Audit trail", sub: d.audit.sub,
          body: DS.dataTable(d.audit.headers, d.audit.rows.map(([when, actor, action, conv, ai]) =>
            `<tr><td class="ds-mono ds-muted2">${esc(when)}</td><td>${actor === "ai" ? DS.statusBadge("ai", actor) : DS.badge("neutral", actor)}</td><td><span class="ds-small">${esc(action)}</span></td><td class="ds-mono ds-muted2">${esc(conv)}</td><td>${ai ? DS.statusBadge("ai", "ai") : DS.badge("neutral", "—")}</td></tr>`).join("")) +
            `<div class="ds-scroll-140">${d.audit.rows.filter((r) => r[5]).map(([, , , , , detail]) => `<div class="ds-mono ds-small ds-muted2">${esc(detail)}</div>`).join("")}</div>` }),
        DS.card({ title: "Sync reports", sub: "The outcome of every recent sync run (newest first).",
          body: DS.dataTable(d.syncReports.headers, d.syncReports.rows.map(([id, kind, state, rec, err, started]) =>
            `<tr>${DS.tdn(id)}<td>${mono(kind)}</td><td>${DS.statusBadge(SYNC[state] || "stale", state)}</td>${DS.tdn(rec)}${DS.tdn(err)}<td class="ds-mono ds-muted2">${esc(started)}</td></tr>`).join("")) }),
        DS.card({ title: "Outbound deliveries", sub: d.outbound.sub,
          body: DS.dataTable(d.outbound.headers, d.outbound.rows.map(([id, kind, at, err]) =>
            `<tr>${DS.tdn(id)}<td>${mono(kind)}</td><td class="ds-mono ds-muted2">${esc(at)}</td><td>${smallMuted(err)}</td></tr>`).join("")) }),
      ];
    },
    act: { "dg-clear": () => DS.toast("Clear log is mocked in the design lab — the log is append-only on the data track.") },
  });

  /* ============================ Sync Health (DS-A-06) ===================== */
  DS.definePage("sync-health", {
    archetype: "console",
    meta: () => ({ title: M("sync-health").title, subtitle: M("sync-health").subtitle }),
    hero: () => {
      const d = LAB.PAGES["sync-health"];
      return DS.hero(d.engine.state, `last success: ${d.engine.last} · never shows relative time — forensic value`, DS.statusBadge(SYNC[d.engine.state] || "stale", d.engine.state));
    },
    blocks: () => {
      const d = LAB.PAGES["sync-health"];
      return [
        DS.hint("progress: conversations · resource 3/22 · 1204 record(s) so far"),
        DS.card({ title: "Engine", body: DS.hint(d.webhook) + DS.hint(d.health) }),
        DS.toolbar(`${DS.btn.primary("Run initial sync")}${DS.btn.outline("Run incremental sync")}${DS.btn.outline("Run reconcile")}`, DS.btn.danger("Cancel running sync", "sh-cancel")),
        DS.card({ title: "Checkpoints", body: DS.dataTable(d.checkpoints.headers, d.checkpoints.rows.map(([r, s, p, f, rt, ls]) =>
          `<tr><td>${mono(r)}</td><td>${esc(s)}</td>${DS.tdn(p)}${DS.tdn(f)}${DS.tdn(rt)}<td class="ds-mono ds-muted2">${esc(ls)}</td></tr>`).join("")) }),
        DS.card({ title: "Recent runs", body: DS.dataTable(d.runs.headers, d.runs.rows.map((r) =>
          `<tr>${r.map((c, i) => (i === 0 ? DS.tdn(c) : typeof c === "number" ? DS.tdn(c) : i >= 3 ? `<td class="ds-mono ds-muted2">${esc(c)}</td>` : `<td>${mono(c)}</td>`)).join("")}</tr>`).join("")) }),
      ];
    },
    act: { "sh-cancel": () => DS.toast("Cancel is mocked in the design lab — the engine stops at the next checkpoint.") },
  });

  /* ============================ Settings (DS-A-04) ======================== */
  DS.definePage("settings", {
    archetype: "stack",
    state: { tab: 0 },
    meta: () => ({ title: M("settings").title, subtitle: M("settings").subtitle }),
    tabs: (st) => ({ items: LAB.PAGES.settings.tabs, active: st.tab }),
    blocks: (st) => {
      const d = LAB.PAGES.settings;
      if (st.tab === 0) return [
        DS.card({ title: "Synchronization", body: DS.formGrid([
          ["max quiet sync interval (minutes):", DS.combo("1")],
          ["api concurrency:", DS.combo("2")],
          ["log level:", DS.combo("info")],
          ["agent language:", DS.input("", 220)],
          ["status after reply:", DS.combo("pending (await customer)")],
        ]) }),
        DS.card({ title: "AI behaviour", sub: "safe defaults — review before enabling write actions",
          body: `<div class="ds-form-grid four">${d.aiBehaviour.map(([label, on]) => `<label class="ds-check"><input type="checkbox" ${on ? "checked" : ""}> ${esc(label)}</label>`).join("")}</div>` }),
        DS.card({ title: "About & updates", body: `<div>Running version: v1.8.0</div><div class="ds-tool-row">${DS.btn.outline("check for updates")}</div>${DS.hint("The check reads the public GitHub release feed once, on click. The app never sends telemetry.")}` }),
        DS.btn.primary("save settings"),
      ];
      if (st.tab === 1) return [
        DS.hint("loopback listener answering on :3001 (webhook ingress + OAuth callback)"),
        DS.card({ title: "Help Scout connection", sub: "Connect with client credentials (recommended — no browser, no redirect URL) or the advanced browser flow",
          body: DS.formGrid([["credentials stored:", DS.badge("success", d.connection.badge)], ["redirect uri:", `<span class="ds-mono ds-small">${esc(d.connection.redirect)}</span>`]]) +
            DS.toolbar(DS.btn.danger("disconnect", "st-disconnect"), DS.btn.primary("connect with client credentials (recommended)")) }),
        DS.card({ title: "Connection Doctor", sub: "five checks: reach the API → sign in → read the user → list mailboxes → rate-limit budget",
          body: DS.btn.outline("run the checks") + `<div class="ds-scroll-220">` + d.doctor.map(([m, detail]) =>
            DS.row(`${DS.dot(m.startsWith("✓") ? "ok" : m.startsWith("!") ? "stale" : "error")} <b>${esc(m)}</b>`, `<span class="ds-small ds-muted2">${esc(detail)}</span>`)).join("") + `</div>` }),
        DS.card({ title: "Help Scout credentials", sub: "Stored encrypted on this machine. Save them and the app connects immediately — the Connection Doctor verifies the chain and the first sync starts by itself. Environment variables override these values.",
          body: DS.formGrid([
            ["client id:", DS.input("Help Scout custom app — client ID", 320)],
            ["client secret:", `<input class="ds-text-input" type="password" placeholder="leave blank to keep the stored value" aria-label="client secret">`],
            ["webhook secret:", `<input class="ds-text-input" type="password" placeholder="verifies incoming Help Scout webhooks" aria-label="webhook secret">`],
            ["docs api key:", `<input class="ds-text-input" type="password" placeholder="optional — leave blank to keep the stored value" aria-label="docs api key">`],
          ]) + DS.toolbar(`<label class="ds-check"><input type="checkbox"> show secrets</label>`, `${DS.btn.primary("save credentials")}${DS.btn.danger("clear stored", "st-clear")}`) +
            DS.formGrid(d.sources.map(([k, v]) => [k, DS.badge(v === "not set" ? "neutral" : "success", v)])) }),
        DS.card({ title: "Webhook relay & health", sub: "the listener stays loopback-only by design; a user-run relay bridges the NAT, and the health check re-registers the hook when the platform silently drops it",
          body: DS.formGrid([["public relay URL:", DS.input("https://your-relay → forwards to this app's loopback listener", 320)]]) +
            DS.toolbar(`${DS.btn.outline("save relay URL")}`, DS.btn.primary("check now")) +
            DS.hint("relay options: cloudflared tunnel --url http://127.0.0.1:PORT · ngrok http 127.0.0.1:PORT · ssh -N -R 80:127.0.0.1:PORT serveo.net — the registered URL must forward to the loopback listener") +
            DS.hint("prove the chain from outside: curl -X POST <relay>/webhook -d '{}' must answer 401 (signature check) — anything else means the relay is not reaching the app") +
            DS.hint("one honest limit: the API never reveals a hook's stored secret, so rotating the local webhook secret needs a re-register — the check does that for you when it finds the hook stale") +
            DS.formGrid([["last check:", `${DS.badge("success", "healthy")} ${smallMuted("· 2026-10-27T04:30:00Z")}`]]) }),
        DS.card({ danger: true, title: "Danger zone — Workspace data", sub: "irreversible",
          body: `<span class="ds-small ds-muted2">Deletes every conversation, customer, thread, rating, knowledge document, incident, report and AI record — all synced content — and arms a fresh first sync. Keeps: credentials, settings, business hours, automation rules, saved views and replies, custom object types.</span>${DS.btn.danger("delete all workspace data", "st-wipe")}` }),
      ];
      if (st.tab === 2) return [
        DS.card({ title: "LM Studio (local AI gateway)", sub: "Test the connection from the AI Center page.",
          body: DS.formGrid([["base url:", DS.input("http://localhost:1234/v1", 320)], ["chat model:", DS.input("", 320)], ["embedding model:", DS.input("", 320)]]) + DS.btn.primary("save") }),
      ];
      if (st.tab === 3) return [
        DS.card({ title: "Vector index", sub: "Dense vectors live in SQLite next to the application database (decision D4: the chunk count is far below the 100k gate). The index follows the qdrant_enabled setting; searches fall back to FTS5 when disabled.",
          body: DS.formGrid([["store:", `<span class="ds-mono ds-small">${esc(d.vectorStore)}</span>`]]) + `<label class="ds-check"><input type="checkbox" checked> vector index enabled</label>${DS.hint("Dense vectors are embedded and searched while enabled; searches fall back to FTS5 when disabled. Applies on the next start — the workers open the store at boot.")}` }),
      ];
      if (st.tab === 4) return [
        DS.card({ title: "Business hours per mailbox", body: d.businessHours.map((b) =>
          DS.row(`<span class="ds-strong">${esc(b.name)}</span> ${DS.badge(b.configured ? "success" : "neutral", b.configured ? "configured" : "not configured")}${b.hint ? ` ${smallMuted(b.hint)}` : ""}`, DS.btn.outline("edit"))).join("") }),
      ];
      return [
        DS.card({ title: "Backups & export", body: DS.toolbar(`${DS.btn.primary("create backup")}${DS.btn.outline("export JSON")}${DS.btn.outline("export conversations CSV")}`) +
          d.backups.map(([f, size, ts], i) => DS.row(mono(f), `${smallMuted(size)}${smallMuted(ts)}${DS.btn.outline("restore", "st-restore", i)}`)).join("") }),
        DS.card({ danger: true, title: "Danger zone — Restore", sub: "irreversible", body: `<span class="ds-small ds-muted2">Restoring replaces the current database after verification — the two-step arm survives exactly as shipped.</span>${DS.btn.outline("restore (arms two-step confirm)")}` }),
      ];
    },
    act: {
      "st-disconnect": () => DS.toast("Disconnect is mocked in the design lab."),
      "st-clear": () => DS.toast("Clear stored is mocked in the design lab."),
      "st-wipe": () => DS.toast("Danger zone: the real flow arms a two-step confirm — mocked here."),
      "st-restore": () => DS.toast("Restore arms a two-step confirm — mocked here."),
    },
  });

  /* ============================ Onboarding ================================ */
  DS.definePage("onboarding", {
    archetype: "stack",
    meta: () => ({ title: M("onboarding").title, subtitle: M("onboarding").subtitle }),
    blocks: () => {
      const d = LAB.PAGES.onboarding;
      return [
        DS.card({ title: "Setup status", body: d.rows.map(([k, v]) => {
          const okFlag = k === "conversations:" ? Number(v) > 0 : v === "true" || v === "receiving";
          return DS.row(`${DS.dot(okFlag ? "ok" : "closed")} <span class="ds-strong">${esc(k)}</span>`, mono(v));
        }).join("") }),
        `<span class="ds-status-line ok">status: Initial sync started. Watch Sync Health or Diagnostics for progress and errors.</span>`,
        DS.toolbar(`${DS.btn.primary("Run initial sync")}`, DS.btn.outline("Mark complete")),
        d.hints.map((h) => DS.hint(h)).join(""),
      ];
    },
  });

  /* ============================ Shell ===================================== */
  DS.definePage("shell", {
    archetype: "stack",
    meta: () => ({ title: M("shell").title, subtitle: M("shell").subtitle,
      actions: DS.btn.outline(`switch to ${document.body.dataset.theme === "light" ? "dark" : "light"}`, "sh-theme") }),
    blocks: () => {
      const d = LAB.PAGES.shell;
      return [
        DS.card({ title: "Runtime", body: DS.formGrid(d.runtime.map(([k, v]) => [String(k), mono(v)])) }),
        `<div class="ds-hint">${esc(d.hint)} <span class="ds-stale-tag">stale checklist — omits live chats</span></div>`,
      ];
    },
    act: { "sh-theme": () => { document.body.dataset.theme = document.body.dataset.theme === "light" ? "dark" : "light"; } },
  });
})();
