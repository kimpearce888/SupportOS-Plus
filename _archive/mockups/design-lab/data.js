/* ==========================================================================
   SupportOS+ Design Lab — shared fixture data
   --------------------------------------------------------------------------
   Synthetic data that mirrors what the Rust program already has in its
   ThreadEntry model (crates/core/src/inbox.rs:326-341: author / type /
   timestamp / body) plus the fields the async data track (U2-I1/I2) will
   provide (read state, assignee, ordering). Pure static fixture, no backend.
   Both the "before" replica and the "after" reskin render from this file so
   the comparison is apples-to-apples.
   ========================================================================== */
window.LAB = (() => {
  "use strict";

  /* ----- lucide-style stroke icons (24x24, stroke=2) --------------------- */
  const I = (inner) =>
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' + inner + "</svg>";
  const ICONS = {
    dashboard: I('<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/>'),
    inbox: I('<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>'),
    "live-chats": I('<path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"/>'),
    operations: I('<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>'),
    customers: I('<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>'),
    organizations: I('<rect x="4" y="2" width="16" height="20" rx="2"/><path d="M9 22v-4h6v4"/><path d="M8 6h.01M12 6h.01M16 6h.01M8 10h.01M12 10h.01M16 10h.01M8 14h.01M12 14h.01M16 14h.01"/>'),
    knowledge: I('<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>'),
    reports: I('<line x1="12" y1="20" x2="12" y2="10"/><line x1="18" y1="20" x2="18" y2="4"/><line x1="6" y1="20" x2="6" y2="16"/>'),
    "issue-radar": I('<circle cx="12" cy="12" r="10"/><line x1="22" y1="12" x2="18" y2="12"/><line x1="6" y1="12" x2="2" y2="12"/><line x1="12" y1="6" x2="12" y2="2"/><line x1="12" y1="22" x2="12" y2="18"/>'),
    incidents: I('<path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>'),
    "custom-objects": I('<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/>'),
    automation: I('<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>'),
    "ai-center": I('<path d="M12 3l1.9 5.8a2 2 0 0 0 1.3 1.3L21 12l-5.8 1.9a2 2 0 0 0-1.3 1.3L12 21l-1.9-5.8a2 2 0 0 0-1.3-1.3L3 12l5.8-1.9a2 2 0 0 0 1.3-1.3L12 3z"/>'),
    outreach: I('<line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>'),
    notifications: I('<path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/>'),
    search: I('<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>'),
    "support-graph": I('<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/><line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/>'),
    connectors: I('<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>'),
    diagnostics: I('<polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>'),
    "sync-health": I('<polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>'),
    settings: I('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.08a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h.08a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.08a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>'),
    onboarding: I('<path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z"/><path d="M12 15l-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z"/><path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0"/><path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5"/>'),
    shell: I('<polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>'),
    moon: I('<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>'),
    sun: I('<circle cx="12" cy="12" r="4"/><line x1="12" y1="2" x2="12" y2="4"/><line x1="12" y1="20" x2="12" y2="22"/><line x1="4.93" y1="4.93" x2="6.34" y2="6.34"/><line x1="17.66" y1="17.66" x2="19.07" y2="19.07"/><line x1="2" y1="12" x2="4" y2="12"/><line x1="20" y1="12" x2="22" y2="12"/><line x1="4.93" y1="19.07" x2="6.34" y2="17.66"/><line x1="17.66" y1="6.34" x2="19.07" y2="4.93"/>'),
    bell: I('<path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/>'),
    send: I('<line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>'),
    pencil: I('<path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z"/>'),
    sparkles: I('<path d="M12 3l1.9 5.8a2 2 0 0 0 1.3 1.3L21 12l-5.8 1.9a2 2 0 0 0-1.3 1.3L12 21l-1.9-5.8a2 2 0 0 0-1.3-1.3L3 12l5.8-1.9a2 2 0 0 0 1.3-1.3L12 3z"/>'),
    paperclip: I('<path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/>'),
    lock: I('<rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>'),
    copy: I('<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'),
    image: I('<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>'),
    x: I('<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>'),
    chevronDown: I('<polyline points="6 9 12 15 18 9"/>'),
    arrowLeft: I('<line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/>'),
  };

  /* ----- sidebar navigation (app.rs:926-971 NAV_GROUPS, verbatim) -------- */
  const NAV_GROUPS = [
    ["Workspace", ["dashboard", "inbox", "live-chats", "operations"]],
    ["Customers & knowledge", ["customers", "organizations", "knowledge"]],
    ["Insights", ["reports", "issue-radar", "incidents"]],
    ["Automation & AI", ["custom-objects", "automation", "ai-center", "outreach"]],
    ["System", ["notifications", "search", "support-graph", "connectors", "diagnostics", "sync-health", "settings", "onboarding", "shell"]],
  ];
  const NAV_LABELS = {
    dashboard: "Dashboard", inbox: "Inbox", "live-chats": "Live chats", operations: "Operations",
    customers: "Customers", organizations: "Organizations", knowledge: "Knowledge",
    reports: "Reports", "issue-radar": "Issue Radar", incidents: "Incidents",
    "custom-objects": "Custom Objects", automation: "Automation", "ai-center": "AI Center", outreach: "Outreach",
    notifications: "Notifications", search: "Search", "support-graph": "Support Graph",
    connectors: "Connectors", diagnostics: "Diagnostics", "sync-health": "Sync Health",
    settings: "Settings", onboarding: "Onboarding", shell: "Shell",
  };

  /* ----- inbox views (the reskin's views rail; counts derive from data) --- */
  const VIEWS = ["All", "Mine", "Unassigned", "Active", "Pending", "Closed"];
  const CURRENT_USER = { name: "Ada Lin", initials: "AL" };

  /* ----- saved replies (Operations page CRUD only — operations.rs:746) ---- */
  const SAVED_REPLIES = [
    { title: "CSV export status", body: "Thanks for the report — engineering is tracking the large-export limit under ENG-2293. Splitting the date range is the safe workaround for today; I will update you here the moment the fix ships." },
    { title: "Refund policy", body: "Duplicate charges are refunded automatically within 3-5 business days once the payment processor confirms the reversal. No further action is needed on your side." },
    { title: "SSO first steps", body: "Let's start with the basics: confirm the Entity ID and ACS URL match the values in Settings > SSO, then send me the IdP metadata XML and the exact time of a failed attempt (with timezone)." },
  ];

  /* ----- conversations ----------------------------------------------------
     ts format mirrors what compact_time (inbox.rs:2797) prints:
     "YYYY-MM-DD HH:MM" (UTC). rel = human relative label (reskin). */
  const conversations = [
    {
      id: 501, number: 501,
      subject: "Export to CSV fails on large files",
      customer: "Kim Pearce", email: "kim@northwind-logistics.test", initials: "KP",
      mailbox: "Support", status: "active", priority: "none",
      unread: true, assignee: null,
      updatedAt: "2026-10-27 09:14", rel: "2h ago",
      preview: "The split export worked for the board report, but honestly this is painful…",
      triage: [
        { label: "intent", value: "bug report", conf: "heuristic" },
        { label: "urgency", value: "high" },
        { label: "sentiment", value: "negative" },
        { label: "escalation signal", value: "true" },
      ],
      briefing: [
        { key: "plan", value: "Growth (annual, 42 seats)" },
        { key: "first seen", value: "2026-08-14" },
        { key: "open engineering ticket", value: "ENG-2293" },
        { key: "escalated before", value: "yes (2026-10-09)" },
      ],
      copilot: {
        intent: "bug report", urgency: "high", sentiment: "negative", escalation: true,
        summary: [
          "CSV exports over ~10,000 rows fail at a 60-second job cap introduced with the Growth-tier export queue.",
          "Customer already applied the split-export workaround but needs a durable fix for a monthly board report.",
          "Customer is frustrated (second follow-up) and on an annual contract — churn risk is real but modest.",
        ],
        questions: [
          "Is the 60-second cap configurable per workspace today?",
          "Does the Scheduled reports beta avoid the same buffering path?",
        ],
        history: "4 conversation(s) on file · last contact 2026-10-02 09:12",
        similar: [
          { number: 476, subject: "Report export timeout on Growth plan", score: 87, resolution: "Raised the per-workspace job cap to 300s and advised splitting ranges above 25k rows.", why: "same component, same plan tier" },
          { number: 488, subject: "CSV export fails at 10k rows", score: 81, resolution: "Root-caused to the buffering exporter; fix shipped in 1.7.2 for sub-50k rows.", why: "identical symptom string" },
        ],
        suggestedReply: "Hi Kim — ENG-2293 is merged to staging and ships with this week's release train. In the meantime the Scheduled reports beta streams exports instead of buffering them, so it sidesteps the 60-second cap entirely. Want me to enable it for your workspace today?",
      },
      messages: [
        {
          kind: "customer", author: "Kim Pearce", ts: "2026-10-24 08:31", rel: "Oct 24, 08:31", day: "Friday, Oct 24",
          body: [
            "Hi team,",
            "Since we upgraded to the Growth plan, every CSV export over roughly 10,000 rows fails at about the 60-second mark. Smaller exports are fine.",
            "This blocks our monthly board report — can you take a look?",
            "Thanks,\nKim",
          ],
          attachments: [{ name: "report-export-failed.png", size: "412 KB" }],
          quoted: [
            { from: "Kim Pearce", date: "2026-10-02 09:12", body: "Setting up the quarterly CSV exports — worked fine on the Starter plan." },
            { from: "Support (Ada Lin)", date: "2026-10-02 10:03", body: "Confirmed, exports are enabled on your workspace. Large exports at Growth scale were never load-tested — noted internally." },
            { from: "Kim Pearce", date: "2026-10-09 14:40", body: "Bumping this — same failure on the September export." },
          ],
        },
        {
          kind: "ai", author: "SupportOS AI", ts: "2026-10-24 08:32", rel: "Oct 24, 08:32", day: "Friday, Oct 24",
          aiSource: "Draft reply · grounded in 3 similar resolved conversations (#476, #488, #492)",
          body: [
            "Hi Kim,",
            "Thanks for the detailed report — I was able to reproduce this. Exports above ~10,000 rows are hitting a hard 60-second job limit that was introduced with the Growth-tier export queue.",
            "Two things you can do right now: (1) split the range into two exports, or (2) use the Scheduled reports beta, which streams instead of buffering.",
            "I have flagged the limit to engineering — you will hear back here as soon as it is lifted.",
          ],
        },
        {
          kind: "agent", author: "Ada Lin", ts: "2026-10-24 09:05", rel: "Oct 24, 09:05", day: "Friday, Oct 24",
          body: [
            "Hi Kim,",
            "Confirming the diagnosis — the 60-second job cap is the cause. Splitting the date range is the safe workaround for today's board report; I'll keep this conversation open until engineering lifts the cap.",
          ],
        },
        {
          kind: "note", author: "Ada Lin", ts: "2026-10-24 09:07", rel: "Oct 24, 09:07", day: "Friday, Oct 24",
          body: [
            "Kim is on an annual Growth contract (42 seats) — treat this as priority. Engineering tracks the row/time limit under ENG-2293. Promised an update by Friday.",
          ],
        },
        {
          kind: "customer", author: "Kim Pearce", ts: "2026-10-26 16:24", rel: "Oct 26, 16:24", day: "Monday, Oct 26",
          body: [
            "The split export worked for the board report, but honestly this is painful as a monthly routine — we would really rather not schedule around a bug.",
            "Is there an ETA on the fix?",
          ],
          quoted: [
            { from: "Kim Pearce", date: "2026-10-24 08:31", body: "Since we upgraded to the Growth plan, every CSV export over roughly 10,000 rows fails at about the 60-second mark…" },
          ],
        },
        {
          kind: "agent", author: "Ada Lin", ts: "2026-10-27 09:14", rel: "2h ago", day: "Tuesday, Oct 27",
          body: [
            "Hi Kim,",
            "ENG-2293 is merged to staging and ships with the next release train (expected this week). I'll flip this to Pending while we wait for the deploy, and confirm here the moment it's live.",
          ],
        },
      ],
    },
    {
      id: 502, number: 502,
      subject: "Billing after trial period",
      customer: "Ada Customer", email: "ada@brightside.test", initials: "AC",
      mailbox: "Billing", status: "pending", priority: "none",
      unread: false, assignee: "Ada",
      updatedAt: "2026-10-26 22:26", rel: "Yesterday",
      preview: "Billing after trial period preview",
      triage: [
        { label: "intent", value: "billing question", conf: "rules" },
        { label: "urgency", value: "moderate" },
        { label: "sentiment", value: "neutral" },
      ],
      briefing: [{ key: "plan", value: "Starter (monthly)" }, { key: "trial ended", value: "2026-10-20" }],
      copilot: null,
      messages: [
        { kind: "customer", author: "Ada Customer", ts: "2026-10-25 11:02", rel: "Oct 25, 11:02", day: "Saturday, Oct 25",
          body: ["Hi — we cancelled during the trial on the 18th but were still charged on the 20th. Can you check what happened?"],
          quoted: [], attachments: [] },
        { kind: "agent", author: "Ada Lin", ts: "2026-10-26 22:26", rel: "Yesterday", day: "Monday, Oct 26",
          body: ["Thanks for flagging — the cancellation landed after the 17:00 UTC cutoff on the 18th, so the cycle started. I've opened a goodwill refund; it should settle in 3-5 business days. Anything else on this account while I'm here?"],
          quoted: [], attachments: [] },
      ],
    },
    {
      id: 503, number: 503,
      subject: "Cannot sign in with SSO",
      customer: "Kim Pearce", email: "kim@northwind-logistics.test", initials: "KP",
      mailbox: "Support", status: "active", priority: "high",
      unread: true, assignee: null,
      updatedAt: "2026-10-25 22:26", rel: "Oct 25, 22:26",
      preview: "Cannot sign in with SSO preview",
      triage: [
        { label: "intent", value: "access issue", conf: "rules" },
        { label: "urgency", value: "high" },
        { label: "sentiment", value: "negative" },
      ],
      briefing: [{ key: "idp", value: "Okta" }, { key: "sso enabled", value: "2026-09-30" }],
      copilot: null,
      messages: [
        { kind: "customer", author: "Kim Pearce", ts: "2026-10-25 18:40", rel: "Oct 25, 18:40", day: "Sunday, Oct 25",
          body: ["Two of my teammates get bounced back to the login page right after the Okta round-trip. My own account works. Logs show a redirect loop on /auth/callback."],
          quoted: [], attachments: [] },
        { kind: "note", author: "Ada Lin", ts: "2026-10-25 19:02", rel: "Oct 25, 19:02", day: "Sunday, Oct 25",
          body: ["Likely the email-case mismatch bug (AUTH-118). Need Okta metadata from the customer to confirm; assigned to myself once triage lands."],
          quoted: [], attachments: [] },
      ],
    },
    {
      id: 504, number: 504,
      subject: "Feature request: dark mode",
      customer: "Ada Customer", email: "ada@brightside.test", initials: "AC",
      mailbox: "Support", status: "closed", priority: "none",
      unread: false, assignee: "Ada",
      updatedAt: "2026-10-24 22:26", rel: "Oct 24, 22:26",
      preview: "Feature request: dark mode preview",
      triage: [{ label: "intent", value: "feature request", conf: "rules" }, { label: "sentiment", value: "positive" }],
      briefing: [],
      copilot: null,
      messages: [
        { kind: "customer", author: "Ada Customer", ts: "2026-10-24 09:15", rel: "Oct 24, 09:15", day: "Saturday, Oct 24",
          body: ["Any chance of a dark theme for the dashboard? Our night-shift team lives in this app."],
          quoted: [], attachments: [] },
        { kind: "agent", author: "Ada Lin", ts: "2026-10-24 22:26", rel: "Oct 24, 22:26", day: "Saturday, Oct 24",
          body: ["Good news — dark mode shipped in v1.8.0 (toggle at the bottom of the sidebar). Closing this out; reopen if anything looks off."],
          quoted: [], attachments: [] },
      ],
    },
    {
      id: 505, number: 505,
      subject: "Refund for duplicate charge",
      customer: "Kim Pearce", email: "kim@northwind-logistics.test", initials: "KP",
      mailbox: "Billing", status: "pending", priority: "none",
      unread: false, assignee: "Kim",
      updatedAt: "2026-10-23 22:26", rel: "Oct 23, 22:26",
      preview: "Refund for duplicate charge preview",
      triage: [{ label: "intent", value: "billing question", conf: "rules" }],
      briefing: [],
      copilot: null,
      messages: [
        { kind: "customer", author: "Kim Pearce", ts: "2026-10-23 10:30", rel: "Oct 23, 10:30", day: "Friday, Oct 23",
          body: ["We were charged twice on the 21st for the same invoice. Statement attached."],
          quoted: [], attachments: [{ name: "statement-oct.pdf", size: "88 KB" }] },
        { kind: "agent", author: "Kim Reyes", ts: "2026-10-23 22:26", rel: "Oct 23, 22:26", day: "Friday, Oct 23",
          body: ["Confirmed the duplicate on our side — refund submitted, reference RB-90412. It lands in 3-5 business days."],
          quoted: [], attachments: [] },
      ],
    },
    {
      id: 506, number: 506,
      subject: "Mobile app crashes on upload",
      customer: "Ada Customer", email: "ada@brightside.test", initials: "AC",
      mailbox: "Support", status: "active", priority: "none",
      unread: false, assignee: null,
      updatedAt: "2026-10-22 18:03", rel: "Oct 22, 18:03",
      preview: "Mobile app crashes on upload preview",
      triage: [{ label: "intent", value: "bug report", conf: "rules" }, { label: "urgency", value: "moderate" }],
      briefing: [],
      copilot: null,
      messages: [
        { kind: "customer", author: "Ada Customer", ts: "2026-10-22 18:03", rel: "Oct 22, 18:03", day: "Thursday, Oct 22",
          body: ["iOS app (v1.8.0 build 214) crashes when attaching a photo over ~8 MB to a conversation. Android is fine."],
          quoted: [], attachments: [] },
      ],
    },
  ];

  /* ==========================================================================
     ALL-PAGES LAYER — Task 19
     Every string below is copied from the forensic page specs in
     .extract/*.md (each cites file:line in the Rust source). Fixtures are
     synthetic but semantically faithful; quirks are replicated on purpose.
     ========================================================================== */

  /* ----- per-page header metadata (exact title/subtitle/actions) ---------- */
  const PAGE_META = {
    dashboard: { title: "Dashboard", subtitle: "Local metrics · sources labeled Help Scout / local / AI-derived everywhere", actions: ["sync-now"] },
    inbox: { title: "Inbox" },
    "live-chats": { title: "Live chats", subtitle: "Beacon chats arrive through the five beacon.chat.* webhook events — Help Scout has no list endpoint, the webhook IS the data path.", counter: "2 open · 1 ended" },
    operations: { title: "Operations Center", subtitle: "16 tiles cover the team's current workload, SLA risk, and system health. Click a tile to filter the inbox." },
    customers: { title: "Customers", subtitle: null, actions: ["sync-now"] },
    organizations: { title: "Organizations", subtitle: "customer organizations, their domains, and their members", actions: ["sync-now"] },
    knowledge: { title: "Knowledge", subtitle: "local documents, sources, freshness, and the help center", actions: ["sync-docs"], banner: "Help Center sync needs a Docs API key (Help Scout Profile → Security & Access → Docs API). Add it in Settings → Help Scout, then press \"Sync Help Center now\"." },
    reports: { title: "Reports", subtitle: "local analytics over the selected range", actions: ["ranges", "sync-now"] },
    "issue-radar": { title: "Issue Radar", subtitle: "AI-derived alerts, clusters, and known issues", actions: ["cluster-now", "reload-clusters", "detect-gaps"] },
    incidents: { title: "Incidents", subtitle: "3 incident(s)", actions: ["new-incident"] },
    "custom-objects": { title: "Custom Objects", subtitle: "3 type(s) · 15 object(s) · 2 link(s)", actions: ["new-type"] },
    automation: { title: "Automation", subtitle: "Rules trigger actions when conversations match certain conditions. Actions that need review park in the approvals queue.", actions: ["new-rule"] },
    "ai-center": { title: "AI Center", subtitle: null },
    outreach: { title: "Outreach", subtitle: null },
    notifications: { title: "Notifications", subtitle: "37 total · 2 unread", actions: ["mark-all-read"] },
    search: { title: "Search", subtitle: "conversations, knowledge, and customers in one query" },
    "support-graph": { title: "Support Graph", subtitle: "15 conversation links · 7 customer links · 3 human edges" },
    connectors: { title: "Connectors", subtitle: "Attach external data sources (HTTP endpoints, CSV files inside the data directory) as read-only tables; the AI tools can query them when allowed.", actions: ["new-connector"] },
    diagnostics: { title: "Diagnostics", subtitle: "Every error and report the app records: sync failures, connection problems, webhook rejections and background job failures — with the outcome of every sync run." },
    "sync-health": { title: "Sync Health", subtitle: "engine state, checkpoints, and recent runs" },
    settings: { title: "Settings", subtitle: "local configuration, persisted in the settings table", actions: ["settings-tabs"] },
    onboarding: { title: "Welcome to SupportOS+", subtitle: "Connect your Help Scout account in Settings to pull your real conversations. The app starts empty: no sample content, no stand-ins — only your real history." },
    shell: { title: "Shell", subtitle: "native shell diagnostics (Phase 2 surface)", actions: ["theme-switch"] },
  };

  /* ----- per-page fixtures ------------------------------------------------ */
  const PAGES = {
    dashboard: {
      metrics: [
        { label: "New", value: 14 }, { label: "Active", value: 23 }, { label: "Pending", value: 7 }, { label: "Closed", value: 9 },
        { label: "Unassigned", value: 3 }, { label: "Backlog (7d stale)", value: 5 }, { label: "Replies sent", value: 63 },
        { label: "First response avg", value: 0, hint: "min" }, { label: "Great ratings", value: 7 }, { label: "Okay ratings", value: 2 },
        { label: "Not-good ratings", value: 1 }, { label: "Resolution avg", value: 0, hint: "min" },
      ],
      chart: { title: "New conversations per day", hint: "2026-09-28 → 2026-10-27 · 14 points", series: "new conversations", points: [3, 5, 4, 6, 8, 7, 9, 5, 6, 8, 11, 7, 9, 12] },
      breakdowns: [
        { title: "By mailbox", rows: [["Support", 86], ["Billing", 34]] },
        { title: "By tag", rows: [["export-bug", 12], ["sso", 9], ["billing", 7], ["dark-mode", 4], ["onboarding", 3]] },
        { title: "By channel", rows: [["email", 97], ["chat", 21], ["unknown", 2]], note: "ignores the channel chip by design (dashboard.rs:337-345)" },
        { title: "By agent", rows: [["Ada Lin", 31], ["Kim Reyes", 22], ["Grace Park", 14]] },
        { title: "By team", rows: [["Tier 1", 44], ["Tier 2", 23]] },
        { title: "By channel", rows: [["email", 97], ["chat", 21], ["unknown", 2]] },
      ],
      channelMetrics: { title: "Channel metrics", headers: ["channel", "count", "first resp (min)", "resolution (min)"], rows: [["email", 97, 42, 310], ["chat", 21, 3, 85], ["unknown", 2, "—", "—"]] },
      mailboxComparison: {
        title: "Mailbox comparison",
        headers: ["mailbox", "new", "active", "closed", "backlog", "first resp (min)", "resolution (min)", "great", "ratings"],
        rows: [["Support", 86, 23, 41, 5, 42, 310, 5, 9], ["Billing", 34, 7, 18, 0, 55, "—", 2, 3]],
      },
      radar: { hint: "AI-derived alerts from local conversation data", alerts: [
        { sev: "high", text: "Rising volume: Export failures · 6 conversations" },
        { sev: "medium", text: "Cluster \"Login loop\" is associated with poor ratings · 4 conversations" },
      ] },
    },
    "live-chats": {
      counter: "2 open · 1 ended",
      chats: [
        { id: "chat-1", name: "Kim One", state: "open", last: "2026-10-27T09:41:00Z", preview: "help the export is broken", mailbox: 3, assignee: null,
          messages: [
            { who: "customer · Kim One", ts: "2026-10-27T09:39:10Z", text: "help the export is broken" },
            { who: "agent · you", ts: "2026-10-27T09:41:00Z", text: "Hi Kim — I'm on it. Can you open the export screen and tell me the row count?" },
          ] },
        { id: "chat-2", name: null, state: "open", last: "2026-10-27T08:12:00Z", preview: "(no message yet)", mailbox: 3, assignee: "Ada",
          messages: [] },
        { id: "chat-3", name: "Grace N", state: "ended", last: "2026-10-26T17:33:00Z", preview: "thanks, that fixed it", mailbox: 3, endedAs: 499,
          messages: [
            { who: "customer · Grace N", ts: "2026-10-26T17:31:00Z", text: "the beacon wasn't loading at all" },
            { who: "agent · Ada", ts: "2026-10-26T17:32:40Z", text: "Should be fixed now — hard refresh and the loader is back." },
            { who: "customer · Grace N", ts: "2026-10-26T17:33:00Z", text: "thanks, that fixed it" },
          ] },
      ],
    },
    operations: {
      generated: "generated 2026-10-27T09:59:59.123Z · waiting threshold 120 min",
      tiles: [
        { key: "sla_breached", label: "SLA breached", count: 2, sev: "Critical", drill: false },
        { key: "needs_first_response", label: "Needs first response", count: 5, sev: "Warning", drill: true },
        { key: "waiting_over_threshold", label: "Waiting over threshold", count: 3, sev: "Warning", drill: true },
        { key: "urgent", label: "Urgent", count: 1, sev: "Warning", drill: true },
        { key: "sla_at_risk", label: "SLA at risk", count: 6, sev: "Warning", drill: false },
        { key: "ai_escalation", label: "AI escalation", count: 1, sev: "Warning", drill: false },
        { key: "issue_spike", label: "Issue spike", count: 1, sev: "Warning", drill: false },
        { key: "automation_approvals", label: "Automation approvals", count: 2, sev: "Warning", drill: false },
        { key: "failed_jobs", label: "Failed jobs", count: 1, sev: "Warning", drill: false },
        { key: "unassigned", label: "Unassigned", count: 4, sev: "Info", drill: true },
        { key: "customer_waiting", label: "Customer waiting", count: 9, sev: "Info", drill: true },
        { key: "high_effort", label: "High effort", count: 3, sev: "Info", drill: true },
        { key: "repeated_issue", label: "Repeated issue", count: 2, sev: "Info", drill: false },
        { key: "known_issue", label: "Known issue", count: 1, sev: "Info", drill: false },
        { key: "campaign_activity", label: "Campaign activity", count: 1, sev: "Info", drill: false },
      ],
      workload: {
        headers: ["Agent", "Open", "Waiting", "Capacity", "Load", "Pressure", "Actions"],
        rows: [
          { agent: "Ada", open: 9, waiting: 4, capacity: 12, load: "43%", pressure: "3.50" },
          { agent: "Grace", open: 6, waiting: 2, capacity: 10, load: "31%", pressure: "2.05" },
          { agent: "Kim", open: 12, waiting: 7, capacity: 14, load: "58%", pressure: "4.75" },
        ],
        suggested: ["#501 Export to CSV fails on large files → Ada (lowest load)", "#503 Cannot sign in with SSO → Grace (lowest load)"],
      },
      mailboxes: ["Support", "Billing"],
      workflows: [
        { name: "Escalate", kind: "manual", status: "active" },
        { name: "Weekly digest export", kind: "scheduled", status: "active" },
      ],
    },
    customers: {
      results: [
        { name: "Kim Pearce", email: "kim@northwind-logistics.test", conversations: 4, last: "2026-10-27 09:14" },
        { name: "Ada Customer", email: "ada@brightside.test", conversations: 2, last: "2026-10-26 22:26" },
        { name: "Nabin Sayeed", email: "nabin@brightside.test", conversations: 1, last: "2026-10-18 10:00:00" },
        { name: "Marta Keller", email: "marta@acme.test", conversations: 0, last: "—" },
      ],
      profile: {
        name: "Kim Pearce", email: "kim@northwind-logistics.test", phone: "+880 17 555 0142", company: "Northwind Logistics", lastActivity: "2026-10-27 09:14",
        health: { "open conversations:": 3, "not-good ratings (90d):": 1, "SLA breaches:": 2 },
        eventMix: [["conversation_created", 4], ["customer_replied", 9], ["agent_replied", 7], ["rating_received", 3]],
        profile: { communication_style: "direct, technical", prefers: "email follow-ups", timezone_hint: "UTC+1", escalation_pattern: "escalates after 2 unresolved follow-ups" },
        stage: "ai + heuristics",
        recommendation: { approach: "lead with the engineering ticket number, then the workaround", tone: "candid, low ceremony" },
        provenance: "prompt_version v3 · model gpt-4o-mini",
        timeline: [
          ["2026-10-27 09:14", "agent_replied", 501, "ENG-2293 merged to staging; update promised"],
          ["2026-10-26 16:24", "customer_replied", 501, "Asks for an ETA on the export fix"],
          ["2026-10-24 08:31", "conversation_created", 501, "Export to CSV fails on large files"],
          ["2026-10-02 09:12", "conversation_created", 476, "Report export timeout on Growth plan"],
        ],
        milestones: [
          ["2026-10-24 08:40", "sla_breach", "breach: first_response"],
          ["2026-10-09 14:41", "high_effort", "effort score 7.20"],
        ],
        conversations: [
          [501, "Export to CSV fails on large files", "active", "2026-10-27 09:14"],
          [503, "Cannot sign in with SSO", "active", "2026-10-25 22:26"],
          [505, "Refund for duplicate charge", "pending", "2026-10-23 22:26"],
        ],
      },
    },
    organizations: {
      orgs: [
        { name: "Northwind Logistics", domains: "northwind-logistics.test", customers: 3, conversations: 7,
          members: [["Kim Pearce", "kim@northwind-logistics.test", "Head of Support"]] },
        { name: "Acme", domains: "acme.com, acme.io", customers: 12, conversations: 34,
          members: [["Marta Keller", "marta@acme.test", "Ops lead"], ["John Smith", "john@acme.test", null]] },
        { name: "Globex", domains: "globex.com", customers: 4, conversations: 9, members: [] },
      ],
    },
    knowledge: {
      documents: [
        ["Troubleshooting logins", "#1", "internal_only", 3],
        ["CSV export guide", "#2", "customer_safe", 5],
        ["Refund policy", "#1", "internal_only", 2],
      ],
      sources: [
        ["Manual import", "manual", "internal", 2, "2026-10-01 14:22:31"],
        ["Handbook", "file", "customer_safe", 1, "2026-10-12 09:10:03"],
      ],
      freshnessHint: "Documents by review state (stale / unreviewed / unverified / low usage)",
      docsStats: [
        { label: "collections", value: 3 },
        { label: "articles", value: 12, hint: "published 9 · draft 2 · internal 1" },
        { label: "views", value: 1240 },
      ],
      gaps: ["reset password ×3", "bulk rename ×2", "webhook relay ×1"],
      stale: "2 stale doc(s) tracked — re-ingest to refresh",
      articles: [
        ["Export guide", "Getting started", "published", 41],
        ["Reset your password", "Account", "published", 87],
        ["SSO setup (Okta)", "Account", "draft", 0],
        ["Webhook relay guide", "Advanced", "internal", 12],
      ],
      drafts: [
        { id: 4, title: "How to bulk-rename workspaces", status: "awaiting review", publish: null },
        { id: 3, title: "Export limits on the Growth plan", status: "approved", publish: "published" },
      ],
    },
    reports: {
      ranges: ["7 days", "30 days", "90 days", "1 year"],
      tabs: ["Overview", "SLA", "Why contacting", "Top questions", "Quality", "Issue radar", "Digest", "Happiness", "Help Scout", "Builder", "Insights"],
      overview: [
        { label: "New", value: 14 }, { label: "Closed", value: 9 }, { label: "Replies sent", value: 63 }, { label: "Backlog", value: 5 },
        { label: "Great ratings", value: 7 }, { label: "Okay ratings", value: 2 }, { label: "Not-good ratings", value: 1 }, { label: "Unassigned", value: 3 },
      ],
      ratingsNote: "Ratings come from the satisfaction webhook; Help Scout's own happiness reports need a Plus/Pro plan and are not fetched.",
      chart: { title: "New conversations per day", annotation: "new conversations · 3 → 12", points: [3, 5, 4, 6, 8, 7, 9, 5, 6, 8, 11, 7, 9, 12] },
      sla: {
        headers: ["mailbox", "in range", "fr met/missed", "fr avg (min)", "res met/missed", "res avg (min)", "waiting oldest (min)"],
        rows: [["Support", 128, "110/18", 42, "96/32", 310, 815], ["Billing", 51, "44/7", 55, "39/12", 460, "—"]],
        bars: [["Support", 42], ["Billing", 55]],
      },
      whyContacting: [["billing", 3], ["sso", 2], ["export", 6]],
      topQuestions: [["How do I export data?", 12], ["SSO redirect loop after login", 8], ["refund timeline", 5]],
      quality: { closed: 41, rows: 41, aiLayer: 12, recent: [
        { id: 5021, badges: ["b&f 6", "handoffs 2", "res min 214", "AI review"] },
        { id: 5018, badges: ["b&f 2", "res min 45"] },
        { id: 5012, badges: ["handoffs 1", "res min 1300"] },
      ] },
      radar: [{ sev: "medium", text: "Rising volume: Export failures · 6 conversations" }],
      digest: { chips: [["2026-10-20T09:0", "facts + narrative"], ["2026-10-13T09:0", "facts + narrative"]], detail: {
        title: "Digest 2026-10-13 → 2026-10-20", narrative: "Volume rose 18% week over week, driven by export failures on the Growth tier. Sentiment stayed neutral overall with one negative cluster around SSO redirects. Workload concentrated on Ada (43% load).",
        anomalies: ["Export failure cluster tripled (2 → 6 conversations)", "First response avg regressed 38 → 42 min"],
        changed: ["new gap: how do I bulk rename workspaces", "article changed: Export guide"],
      } },
      happiness: { stats: [{ label: "Great", value: 7 }, { label: "Okay", value: 2 }, { label: "Not good", value: 1 }, { label: "CSAT (great ÷ all)", value: "70%" }], note: "10 ratings received in the last 30 days. Ratings arrive via the satisfaction webhook — Help Scout has no list-ratings API, so an empty table means none arrived, not zero happened.", rows: [["Great", 5021, "Nabin Sayeed", "Fast resolution, thanks!"], ["Not good", 5012, "Kim Pearce", "Took five days to acknowledge the ticket."]] },
      helpScout: { banner: "Your Help Scout plan does not include the Reports API — it is available on Plus and Pro plans (or your account lacks report permission). The reports below are computed locally from your synced data." },
      builder: {
        metrics: ["Conversations", "Unique customers", "Organizations", "First responses", "Agent replies", "Customer replies", "Closures", "Avg first response time (minutes)", "Avg resolution time (minutes)", "Avg customer waiting (hours)", "SLA breached conversations", "Custom state changes", "Avg time in custom state (hours)", "High-priority share", "Issue-linked share", "AI attribute share", "Avg customer effort", "High-friction share", "Campaign messages sent", "Campaign replies", "Campaign reply rate"],
        dimensions: ["No grouping (total)", "Day", "Week", "Month", "Mailbox", "Channel", "Tag", "Assignee", "Team", "Status", "Priority", "Custom state", "Response state", "Issue"],
        saved: [{ name: "conversations-by-mailbox", created: "2026-10-20 14:02:11" }],
        result: { title: "Conversations — Mailbox", sub: "2025-12-14 → 2026-01-13", headers: ["group", "value"], rows: [["Support", 128], ["Billing", 51]] },
      },
      insights: {
        health: [{ label: "conversations", value: 120 }, { label: "SLA breaches (tracked)", value: 9, hint: "7.5% of conversations" }, { label: "high friction", value: 6, hint: "5.0% of conversations" }, { label: "avg first response", value: "42 min" }, { label: "resolution rate", value: "81.2%" }],
        qa: { avg: "0.74", checks: 41, dist: [["low: 8", "neutral"], ["medium: 19", "neutral"], ["high: 14", "neutral"]] },
        friction: { headers: ["conversation", "effort", "messages", "resolution", "reopens", "escalated"], rows: [[5021, "7.20", 18, "51.3 h", 2, "yes"], [5018, "6.10", 14, "31.0 h", 1, "no"], [5012, "5.75", 9, "21.7 h", 0, "yes"]] },
        graph: { headers: ["kind", "label", "entity", "created", ""], rows: [["campaign", "Spring push", "—", "2026-01-10 11:04:00", "inspect"], ["product", "Beacon widget", "—", "2026-01-08 09:12:40", "inspect"]] },
      },
    },
    "issue-radar": {
      alerts: [
        { sev: "critical", text: "New issue cluster: Login loop · 6 conversations" },
        { sev: "warning", text: "Rising volume: Export failures · 6 conversations" },
        { sev: "info", text: "High-volume question (4 tickets) · 4 conversations" },
      ],
      sla: { title: "SLA: 2 breached · 5 at risk", rows: [
        ["#501 Export to CSV fails on large files · Support · breached", "danger"],
        ["#505 Refund for duplicate charge · Billing · at_risk", "warning"],
      ] },
      clusters: [
        { title: "Login loop", trend: "rising", hint: "6 conversations · 4 customers", summary: "Users stuck at SSO redirect after the 2.1 deploy" },
        { title: "Export failures", trend: "new", hint: "6 conversations · 5 customers", summary: null },
      ],
      gapBadges: [["3 open", "warning"], ["1 approved", "success"], ["1 rejected", "neutral"]],
      gapKinds: [
        { kind: "Repeated question, no covering document", cands: [{ count: 7, query: "how do I bulk rename workspaces", status: "open", explain: "Asked 7 times across reopened conversations; no document mentions bulk rename.", examples: ["#5021", "#5033"] }] },
        { kind: "Repeated question, existing docs did not solve it", cands: [{ count: 4, query: "webhook relay returns 401", status: "approved", explain: "Docs cover the relay setup but not the 401 signature case.", examples: ["#4999"] }] },
      ],
      knownIssues: [
        { title: "Login loop", status: "identified", convs: 4, symptoms: "SSO redirect loop after password change", workaround: "Clear cookies and retry" },
        { title: "Export row cap", status: "fix_in_progress", convs: 6, symptoms: "Exports over ~10,000 rows fail at 60 s", workaround: "Split the date range" },
      ],
    },
    incidents: {
      list: [
        { code: "INC-001", title: "Checkout outages for EU customers", sev: "sev1", status: "investigating", convs: 14 },
        { code: "INC-002", title: "SSO redirect loop", sev: "sev2", status: "identified", convs: 6 },
        { code: "INC-003", title: "CSV export failures", sev: "sev3", status: "monitoring", convs: 9 },
      ],
      detail: {
        code: "INC-001", title: "Checkout outages for EU customers", sev: "sev1", status: "investigating", started: "2026-01-12 08:41:03", owner: "Nabin Sayeed",
        description: "Payment provider EU region returning 502s; checkout conversions down 60% in the affected region.",
        customerSafe: "Some customers in Europe may see an error at checkout. Our team is on it.",
        linked: [[5021, "Export broken for CSV files"], [5019, "Card declined for valid card"]],
        customers: ["Kim Pearce", "Nabin Sayeed"],
        notes: [["2026-01-12 09:15:44", "Mitigated by rolling back the payments deploy"]],
        releases: [["2.1.4", "shipped"], ["2.1.5", "in_progress"]],
      },
      statuses: ["investigating", "identified", "fix_in_progress", "monitoring", "resolved"],
      severities: ["sev1", "sev2", "sev3", "sev4"],
    },
    "custom-objects": {
      types: [
        { name: "Warranty Claims", count: 3, desc: "Warranty claim records linked from conversations" },
        { name: "Inventory", count: 12, desc: null },
        { name: "Rmas", count: 0, desc: null },
      ],
      objects: [
        { title: "Broken hinge", props: [["sku", "A-100"], ["qty", 42], ["in_stock", true], ["unit_cost", 1.5]] },
        { title: "Loose panel", props: [["sku", "A-114"], ["qty", 3], ["in_stock", false]] },
      ],
      fieldTypes: ["text", "long_text", "number", "date", "select", "boolean"],
    },
    automation: {
      rules: {
        headers: ["name", "when", "then", ""],
        rows: [
          { name: "Escalate urgent", when: "tag 'urgent' added", then: "change status to pending", approval: true, enabled: true },
          { name: "Auto-triage SLA", when: "SLA at risk", then: "add tag 'follow-up'", approval: false, enabled: false },
        ],
      },
      approvals: [
        { text: "rule #3 on conversation #47 · {\"kind\":\"assign\",\"assignee_remote_id\":506}" },
      ],
      triggers: ["Status changed", "Tag added", "SLA at risk"],
      actions: ["Assign", "Add tag", "Send note", "Change status", "Set priority"],
    },
    "ai-center": {
      tabs: ["Health", "Analytics", "Attributes", "Copilot", "Usage", "Jobs"],
      health: {
        lmstudio: [
          ["AI enabled:", "badge-success|enabled"],
          ["provider:", "mono|lmstudio"],
          ["base url:", "mono|http://127.0.0.1:1234"],
          ["chat model:", "mono|qwen2.5-7b-instruct"],
          ["embedding model:", "mono|(unset)"],
          ["timeout:", "mono|120000 ms"],
          ["concurrency:", "mono|2"],
        ],
        providerRecord: { chips: ["None (no AI provider)", "LM Studio"], ready: "features ready", stored: "provider_kind=lmstudio chat_model=qwen2.5-7b-instruct embedding_model=(unset) dim=(unset) base_url=http://127.0.0.1:1234" },
        endpoints: [
          { name: "LM Studio", url: "http://127.0.0.1:1234", on: true, keySet: false, builtin: true, probe: "reachable — 3 model(s) · 41 ms" },
          { name: "Ollama", url: "http://127.0.0.1:11434", on: false, keySet: false, builtin: false, probe: null },
        ],
        routing: [
          ["embed", "embeddings — local by default; cloud is an explicit opt-in"],
          ["classify", "triage, sentiment, topic, urgency — high volume, low stakes"],
          ["summarize", "thread summaries, memory extraction, notes, digest"],
          ["draft", "customer-facing drafts — the only role where cloud is recommended"],
          ["agent", "the tool-calling copilot chat"],
        ],
        features: ["ticket_analysis", "ai_note", "draft", "attributes", "memory_extraction", "issue_clustering", "copilot", "draft_coaching", "translation", "post_resolution_qa", "interaction_intelligence", "retrieval", "golden_eval", "gap_detection", "article_drafting", "weekly_digest"],
        ledger: {
          headers: ["day", "feature", "model", "runs", "tokens (p/c)", "latency", "est. cost"],
          rows: [
            ["2026-10-27", "ticket_analysis", "qwen2.5-7b", 12, "8421 / 1900", "15320 ms", "$0.0000"],
            ["2026-10-27", "draft", "qwen2.5-7b", 3, "1200 / 340", "8900 ms", "$0.0000"],
            ["2026-10-26", "copilot", "qwen2.5-7b", 5, "4300 / 900", "12300 ms", "$0.0121"],
          ],
        },
      },
      analytics: [
        ["tickets analyzed:", 96], ["analysis success rate:", "97.9%"], ["drafts:", "14 total · 9 accepted · 2 rejected"], ["draft edit rate:", "33%"], ["verification warnings:", 4], ["unsupported claim rate:", "1.2%"],
      ],
      attributes: [
        { label: "Intent", key: "intent", known: 92, unknown: 4, values: [["bug report", 38, "41.3"], ["billing question", 24, "26.1"], ["feature request", 18, "19.6"], ["access issue", 12, "13.0"]] },
        { label: "Urgency", key: "urgency", known: 88, unknown: 8, values: [["moderate", 51, "58.0"], ["high", 29, "33.0"], ["low", 8, "9.1"]] },
        { label: "Escalation signal", key: "escalation_signal", known: 90, unknown: 6, values: [["true", 11, "12.2"], ["false", 79, "87.8"]] },
      ],
      copilot: {
        sessions: [["Refund policy question", 4], ["Export limits", 2], ["(untitled)", 0]],
        chat: [
          { role: "user", text: "what is our refund policy for duplicate charges?" },
          { role: "assistant", text: "Duplicate charges are refunded automatically within 3-5 business days once the payment processor confirms the reversal. The last duplicate-charge case (#505) was refunded with reference RB-90412.", cited: ["Conversation #505", "Saved reply: Refund policy"] },
        ],
      },
      usage: {
        headers: ["day", "feature", "model", "runs", "prompt tok", "completion tok", "latency s", "est cost"],
        rows: [["2026-10-27", "ticket_analysis", "qwen2.5-7b", 12, 8421, 1900, "15.3", "$0.0000"], ["2026-10-27", "copilot", "qwen2.5-7b", 5, 4300, 900, "12.3", "$0.0121"], ["2026-10-26", "draft", "qwen2.5-7b", 3, 1200, 340, "8.9", "$0.0000"]],
        feedback: [["14 drafts", "neutral"], ["9 accepted", "success"], ["2 rejected", "warning"], ["11 feedback rows", "info"], ["33% edited (>20 chars)", "info"], ["avg edit distance 47", "neutral"]],
        golden: { badge: "9/9 passed", backend: "backend: lm_studio://lmstudio", run: "run #3 — 2026-10-27T10:00:00Z" },
      },
      jobs: {
        headers: ["id", "type", "status", "conversation", "model", "created"],
        rows: [
          [7, "ticket_analysis", "completed", "#501", "qwen2.5-7b", "2026-10-27T09:15:00Z"],
          [6, "copilot_chat", "completed", "—", "qwen2.5-7b", "2026-10-27T08:40:12Z"],
          [5, "draft", "completed", "#501", "qwen2.5-7b", "2026-10-27T08:32:00Z"],
          [4, "golden_eval", "succeeded", "—", "qwen2.5-7b", "2026-10-27T07:00:00Z"],
          [3, "ticket_analysis", "failed", "#499", "qwen2.5-7b", "2026-10-26T22:10:44Z"],
          [2, "weekly_digest", "queued", "—", "—", "2026-10-26T21:00:00Z"],
        ],
      },
    },
    outreach: {
      campaigns: [
        { name: "Check-in — How is it going?", status: "draft", hint: "3 recipients · 0 sent · 0 failed · 0 replied · mailbox Support" },
        { name: "Spring push — Feature launch", status: "completed", hint: "120 recipients · 118 sent · 2 failed · 31 replied · mailbox Outreach" },
      ],
      report: [["total", 0], ["sent", 0], ["failed", 0], ["skipped", 0], ["replied", 0], ["unknown", 0], ["remaining", 0]],
      reportNote: "The report panel reads top-level keys while the core nests them under totals (outreach.rs:1336) — every row renders 0. Data-layer bug, out of reskin scope; documented, not fixed here.",
      segments: [["#1 Ticket owners", "{\"combinator\":\"all\",\"conditions\":[{\"kind\":\"history\",\"metric\":\"ticket_count\",\"op\":\"gte\",\"value\":1}],\"exclude\":[]}"], ["#2 Churn risk", "{\"combinator\":\"any\",\"conditions\":[{\"kind\":\"rating\",\"metric\":\"not_good_90d\",\"op\":\"gte\",\"value\":1}],\"exclude\":[]}"]],
      dnc: [["customer #9", "asked politely", "2026-01-01T00:00:00Z"], ["customer #14", "manual", "2026-01-14T11:20:03Z"]],
    },
    notifications: {
      rows: [
        { unread: true, type: "customer_replied", sev: "info", ts: "2026-10-27T05:01:41.123Z", title: "Customer replied on #501 Export to CSV fails on large files", body: "Is there an ETA on the fix?" },
        { unread: true, type: "sla_breach", sev: "error", ts: "2026-10-27T04:12:09.001Z", title: "SLA breached on #505 Refund for duplicate charge", body: null },
        { unread: false, type: "mentioned", sev: "info", ts: "2026-10-26T18:40:22.500Z", title: "Ada mentioned you on #503 Cannot sign in with SSO", body: "\"need Okta metadata from the customer to confirm\"" },
        { unread: false, type: "sync_failure", sev: "error", ts: "2026-10-26T05:02:00.000Z", title: "Sync resource conversations failed: HTTP 401", body: "Token expired mid-run; re-registered automatically." },
        { unread: false, type: "automation_approval", sev: "info", ts: "2026-10-25T14:20:11.300Z", title: "Automation rule #3 needs approval on #47", body: null },
        { unread: false, type: "campaign_reply", sev: "info", ts: "2026-10-25T09:12:40.100Z", title: "Reply on campaign Spring push", body: "Marta Keller replied to the launch email." },
      ],
    },
    search: {
      modeNote: "semantic lane skipped — vector index disabled (FTS5 results only)",
      hits: [
        { scope: "conversation", title: "#501 Export to CSV fails on large files", subtitle: "active", why: ["keyword match", "recent"], snippet: "…every CSV export over roughly 10,000 rows fails at about the 60-second mark…" },
        { scope: "knowledge", title: "CSV export guide", subtitle: null, why: ["keyword match"], snippet: "…exports above ~10,000 rows hit the job cap; split the range…" },
        { scope: "customer", title: "Kim Pearce", subtitle: "kim@northwind-logistics.test", why: ["keyword match"], snippet: null },
        { scope: "conversation", title: "#476 Report export timeout on Growth plan", subtitle: "closed", why: ["semantic match"], snippet: "…raised the per-workspace job cap to 300s and advised splitting ranges…" },
      ],
    },
    "support-graph": {
      stats: "15 conversation links · 7 customer links · 3 human edges",
      results: [
        { kind: "customer", label: "Kim Pearce", sub: "northwind-logistics.test" },
        { kind: "conversation", label: "#501 Export to CSV fails on large files", sub: "active" },
        { kind: "conversation", label: "#503 Cannot sign in with SSO", sub: "active" },
        { kind: "organization", label: "Northwind Logistics", sub: null },
      ],
      neighborhood: {
        center: { kind: "customer", id: 9, label: "Kim Pearce" },
        edges: ["conversation (helpscout_mirror) #501 Export to CSV fails on large files", "conversation (helpscout_mirror) #503 Cannot sign in with SSO", "belongs_to (helpscout_mirror) Northwind Logistics", "owner (human_local) Ada"],
      },
    },
    connectors: {
      headers: ["name", "kind", "health", "refresh", "last sync", ""],
      rows: [
        { name: "Billing export", kind: "csv", health: "ok", refresh: "manual", last: "2026-10-27 05:01:41" },
        { name: "Status page", kind: "http", health: "stale", refresh: "automatic (3600s)", last: "2026-10-25 05:01:44" },
        { name: "Changelog feed", kind: "http", health: "error", refresh: "manual", last: "—" },
      ],
      rowsViewer: { title: "Rows (2)", rows: ["1 {\"sku\":\"A-100\",\"qty\":42}", "2 {\"sku\":\"A-114\",\"qty\":3}"] },
    },
    diagnostics: {
      eventLog: {
        sub: "6 event(s) shown — 2 error(s), 1 warning(s)",
        rows: [
          { level: "error", ts: "2026-10-27 05:02:00", service: "outbound", msg: "Outbound job 1194 failed: HTTP 400: type is required", ctx: "{\"kind\":\"create_reply\"}" },
          { level: "error", ts: "2026-10-27 05:01:41", service: "sync", msg: "Sync resource conversations failed: HTTP 401", ctx: "{\"resource\":\"conversations\"}" },
          { level: "warn", ts: "2026-10-27 04:58:12", service: "webhook", msg: "Webhook signature mismatch — retried with the rotation window", ctx: null },
          { level: "info", ts: "2026-10-27 04:40:03", service: "sync", msg: "Incremental sync finished: 15 record(s) updated", ctx: null },
          { level: "info", ts: "2026-10-26 22:10:44", service: "jobs", msg: "AI job 3 (ticket_analysis) failed: model offline", ctx: null },
          { level: "info", ts: "2026-10-26 21:00:02", service: "connect", msg: "Loopback listener answering on :3001", ctx: null },
        ],
      },
      audit: {
        sub: "5 audited change(s) shown — newest first",
        headers: ["when", "actor", "action", "conversation", "ai"],
        rows: [
          ["2026-10-27 05:01:47", "ai", "draft_inserted", "#501", true, "after: {\"composer\":\"AI draft inserted (412 chars)\"}"],
          ["2026-10-27 04:59:12", "user", "update_status", "#501", false, "before: {\"status\":\"open\"} after: {\"status\":\"pending\"} remote: create_reply"],
          ["2026-10-26 18:22:30", "user", "assign", "#503", false, "after: {\"assignee\":506} remote: assign"],
          ["2026-10-26 17:40:09", "system", "sync_complete", "—", false, null],
          ["2026-10-26 09:15:44", "ai", "note_created", "#501", true, null],
        ],
      },
      syncReports: {
        headers: ["run", "kind", "state", "records", "errors", "started"],
        rows: [
          [381, "incremental", "LIVE", 15, 0, "2026-10-27T05:01:41"],
          [380, "docs", "LIVE", 12, 0, "2026-10-27T04:30:00"],
          [379, "initial", "LIVE", 1204, 0, "2026-10-20T09:00:00"],
          [378, "incremental", "ERROR", 0, 3, "2026-10-20T05:01:41"],
        ],
      },
      outbound: { sub: "14 delivered · 0 queued · 0 sending · 1 failed", headers: ["job", "kind", "at", "error"], rows: [[1194, "create_reply", "2026-10-27 05:02:00", "HTTP 400: type is required"]] },
    },
    "sync-health": {
      engine: { state: "LIVE", last: "2026-10-27T05:01:47.123Z" },
      webhook: "webhook queue: 5 total · 1 pending · 2 processed · 1 failed · 1 duplicates",
      health: "webhook health: healthy · checked 2026-10-27T04:30:00Z · probe: signature verified",
      checkpoints: {
        headers: ["resource", "status", "processed", "failed", "retries", "last success"],
        rows: [
          ["conversations", "ok", 1204, 0, 0, "2026-10-27T05:01:47.123Z"],
          ["customers", "ok", 512, 0, 0, "2026-10-27T05:01:44.001Z"],
          ["mailboxes", "ok", 3, 0, 0, "2026-10-27T05:01:40.900Z"],
          ["docs_articles", "ok", 12, 0, 1, "2026-10-27T04:30:02.000Z"],
          ["tags", "pending", 0, 0, 0, "—"],
        ],
      },
      runs: {
        headers: ["id", "kind", "state", "started", "resources", "records", "errors"],
        rows: [
          [381, "incremental", "LIVE", "2026-10-27T05:01:41", "19/22", 15, 0],
          [380, "docs", "LIVE", "2026-10-27T04:30:00", "1/1", 12, 0],
          [379, "initial", "LIVE", "2026-10-20T09:00:00", "22/22", 1204, 0],
        ],
      },
    },
    settings: {
      tabs: ["Sync & AI", "Help Scout", "LM Studio", "Vector index", "Business hours", "Backups & export"],
      aiBehaviour: [["AI enabled", true], ["Automatic analysis", true], ["Automatic internal notes", false], ["Automatic draft replies", false], ["Automatic reply sending", false], ["Automation rules", false], ["Automation write actions", false], ["Attachment auto download", true]],
      connection: { badge: "connected", redirect: "http://localhost:3001/oauth/callback" },
      doctor: [["✓ ok", "reach the API — 41 ms"], ["✓ ok", "sign in — token valid"], ["✓ ok", "read the user — Kim (admin)"], ["! warning", "list mailboxes — 2 of 3 mailboxes visible (permission)"], ["✓ ok", "rate-limit budget — 87 req/min remaining"]],
      sources: [["client id:", "environment"], ["client secret:", "app settings"], ["webhook secret:", "app settings"], ["docs api key:", "not set"]],
      businessHours: [
        { name: "Support", configured: true, hint: "Asia/Dhaka · [1,2,3,4,5] · 10:00-18:20 · fr 240m · res 1440m" },
        { name: "Billing", configured: false, hint: null },
      ],
      backups: [["supportos-backup-20261027T050147Z.db", "34,603,392 bytes", "2026-10-27 05:01:47"]],
      vectorStore: "/home/user/.local/share/supportos/vectors/vectors.db",
    },
    onboarding: {
      rows: [["step:", "welcome"], ["onboarding complete:", "false"], ["conversations:", "15"], ["Help Scout credentials:", "true"], ["sync state:", "receiving"]],
      hints: ["Onboarding is not marked complete yet."],
    },
    shell: {
      runtime: [["database", "/home/user/.local/share/supportos/supportos.db"], ["schema", "v53"], ["background events", "241"], ["last event", "SyncCompleted"], ["sync engine", "idle"]],
      hint: "all pages ported: dashboard, inbox, operations, customers, organizations, knowledge/docs, reports, issue radar, incidents, custom objects, automation, AI center, outreach, notifications, search, support graph, connectors, sync health, settings, onboarding",
    },
  };

  /* ----- per-page reskin notes (After view; receipts cite the specs) ------- */
  const PAGE_NOTES = {
    dashboard: [
      { title: "Metric cards keep exact labels, gain context", fix: "Same 12 labels; values get tabular numerals and the avg cards keep their honest 0-until-data behavior visible via the 'min' hint.", was: "Metric card: label 12px muted + bare 24px integer (design.rs:797-821).", receipt: "dashboard.rs:641-657" },
      { title: "Breakdowns get ordered bars, not bare rows", fix: "Count-proportional bars with right-aligned counts make distribution readable at a glance.", was: "Striped 2-col grid name/count (dashboard.rs:750-778).", receipt: "dashboard.rs:750-778" },
      { title: "Quirk preserved: By channel ignores the channel chip", fix: "The data-layer quirk stays (presentation cannot fix it); a footnote names it instead of hiding it.", was: "Deliberate behavior (dashboard.rs:337-345).", receipt: "dashboard.rs:989-993" },
    ],
    operations: [
      { title: "Tiles grouped by severity with count-first hierarchy", fix: "Critical band on top with count-first tiles; drill tiles keep the 'open in inbox →' affordance as a real link-chip.", was: "Wrapped tile cards min 196×52, severity-colored count (operations.rs:998-1049).", receipt: "operations.rs:334-383" },
      { title: "Workload table: Load and Pressure un-swapped", fix: "Presentation fixes the value swap the data layer ships: Load shows the weighted number, Pressure the %, correctly labeled.", was: "Headers swapped vs values (Load={pressure*100}%, Pressure={weighted_load}) operations.rs:425-426.", receipt: "operations.rs:410-449" },
      { title: "Saved replies reachable from the composer", fix: "Library stays the CRUD home, but the Inbox composer inserts the same saved replies (already prototyped in the Inbox After view).", was: "Composer had zero path to saved replies (operations.rs:746).", receipt: "operations.rs:746" },
    ],
    customers: [
      { title: "Profile becomes a two-column dossier", fix: "Support health + event mix left, AI interaction profile right — same content, clearer scan order with the AI provenance pinned.", was: "Two equal columns via ui.columns(2) (customers.rs:571-583).", receipt: "customers.rs:602-719" },
      { title: "AI identity on the interaction profile", fix: "The 'AI' badge becomes the AI token used everywhere (violet) with the stage/provenance line under it.", was: "Sparkles + Info-blue 'AI' badge (customers.rs:658-674).", receipt: "customers.rs:646-719" },
    ],
    organizations: [
      { title: "Chips become list rows with counts as data", fix: "Org rows show name + customers/conversations as aligned mono figures instead of one long chip string.", was: "Chip '{name} · {n} customers · {n} conversations' (organizations.rs:216-230).", receipt: "organizations.rs:216-230" },
    ],
    knowledge: [
      { title: "Freshness tab honesty", fix: "The tab renders only its hint line today (report['flags'] never exists — knowledge.rs:1249 vs knowledge_store.rs:516-530). The reskin keeps the hint and adds a visible 'no freshness data — data-layer' state instead of a silent void.", was: "Four flag cards that can never render.", receipt: "knowledge.rs:1249-1256 + knowledge_store.rs:516-530" },
      { title: "Visibility vocabulary unified", fix: "Import chips say 'internal-only (never in customer drafts)' while the table shows raw 'internal_only' — reskin shows the human label with the raw token as a sub-line.", was: "Two vocabularies for the same enum (knowledge.rs:31-37 vs 798-839).", receipt: "knowledge.rs:633-641" },
    ],
    reports: [
      { title: "Charts get axis context", fix: "Index-based charts keep their honest annotation but gain first/last labels so '4 → 9' reads as values, not dates.", was: "x = row index, no axis labels (chart.rs:100-106).", receipt: "reports.rs:515-527" },
      { title: "Radar badge honesty", fix: "This tab always renders 'medium'/Warning because the engine never emits 'high' — reskin shows the raw severity token to stop the lie.", was: "severity=='high' ? high : medium (reports.rs:1997-2001).", receipt: "reports.rs:1974-2014" },
      { title: "11 tabs regrouped", fix: "Eleven flat chips become three clusters (Overview · Quality · Sources) with the same destinations — fewer anchors to parse.", was: "Flat 11-chip row (reports.rs:433-439).", receipt: "reports.rs:41-69" },
    ],
    "issue-radar": [
      { title: "Raw severity tokens", fix: "Alerts show critical/warning/info as stored (info keeps Warning tone — engine fact, not a styling choice).", was: "Raw strings with only critical→red mapping (issue_radar.rs:703-723).", receipt: "issue_radar.rs:703-723" },
      { title: "Gap pipeline reads as a queue", fix: "Open/approved/rejected counts become a header strip; candidates get approve/reject as compact split-buttons.", was: "Badges row + inline buttons (issue_radar.rs:497-686).", receipt: "issue_radar.rs:581-686" },
    ],
    incidents: [
      { title: "Status/severity as one badge pair", fix: "List rows keep INC codes; sev and status badges align into one visual unit with the conversation count right-aligned.", was: "Badge row + trailing count (incidents.rs:440-485).", receipt: "incidents.rs:440-485" },
      { title: "Cap disclosure stays in the subtitle", fix: "'showing first 100 of N' stays a subtitle disclosure (no fake pager) — rendered as a pill so it reads as a limit, not an error.", was: "Dynamic subtitle doubles as the pager (incidents.rs:343-354).", receipt: "incidents.rs:343-354" },
    ],
    "custom-objects": [
      { title: "Objects as property sheets", fix: "Object cards keep monospace key: value lines but get a subtle key column so repeated scans align.", was: "Per-card monospace lines (custom_objects.rs:497-544).", receipt: "custom_objects.rs:497-544" },
    ],
    automation: [
      { title: "Approvals stop showing raw JSON first", fix: "The proposed action renders human-first ('change status to pending') with the raw JSON as a collapsible detail line.", was: "Literal proposed_action_json as the row label (automation.rs:492-501).", receipt: "automation.rs:469-521" },
      { title: "Rules table 'when/then' as trigger→action chips", fix: "Same strings, chip-wrapped so the trigger and the approval marker cannot blur together.", was: "Plain 13px text + ⚠ (needs approval) inline (automation.rs:414-433).", receipt: "automation.rs:386-457" },
    ],
    "ai-center": [
      { title: "AI gets a house style", fix: "All AI-derived surfaces (Analytics, Attributes, Jobs, Copilot) share the violet AI token instead of Info-blue 'AI' badges.", was: "Sparkles + Info 'AI' badge on three headers (ai_center.rs:1756-1760, 1822-1826, 1894-1899).", receipt: "ai_center.rs:1749-1939" },
      { title: "Routing combo names the endpoint", fix: "The closed combo shows the endpoint NAME, not its index ('0', '1'). Presentation can fix this one.", was: "position(...).to_string() as the closed label (ai_center.rs:1444-1448).", receipt: "ai_center.rs:1420-1521" },
      { title: "Copilot chat gets message cards", fix: "Assistant/user rows become role-typed bubbles with cited-source chips — the same card language as the Inbox AI card.", was: "Badge + small text rows, tool messages skipped (ai_center.rs:695-741).", receipt: "ai_center.rs:612-797" },
    ],
    outreach: [
      { title: "Report zeros stay, labeled", fix: "The seven-zero report panel is a data-layer bug (top-level vs totals keys) — out of reskin scope; the panel gains a footnote pointing at the bug instead of fake numbers.", was: "All seven rows render 0 (outreach.rs:568-593 vs core outreach.rs:1336).", receipt: "outreach.rs:562-593" },
      { title: "Lifecycle buttons grouped", fix: "queue/pause/resume cluster as 'run' actions; cancel remaining / retry failed sit in a danger-adjacent group.", was: "Five equal buttons in a row (outreach.rs:507-531).", receipt: "outreach.rs:470-541" },
    ],
    notifications: [
      { title: "Unread becomes a state, not a dot in a badge", fix: "Unread rows get a leading dot + bolder title; the type token loses the '● ' prefix hack.", was: "Badge text '● customer_replied' vs ' customer_replied' (notifications.rs:197-208).", receipt: "notifications.rs:172-234" },
      { title: "Relative time + absolute on hover", fix: "Raw RFC-3339 strings become relative stamps with the raw value preserved in the title attribute.", was: "Raw created_at in small monospace (notifications.rs:209-213).", receipt: "notifications.rs:172-234" },
    ],
    search: [
      { title: "Provenance badges stay", fix: "keyword/semantic match chips survive (they are the honesty layer); scope badges get icons.", was: "Neutral scope badge + reason badges (search.rs:232-252).", receipt: "search.rs:218-259" },
    ],
    "support-graph": [
      { title: "Neighborhood as a mini-graph", fix: "Edge lines gain relation/origin styling so 'owner (human_local)' reads as relation + provenance.", was: "Plain 13px edge labels (support_graph.rs:293-307).", receipt: "support_graph.rs:267-311" },
    ],
    connectors: [
      { title: "Health as a status light + word", fix: "ok/stale/error badges gain a leading dot for pre-attentive scanning; '—' for never-synced stays.", was: "Badge only (connectors.rs:60-67).", receipt: "connectors.rs:305-354" },
    ],
    diagnostics: [
      { title: "Event log keeps raw UTC — adds level color bars", fix: "Timestamps stay raw (they are forensic data); a 2px level bar precedes each row for scanning.", was: "Level badge + 12px weak mono timestamp (diagnostics.rs:297-301).", receipt: "diagnostics.rs:240-329" },
      { title: "Sync reports state colors", fix: "LIVE/ERROR/CANCELLED keep the Success/Destructive/Warning tones — unchanged, already correct.", was: "Badge tone map (diagnostics.rs:447-451).", receipt: "diagnostics.rs:424-469" },
    ],
    "sync-health": [
      { title: "Engine state as a status hero", fix: "state:/last success: become a header strip with a colored state dot (LIVE green, ERROR red, NEW neutral); tables unchanged.", was: "Field-grid mono values (sync_health.rs:427-465).", receipt: "sync_health.rs:427-493" },
    ],
    settings: [
      { title: "Tabs move into the page", fix: "Six chips sit right of the title today (settings.rs:1185-1192) — reskin gives them a standard tab bar under the header.", was: "Header action slot chips (settings.rs:1183-1192).", receipt: "settings.rs:1183-1192" },
      { title: "Danger zones isolated", fix: "Workspace data wipe and two-step restore keep their exact copy but gain a bordered danger container so scanning cannot miss them.", was: "Inline destructive buttons (settings.rs:1689-1727, 2346-2383).", receipt: "settings.rs:1689-1727" },
    ],
    onboarding: [
      { title: "Setup status becomes a checklist", fix: "The five mono rows render as a checklist with state dots; the honest 'starts empty' subtitle stays verbatim.", was: "form_grid label→monospace value (onboarding.rs:131-154).", receipt: "onboarding.rs:121-195" },
    ],
    shell: [
      { title: "Dev page stays dev", fix: "Runtime grid keeps mono values; the stale 'all pages ported' hint (omits live chats) gets a 'stale checklist' tag — honesty over cosmetics.", was: "Static hint paragraph (app.rs:416-422).", receipt: "app.rs:358-423" },
    ],
    "live-chats": [
      { title: "Chat list becomes conversation rows", fix: "Two-line selectable rows keep raw UTC (webhook data), but gain state dots and the send-rule hint rendered as a proper callout.", was: "selectable_label 2-line rows + plain hints (live_chats.rs:170-335).", receipt: "live_chats.rs:116-335" },
    ],
  };

  const NOTES = [
    { n: 1, title: "Role-typed message cards", fix: "Each entry is now a semantic object: avatar, author, role chip, relative time, role-tinted surface.", was: "thread_entry_ui painted every entry the same: 12px badge + 13px name + raw UTC stamp, HTML flattened to full-width plain text, 1px divider. No message object existed.", receipt: "inbox.rs:2858-2930" },
    { n: 2, title: "AI has its own identity", fix: "Dedicated violet token + 'AI draft' chip + grounding source line + Insert/Copy actions.", was: "AI entries shared the exact same Info-blue badge as Agent replies (BadgeTone::Info for \"message\" and for kind.starts_with(\"ai\")).", receipt: "inbox.rs:2871 vs 2875" },
    { n: 3, title: "Full-height reader", fix: "The thread fills the reader column; the composer stays pinned below.", was: "The whole thread scrolled inside a 300px-tall ScrollArea while every entry's full HTML was loaded with no LIMIT.", receipt: "inbox.rs:2306-2314 + core/inbox.rs:326-341" },
    { n: 4, title: "Quoted email history collapsed", fix: "One line: 'Show N earlier messages' — expands on demand.", was: "Quoted history rendered inline as weak-color text, blending into the active reply.", receipt: "inbox.rs:2916-2918" },
    { n: 5, title: "Attachments surfaced", fix: "File chips with name + size on the message that carries them.", was: "Attachments were downloaded but never displayed anywhere in the thread UI.", receipt: "task 15 forensic finding" },
    { n: 6, title: "Saved replies in the composer", fix: "One click to insert, fully editable before sending.", was: "Saved Replies lived only on the Operations page; the composer had zero path to them (0 hits in inbox.rs).", receipt: "operations.rs:746" },
    { n: 7, title: "Copilot split into typed cards", fix: "Signals / Summary / Similar issues / Suggested reply — each with its own weight and actions.", was: "One dense card mixed chips, bullets, history, similar issues and the suggestion at the same small size.", receipt: "inbox.rs:1473-1589" },
    { n: 8, title: "Human time + date dividers", fix: "Relative timestamps in the list, absolute on cards, day separators between bursts.", was: "Raw UTC stamps ('2026-10-27 09:14') everywhere via compact_time.", receipt: "inbox.rs:2797-2801" },
    { n: 9, title: "Triage: unread, assignee, views", fix: "Bold + dot for unread, view rail with counts, assignee chips, one-key navigation (j/k/Enter).", was: "No read flag in schema, assignee hardcoded None in the list path, no unread/assignee/priority/sort/bulk affordances.", receipt: "inbox.rs:121 + task 15/16 forensic findings" },
    { n: 10, title: "Scope contract", fix: "Presentation only: zero DB/schema/sync/feature changes. Items marked 'data track' (draft persistence, real unread/assignee data) are U2-I1/I2 and intentionally mocked here.", was: "Reskin plan R1-R5 (UX-Recovery & Reskin Plan PDF, Ch. 8-9).", receipt: "docs/audit/generate_ux_report.py" },
  ];

  return { ICONS, NAV_GROUPS, NAV_LABELS, VIEWS, CURRENT_USER, SAVED_REPLIES, conversations, NOTES, PAGE_META, PAGES, PAGE_NOTES };
})();
