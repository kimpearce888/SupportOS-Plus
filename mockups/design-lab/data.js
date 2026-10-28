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

  return { ICONS, NAV_GROUPS, NAV_LABELS, VIEWS, CURRENT_USER, SAVED_REPLIES, conversations, NOTES };
})();
