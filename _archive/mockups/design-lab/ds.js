/* ==========================================================================
   SupportOS+ DESIGN SYSTEM v1.0 — canonical renderers + registry page logic
   --------------------------------------------------------------------------
   window.DS is the ONLY component vocabulary pages may compose from.
   Every renderer maps 1:1 to a future egui widget (R1-R4) and every tone
   decision flows through DS.TONE — one meaning, one color, everywhere.
   ========================================================================== */
(() => {
  "use strict";

  /* ----- inline icon set (15px stroke style, matching the lab) ----------- */
  const I = (p, extra) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"${extra || ""}>${p}</svg>`;
  const ICONS = {
    sparkles: I('<path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3z"/><path d="M19 15l.9 2.4L22 18.3l-2.1.9L19 21.6l-.9-2.4-2.1-.9 2.1-.9L19 15z"/>'),
    search: I('<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/>'),
    bell: I('<path d="M18 8a6 6 0 10-12 0c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.7 21a2 2 0 01-3.4 0"/>'),
    sun: I('<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4l1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>'),
    moon: I('<path d="M21 12.8A9 9 0 1111.2 3 7 7 0 0021 12.8z"/>'),
    paperclip: I('<path d="M21.4 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48"/>'),
    chevron: I('<path d="M9 18l6-6-6-6"/>'),
    chevronD: I('<path d="M6 9l6 6 6-6"/>'),
    lock: I('<rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0110 0v4"/>'),
    bot: I('<rect x="4" y="8" width="16" height="12" rx="2"/><path d="M12 8V4m-4 9h.01M16 13h.01M9 17h6"/>'),
    user: I('<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 3.6-6 8-6s8 2 8 6"/>'),
    pencil: I('<path d="M17 3a2.8 2.8 0 014 4L7.5 20.5 2 22l1.5-5.5L17 3z"/>'),
    send: I('<path d="M22 2L11 13"/><path d="M22 2l-7 20-4-9-9-4 20-7z"/>'),
    file: I('<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/>'),
    inbox: I('<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.5 5.1L2 12v6a2 2 0 002 2h16a2 2 0 002-2v-6l-3.5-6.9A2 2 0 0016.7 4H7.3a2 2 0 00-1.8 1.1z"/>'),
    alert: I('<path d="M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z"/><path d="M12 9v4m0 4h.01"/>'),
    check: I('<path d="M20 6L9 17l-5-5"/>'),
    x: I('<path d="M18 6L6 18M6 6l12 12"/>'),
    layers: I('<path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>'),
  };

  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

  /* ========================================================================
     DS.TONE — the ONLY status → color mapping in the product.
     A status may appear in badges, dots, tiles, bands — never a new color.
     ======================================================================== */
  const TONE = {
    /* conversation lifecycle */
    open:      { tone: "info",    label: "Open" },
    pending:   { tone: "warning", label: "Pending" },
    waiting:   { tone: "warning", label: "Waiting" },
    resolved:  { tone: "success", label: "Resolved" },
    closed:    { tone: "neutral", label: "Closed" },
    spam:      { tone: "danger",  label: "Spam" },
    active:    { tone: "info",    label: "Active" },
    /* knowledge / articles */
    published: { tone: "success", label: "Published" },
    draft:     { tone: "neutral", label: "Draft" },
    internal:  { tone: "info",    label: "Internal" },
    /* incident severity (bands) */
    critical:  { tone: "danger",  label: "Critical" },
    high:      { tone: "warning", label: "High" },
    medium:    { tone: "info",    label: "Medium" },
    low:       { tone: "neutral", label: "Low" },
    /* health / sync */
    ok:        { tone: "success", label: "OK" },
    healthy:   { tone: "success", label: "Healthy" },
    live:      { tone: "success", label: "Live" },
    stale:     { tone: "warning", label: "Stale" },
    error:     { tone: "danger",  label: "Error" },
    syncing:   { tone: "info",    label: "Syncing" },
    catching_up: { tone: "info",  label: "Catching up" },
    /* authorship roles */
    ai:        { tone: "ai",      label: "AI" },
    note:      { tone: "warning", label: "Internal note" },
    customer:  { tone: "neutral", label: "Customer" },
    agent:     { tone: "neutral", label: "Agent" },
  };
  const toneOf = (status) => (TONE[status] ? TONE[status].tone : "neutral");

  /* ========================================================================
     Renderers — every one becomes one typed egui widget in R2-R4.
     ======================================================================== */
  const badge = (tone, text) => `<span class="ds-badge ${esc(tone)}">${esc(text)}</span>`;
  const statusBadge = (status, labelOverride) => { const t = TONE[status] || { tone: "neutral", label: status }; return badge(t.tone, labelOverride || t.label); };
  const aiBadge = (text) => `<span class="ds-badge ai">${ICONS.sparkles} ${esc(text || "AI")}</span>`;
  const chip = (text, on, act, i) => `<button class="ds-chip${on ? " on" : ""}"${act ? ` data-ds-act="${act}"${i != null ? ` data-i="${i}"` : ""}` : ""} type="button">${esc(text)}</button>`;
  const dot = (status, big) => { const t = toneOf(status); const cls = t === "neutral" ? "" : ` ${t === "success" ? "ok" : t === "warning" ? "warn" : t === "danger" ? "err" : t}`; return `<span class="ds-state-dot${big ? " big" : ""}${cls}"></span>`; };

  const pageHeader = ({ title, subtitle, actions, counter }) =>
    `<div class="ds-page-head"><div><h1>${esc(title)}</h1>${subtitle ? `<div class="ds-page-sub">${subtitle}</div>` : ""}</div><div class="ds-page-actions">${counter ? `<span class="ds-counter">${counter}</span>` : ""}${actions || ""}</div></div>`;

  const card = ({ title, sub, body, danger }) =>
    `<div class="ds-card${danger ? " danger" : ""}">${title ? `<div class="ds-card-h">${esc(title)}${sub ? `<span class="ds-card-sub">${esc(sub)}</span>` : ""}</div>` : ""}<div class="ds-card-b">${body}</div></div>`;

  const sectionTitle = (t, big) => `<div class="ds-section-title${big ? " big" : ""}">${esc(t)}</div>`;
  const twoCol = (l, r, wideLeft) => `<div class="ds-two-col${wideLeft ? " wide-left" : ""}"><div>${l}</div><div>${r}</div></div>`;

  const statGrid = (items) => `<div class="ds-stat-grid">${items.map((s) => `<div class="ds-stat"><div class="l">${esc(s.label)}</div><div class="v">${esc(s.value)}</div>${s.hint ? `<div class="h">${esc(s.hint)}</div>` : ""}</div>`).join("")}</div>`;

  const metricTile = ({ cnt, lbl, level, drill }) =>
    `<div class="ds-tile${level ? ` ${level}` : ""}${drill ? " drill" : ""}"${typeof drill === "string" ? ` data-ds-act="${esc(drill)}"` : ""} tabindex="${drill ? "0" : "-1"}"><span class="cnt">${esc(cnt)}</span><span class="lbl">${esc(lbl)}</span>${drill ? `<span class="ds-drill-line">open →</span>` : ""}</div>`;

  const dataTable = (headers, rowsHTML) =>
    `<div class="ds-tbl-wrap"><table class="ds-tbl"><thead><tr>${headers.map((h) => `<th>${esc(h)}</th>`).join("")}</tr></thead><tbody>${rowsHTML}</tbody></table></div>`;
  const td = (c, cls) => `<td${cls ? ` class="${cls}"` : ""}>${c}</td>`;
  const tds = (c) => `<td class="strong">${esc(c)}</td>`;
  const tdn = (c) => `<td class="num">${esc(c)}</td>`;

  const tabs = (items, active, act, off) =>
    `<div class="ds-tabs">${items.map((t, i) => `<button class="ds-tab${i === active ? " active" : ""}"${act ? ` data-ds-act="${act}" data-i="${(off || 0) + i}"` : ""} role="tab" type="button">${esc(t)}</button>`).join("")}</div>`;
  const tabGroups = (groups) => `<div class="ds-tabgroups">${groups.map((g) => `<div><div class="tg-label">${esc(g.label)}</div>${tabs(g.tabs, g.active)}</div>`).join("")}</div>`;

  const toolbar = (left, right, split) => `<div class="ds-tool-row${split ? " split" : ""}"><div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap">${left || ""}</div>${right ? `<div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap">${right}</div>` : ""}</div>`;

  /* RS-01: every button variant accepts an optional action key (+index) so pages stay declarative. */
  const mkBtn = (cls, icon) => (t, act, i) => `<button class="ds-btn ${cls}" type="button"${act ? ` data-ds-act="${esc(act)}"${i != null ? ` data-i="${esc(i)}"` : ""}` : ""}>${icon ? icon + " " : ""}${esc(t)}</button>`;
  const btn = { primary: mkBtn("primary"), outline: mkBtn("outline"), outlineAi: mkBtn("outline ai", ICONS.sparkles), danger: mkBtn("danger"), text: mkBtn("text"), ai: mkBtn("ai", ICONS.sparkles), sm: (t, act, i) => `<button class="ds-btn outline sm" type="button"${act ? ` data-ds-act="${esc(act)}"${i != null ? ` data-i="${esc(i)}"` : ""}` : ""}>${esc(t)}</button>` };

  const input = (ph, w, id, val) => `<input class="ds-text-input"${id ? ` id="${esc(id)}"` : ""}${w ? ` style="width:${w}px"` : ""} placeholder="${esc(ph)}" aria-label="${esc(ph)}"${val != null && val !== "" ? ` value="${esc(val)}"` : ""}>`;
  const combo = (label) => `<button class="ds-combo" type="button"><span>${esc(label)}</span>${ICONS.chevronD}</button>`;
  const formRow = (label, ctl) => `<div class="ds-fg-label">${esc(label)}</div><div class="ds-fg-ctl">${ctl}</div>`;
  const formGrid = (rows, four) => `<div class="ds-form-grid${four ? " four" : ""}">${rows.map(([l, c]) => formRow(l, c)).join("")}</div>`;
  const fieldLabel = (t) => `<div class="ds-field-label">${esc(t)}</div>`;

  const emptyState = (icon, title, sub, slim, cta) => `<div class="ds-empty${slim ? " slim" : ""}">${icon || ICONS.inbox}<div class="t">${esc(title)}</div><div class="s">${esc(sub)}</div>${cta ? `<div class="ds-empty-cta">${cta}</div>` : ""}</div>`;
  const hint = (t) => `<div class="ds-hint">${esc(t)}</div>`;
  const banner = (t) => `<div class="ds-banner">${esc(t)}</div>`;
  const callout = (t, kind) => `<div class="ds-callout${kind ? ` ${kind}` : ""}">${esc(t)}</div>`;
  const quirk = (t) => `<div class="ds-quirk">${ICONS.lock} ${esc(t)}</div>`;
  const kvRow = (k, v) => `<div class="ds-kv-row"><span class="ds-muted2">${esc(k)}:</span> ${v}</div>`;
  const tlRow = (ts, kind, rest) => `<div class="ds-tl-row"><span class="ds-tl-ts">${esc(ts)}</span><span class="ds-tl-kind">${esc(kind)}</span><span>${rest || ""}</span></div>`;

  const avatar = (name, size) => `<span class="ds-av ${size || "md"}">${esc(initials(name))}</span>`;
  const initials = (n) => String(n || "?").split(/\s+/).map((w) => w[0]).slice(0, 2).join("").toUpperCase();
  const assignee = (name) => name ? `<span class="ds-assignee"><span class="ds-av sm">${esc(initials(name))}</span> ${esc(name)}</span>` : `<span class="ds-assignee dashed">unassigned</span>`;

  const roleIcon = { ai: ICONS.bot, note: ICONS.pencil, agent: ICONS.user, customer: ICONS.user };
  const messageCard = ({ role, name, roleLabel, time, body, quoted, attachments, actions }) => {
    const rl = roleLabel || (TONE[role] ? TONE[role].label : role);
    return `<article class="ds-msg ${esc(role)}"><div class="ds-msg-head">${role === "ai" ? `<span class="ds-av md">${ICONS.bot}</span>` : avatar(name, "md")}<span class="ds-msg-name">${esc(name)}</span><span class="ds-msg-role">${roleIcon[role] || ""} ${esc(rl)}</span><span class="ds-msg-time" title="full timestamp">${esc(time)}</span></div><div class="ds-msg-body">${body}</div>${quoted ? `<details class="ds-quoted"><summary>${ICONS.chevron} quoted history — ${esc(quoted.length)} earlier message${quoted.length === 1 ? "" : "s"}</summary>${quoted.map((q) => `<div class="ds-q-item"><div class="ds-q-from">${esc(q.from)}</div><div class="ds-q-body">${esc(q.body)}</div></div>`).join("")}</details>` : ""}${attachments && attachments.length ? `<div class="ds-attachments">${attachments.map((a) => `<span class="ds-file-chip">${ICONS.file} ${esc(a.name)} <span class="size">${esc(a.size)}</span></span>`).join("")}</div>` : ""}${actions ? `<div class="ds-msg-actions">${actions}</div>` : ""}</article>`;
  };
  const dayDivider = (t) => `<div class="ds-day-divider">${esc(t)}</div>`;

  const fileChip = (name, size) => `<span class="ds-file-chip">${ICONS.file} ${esc(name)} <span class="size">${esc(size)}</span></span>`;
  const composerDemo = (mode) =>
    `<div class="ds-cp-tabs"><button class="ds-cp-tab${mode !== "note" ? " active" : ""}" type="button">${ICONS.send} Reply</button><button class="ds-cp-tab${mode === "note" ? " active" : ""}" type="button">${ICONS.pencil} Internal note</button></div>` +
    `<div class="ds-cp-tools">${btn.sm("Saved replies")}${btn.sm("Attach")}<button class="ds-tool-chip ai" type="button">${ICONS.sparkles} AI draft</button></div>` +
    `<textarea class="ds-textarea" rows="3" placeholder="${mode === "note" ? "Add an internal note — visible to teammates only…" : "Write a reply…  (Ctrl+Enter to send)"}"></textarea>` +
    `<div class="ds-tool-row" style="justify-content:space-between"><span class="ds-hint" style="margin:0">Replying to customer</span><button class="ds-send-btn${mode === "note" ? " note" : ""}" type="button">${ICONS.send} ${mode === "note" ? "Add note" : "Send reply"}</button></div>`;

  const copSuggest = (body, actions) => `<div class="ds-cop-card suggest"><div class="ds-cop-card-title">${ICONS.sparkles} Suggested draft</div><div class="ds-cop-suggest-body">${esc(body)}</div><div class="ds-msg-actions">${actions || `${btn.ai("Insert into reply")}${btn.outline("Regenerate")}`}</div></div>`;
  const copSummary = (items) => `<div class="ds-cop-card"><div class="ds-cop-card-title">${ICONS.bot} Conversation summary</div>${items.map((x) => `<div class="ds-cop-bullet">${esc(x)}</div>`).join("")}</div>`;
  const copSimilar = (sims) => `<div class="ds-cop-card"><div class="ds-cop-card-title">${ICONS.layers} Similar past conversations</div>${sims.map((s) => `<div class="ds-cop-sim"><div class="ds-cop-sim-head">${esc(s.id)} — ${esc(s.subject)}</div><div class="ds-cop-sim-res">${esc(s.resolution)}</div></div>`).join("")}</div>`;
  const copChat = (turns) => turns.map((t) => `<div class="ds-cop-msg ${t.who}"><span class="ds-who">${t.who === "assistant" ? aiBadge("Fin") : avatar("You", "sm")}</span><div class="ds-cop-bubble">${esc(t.text)}${t.cited ? `<div class="ds-cited">source: ${esc(t.cited)}</div>` : ""}</div></div>`).join("");

  const hero = (state, sub, right) => `<div class="ds-hero"><span class="ds-hero-state">${esc(state)}</span><span class="ds-muted2">${esc(sub)}</span><span style="margin-left:auto;display:flex;gap:8px">${right || ""}</span></div>`;
  const evRow = (ts, level, line, ctx) => `<div class="ds-ev-row${level === "error" ? " lv-error" : level === "warn" ? " lv-warn" : ""}"><div class="ds-ev-line"><span class="ds-tl-ts">${esc(ts)}</span>${dot(level)}<span>${line}</span></div>${ctx ? `<div class="ds-tl-ts" style="padding-left:4px;display:block">${esc(ctx)}</div>` : ""}</div>`;

  const selList = (items, sel) => `<div class="ds-sel-list">${items.map((it, i) => `<div class="ds-sel-row${i === sel ? " selected" : ""}" data-ds-act="demo-sel" data-i="${i}" tabindex="0" role="listitem"><div class="lr1">${esc(it.t)}</div><div class="lr2">${esc(it.s)}</div></div>`).join("")}</div>`;

  const lineChart = (points, annotation) => {
    if (points.length < 2) return `<div class="ds-hint">(not enough data to plot)</div>`;
    const w = 900, h = 180, pad = 12, max = Math.max(...points, 1);
    const px = (i) => pad + (i * (w - 2 * pad)) / (points.length - 1);
    const py = (v) => h - 26 - (v / max) * (h - 52);
    const poly = points.map((p, i) => `${px(i).toFixed(1)},${py(p).toFixed(1)}`).join(" ");
    const area = `${pad},${h - 26} ${poly} ${w - pad},${h - 26}`;
    const dots = points.map((p, i) => `<circle cx="${px(i).toFixed(1)}" cy="${py(p).toFixed(1)}" r="3"/>`).join("");
    return `<div style="position:relative"><svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" role="img" aria-label="${esc(annotation)}" style="width:100%;height:180px;display:block">
      <line x1="${pad}" y1="40" x2="${w - pad}" y2="40" stroke="var(--ds-border)"/><line x1="${pad}" y1="90" x2="${w - pad}" y2="90" stroke="var(--ds-border)"/><line x1="${pad}" y1="140" x2="${w - pad}" y2="140" stroke="var(--ds-border)"/>
      <polygon points="${area}" fill="var(--ds-ai)" opacity="0.1"/><polyline points="${poly}" fill="none" stroke="var(--ds-ai)" stroke-width="2"/>${dots}</svg>
      <div class="ds-hint" style="padding-top:4px">${esc(annotation)}</div></div>`;
  };
  const barRow = (label, v, max, slim) => `<div class="ds-bar-row${slim ? " slim" : ""}"><span class="ds-bar-label${slim ? " w120" : ""}">${esc(label)}</span><div class="ds-bar-track"><i style="width:${Math.max(3, (v / max) * 100)}%"></i></div><span class="ds-bar-val">${esc(v)}</span></div>`;

  /* toast */
  let toastEl = null, toastTimer = null;
  const toast = (msg) => {
    if (!toastEl) { toastEl = document.createElement("div"); toastEl.className = "ds-toast"; toastEl.setAttribute("role", "status"); document.body.appendChild(toastEl); }
    toastEl.textContent = msg; toastEl.classList.add("show");
    clearTimeout(toastTimer); toastTimer = setTimeout(() => toastEl.classList.remove("show"), 2200);
  };

  /* detail panel (DS-C-17) */
  const panel = {
    open(title, bodyHTML) {
      let scrim = document.querySelector(".ds-panel-scrim"), p = document.querySelector(".ds-panel");
      if (!p) {
        scrim = document.createElement("div"); scrim.className = "ds-panel-scrim"; document.body.appendChild(scrim);
        p = document.createElement("aside"); p.className = "ds-panel"; p.setAttribute("aria-label", "Detail panel"); document.body.appendChild(p);
        scrim.addEventListener("click", () => panel.close());
      }
      p.innerHTML = `<div class="ds-panel-h"><span class="ds-panel-title">${esc(title)}</span><button class="ds-btn text" type="button" data-ds-act="panel-close" style="margin-left:auto">Close ✕</button></div><div class="ds-panel-b">${bodyHTML}</div>`;
      document.body.classList.add("panel-open");
    },
    close() { document.body.classList.remove("panel-open"); },
  };

  /* ========================================================================
     Foundations data (token documentation)
     ======================================================================== */
  const TOKEN_GROUPS = [
    { name: "Surfaces", items: [
      ["bg", "App canvas behind all cards", "Aliases --bg"],
      ["surface", "Cards, raised content, menus", "Aliases --card"],
      ["surface-2", "Chrome rails, faint rows, table head", "Aliases --faint / --sidebar"],
      ["input", "Text inputs, textareas", "Aliases --input"],
      ["border", "Hairlines, card edges", "Aliases --border"],
      ["border-strong", "Hovered edges, dashed dividers", "Aliases --border-strong"],
    ]},
    { name: "Text & primary", items: [
      ["text", "Primary content", "Aliases --fg"],
      ["text-2", "Secondary text, labels, meta", "Aliases --muted-fg"],
      ["primary", "Primary buttons, active tab underline, selected row inset", "Aliases --primary"],
      ["on-primary", "Text on primary", "Aliases --primary-fg"],
      ["hover", "Hover wash on rows/items", "Aliases --hover"],
    ]},
    { name: "Status tones", items: [
      ["info", "Informational states (open, active, medium)", "bg / fg variants"],
      ["success", "Positive states (resolved, published, healthy)", "bg / fg variants"],
      ["warning", "Caution states (pending, stale, high) — also the internal-note surface", "bg / fg / border variants"],
      ["danger", "Destructive & critical (spam, sev-1, delete)", "bg / fg variants"],
      ["neutral-bg/fg", "Everything else (closed, draft, low)", "pill counters"],
    ]},
    { name: "Role surfaces (token v3)", items: [
      ["ai", "AI identity everywhere — badges, bubbles, copilot cards, AI actions", "violet; aliases --ai*"],
      ["agent-bg/-border", "Agent-authored message surface", "aliases --agent-*"],
      ["note", "Internal notes reuse warning surface", "yellow, per Intercom convention"],
    ]},
  ];
  const SCALES = {
    space: [["sp-1", "4px", "micro gaps inside controls"], ["sp-2", "8px", "control gaps, chip rows"], ["sp-3", "12px", "card padding, list rhythm"], ["sp-4", "16px", "page padding, msg card padding"], ["sp-5", "20px", "reader padding"], ["sp-6", "24px", "section rhythm"], ["sp-7", "32px", "page bottom"]],
    type: [["meta", "11 / 11.5px", "timestamps, badges, kbd"], ["small", "12 / 12.5px", "hints, secondary cells, controls"], ["body", "13.5px", "default text, nav, buttons"], ["lead", "14px", "message body, card titles"], ["section", "15 / 18px", "section titles"], ["stat", "20 / 26px", "metric values"], ["title", "22px", "page h1"]],
    radius: [["r-sm", "6px", "badges, text buttons"], ["r-md", "8px", "buttons, inputs, nav items"], ["r-lg", "10px", "tables, menus, lists"], ["r-xl", "12px", "cards, message cards, panels"], ["r-pill", "999px", "chips, counters, dots"]],
    motion: [["t-fast", "120ms", "hovers, tile lift"], ["t-slow", "200ms", "drawers, toast, quoted expand"]],
  };

  /* ========================================================================
     Mismatch Register (DS-0 evidence → DS-4 worklist)
     ======================================================================== */
  const REGISTER = [
    ["A-1", "A", "Master–detail flow differs per page", "Customers: search→table→open→full-page profile + back button. Organizations: left sel-list + right column. Others: table only.", "CLOSED in DS-4 (design-lab-v4): One List-page archetype (DS-A-01): toolbar → table → row action opens DetailPanel (DS-C-17)."],
    ["A-2", "A", "Search/apply action uses different button styles", "customers: input + primary 'search'. organizations: input + outline 'filter'. knowledge/help-center: combos + outline 'apply' AND a second input + primary 'search' stacked.", "CLOSED in DS-4 (design-lab-v4): FilterBar (DS-C-10): filters left (input/combo), one primary apply; Ctrl/⌘K global search stays in sidebar."],
    ["A-3", "A", "Status→color decided in ≥4 parallel systems", "statusTone() map, inline ternaries (knowledge articles), .state-dot classes, .health-dot classes, .tile level classes.", "CLOSED in DS-4 (design-lab-v4): Single DS.TONE map (this file); badges, dots, tiles, bands all read from it."],
    ["A-4", "A", "Detail record header invented per page", "customers/orgs: section-title.big + hint line. inbox: reader-head avatar row. diagnostics: engine hero.", "CLOSED in DS-4 (design-lab-v4): DetailHeader archetype (DS-A-03): avatar + name + meta pills; engine hero only on console pages (DS-C-24)."],
    ["A-5", "A", "Row-action buttons inconsistent", "knowledge rows: outline+outline+danger triple. customers: outline 'open'. notifications: none (mark-all only).", "CLOSED in DS-4 (design-lab-v4): Rule: one text/outline row action; destructive always danger + confirm dialog; never >2 actions inline."],
    ["A-6", "A", "Counts & record labels formatted differently", "'4 customers (of 4)' in card title vs 'N organization(s)' as section title vs inbox view counts.", "CLOSED in DS-4 (design-lab-v4): DS-C-01 counter pill; card titles never carry counts."],
    ["A-7", "A", "Empty states ad-hoc", "emptyState() component vs hint '(no milestones recorded)' vs silent gaps.", "CLOSED in DS-4 (design-lab-v4): DS-C-14 everywhere; slim variant for inline panes; every empty state says why + what to do."],
    ["A-8", "A", "Sync feedback only on some pages", "sync-note span after Sync now on dashboard/customers/organizations; knowledge sync has no feedback.", "CLOSED in DS-4 (design-lab-v4): Sync is one component: button + inline status line (syncing… → up to date / error)."],
    ["B-1", "B", "Five overlapping info-text patterns", "banner, hint, callout, status-line, quirk-note used interchangeably.", "CLOSED in DS-4 (design-lab-v4): Fixed roles: banner=page context, hint=field help, callout=in-card context, status-line=action result, quirk=receipt-cited quirk (stays)."],
    ["B-2", "B", "One-off composites", "engine-hero, approval-line/when-chip, gap-cand, ep-block.", "CLOSED in DS-4 (design-lab-v4): Engine hero → DS-C-24; approval/when-chip → chip + status-line composition; others fold into cards+kv."],
    ["B-3", "B", "Off-scale spacing", "Micro-gaps of 5/7/9/11px appear across pages (conv rows, tool rows).", "CLOSED in DS-4 (design-lab-v4): DS-4 normalizes to the 4pt scale; 6px allowed only inside control clusters."],
    ["C-1", "C", "Receipt-cited quirk footnotes", "Per-page Reskin notes + quirk notes citing knowledge.rs, inbox.rs, palette.rs etc.", "Intentional — stays. Data-layer bugs are footnoted, never faked fixed."],
    ["C-2", "C", "Data-track items labeled mocked", "unread counts, assignee, draft persistence flagged in replica tags.", "Intentional — stays until the async data track (U2) lands."],
  ];

  /* ========================================================================
     Registry page (ds.html) rendering
     ======================================================================== */
  const swatches = (group) =>
    `<div class="ds-swatch-grid">${group.items.map(([n, use, alias]) => {
      const v = getComputedStyle(document.body).getPropertyValue(`--ds-${n}`).trim();
      return `<div class="ds-swatch"><div class="sw" style="background:var(--ds-${n})"></div><div class="meta"><b>--ds-${n}</b><span class="hex">${esc(v || "(pair)")}</span><span class="use">${esc(use)} · ${esc(alias)}</span></div></div>`;
    }).join("")}</div>`;

  const specTable = (rows) => `<table class="dsreg-spec">${rows.map(([k, v]) => `<tr><td>${esc(k)}</td><td>${v}</td></tr>`).join("")}</table>`;

  const comp = (id, name, spec, demo, dodont) =>
    `<div class="dsreg-h" id="${id}">${name}<span class="ds-comp-id">${id.toUpperCase()}</span></div>${specTable(spec)}<div class="ds-demo-frame">${demo}</div>${dodont || ""}`;

  const dodontPair = (doTxt, dontTxt) => `<div class="ds-dodont"><div class="do"><b>Do</b>${doTxt}</div><div class="dont"><b>Don't</b>${dontTxt}</div></div>`;

  function renderRegistry() {
    const root = document.getElementById("dsreg-root");
    const B = [];

    /* ---- overview ---- */
    B.push(`<div class="dsreg-h" id="overview">Overview</div>`);
    B.push(`<p class="dsreg-lead">This is the <strong>Design System v1.0</strong> for SupportOS+ — the contract that every screen of the reskinned program must satisfy. It is extracted from the approved reskin proposal (design lab, Tasks 18–19), not invented: every token value, component and rule below already renders in the lab's After pages. What changed is <em>governance</em>: pages may only compose these components, tones are decided in one map, and deviations require a registered exception. In the egui reskin (R1–R4) each <span class="ds-comp-id">DS-C-xx</span> maps 1:1 to one typed widget and each token to one <span class="ds-mono">design.rs</span> constant.</p>`);
    B.push(specTable([["Composition rule", "A page = archetypes (DS-A) + components (DS-C). No bespoke CSS, no per-page variants."], ["Tone rule", "Status → color only via <b>DS.TONE</b>. One meaning = one token, in both themes."], ["Scale rule", "Spacing 4pt scale · type roles · radius/elevation/motion tokens. Off-scale = bug."], ["Change control", "System changes version-bump here first, pages update after. The Rust reskin tracks this file."], ["Honesty contract", "Receipt-cited quirks (C-class) stay visible; data-layer bugs are footnoted, never faked fixed."]]));

    /* ---- foundations: color ---- */
    B.push(`<div class="dsreg-h" id="foundations-color">Foundations — color tokens</div>`);
    B.push(`<p class="dsreg-sub">Values are live (swatch shows the current theme). Switch theme with the button in the top bar — every token has exactly one dark value.</p>`);
    TOKEN_GROUPS.forEach((g) => { B.push(sectionTitle(g.name)); B.push(swatches(g)); });

    /* ---- foundations: scales ---- */
    B.push(`<div class="dsreg-h" id="foundations-scales">Foundations — scales</div>`);
    B.push(sectionTitle("Spacing (4pt scale)"));
    B.push(`<div class="ds-scale">${SCALES.space.map(([n, v, u]) => `<div class="row"><code>${n}</code><span class="box" style="width:${v}"></span><span><b>${v}</b> — ${u}</span></div>`).join("")}</div>`);
    B.push(sectionTitle("Type roles"));
    B.push(`<table class="dsreg-table"><tr><th>role</th><th>size</th><th>used for</th><th>specimen</th></tr>${SCALES.type.map(([r, s, u]) => `<tr><td class="mono">${r}</td><td class="mono">${s}</td><td>${u}</td><td><span style="${r === "title" ? "font-size:22px;font-weight:500" : r === "stat" ? "font-size:20px;font-weight:500" : r === "section" ? "font-size:15px;font-weight:500" : r === "lead" ? "font-size:14px" : r === "meta" ? "font-size:11px;color:var(--ds-text-2)" : r === "small" ? "font-size:12px;color:var(--ds-text-2)" : ""}">${r === "title" ? "Page title" : r === "stat" ? "1,024" : r === "section" ? "Section title" : r === "lead" ? "Message body text" : "Support text"}</span></td></tr>`).join("")}</table>`);
    B.push(sectionTitle("Radius & motion"));
    B.push(specTable([...SCALES.radius.map(([n, v, u]) => [n, `${v} — ${u}`]), ...SCALES.motion.map(([n, v, u]) => [n, `${v} — ${u}`])]));

    /* ---- components ---- */
    B.push(`<div class="dsreg-h" id="components">Components</div>`);
    B.push(`<p class="dsreg-sub">One canonical definition per component. Variants live <em>inside</em> the component; pages never restyle them.</p>`);

    B.push(comp("c01", "PageHeader", [["Anatomy", "h1 (22/title) · optional subtitle (12.5) · right-aligned action group · optional counter pill"], ["States", "actions wrap on narrow widths; counter never wraps into title"], ["Used by", "every page (23/23) — replaces per-page header inventions"]],
      pageHeader({ title: "Contacts", subtitle: "People and organizations across every conversation", counter: "4 customers (of 4)", actions: btn.outline("Sync now") + btn.primary("New conversation") }),
      dodontPair("Counts live in the counter pill; actions right-aligned; one h1 per page.", "Card titles carrying counts like “4 customers (of 4)”, action buttons drifting left, subtitles per button.")));

    B.push(comp("c02", "Card", [["Anatomy", "optional header (title 14/500 + sub 12) · body 12/14 padding · radius-xl · e1 shadow"], ["Variants", "default · <b>danger</b> (1px danger border + 4px inset — destructive zones only)"], ["Used by", "every page section; composer; detail panels"]],
      card({ title: "Delivery settings", sub: "applies to all outbound mail", body: formGrid([["From address", combo("support@acme.io")], ["Signature", btn.outline("Edit signature")]]) }) + card({ danger: true, title: "Danger zone", sub: "irreversible", body: `<div style="display:flex;justify-content:space-between;gap:10px;flex-wrap:wrap"><span>Permanently delete this knowledge source</span>${btn.danger("Delete source")}</div>` }),
      dodontPair("One card = one job. Danger zone isolates destructive actions.", "Nesting cards in cards; danger styling for non-destructive warnings; headers without titles.")));

    B.push(comp("c03", "StatTile · MetricTile", [["Anatomy", "stat: label 12 · value 26/500 tabular · optional hint. metric tile: count 20 + label + level"], ["Variants", "tile levels: <span class='ds-badge danger'>crit</span> <span class='ds-badge warning'>warning</span> <span class='ds-badge info'>info</span> · drill affordance (lift + “open →”)"], ["Used by", "dashboard, operations, knowledge, reports"]],
      statGrid([{ label: "Open", value: "23" }, { label: "First response", value: "18m", hint: "median" }]) + `<div class="ds-tile-wrap band-critical">${metricTile({ cnt: "7", lbl: "SLA breaches", level: "crit", drill: true })}${metricTile({ cnt: "12", lbl: "Waiting > 24h", level: "warning", drill: true })}${metricTile({ cnt: "48", lbl: "Resolved this week", level: "info" })}</div>`,
      dodontPair("Bands group tiles by severity; drill tiles navigate.", "Decorative colors on neutral stats; more than one banded band per row group.")));

    B.push(comp("c05", "DataTable", [["Anatomy", "bordered wrap radius-lg · uppercase 11px head on surface-2 · rows 9/12 padding · hover wash"], ["Cell roles", "strong (lead col) · num (tabular right) · mono · muted2 · badge/status"], ["Row actions", "≤2 inline; one text/outline action; destructive = danger + confirm (A-5)"], ["Used by", "all list pages — the only table pattern"]],
      dataTable(["customer", "email", "conversations", "status", ""], [
        `<tr>${tds("Marek Volf")}${td('<span class="ds-muted2">marek@volflabs.io</span>')}${tdn("12")}${td(statusBadge("open"))}${td(btn.text("Open"))}</tr>`,
        `<tr>${tds("Priya Nair")}${td('<span class="ds-muted2">priya@northwind.co</span>')}${tdn("5")}${td(statusBadge("pending"))}${td(btn.text("Open"))}</tr>`,
        `<tr>${tds("Tomás Rivera")}${td('<span class="ds-muted2">tomas@rivera.dev</span>')}${tdn("3")}${td(statusBadge("resolved"))}${td(btn.text("Open"))}</tr>`,
      ]),
      dodontPair("Badges from DS.TONE; counts in num cells; one obvious row action.", "Raw status text without tone; mixed button styles per row; counts as plain left text.")));

    B.push(comp("c06", "Badge & Chip & StatusDot", [["Badge", "18px tinted pill, tone from DS.TONE — status/category text"], ["Chip", "26px pill button — filters, selections, navigation (on = filled)"], ["StatusDot", "8/11px dot bound to the same tone map — table rows, lists"], ["Used by", "everywhere status or a filter appears"]],
      `<div class="ds-chip-row">${statusBadge("open")}${statusBadge("pending")}${statusBadge("resolved")}${statusBadge("closed")}${statusBadge("spam")}${statusBadge("critical")}${statusBadge("high")}${statusBadge("published")}${statusBadge("draft")}${statusBadge("stale")}${aiBadge("AI")}</div><div class="ds-chip-row">${chip("All", true)}${chip("Mine")}${chip("Unassigned")}${chip("Priority")}</div><div class="ds-chip-row">${dot("ok")}<span>healthy</span> ${dot("stale")}<span>stale</span> ${dot("error")}<span>error</span> ${dot("ai")}<span>AI involved</span></div>`,
      dodontPair("Same status = same badge tone on every page; dots for compact contexts.", "Inventing a new color per page; using badges as buttons; dots without a text label nearby.")));

    B.push(comp("c09", "Tabs", [["Anatomy", "underline style: 34px rows, 2px primary underline on active, border-bottom rail"], ["Variants", "grouped (labeled groups, AI Center) · composer Reply|Note variant (DS-C-21)"], ["Used by", "knowledge (5), AI Center (6), settings (6), reports, operations"]],
      tabGroups([{ label: "Workspace", tabs: ["Documents", "Sources", "Freshness"], active: 0 }, { label: "Publishing", tabs: ["Help center", "Article drafts"], active: 1 }]),
      dodontPair("Underline = page-level sections. Group labels when tabs exceed 5.", "Pill tabs for page sections; mixing two tab styles on one page.")));

    B.push(comp("c10", "FilterBar / Toolbar", [["Anatomy", "left: inputs + combos (filters) · right: primary apply + context actions · wraps on narrow"], ["Search", "list search is in-page; ⌘K global search stays in the sidebar (A-2)"], ["Used by", "every list page — one pattern replaces 4 ad-hoc toolbars"]],
      toolbar(`${input("Filter by name…", 240)}${combo("Status: any")}${combo("Owner: anyone")}`, btn.primary("Apply"), true) + toolbar(`${input("Search customers by name or email…", 280)}`, btn.primary("Search")),
      dodontPair("One primary action per toolbar; filters left, actions right.", "Outline 'filter' vs primary 'search' mixing; stacking two toolbars for one flow.")));

    B.push(comp("c11", "Buttons", [["Set", "primary (one per view) · outline (secondary) · text (row/tertiary) · danger (destructive) · ai (AI actions, violet) · sm (toolbar chips)"], ["Sizes", "32px default · 28px in-card (AI) · 24px sm"], ["Used by", "everywhere"]],
      `<div class="ds-chip-row">${btn.primary("Send reply")}${btn.outline("Sync now")}${btn.outlineAi("AI draft")}${btn.ai("Insert into reply")}${btn.danger("Delete source")}${btn.text("Open")}</div>`,
      dodontPair("AI actions are always violet — the AI identity is non-negotiable.", "Multiple primary buttons per view; danger for non-destructive actions; AI actions in neutral gray.")));

    B.push(comp("c12", "Inputs & forms", [["Anatomy", "32px inputs / combo / check · field-label (11.5 uppercase) for section labels · form-grid 220px label column"], ["Focus", "border-strong; textarea adds AI-tinted focus ring (composer)"], ["Used by", "settings, automation, knowledge, organizations"]],
      card({ title: "New organization", body: formGrid([["Name", input("Acme Inc", 220)], ["Domains", input("acme.com, acme.io", 220)], ["Visible to AI", `<label class="ds-check"><input type="checkbox" checked> include in AI answers</label>`]]) }),
      dodontPair("Labels stay in the 220px column; checkboxes for boolean settings.", "Placeholder-only labels; inventing new input heights per page.")));

    B.push(comp("c14", "EmptyState", [["Anatomy", "icon 38px · title 15/500 · sub 12.5 (why + what to do) · optional CTA · slim variant (120px) for inline panes"], ["Used by", "every list/tab/pane (A-7)"]],
      emptyState(ICONS.search, "No freshness data", "The current build reads report[\"flags\"], which freshness_report() never emits — this tab names the void instead of failing silently.") + emptyState(ICONS.user, "Select an organization", "Its members, domains, and conversation counts appear here.", true),
      dodontPair("Every empty state explains why and offers the next step.", "Silent blank areas; hint text pretending to be an empty state.")));

    B.push(comp("c15", "Callout set", [["Fixed roles", "banner = page-level context · hint = field help · callout (warn/err) = in-card context · status-line = action result · quirk = receipt-cited quirk"], ["Used by", "all pages (B-1)"]],
      banner("Reports read from the last completed intelligence sweep.") + callout("Zero-hit searches are recorded as knowledge gaps.", "warn") + callout("Sync failed: Help Scout returned 503. Retry from Sync Health.", "err") + `<div class="ds-status-line ok">up to date — nothing new from Help Scout</div>` + quirk("presentation-fixable: routing combo shows endpoint index instead of name (ai_center.rs:211)."),
      dodontPair("Pick by role, not by looks. Quirks always cite their receipt.", "Using danger callouts for success info; hiding quirks once fixed elsewhere.")));

    B.push(comp("c17", "DetailPanel", [["Anatomy", "right drawer 420px · header (title + close) · scroll body (kv, timeline, actions) · scrim click closes"], ["Used by", "row opens in every list archetype (A-1) — replaces full-page profile swaps"]],
      `<button class="ds-btn primary" data-ds-act="panel-demo" type="button">Open detail panel demo</button>`,
      dodontPair("Context stays: the list remains visible behind the scrim.", "Navigating away to show a record; drawers without a close affordance.")));

    B.push(comp("c19", "Avatar & Assignee", [["Anatomy", "initials avatar (16/28/34) · assignee pill · dashed 'unassigned' pill when nobody owns it"], ["Used by", "conversation list, reader head, tables, timeline"]],
      `<div class="ds-chip-row">${avatar("Marek Volf", "lg")}<span>Marek Volf</span>${avatar("Priya Nair", "md")}${assignee("Tomás Rivera")}${assignee(null)}</div>`,
      dodontPair("Unassigned is explicit, not blank.", "Color-coded avatars per role (role is carried by badges/surfaces, not avatars).")));

    B.push(comp("c20", "Message Card (role-typed)", [["Anatomy", "radius-xl card · head: avatar + name + role label + relative time · body 14/1.62 · optional quoted history (collapsed) · attachment chips · per-role actions"], ["Roles", "customer (white) · agent (tinted neutral) · note (amber, internal) · <b>ai (violet — its own identity)</b>"], ["Used by", "inbox reader, conversation previews in copilot"]],
      dayDivider("Yesterday") + messageCard({ role: "customer", name: "Marek Volf", time: "14:02", body: "The export button fails with a 500 on files over 100MB.\nThis blocks our monthly report — urgent.", attachments: [{ name: "export-log.txt", size: "4 KB" }] }) + messageCard({ role: "agent", name: "Priya Nair", roleLabel: "Agent", time: "14:11", body: "Thanks Marek — I can reproduce it. Checking the export worker now.", quoted: [{ from: "Marek Volf · Mon", body: "Any timeline for the export fix?" }] }) + dayDivider("Today") + messageCard({ role: "note", name: "Priya Nair", roleLabel: "Internal note", time: "09:30", body: "Root cause: stream timeout at 60s in export worker. Fix incoming — do not promise a date yet." }) + messageCard({ role: "ai", name: "SupportOS AI", roleLabel: "AI answer", time: "09:41", body: "I found the failing worker config. The timeout is set to 60s; your largest export needs ~240s. This is escalated to engineering.", actions: btn.ai("Insert into reply") + btn.outline("Show sources") }),
      dodontPair("AI is always violet with its own badge; notes are amber and say Internal note; quoted history is collapsed by default.", "AI messages styled like agent messages (the old bug); raw UTC timestamps; quoted history inlined uninvited.")));

    B.push(comp("c21", "Composer", [["Anatomy", "Reply | Note tabs · tool row (saved replies, attach, AI draft) · textarea (amber in note mode) · footer: context left, send right"], ["Accelerators", "Ctrl+Enter send · / filter · saved replies menu (R3)"], ["Used by", "inbox reader"]],
      `<div style="max-width:640px">${composerDemo("reply")}</div>`,
      dodontPair("Note mode changes the textarea AND the send button color.", "Saved replies buried in a settings page; drafts without the draft flag.")));

    B.push(comp("c22", "AI identity set", [["Pieces", "aiBadge (badge.ai) · AI message card (violet) · copilot card types: summary (bullets) · suggested draft (violet) · similar conversations · chat turns with cited sources"], ["Rule", "Every AI surface carries the violet token + sparkles/bot mark. AI never renders as plain agent content."]],
      `<div style="max-width:420px;display:flex;flex-direction:column;gap:9px">${copSummary(["Customer blocked by export 500s >100MB", "Agent reproduced; escalation open", "Promise-sensitive: no date given yet"])}</div><div style="max-width:420px;display:flex;flex-direction:column;gap:9px;margin-top:9px">${copSuggest("Hi Marek — we reproduced the export failure and found the timeout config. Engineering is on a fix; I'll update you today.")}</div><div style="max-width:420px;margin-top:9px">${copChat([{ who: "user", text: "why did the export fail?" }, { who: "assistant", text: "The stream timeout (60s) is shorter than your largest export needs (~240s).", cited: "core/export/worker.rs:88" }])}</div>`,
      dodontPair("Cite sources on AI claims; type each copilot card by its job.", "Blending copilot card types into one dense blob (the audited v1.8.0 pattern).")));

    B.push(comp("c24", "Hero & event rows (console)", [["Anatomy", "hero: mono state 17 + description + right actions · event rows: ts + status dot + line, level insets (error/warn)"], ["Used by", "diagnostics, sync health, run logs (DS-A-06)"]],
      hero("RUNNING", "background sync · last checkpoint 2m ago", btn.outline("Pause")) + `<div style="max-width:640px">${evRow("09:41:02", "ok", "sync completed — 0 new events")}${evRow("09:12:55", "warn", "checkpoint older than 30m", "sync_worker restarted")}${evRow("08:59:10", "error", "Help Scout API 503", "retry in 64s (attempt 3/8)")}</div>`,
      dodontPair("Level color carries meaning via inset + dot, never the whole row.", "Walls of mono text without level structure.")));

    /* ---- archetypes ---- */
    B.push(`<div class="dsreg-h" id="archetypes">Page archetypes</div>`);
    B.push(`<p class="dsreg-sub">A page may only be a composition of these archetypes + the components above. Anything bespoke must be registered as an exception.</p>`);

    B.push(`<div class="dsreg-h" id="a01">List page<span class="ds-comp-id">DS-A-01</span></div>`);
    B.push(`<p class="dsreg-sub">Contacts · Incidents · Outreach · Knowledge lists · Automation rules — one flow: toolbar → table → row action → DetailPanel.</p>`);
    B.push(`<div class="ds-demo-frame">${pageHeader({ title: "Contacts", subtitle: "People and organizations across every conversation", counter: "4 (of 4)", actions: btn.outline("Sync now") })}${toolbar(`${input("Filter…", 200)}${combo("Status: any")}`, btn.primary("Apply"), true)}${dataTable(["name", "email", "conversations", "status", ""], [`<tr data-ds-act="panel-demo" style="cursor:pointer">${tds("Marek Volf")}${td('<span class="ds-muted2">marek@volflabs.io</span>')}${tdn("12")}${td(statusBadge("open"))}${td(btn.text("Open"))}</tr>`, `<tr>${tds("Priya Nair")}${td('<span class="ds-muted2">priya@northwind.co</span>')}${tdn("5")}${td(statusBadge("pending"))}${td(btn.text("Open"))}</tr>`, `<tr>${tds("Tomás Rivera")}${td('<span class="ds-muted2">tomas@rivera.dev</span>')}${tdn("3")}${td(statusBadge("resolved"))}${td(btn.text("Open"))}</tr>`])}</div>`);

    B.push(`<div class="dsreg-h" id="a02">Dashboard<span class="ds-comp-id">DS-A-02</span></div>`);
    B.push(`<p class="dsreg-sub">Dashboard · Reports · Operations — stat grid → trend chart → breakdown bars → drill tiles.</p>`);
    B.push(`<div class="ds-demo-frame">${pageHeader({ title: "Dashboard", actions: btn.outline("Sync now") })}${statGrid([{ label: "Open", value: "23" }, { label: "Resolved today", value: "14" }, { label: "First response", value: "18m" }, { label: "AI resolutions", value: "31%" }])}${card({ title: "Conversations per day", sub: "last 14 days", body: lineChart([12, 18, 9, 22, 30, 26, 41, 38, 25, 33, 44, 39, 51, 47], "14-day trend · AI-involved share in violet") })}${card({ title: "By channel", body: barRow("Email", 128, 128) + barRow("Chat", 77, 128) + barRow("Widget", 41, 128) })}</div>`);

    B.push(`<div class="dsreg-h" id="a03">Detail record<span class="ds-comp-id">DS-A-03</span></div>`);
    B.push(`<p class="dsreg-sub">Customer profile · Organization · Knowledge article — identity header → two-col facts/AI → timeline.</p>`);
    B.push(`<div class="ds-demo-frame">` +
      `<div style="display:flex;align-items:center;gap:10px;margin-bottom:14px">${avatar("Marek Volf", "lg")}<div><div style="font-size:18px;font-weight:500">Marek Volf</div><div class="ds-page-sub">marek@volflabs.io · Volf Labs · last activity 2h ago</div></div><div style="margin-left:auto;display:flex;gap:8px">${statusBadge("open")}${aiBadge("AI profile")}</div></div>` +
      twoCol(
        card({ title: "Support health", body: formGrid([["Total conversations", "12"], ["Open", "1"], ["SLA breaches", "0"]], true) }) +
        card({ title: "Timeline", body: `<div class="ds-scroll-220">${tlRow("2026-06-01 14:02", "conversation", "Export fails on large files")}${tlRow("2026-06-01 14:11", "reply", "Priya Nair reproduced the issue")}${tlRow("2026-06-02 09:41", "ai", "AI drafted follow-up with sources")}</div>` }),
        card({ title: "AI interaction profile", body: kvRow("Stage", statusBadge("active")) + kvRow("Sentiment", statusBadge("ok")) + kvRow("Preferred channel", "email") + kvRow("AI relevance", "high") + hint("Derived from the last 12 conversations · provenance: intelligence sweep v3") }),
        true) +
      `</div>`);

    B.push(`<div class="dsreg-h" id="a04">Settings page<span class="ds-comp-id">DS-A-04</span></div>`);
    B.push(`<p class="dsreg-sub">Settings · Connectors · Automation config — tab rail → grouped cards → form grids → isolated danger zone.</p>`);
    B.push(`<div class="ds-demo-frame">${tabs(["General", "Mailboxes", "AI", "Danger zone"], 2)}${card({ title: "AI assistance", sub: "applies to every mailbox", body: formGrid([["Draft replies", `<label class="ds-check"><input type="checkbox" checked> propose drafts in composer</label>`], ["Auto-answer", combo("Only high-confidence")], ["Knowledge scope", combo("Published only")]]) })}${card({ danger: true, title: "Danger zone", sub: "irreversible", body: `<div style="display:flex;justify-content:space-between;gap:10px;flex-wrap:wrap"><span>Delete all AI conversation summaries</span>${btn.danger("Delete…")}</div>` })}</div>`);

    B.push(`<div class="dsreg-h" id="a05">Inbox workspace<span class="ds-comp-id">DS-A-05</span></div>`);
    B.push(`<p class="dsreg-sub">The 4-pane workspace: views rail → conversation list → reader → copilot. Message cards + composer per DS-C-20/21/22.</p>`);
    B.push(`<div class="ds-demo-frame" style="display:grid;grid-template-columns:150px 200px 1fr;gap:10px">
      <div>${sectionTitle("Views")}${[`<div class="ds-sel-row selected" style="padding:7px 9px">All <span class="ds-counter" style="margin-left:6px">23</span></div>`, `<div class="ds-sel-row" style="padding:7px 9px">Mine</div>`, `<div class="ds-sel-row" style="padding:7px 9px">Unassigned</div>`, `<div class="ds-sel-row" style="padding:7px 9px">Priority</div>`].join("")}${hint("j / k · Enter · /")}</div>
      <div>${input("Filter…  ( / )", 0).replace('class="ds-text-input"', 'class="ds-text-input" style="width:100%"')}${[`<div class="ds-sel-row selected" style="padding:8px 9px;margin-top:8px"><div class="lr1">${dot("info", true)}Marek Volf</div><div class="lr2">Export fails on large files…</div></div>`, `<div class="ds-sel-row" style="padding:8px 9px"><div class="lr1">Priya Nair</div><div class="lr2">Billing question about…</div></div>`].join("")}</div>
      <div>${messageCard({ role: "customer", name: "Marek Volf", time: "09:12", body: "Export button fails with 500 on files over 100MB — blocking our monthly report." })}${messageCard({ role: "ai", name: "SupportOS AI", roleLabel: "AI answer", time: "09:14", body: "Found it: stream timeout 60s vs needed ~240s. Escalated to engineering.", actions: btn.ai("Insert into reply") })}${composerDemo("reply")}</div></div>`);

    B.push(`<div class="dsreg-h" id="a06">Console / log<span class="ds-comp-id">DS-A-06</span></div>`);
    B.push(`<p class="dsreg-sub">Diagnostics · Sync health — hero state → filter chips → event rows with level insets.</p>`);
    B.push(`<div class="ds-demo-frame">${pageHeader({ title: "Diagnostics", subtitle: "Live health of the background engine", actions: btn.outline("Copy report") })}${hero("RUNNING", "engine healthy · 0 zombie workers", `${btn.outline("Pause")} ${btn.outline("Restart")}`)}<div class="ds-chip-row">${chip("All", true)}${chip("Errors")}${chip("Warnings")}${chip("Sync")}</div><div style="max-width:640px">${evRow("09:41:02", "ok", "sync completed — 0 new events")}${evRow("08:59:10", "error", "Help Scout API 503 — retry in 64s")}${evRow("08:12:44", "warn", "checkpoint older than 30m")}</div></div>`);

    /* ---- tone map ---- */
    B.push(`<div class="dsreg-h" id="tone-map">Tone map — the only status → color table</div>`);
    B.push(`<p class="dsreg-sub">Every status in the product resolves here. Adding a status = adding a row here first (version bump), never a new color at the page.</p>`);
    B.push(`<table class="dsreg-table"><tr><th>status</th><th>tone</th><th>badge</th><th>dot</th></tr>${Object.entries(TONE).map(([k, v]) => `<tr><td class="mono">${k}</td><td class="mono">${v.tone}</td><td>${badge(v.tone, v.label)}</td><td>${dot(k)}</td></tr>`).join("")}</table>`);

    /* ---- rules ---- */
    B.push(`<div class="dsreg-h" id="rules">Rules</div>`);
    B.push(specTable([
      ["1 · Composition", "Pages compose DS archetypes + components only. Bespoke UI = registered exception or it doesn't ship."],
      ["2 · Tone", "Status → color via DS.TONE only. AI is always violet. Notes are always amber. One meaning, one token."],
      ["3 · Scales", "Spacing on the 4pt scale; type by role; radius/elevation/motion tokens. Off-scale values are bugs (DS-4 normalizes)."],
      ["4 · Hierarchy", "One h1, one primary button per view, counts in counter pills, destructive actions isolated in danger zones."],
      ["5 · Honesty", "Quirks and data-track limits stay visible and cited (C-class). The system never fakes a fix."],
      ["6 · Change control", "This registry is versioned. System change → bump + changelog → pages follow. The egui reskin tracks this file, not individual pages."],
      ["7 · egui mapping", "Every DS-C becomes one typed widget; every --ds-* token one design.rs constant; DS.TONE one match arm. Snapshot tests (Task 20/F3) lock the result."],
    ]));

    /* ---- register ---- */
    B.push(`<div class="dsreg-h" id="register">Mismatch register (DS-0 audit)</div>`);
    B.push(`<p class="dsreg-sub">The evidence behind this system: what mismatched across the 23 lab pages, and the DS answer. STATUS after DS-4 (design-lab-v4): all A/B items CLOSED — the 23 pages compose only DS archetypes + components (pages-ds.js); C items stay by contract. Full receipts: <span class="ds-mono">mismatch-register.md</span>.</p>`);
    B.push(`<table class="dsreg-table"><tr><th>id</th><th>class</th><th>finding</th><th>evidence</th><th>DS resolution</th></tr>${REGISTER.map(([id, cls, f, ev, res]) => `<tr><td class="mono">${id}</td><td>${cls === "A" ? '<span class="ds-badge danger">A</span>' : cls === "B" ? '<span class="ds-badge warning">B</span>' : '<span class="ds-badge success">C · keep</span>'}</td><td class="strong">${esc(f)}</td><td>${esc(ev)}</td><td>${esc(res)}</td></tr>`).join("")}</table>`);

    root.innerHTML = B.join("");
  }

  /* ----- registry shell: nav, theme, router -------------------------------- */
  const NAV = [
    ["Overview", "overview", ""],
    ["Foundations", "", ""],
    ["Color tokens", "foundations-color", ""],
    ["Scales", "foundations-scales", ""],
    ["Components", "", ""],
    ["PageHeader", "c01", "C-01"], ["Card", "c02", "C-02"], ["Stat/Metric tiles", "c03", "C-03"], ["DataTable", "c05", "C-05"],
    ["Badge·Chip·Dot", "c06", "C-06"], ["Tabs", "c09", "C-09"], ["FilterBar", "c10", "C-10"], ["Buttons", "c11", "C-11"],
    ["Inputs & forms", "c12", "C-12"], ["EmptyState", "c14", "C-14"], ["Callout set", "c15", "C-15"], ["DetailPanel", "c17", "C-17"],
    ["Avatar/Assignee", "c19", "C-19"], ["Message Card", "c20", "C-20"], ["Composer", "c21", "C-21"], ["AI identity", "c22", "C-22"], ["Hero & events", "c24", "C-24"],
    ["Archetypes", "", ""],
    ["List page", "a01", "A-01"], ["Dashboard", "a02", "A-02"], ["Detail record", "a03", "A-03"], ["Settings", "a04", "A-04"], ["Inbox workspace", "a05", "A-05"], ["Console", "a06", "A-06"],
    ["Tone map", "tone-map", ""],
    ["Rules", "rules", ""],
    ["Mismatch register", "register", ""],
  ];

  function initShell() {
    const nav = document.getElementById("dsreg-nav");
    if (!nav) return; /* library-only load (e.g. reskin.html) — the registry shell only exists on ds.html */
    nav.innerHTML = `<div class="grp" style="padding-top:0">Design System v1.0</div>` + NAV.map(([label, id, tag]) =>
      id ? `<a href="#${id}" data-sec="${id}">${tag ? `<span class="id">${tag}</span>` : ""} ${esc(label)}</a>` : `<div class="grp">${esc(label)}</div>`
    ).join("");

    const themeBtn = document.getElementById("ds-theme");
    const applyThemeIcon = () => { themeBtn.innerHTML = (document.body.dataset.theme === "light" ? ICONS.moon : ICONS.sun) + `<span style="margin-left:6px">${document.body.dataset.theme === "light" ? "Dark" : "Light"}</span>`; };
    themeBtn.addEventListener("click", () => { document.body.dataset.theme = document.body.dataset.theme === "light" ? "dark" : "light"; applyThemeIcon(); renderRegistry(); });
    applyThemeIcon();

    nav.addEventListener("click", (e) => {
      const a = e.target.closest("a[data-sec]"); if (!a) return;
      nav.querySelectorAll("a").forEach((x) => x.classList.toggle("active", x === a));
    });

    document.addEventListener("click", (e) => {
      const el = e.target.closest("[data-ds-act]"); if (!el) return;
      const act = el.dataset.dsAct;
      if (act === "panel-demo") panel.open("Marek Volf — customer", kvRow("Email", "marek@volflabs.io") + kvRow("Organization", "Volf Labs") + kvRow("Status", statusBadge("open")) + kvRow("Conversations", "12") + `<div class="ds-t-divider"></div>` + sectionTitle("Recent") + tlRow("2026-06-01 14:02", "conversation", "Export fails on large files") + tlRow("2026-06-01 14:11", "reply", "Priya Nair reproduced the issue") + tlRow("2026-06-02 09:41", "ai", "AI drafted follow-up with sources") + `<div class="ds-t-divider"></div>` + `${btn.primary("Open conversation")} ${btn.outline("Assign to me")}`);
      if (act === "panel-close") panel.close();
    });
    document.addEventListener("keydown", (e) => { if (e.key === "Escape") panel.close(); });

    renderRegistry();
  }

  window.DS = { ICONS, TONE, toneOf, badge, statusBadge, aiBadge, chip, dot, pageHeader, card, sectionTitle, twoCol, statGrid, metricTile, dataTable, td, tds, tdn, tabs, tabGroups, toolbar, btn, input, combo, formRow, formGrid, fieldLabel, emptyState, hint, banner, callout, quirk, kvRow, tlRow, avatar, assignee, messageCard, dayDivider, fileChip, composerDemo, copSuggest, copSummary, copSimilar, copChat, hero, evRow, selList, lineChart, barRow, toast, panel, REGISTER, TOKEN_GROUPS, SCALES };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", initShell); else initShell();
})();

