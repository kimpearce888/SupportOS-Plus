/* ==========================================================================
   SupportOS+ Design Lab — "AFTER" reskin behaviour (proposal)
   Presentation-only reference implementation of reskin phases R2/R3:
   role-typed message cards, day dividers, collapsed quoted history,
   attachment chips, Reply|Note composer with saved replies + AI draft
   insert, typed Copilot cards, unread/assignee triage, keyboard-first j/k.
   Everything is mock data; nothing here is production code.
   ========================================================================== */
(() => {
  "use strict";
  const LAB = window.LAB;
  const $ = (sel) => document.querySelector(sel);

  const state = {
    theme: "light",
    view: "All",
    filter: "",
    selected: 501,
    composerMode: "reply",
    draft: "",
    notesOn: false,
    copilotOpen: true,
  };

  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  /* close the saved-replies menu on outside clicks (registered once) */
  document.addEventListener("click", (e) => {
    const menu = document.getElementById("sr-menu");
    if (menu && !menu.hidden && !e.target.closest("#sr-menu") && !e.target.closest("#sr-btn")) menu.hidden = true;
  });
  let toastTimer = null;
  function toast(msg) {
    const el = $("#toast");
    el.textContent = msg;
    el.classList.add("show");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => el.classList.remove("show"), 2400);
  }

  /* ----- icons ------------------------------------------------------------- */
  $("#sb-search-ic").innerHTML = LAB.ICONS.search;
  document.querySelector(".ic-search").innerHTML = LAB.ICONS.search;
  document.querySelector(".ic-bell").innerHTML = LAB.ICONS.bell;
  $("#theme-ic").innerHTML = LAB.ICONS.moon;
  $("#notes-ic").innerHTML = LAB.ICONS.pencil;
  $("#notes-x").innerHTML = LAB.ICONS.x;
  $("#notes-count").textContent = LAB.NOTES.length;

  /* ----- sidebar nav --------------------------------------------------------- */
  $("#sb-nav").innerHTML = LAB.NAV_GROUPS.map(([group, keys]) => {
    const items = keys
      .map((k) => {
        const active = k === "inbox" ? " active" : "";
        const soon = k === "inbox" ? "" : ' data-soon="1"';
        return `<button class="nav-item${active}"${soon} type="button" title="${k === "inbox" ? "" : "not part of this replica — Inbox only"}">${LAB.ICONS[k]}<span>${LAB.NAV_LABELS[k]}</span></button>`;
      })
      .join("");
    return `<div class="nav-group-label">${group}</div>${items}`;
  }).join("");
  $("#sb-nav").addEventListener("click", (e) => {
    const b = e.target.closest(".nav-item");
    if (b && b.dataset.soon) toast("This replica focuses on the Inbox screen.");
  });

  /* ----- theme ----------------------------------------------------------------- */
  $("#theme-toggle").addEventListener("click", () => {
    state.theme = state.theme === "light" ? "dark" : "light";
    document.body.dataset.theme = state.theme;
    $("#theme-ic").innerHTML = state.theme === "light" ? LAB.ICONS.moon : LAB.ICONS.sun;
    $("#theme-label").textContent = state.theme === "light" ? "switch to dark theme" : "switch to light theme";
  });

  /* ----- views rail --------------------------------------------------------------- */
  function viewCount(v) {
    const cs = LAB.conversations;
    if (v === "All") return cs.length;
    if (v === "Mine") return cs.filter((c) => c.assignee === LAB.CURRENT_USER.name.split(" ")[0]).length;
    if (v === "Unassigned") return cs.filter((c) => !c.assignee).length;
    return cs.filter((c) => c.status === v.toLowerCase()).length;
  }
  function renderViews() {
    $("#views").innerHTML = LAB.VIEWS.map((v) => {
      const active = state.view === v ? " active" : "";
      return `<button class="view-item${active}" data-view="${v}" type="button">${v}<span class="view-count">${viewCount(v)}</span></button>`;
    }).join("");
  }
  $("#views").addEventListener("click", (e) => {
    const b = e.target.closest("[data-view]");
    if (!b) return;
    state.view = b.dataset.view;
    renderViews();
    renderList();
    const ids = visibleConversations().map((c) => c.id);
    if (!ids.includes(state.selected)) {
      state.selected = ids[0] ?? null;
      renderReader();
    }
    renderHead();
  });

  /* ----- conversation list ------------------------------------------------------------ */
  function visibleConversations() {
    const me = LAB.CURRENT_USER.name.split(" ")[0];
    return LAB.conversations.filter((c) => {
      if (state.view === "Mine" && c.assignee !== me) return false;
      if (state.view === "Unassigned" && c.assignee) return false;
      if (["Active", "Pending", "Closed"].includes(state.view) && c.status !== state.view.toLowerCase()) return false;
      if (state.filter && !(c.subject + " " + c.customer).toLowerCase().includes(state.filter.toLowerCase())) return false;
      return true;
    });
  }
  function statusBadge(status) {
    const map = { active: ["info", "Active"], pending: ["warning", "Pending"], closed: ["neutral", "Closed"], spam: ["danger", "Spam"] };
    const [tone, label] = map[status] || ["neutral", status];
    return `<span class="badge ${tone}">${label}</span>`;
  }
  function renderList() {
    const rows = visibleConversations();
    const unread = rows.filter((c) => c.unread).length;
    const unassigned = rows.filter((c) => !c.assignee).length;
    $("#conv-title").textContent = state.view;
    $("#conv-sub").textContent = `${rows.length} conversation${rows.length === 1 ? "" : "s"} · ${unassigned} unassigned · ${unread} unread`;
    $("#conv-list").innerHTML = rows
      .map((c) => {
        const sel = state.selected === c.id ? " selected" : "";
        const unreadCls = c.unread ? " unread" : "";
        const who = c.assignee
          ? `<span class="assignee"><span class="av">${initials(c.assignee)}</span>${esc(c.assignee)}</span>`
          : `<span class="assignee dashed">Unassigned</span>`;
        return `<div class="crow${sel}${unreadCls}" data-id="${c.id}" role="listitem" tabindex="0" aria-label="${esc(c.subject)}">
          <div class="cr1"><span class="udot"></span><span class="cname">${esc(c.customer)}</span><span class="ctime">${esc(c.rel)}</span></div>
          <div class="cr2">${esc(c.preview)}</div>
          <div class="cr3">${who}${statusBadge(c.status)}</div>
        </div>`;
      })
      .join("");
  }
  const initials = (name) => name.split(/\s+/).map((w) => w[0]).join("").slice(0, 2).toUpperCase();
  $("#conv-list").addEventListener("click", (e) => {
    const row = e.target.closest(".crow");
    if (row) openConversation(Number(row.dataset.id));
  });
  $("#conv-list").addEventListener("keydown", (e) => {
    if ((e.key === "Enter" || e.key === " ") && e.target.classList.contains("crow")) {
      e.preventDefault();
      openConversation(Number(e.target.dataset.id));
    }
  });
  $("#conv-filter").addEventListener("input", () => {
    state.filter = $("#conv-filter").value;
    renderList();
  });

  function openConversation(id) {
    state.selected = id;
    renderList();
    renderHead();
    renderReader();
    $("#reader-scroll").scrollTop = 0;
  }

  /* ----- reader head -------------------------------------------------------------------- */
  function renderHead() {
    const c = LAB.conversations.find((x) => x.id === state.selected);
    if (!c) { $("#reader-head").innerHTML = ""; return; }
    const who = c.assignee
      ? `<button class="pill" type="button" title="assignment is data-track (U2-I1) — mocked here"><span class="assignee" style="color:var(--fg)"><span class="av">${initials(c.assignee)}</span>${esc(c.assignee)}</span></button>`
      : `<button class="pill" type="button" title="assignment is data-track (U2-I1) — mocked here"><span class="assignee dashed" style="font-style:italic">Assign</span></button>`;
    $("#reader-head").innerHTML = `
      <span class="av">${initials(c.customer)}</span>
      <div style="min-width:0">
        <div class="rh-name">${esc(c.customer)}</div>
        <div class="rh-mail">${esc(c.email)} · mailbox: ${esc(c.mailbox)}</div>
      </div>
      <div class="rh-actions">
        ${who}
        ${statusBadge(c.status)}
        <button class="pill${state.copilotOpen ? " copilot-on" : ""}" id="copilot-toggle" type="button" aria-pressed="${state.copilotOpen}">${LAB.ICONS.sparkles} Copilot</button>
      </div>`;
    $("#copilot-toggle").addEventListener("click", () => {
      state.copilotOpen = !state.copilotOpen;
      renderHead();
      renderCopilot();
    });
  }

  /* ----- reader: role-typed message cards -------------------------------------------------- */
  function messageCard(m, idx) {
    const c = LAB.conversations.find((x) => x.id === state.selected);
    const role =
      m.kind === "customer" ? `Customer · email` :
      m.kind === "agent" ? `Agent` :
      m.kind === "note" ? `Internal note ${LAB.ICONS.lock}` :
      `AI · draft`;
    const body = m.body.map((p) => `<p>${esc(p)}</p>`).join("");
    const attachments = (m.attachments || []).length
      ? `<div class="attachments">${m.attachments.map((a) => `<span class="file-chip">${LAB.ICONS.paperclip}${esc(a.name)}<span class="size">${esc(a.size)}</span></span>`).join("")}</div>`
      : "";
    const quoted = (m.quoted || []).length
      ? `<details class="quoted">
          <summary>${LAB.ICONS.arrowLeft.replace('class', 'class')} Show ${m.quoted.length} earlier message${m.quoted.length === 1 ? "" : "s"} in this email thread</summary>
          ${m.quoted.map((q) => `<div class="q-item"><div class="q-from">${esc(q.from)} — ${esc(q.date)} UTC</div><div class="q-body">${esc(q.body)}</div></div>`).join("")}
        </details>`
      : "";
    const aiActions = m.kind === "ai"
      ? `<div class="msg-actions">
          <button class="btn-ai" data-insert-ai="${idx}" type="button">${LAB.ICONS.pencil} Insert into reply</button>
          <button class="btn-ghost" data-copy-ai="${idx}" type="button">${LAB.ICONS.copy} Copy</button>
        </div>`
      : "";
    return `
      <article class="msg ${m.kind}" data-kind="${m.kind}">
        <div class="msg-head">
          <span class="av">${m.kind === "ai" ? LAB.ICONS.sparkles : initials(m.author)}</span>
          <span class="msg-name">${esc(m.author)}</span>
          <span class="msg-role">${role}</span>
          <span class="msg-time">${esc(m.rel)}</span>
        </div>
        <div class="msg-body">${body}</div>
        ${attachments}${quoted}${aiActions}
      </article>`;
  }

  function renderReader() {
    const c = LAB.conversations.find((x) => x.id === state.selected);
    if (!c) {
      $("#reader").innerHTML = "";
      renderComposer();
      renderCopilot();
      return;
    }
    let lastDay = null;
    let aiPinned = false;
    let divPinned = false;
    let attPinned = false;
    let quotedPinned = false;
    const html = c.messages
      .map((m, idx) => {
        let out = "";
        if (m.day !== lastDay) {
          const pin = !divPinned ? ` annotated" data-pin="8` : ``;
          divPinned = true;
          out += `<div class="day-divider${pin}" data-note="8"><span>${esc(m.day)}</span></div>`;
          lastDay = m.day;
        }
        out += messageCard(m, idx);
        return out;
      })
      .join("");
    $("#reader").innerHTML = html;

    // pins on specific features (appear when Design notes is on)
    $("#reader").classList.add("annotated");
    $("#reader").dataset.pin = "1";
    $("#reader").dataset.note = "1";
    const scroll = $("#reader-scroll");
    scroll.classList.add("annotated");
    scroll.dataset.pin = "3";
    scroll.dataset.note = "3";
    const aiCard = $("#reader .msg.ai");
    if (aiCard) { aiCard.classList.add("annotated"); aiCard.dataset.pin = "2"; aiCard.dataset.note = "2"; }
    const att = $("#reader .attachments");
    if (att) { att.classList.add("annotated"); att.dataset.pin = "5"; att.dataset.note = "5"; }
    const quoted = $("#reader .quoted");
    if (quoted) { quoted.classList.add("annotated"); quoted.dataset.pin = "4"; quoted.dataset.note = "4"; }

    // AI actions
    $("#reader").addEventListener("click", readerActions);
    renderComposer();
    renderCopilot();
  }

  function readerActions(e) {
    const ins = e.target.closest("[data-insert-ai]");
    if (ins) {
      const c = LAB.conversations.find((x) => x.id === state.selected);
      const m = c.messages[Number(ins.dataset.insertAi)];
      state.draft = m.body.join("\n\n");
      state.composerMode = "reply";
      renderComposer(true);
      $("#composer textarea").focus();
      toast("AI draft inserted — edit before sending.");
      return;
    }
    const copy = e.target.closest("[data-copy-ai]");
    if (copy) {
      const c = LAB.conversations.find((x) => x.id === state.selected);
      const m = c.messages[Number(copy.dataset.copyAi)];
      if (navigator.clipboard) navigator.clipboard.writeText(m.body.join("\n\n")).catch(() => {});
      toast("AI draft copied to clipboard.");
    }
  }

  /* ----- composer ---------------------------------------------------------------------- */
  function renderComposer(focusAI) {
    const c = LAB.conversations.find((x) => x.id === state.selected);
    const box = $("#composer");
    const note = state.composerMode === "note";
    box.classList.toggle("note-mode", note);
    box.classList.add("annotated");
    box.dataset.pin = "6";
    box.dataset.note = "6";
    const head = $("#reader-head");
    if (head) { head.classList.add("annotated"); head.dataset.pin = "10"; head.dataset.note = "10"; }
    const filterEl = $("#conv-filter");
    filterEl.classList.add("annotated");
    filterEl.dataset.pin = "9";
    filterEl.dataset.note = "9";

    box.innerHTML = `
      <div class="composer-inner">
        <div class="cp-tabs" role="tablist">
          <button class="cp-tab${note ? "" : " active"}" data-tab="reply" role="tab" aria-selected="${!note}" type="button">${LAB.ICONS.send} Reply</button>
          <button class="cp-tab${note ? " active" : ""}" data-tab="note" role="tab" aria-selected="${note}" type="button">${LAB.ICONS.pencil} Internal note</button>
          <span class="draft-flag${state.draft ? " show" : ""}" id="draft-flag">Draft saved</span>
        </div>
        <div class="cp-tools">
          <button class="tool-chip" id="sr-btn" type="button">${LAB.ICONS.chevronDown} Saved replies</button>
          <button class="tool-chip ai" id="ai-btn" type="button" ${c && c.copilot ? "" : "disabled"}>${LAB.ICONS.sparkles} Insert AI draft</button>
          <button class="tool-chip" type="button" title="attachments are data-track (U2-I2) — mocked">${LAB.ICONS.paperclip} Attach</button>
          <div class="sr-menu" id="sr-menu" hidden></div>
        </div>
        <div class="cp-box">
          <textarea id="cp-body" placeholder="${note
            ? "Add an internal note — visible only to your team. @ to mention teammates."
            : c ? `Reply to ${esc(c.customer)}…` : "Reply…"}">${esc(state.draft)}</textarea>
        </div>
        <div class="cp-foot">
          <span class="cp-to">${note
            ? "Note — visible only to your team"
            : c ? `Reply to ${esc(c.customer)} &lt;${esc(c.email)}&gt;` : ""}</span>
          <button class="send-btn${note ? " note" : ""}" id="cp-send" type="button">${note ? LAB.ICONS.lock + " Add note" : LAB.ICONS.send + " Send reply"}</button>
        </div>
      </div>`;

    const body = $("#cp-body");
    body.addEventListener("input", () => {
      state.draft = body.value;
      $("#draft-flag").classList.toggle("show", body.value.length > 0);
    });
    body.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) $("#cp-send").click();
    });
    box.querySelectorAll("[data-tab]").forEach((b) =>
      b.addEventListener("click", () => {
        state.composerMode = b.dataset.tab;
        renderComposer();
        $("#cp-body").focus();
      })
    );

    // Saved replies dropdown (R3: canned responses reachable from the composer)
    const srBtn = $("#sr-btn");
    const srMenu = $("#sr-menu");
    srBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      if (srMenu.hidden) {
        srMenu.innerHTML = LAB.SAVED_REPLIES.map(
          (r, i) => `<button class="sr-item" data-sr="${i}" type="button"><div class="sr-title">${esc(r.title)}</div><div class="sr-body">${esc(r.body)}</div></button>`
        ).join("");
        srMenu.hidden = false;
      } else srMenu.hidden = true;
    });
    srMenu.addEventListener("click", (e) => {
      const b = e.target.closest("[data-sr]");
      if (!b) return;
      state.draft = LAB.SAVED_REPLIES[Number(b.dataset.sr)].body;
      state.composerMode = "reply";
      renderComposer();
      $("#cp-body").focus();
      toast("Saved reply inserted — edit before sending.");
    });

    const aiBtn = $("#ai-btn");
    if (aiBtn && !aiBtn.disabled) {
      aiBtn.addEventListener("click", () => {
        const c2 = LAB.conversations.find((x) => x.id === state.selected);
        state.draft = c2.copilot.suggestedReply;
        state.composerMode = "reply";
        renderComposer();
        $("#cp-body").focus();
        toast("AI draft inserted — edit before sending.");
      });
    }

    $("#cp-send").addEventListener("click", () => {
      const c2 = LAB.conversations.find((x) => x.id === state.selected);
      if (!c2 || !state.draft.trim()) { toast("Nothing to send — the composer is empty."); return; }
      const kind = state.composerMode;
      const last = c2.messages[c2.messages.length - 1];
      c2.messages.push({
        kind: kind === "note" ? "note" : "agent",
        author: LAB.CURRENT_USER.name,
        ts: "2026-10-27 11:20",
        rel: "Just now",
        day: last.day === "Today" ? "Today" : "Today",
        body: [state.draft.trim()],
        quoted: [],
        attachments: [],
      });
      state.draft = "";
      renderReader();
      const el = $("#reader-scroll");
      el.scrollTop = el.scrollHeight;
      toast(kind === "note" ? "Internal note added to the thread." : "Reply queued as a durable outbound job — delivered to Help Scout.");
    });
    if (focusAI) $("#cp-body").focus();
  }

  /* ----- copilot: typed cards -------------------------------------------------------------- */
  function renderCopilot() {
    const panel = $("#copilot");
    panel.classList.toggle("annotated", true);
    panel.dataset.pin = "7";
    panel.dataset.note = "7";
    panel.hidden = !state.copilotOpen;
    if (!state.copilotOpen) return;
    const c = LAB.conversations.find((x) => x.id === state.selected);
    const head = `
      <div class="cop-head">
        <div>
          <div class="cop-title">${LAB.ICONS.sparkles} Copilot</div>
          <div class="cop-sub">typed cards · local engine</div>
        </div>
        <button class="icon-btn" id="cop-close" type="button" aria-label="Close copilot">${LAB.ICONS.x}</button>
      </div>`;
    const body = !c || !c.copilot
      ? `<div class="cop-body"><div class="cp-card"><div class="cp-bullet" style="padding-left:0">No copilot analysis for this fixture conversation.</div></div></div>`
      : (() => {
          const cp = c.copilot;
          return `
      <div class="cop-body">
        <div class="cp-card">
          <div class="cp-card-title">Signals</div>
          <div class="chip-row">
            <span class="badge ai">${esc(cp.intent)}</span>
            <span class="badge ${cp.urgency === "high" ? "danger" : cp.urgency === "medium" ? "warning" : "neutral"}">urgency: ${esc(cp.urgency)}</span>
            <span class="badge ${cp.sentiment === "negative" ? "warning" : cp.sentiment === "positive" ? "success" : "neutral"}">sentiment: ${esc(cp.sentiment)}</span>
            ${cp.escalation ? `<span class="badge danger">escalation risk</span>` : ""}
          </div>
        </div>
        <div class="cp-card">
          <div class="cp-card-title">Summary</div>
          ${cp.summary.map((s) => `<div class="cp-bullet">${esc(s)}</div>`).join("")}
          ${cp.questions.map((q) => `<div class="cp-q">? ${esc(q)}</div>`).join("")}
        </div>
        <div class="cp-card">
          <div class="cp-card-title">Similar past issues</div>
          ${cp.similar.map((s, i) => `
            <div class="cp-sim">
              <div class="cp-sim-head">#${s.number} ${esc(s.subject)} <span class="badge ai">${s.score}%</span></div>
              <div class="cp-sim-res">${esc(s.resolution)}</div>
              <button class="btn-ghost" data-insert-res="${i}" type="button">${LAB.ICONS.pencil} Insert resolution</button>
            </div>`).join("")}
        </div>
        <div class="cp-card cp-suggest">
          <div class="cp-card-title">${LAB.ICONS.sparkles} Suggested reply</div>
          <div class="cp-suggest-body">${esc(cp.suggestedReply)}</div>
          <div class="msg-actions">
            <button class="btn-ai" id="cop-insert" type="button">${LAB.ICONS.pencil} Insert into reply</button>
            <button class="btn-ghost" id="cop-copy" type="button">${LAB.ICONS.copy} Copy</button>
          </div>
        </div>
        <div class="cp-card">
          <div class="cp-card-title">Customer history</div>
          <div class="cp-q">${esc(cp.history)}</div>
        </div>
      </div>`;
        })();
    panel.innerHTML = head + body;
    if (!c || !c.copilot) return;
    const cp = c.copilot;
    $("#cop-close").addEventListener("click", () => {
      state.copilotOpen = false;
      renderHead();
      renderCopilot();
    });
    $("#cop-insert").addEventListener("click", () => {
      state.draft = cp.suggestedReply;
      state.composerMode = "reply";
      renderComposer();
      $("#cp-body").focus();
      toast("AI draft inserted — edit before sending.");
    });
    $("#cop-copy").addEventListener("click", () => {
      if (navigator.clipboard) navigator.clipboard.writeText(cp.suggestedReply).catch(() => {});
      toast("AI draft copied to clipboard.");
    });
    panel.querySelectorAll("[data-insert-res]").forEach((b) =>
      b.addEventListener("click", () => {
        const s = cp.similar[Number(b.dataset.insertRes)];
        state.draft = (state.draft ? state.draft + "\n\n" : "") + s.resolution;
        renderComposer();
        $("#cp-body").focus();
        toast("Resolution from #" + s.number + " inserted.");
      })
    );
  }

  /* ----- design notes drawer ------------------------------------------------------------------ */
  function renderNotes() {
    $("#nd-list").innerHTML = LAB.NOTES.map(
      (n) => `<div class="nd-item" id="note-${n.n}">
        <span class="nd-pin">${n.n}</span>
        <div>
          <div class="nd-title2">${esc(n.title)}</div>
          <div class="nd-fix">${esc(n.fix)}</div>
          <div class="nd-was"><b>WAS</b> — ${esc(n.was)}</div>
          <span class="nd-receipt">${esc(n.receipt)}</span>
        </div>
      </div>`
    ).join("");
  }
  $("#notes-toggle").addEventListener("click", () => {
    state.notesOn = !state.notesOn;
    document.body.classList.toggle("notes-on", state.notesOn);
    $("#notes-toggle").setAttribute("aria-pressed", String(state.notesOn));
  });
  $("#notes-close").addEventListener("click", () => {
    state.notesOn = false;
    document.body.classList.remove("notes-on");
    $("#notes-toggle").setAttribute("aria-pressed", "false");
  });
  document.addEventListener("click", (e) => {
    const pinned = e.target.closest("[data-pin]");
    if (pinned && state.notesOn) {
      const n = Number(pinned.dataset.pin);
      const item = document.getElementById("note-" + n);
      if (item) {
        item.scrollIntoView({ behavior: "smooth", block: "center" });
        item.style.background = "var(--ai-bg)";
        setTimeout(() => (item.style.background = ""), 900);
      }
    }
  });

  /* ----- keyboard-first: j/k navigate, Enter open, / filter ---------------------------- */
  document.addEventListener("keydown", (e) => {
    const tag = (e.target.tagName || "").toLowerCase();
    const typing = tag === "textarea" || tag === "input";
    if (typing) {
      if (e.key === "Escape") e.target.blur();
      return;
    }
    if (e.key === "/") {
      e.preventDefault();
      $("#conv-filter").focus();
      return;
    }
    if (e.key === "j" || e.key === "k") {
      const rows = visibleConversations();
      if (!rows.length) return;
      const idx = rows.findIndex((c) => c.id === state.selected);
      const dir = e.key === "j" ? 1 : -1;
      const next = rows[Math.min(rows.length - 1, Math.max(0, (idx === -1 ? 0 : idx) + dir))];
      openConversation(next.id);
      const el = document.querySelector(`.crow[data-id="${next.id}"]`);
      if (el) el.scrollIntoView({ block: "nearest" });
      return;
    }
    if (e.key === "Enter") {
      const rows = visibleConversations();
      if (rows.length) openConversation(rows[0].id);
    }
  });

  /* ----- boot ---------------------------------------------------------------------------- */
  renderViews();
  renderList();
  renderHead();
  renderReader();
  renderNotes();
})();