/* ==========================================================================
   DS v1.0 — DS-4 conformance engine (RS-01)
   --------------------------------------------------------------------------
   Adds: a few canonical builders (row, selRow, barCell, textarea, sync
   control, grouped tabs, icon button, copy icon), the six page archetypes
   (DS-A-01..06), the page registry (definePage/renderPage) and the
   interactive inbox workspace. Pages may ONLY compose from here.
   ========================================================================== */
(() => {
  "use strict";
  const DS = window.DS;
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const initials = (n) => String(n || "?").split(/\s+/).map((w) => w[0]).slice(0, 2).join("").toUpperCase();

  /* ----- canonical additions ------------------------------------------------ */
  DS.ICONS.copy = DS.ICONS.copy || `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>`;
  DS.row = (main, side) => `<div class="ds-row"><span>${main}</span>${side ? `<span class="ds-row-side">${side}</span>` : ""}</div>`;
  DS.selRow = (l1, l2, opts) => {
    const o = opts || {};
    return `<div class="ds-sel-row${o.selected ? " selected" : ""}"${o.act ? ` data-ds-act="${o.act}"` : ""}${o.id != null ? ` data-id="${esc(o.id)}"` : ""}${o.i != null ? ` data-i="${o.i}"` : ""} tabindex="0" role="listitem"><div class="lr1">${l1}</div>${l2 ? `<div class="lr2">${l2}</div>` : ""}</div>`;
  };
  DS.barCell = (pct, label) => `<span class="ds-bar-cell"><span class="ds-load-bar"><i style="width:${Math.max(3, Math.min(100, pct))}%"></i></span><span class="ds-mono ds-small">${esc(label)}</span></span>`;
  DS.textarea = (ph, rows) => `<textarea class="ds-textarea" rows="${rows || 3}" placeholder="${esc(ph)}"></textarea>`;
  DS.iconBtn = (icon, label) => `<button class="ds-icon-btn" type="button" aria-label="${esc(label)}">${icon}</button>`;
  /* A-8: sync is ONE composite — button + inline status line */
  DS.syncControl = (label) => `<button class="ds-btn outline" data-ds-sync="1" type="button">${esc(label || "Sync now")}</button><span class="ds-status-line" data-sync-line aria-live="polite"></span>`;
  DS.groupedTabs = (groups, act) => `<div class="ds-tabgroups">${groups.map((g) => `<div><div class="tg-label">${esc(g.label)}</div>${DS.tabs(g.tabs, g.active, act, g.from || 0)}</div>`).join("")}</div>`;

  /* ----- page registry -------------------------------------------------------- */
  const PAGES = {};
  DS.definePage = (id, def) => { def.id = id; if (!def.state) def.state = {}; def.archetype = def.archetype || "stack"; PAGES[id] = def; };
  DS.pageDefs = PAGES;

  const notesButton = (id) => {
    const n = ((window.LAB && window.LAB.PAGE_NOTES[id]) || []).length;
    return n ? `<button class="ds-btn outline" data-notes="1" type="button">${DS.ICONS.pencil} Reskin notes <span class="ds-counter">${n}</span></button>` : "";
  };
  const headerHTML = (ctx) => {
    const m = (ctx.def.meta && ctx.def.meta(ctx.state)) || {};
    return DS.pageHeader({ title: m.title || "", subtitle: m.subtitle || "", counter: m.counter || "", actions: `${m.actions || ""}${notesButton(ctx.id)}` });
  };
  const tabStrip = (ctx) => {
    const t = ctx.def.tabs && ctx.def.tabs(ctx.state);
    if (!t) return "";
    if (t.groups) return DS.groupedTabs(t.groups, "tab");
    return DS.tabs(t.items, t.active, "tab");
  };
  const openDetail = (el, ctx) => {
    const d = ctx.def.detail(Number(el.dataset.i), ctx.state, ctx);
    DS.panel.open(d.title, d.body);
  };

  const ARCH = {
    /* DS-A-02 dashboard / DS-A-04 settings — header + optional tabs + block stack */
    stack(root, ctx) {
      root.innerHTML = headerHTML(ctx) + tabStrip(ctx) + (ctx.def.blocks(ctx.state, ctx) || []).join("");
    },
    /* DS-A-06 console — header + optional hero + chips + event/block stack */
    console(root, ctx) {
      const hero = ctx.def.hero ? ctx.def.hero(ctx.state) : "";
      root.innerHTML = headerHTML(ctx) + hero + tabStrip(ctx) + (ctx.def.blocks(ctx.state, ctx) || []).join("");
    },
    /* DS-A-01 list — FilterBar → DataTable → row opens DetailPanel (DS-C-17) */
    list(root, ctx) {
      const L = ctx.def.list(ctx.state, ctx);
      const tool = L.toolbar
        ? DS.toolbar(L.toolbar.left || "", L.toolbar.apply ? `<button class="ds-btn primary" data-ds-act="${L.toolbar.applyAct || "noop"}" type="button">${esc(L.toolbar.apply)}</button>` : "", true)
        : "";
      const raw = typeof L.rows === "function" ? L.rows(ctx.state, ctx) : (L.rows || []);
      const rows = Array.isArray(raw) ? raw : [raw];
      const body = rows.length ? DS.dataTable(L.headers, rows.join("")) : DS.emptyState((L.empty && L.empty.icon) || DS.ICONS.inbox, (L.empty && L.empty.title) || "Nothing here", (L.empty && L.empty.sub) || "");
      root.innerHTML = headerHTML(ctx) + tool + body + (ctx.def.blocks ? (ctx.def.blocks(ctx.state, ctx) || []).join("") : "");
    },
    /* DS-A-03 detail record — entity header (avatar+name+meta+badges+actions) → two-col blocks */
    record(root, ctx) {
      const h = ctx.def.record(ctx.state, ctx) || {};
      const head = `<div class="ds-record-head">${h.avatar || ""}<div class="ds-record-id"><div class="ds-record-name">${h.name || ""}</div>${h.meta ? `<div class="ds-record-meta">${h.meta}</div>` : ""}</div><div class="ds-record-badges">${h.badges || ""}</div><div class="ds-record-actions">${h.actions || ""}</div></div>`;
      root.innerHTML = head + (h.left || h.right ? DS.twoCol((h.left || []).join(""), (h.right || []).join(""), h.wideLeft) : "") + (ctx.def.blocks ? (ctx.def.blocks(ctx.state, ctx) || []).join("") : "");
    },
    /* DS-A-05 inbox workspace — views rail → list → reader → copilot */
    inbox(root, ctx) { renderInbox(root); },
  };
  DS.archetypes = ARCH;

  /* ----- engine ---------------------------------------------------------------- */
  const BUILTIN = {
    tab: (el, ctx) => { ctx.state.tab = Number(el.dataset.i); ctx.rerender(); },
    "open-detail": openDetail,
    "panel-close": () => DS.panel.close(),
    noop: () => {},
  };
  function renderPage(id, root) {
    const def = PAGES[id];
    if (!def) return;
    root.dataset.page = id;
    const ctx = { id, def, state: def.state, root };
    ctx.rerender = () => renderPage(id, root);
    ARCH[def.archetype](root, ctx);
    if (def.after) def.after(root, ctx);
    if (id === "inbox") document.dispatchEvent(new CustomEvent("ds-inbox-rendered"));
    /* ONE delegated dispatcher per page render. The inbox archetype exposes a
       dispatch fn (inboxClick, hoisted) that returns false when it consumed the
       event; everything else resolves through def.act/BUILTIN. */
    root.onclick = (e) => {
      if (def.archetype === "inbox" && inboxClick(e) === false) return;
      const el = e.target.closest("[data-ds-act],[data-ds-sync],[data-notes]");
      if (!el) return;
      if (el.dataset.notes != null) { window.dispatchEvent(new CustomEvent("lab-open-notes", { detail: { page: id } })); return; }
      if (el.dataset.dsSync != null) { doSync(el); return; }
      const act = el.dataset.dsAct;
      const h = (def.act && def.act[act]) || BUILTIN[act];
      if (h) h(el, ctx, e);
    };
  }
  DS.renderPage = renderPage;
  function doSync(btn) {
    const line = btn.parentElement.querySelector("[data-sync-line]") || btn.closest(".ds-page-actions").querySelector("[data-sync-line]");
    if (!line) return;
    line.classList.remove("ok", "err");
    line.textContent = "syncing…";
    setTimeout(() => { line.textContent = "up to date — nothing new from Help Scout"; line.classList.add("ok"); }, 1200);
  }

  /* ===== DS-A-05 — the inbox workspace ========================================= */
  const IB = { view: "All", filter: "", selected: 501, composerMode: "reply", draft: "", copilotOpen: true };
  DS.inboxState = IB;
  const convs = () => window.LAB.conversations;
  const meFirst = () => window.LAB.CURRENT_USER.name.split(" ")[0];
  const cur = () => convs().find((x) => x.id === IB.selected);
  const visibleConversations = () => convs().filter((c) => {
    if (IB.view === "Mine" && c.assignee !== meFirst()) return false;
    if (IB.view === "Unassigned" && c.assignee) return false;
    if (["Active", "Pending", "Closed"].includes(IB.view) && c.status !== IB.view.toLowerCase()) return false;
    if (IB.filter && !((c.subject + " " + c.customer).toLowerCase().includes(IB.filter.toLowerCase()))) return false;
    return true;
  });
  DS.inboxVisible = visibleConversations;
  const viewCount = (v) => {
    const cs = convs();
    if (v === "All") return cs.length;
    if (v === "Mine") return cs.filter((c) => c.assignee === meFirst()).length;
    if (v === "Unassigned") return cs.filter((c) => !c.assignee).length;
    return cs.filter((c) => c.status === v.toLowerCase()).length;
  };

  const assigneePill = (c) =>
    `<button class="ds-pill" type="button" title="assignment is data-track (U2-I1) — mocked here">${c.assignee ? DS.assignee(c.assignee) : `<span class="ds-assignee dashed" style="font-style:italic">Assign</span>`}</button>`;

  const viewsHTML = () => window.LAB.VIEWS.map((v) =>
    `<button class="ds-view-item${IB.view === v ? " active" : ""}" data-ds-act="set-view" data-view="${v}" type="button">${v}<span class="ds-view-count">${viewCount(v)}</span></button>`).join("");

  const convRowHTML = (c) => `<div class="ds-conv-row${IB.selected === c.id ? " selected" : ""}${c.unread ? " unread" : ""}" data-ds-act="open-conv" data-id="${c.id}" role="listitem" tabindex="0" aria-label="${esc(c.subject)}">
    <div class="ds-conv-l1"><span class="ds-udot"></span><span class="ds-conv-name">${esc(c.customer)}</span><span class="ds-conv-time">${esc(c.rel)}</span></div>
    <div class="ds-conv-l2">${esc(c.preview)}</div>
    <div class="ds-conv-l3">${c.assignee ? DS.assignee(c.assignee) : `<span class="ds-assignee dashed">Unassigned</span>`}${DS.statusBadge(c.status)}</div>
  </div>`;

  const headHTML = () => {
    const c = cur();
    if (!c) return "";
    return `<span class="ds-av">${esc(initials(c.customer))}</span>
      <div style="min-width:0"><div class="ds-rh-name">${esc(c.customer)}</div><div class="ds-rh-mail">${esc(c.email)} · mailbox: ${esc(c.mailbox)}</div></div>
      <div class="ds-rh-actions">${assigneePill(c)}${DS.statusBadge(c.status)}
      <button class="ds-pill${IB.copilotOpen ? " on" : ""}" id="copilot-toggle" type="button" aria-pressed="${IB.copilotOpen}">${DS.ICONS.sparkles} Copilot</button></div>`;
  };

  const msgCard = (m, idx) => DS.messageCard({
    role: m.kind,
    name: m.author,
    roleLabel: m.kind === "customer" ? "Customer · email" : m.kind === "agent" ? "Agent" : m.kind === "note" ? "Internal note" : "AI · draft",
    time: m.rel,
    body: m.body.map((p) => `<p>${esc(p)}</p>`).join(""),
    quoted: (m.quoted || []).map((q) => ({ from: `${q.from} — ${q.date} UTC`, body: q.body })),
    attachments: m.attachments,
    actions: m.kind === "ai"
      ? `<button class="ds-btn ai" data-ds-act="insert-ai" data-i="${idx}" type="button">${DS.ICONS.pencil} Insert into reply</button><button class="ds-btn outline" data-ds-act="copy-ai" data-i="${idx}" type="button">${DS.ICONS.copy} Copy</button>`
      : "",
  });

  const readerHTML = () => {
    const c = cur();
    if (!c) return "";
    let lastDay = null;
    return c.messages.map((m, idx) => {
      const div = m.day !== lastDay ? DS.dayDivider(m.day) : "";
      lastDay = m.day;
      return div + msgCard(m, idx);
    }).join("");
  };

  const composerHTML = () => {
    const c = cur();
    const note = IB.composerMode === "note";
    return `<div class="ds-composer-inner">
      <div class="ds-cp-tabs" role="tablist">
        <button class="ds-cp-tab${note ? "" : " active"}" data-ds-act="cp-tab" data-i="reply" role="tab" aria-selected="${!note}" type="button">${DS.ICONS.send} Reply</button>
        <button class="ds-cp-tab${note ? " active" : ""}" data-ds-act="cp-tab" data-i="note" role="tab" aria-selected="${note}" type="button">${DS.ICONS.pencil} Internal note</button>
        <span class="ds-draft-flag${IB.draft ? " show" : ""}" id="draft-flag">Draft saved</span>
      </div>
      <div class="ds-cp-tools">
        <button class="ds-tool-chip" id="sr-btn" type="button">${DS.ICONS.chevronD} Saved replies</button>
        <button class="ds-tool-chip ai" id="ai-btn" type="button" ${c && c.copilot ? "" : "disabled"}>${DS.ICONS.sparkles} Insert AI draft</button>
        <button class="ds-tool-chip" type="button" title="attachments are data-track (U2-I2) — mocked">${DS.ICONS.paperclip} Attach</button>
        <div class="ds-sr-menu" id="sr-menu" hidden></div>
      </div>
      <div class="ds-cp-box"><textarea id="cp-body" placeholder="${note ? "Add an internal note — visible only to your team. @ to mention teammates." : c ? `Reply to ${esc(c.customer)}…` : "Reply…"}">${esc(IB.draft)}</textarea></div>
      <div class="ds-cp-foot">
        <span class="ds-cp-to">${note ? "Note — visible only to your team" : c ? `Reply to ${esc(c.customer)} &lt;${esc(c.email)}&gt;` : ""}</span>
        <button class="ds-send-btn${note ? " note" : ""}" id="cp-send" type="button">${note ? DS.ICONS.lock + " Add note" : DS.ICONS.send + " Send reply"}</button>
      </div>
    </div>`;
  };

  const copHTML = () => {
    const c = cur();
    const head = `<div class="ds-cop-head"><div><div class="ds-cop-title">${DS.ICONS.sparkles} Copilot</div><div class="ds-cop-sub">typed cards · local engine</div></div><button class="ds-icon-btn" id="cop-close" type="button" aria-label="Close copilot">${DS.ICONS.x}</button></div>`;
    if (!c || !c.copilot) return head + `<div class="ds-cop-body">${DS.card({ body: `<div class="ds-cop-bullet">No copilot analysis for this fixture conversation.</div>` })}</div>`;
    const cp = c.copilot;
    return head + `<div class="ds-cop-body">
      ${DS.card({ body: `<div class="ds-cop-card-title">Signals</div><div class="ds-chip-row">${DS.badge("ai", esc(cp.intent))}${DS.badge(cp.urgency === "high" ? "danger" : cp.urgency === "medium" ? "warning" : "neutral", "urgency: " + esc(cp.urgency))}${DS.badge(cp.sentiment === "negative" ? "warning" : cp.sentiment === "positive" ? "success" : "neutral", "sentiment: " + esc(cp.sentiment))}${cp.escalation ? DS.badge("danger", "escalation risk") : ""}</div>` })}
      ${DS.card({ body: `<div class="ds-cop-card-title">Summary</div>${cp.summary.map((s) => `<div class="ds-cop-bullet">${esc(s)}</div>`).join("")}${cp.questions.map((q) => `<div class="ds-cop-q">? ${esc(q)}</div>`).join("")}` })}
      ${DS.card({ body: `<div class="ds-cop-card-title">Similar past issues</div>${cp.similar.map((s, i) => `<div class="ds-cop-sim"><div class="ds-cop-sim-head">#${s.number} ${esc(s.subject)} ${DS.badge("ai", s.score + "%")}</div><div class="ds-cop-sim-res">${esc(s.resolution)}</div><button class="ds-btn outline sm" data-ds-act="insert-res" data-i="${i}" type="button">${DS.ICONS.pencil} Insert resolution</button></div>`).join("")}` })}
      ${DS.card({ body: `<div class="ds-cop-card-title">${DS.ICONS.sparkles} Suggested reply</div><div class="ds-cop-suggest-body">${esc(cp.suggestedReply)}</div><div class="ds-msg-actions"><button class="ds-btn ai" id="cop-insert" type="button">${DS.ICONS.pencil} Insert into reply</button><button class="ds-btn outline" id="cop-copy" type="button">${DS.ICONS.copy} Copy</button></div>` })}
      ${DS.card({ body: `<div class="ds-cop-card-title">Customer history</div><div class="ds-cop-q">${esc(cp.history)}</div>` })}
    </div>`;
  };

  let ibRoot = null;
  const q = (sel) => ibRoot && ibRoot.querySelector(sel);

  function renderInbox(root) {
    ibRoot = root;
    root.innerHTML = `<div class="ds-inbox-shell">
      <aside class="ds-views" aria-label="Inbox views">
        <div class="ds-views-title">Inbox views</div>
        <div id="views"></div>
        <div class="ds-views-foot">
          <button class="notes-toggle" id="notes-toggle" type="button" aria-pressed="false">
            <span id="notes-ic"></span> Design notes <span class="count" id="notes-count">0</span>
          </button>
          <div class="kbd-hint"><kbd>j</kbd> / <kbd>k</kbd> navigate · <kbd>Enter</kbd> open · <kbd>/</kbd> filter · <kbd>Ctrl</kbd>+<kbd>Enter</kbd> send</div>
        </div>
      </aside>
      <section class="ds-conv-col" aria-label="Conversations">
        <div class="ds-conv-head">
          <div class="ds-conv-title" id="conv-title">All</div>
          <div class="ds-conv-sub" id="conv-sub"></div>
          <input class="ds-conv-filter" id="conv-filter" placeholder="Filter conversations…  ( / )" aria-label="Filter conversations">
        </div>
        <div class="ds-conv-list" id="conv-list" role="list"></div>
      </section>
      <section class="ds-reader-col" aria-label="Conversation reader">
        <div class="ds-reader-head" id="reader-head"></div>
        <div class="ds-reader-scroll" id="reader-scroll"><div class="ds-reader-inner" id="reader"></div></div>
        <div class="ds-composer" id="composer"></div>
      </section>
      <aside class="ds-cop-panel" id="copilot" aria-label="Copilot"></aside>
    </div>`;
    const ic = document.getElementById("notes-ic");
    if (ic) ic.innerHTML = DS.ICONS.pencil;
    const nc = document.getElementById("notes-count");
    if (nc) nc.textContent = window.LAB.NOTES.length;
    renderViews();
    renderList();
    renderHead();
    renderReader();
    bindInbox();
  }
  DS.inboxRender = renderInbox;

  function renderViews() { const el = q("#views"); if (el) el.innerHTML = viewsHTML(); }
  function renderList() {
    const rows = visibleConversations();
    const unread = rows.filter((c) => c.unread).length;
    const unassigned = rows.filter((c) => !c.assignee).length;
    const t = q("#conv-title"), s = q("#conv-sub"), l = q("#conv-list");
    if (t) t.textContent = IB.view;
    if (s) s.textContent = `${rows.length} conversation${rows.length === 1 ? "" : "s"} · ${unassigned} unassigned · ${unread} unread`;
    if (l) l.innerHTML = rows.map(convRowHTML).join("");
  }
  function renderHead() { const el = q("#reader-head"); if (el) el.innerHTML = headHTML(); }
  function renderReader() {
    const el = q("#reader");
    if (el) el.innerHTML = readerHTML();
    renderComposer();
    renderCopilot();
  }
  function renderComposer() {
    const box = q("#composer");
    if (!box) return;
    box.classList.toggle("note-mode", IB.composerMode === "note");
    box.innerHTML = composerHTML();
    const body = q("#cp-body");
    if (body) {
      body.addEventListener("input", () => {
        IB.draft = body.value;
        const f = q("#draft-flag");
        if (f) f.classList.toggle("show", body.value.length > 0);
      });
      body.addEventListener("keydown", (e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); const s = q("#cp-send"); if (s) s.click(); } });
    }
    box.querySelectorAll("[data-ds-act='cp-tab']").forEach((b) => b.addEventListener("click", () => {
      IB.composerMode = b.dataset.i;
      renderComposer();
      const nb = q("#cp-body");
      if (nb) nb.focus();
    }));
    const srBtn = q("#sr-btn"), srMenu = q("#sr-menu");
    if (srBtn && srMenu) {
      srBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        if (srMenu.hidden) {
          srMenu.innerHTML = window.LAB.SAVED_REPLIES.map((r, i) =>
            `<button class="ds-sr-item" data-sr="${i}" type="button"><div class="ds-sr-title">${esc(r.title)}</div><div class="ds-sr-body">${esc(r.body)}</div></button>`).join("");
          srMenu.hidden = false;
        } else srMenu.hidden = true;
      });
      srMenu.addEventListener("click", (e) => {
        const b = e.target.closest("[data-sr]");
        if (!b) return;
        IB.draft = window.LAB.SAVED_REPLIES[Number(b.dataset.sr)].body;
        IB.composerMode = "reply";
        renderComposer();
        const nb = q("#cp-body");
        if (nb) nb.focus();
        DS.toast("Saved reply inserted — edit before sending.");
      });
    }
    const aiBtn = q("#ai-btn");
    if (aiBtn && !aiBtn.disabled) aiBtn.addEventListener("click", () => {
      const c = cur();
      if (!c || !c.copilot) return;
      insertDraft(c.copilot.suggestedReply);
      DS.toast("AI draft inserted — edit before sending.");
    });
    const send = q("#cp-send");
    if (send) send.addEventListener("click", () => {
      const c = cur();
      if (!c || !IB.draft.trim()) { DS.toast("Nothing to send — the composer is empty."); return; }
      const kind = IB.composerMode;
      const last = c.messages[c.messages.length - 1];
      c.messages.push({
        kind: kind === "note" ? "note" : "agent",
        author: window.LAB.CURRENT_USER.name,
        ts: "2026-10-27 11:20",
        rel: "Just now",
        day: last && last.day === "Today" ? "Today" : "Today",
        body: [IB.draft.trim()],
        quoted: [],
        attachments: [],
      });
      IB.draft = "";
      renderReader();
      const sc = q("#reader-scroll");
      if (sc) sc.scrollTop = sc.scrollHeight;
      DS.toast(kind === "note" ? "Internal note added to the thread." : "Reply queued as a durable outbound job — delivered to Help Scout.");
    });
  }
  function renderCopilot() {
    const panel = q("#copilot");
    if (!panel) return;
    panel.hidden = !IB.copilotOpen;
    if (!IB.copilotOpen) return;
    panel.innerHTML = copHTML();
    const close = q("#cop-close");
    if (close) close.addEventListener("click", () => { IB.copilotOpen = false; renderHead(); renderCopilot(); });
    const ins = q("#cop-insert");
    if (ins) ins.addEventListener("click", () => {
      const c = cur();
      if (!c || !c.copilot) return;
      insertDraft(c.copilot.suggestedReply);
      DS.toast("AI draft inserted — edit before sending.");
    });
    const cpy = q("#cop-copy");
    if (cpy) cpy.addEventListener("click", () => {
      const c = cur();
      if (c && c.copilot && navigator.clipboard) navigator.clipboard.writeText(c.copilot.suggestedReply).catch(() => {});
      DS.toast("AI draft copied to clipboard.");
    });
    panel.querySelectorAll("[data-ds-act='insert-res']").forEach((b) => b.addEventListener("click", () => {
      const c = cur();
      if (!c || !c.copilot) return;
      const s = c.copilot.similar[Number(b.dataset.i)];
      IB.draft = (IB.draft ? IB.draft + "\n\n" : "") + s.resolution;
      renderComposer();
      const nb = q("#cp-body");
      if (nb) nb.focus();
      DS.toast("Resolution from #" + s.number + " inserted.");
    }));
  }
  function insertDraft(text) {
    IB.draft = text;
    IB.composerMode = "reply";
    renderComposer();
    const nb = q("#cp-body");
    if (nb) nb.focus();
  }
  function openConversation(id) {
    IB.selected = id;
    renderList();
    renderHead();
    renderReader();
    const sc = q("#reader-scroll");
    if (sc) sc.scrollTop = 0;
  }
  DS.inboxOpen = openConversation;

  /* delegated inbox acts — the engine dispatcher calls this first; returning
     false means the event was consumed here */
  function inboxClick(e) {
    const el = e.target.closest("[data-ds-act]");
    if (!el) return;
    const act = el.dataset.dsAct;
    if (act === "set-view") {
      IB.view = el.dataset.view;
      renderViews();
      renderList();
      const ids = visibleConversations().map((c) => c.id);
      if (!ids.includes(IB.selected)) {
        IB.selected = ids[0] != null ? ids[0] : null;
        renderHead();
        renderReader();
      } else { renderHead(); }
      return false;
    }
    if (act === "open-conv") { openConversation(Number(el.dataset.id)); return false; }
    if (act === "insert-ai") {
      const c = cur();
      const m = c && c.messages[Number(el.dataset.i)];
      if (m) { insertDraft(m.body.join("\n\n")); DS.toast("AI draft inserted — edit before sending."); }
      return false;
    }
    if (act === "copy-ai") {
      const c = cur();
      const m = c && c.messages[Number(el.dataset.i)];
      if (m && navigator.clipboard) navigator.clipboard.writeText(m.body.join("\n\n")).catch(() => {});
      DS.toast("AI draft copied to clipboard.");
      return false;
    }
    if (act === "cp-tab" || act === "panel-close") return; /* bound directly */
  }

  function bindInbox() {
    /* delegated acts live in inboxClick (hoisted, called by the engine dispatcher) */
    const filter = q("#conv-filter");
    if (filter) filter.addEventListener("input", () => { IB.filter = filter.value; renderList(); });
    const list = q("#conv-list");
    if (list) list.addEventListener("keydown", (e) => {
      if ((e.key === "Enter" || e.key === " ") && e.target.classList.contains("ds-conv-row")) {
        e.preventDefault();
        openConversation(Number(e.target.dataset.id));
      }
    });
    const head = q("#reader-head");
    if (head) head.addEventListener("click", (e) => {
      if (e.target.closest("#copilot-toggle")) {
        IB.copilotOpen = !IB.copilotOpen;
        renderHead();
        renderCopilot();
      }
    });
  }
})();
