# SupportOS+ — User Manual

**Version 1.8.0** · For everyday computer users — no technical knowledge required.

SupportOS+ is a desktop program for Linux that connects to your **Help Scout** help desk and puts your whole support operation on your own computer: your conversations, your customers, your help-center articles, your reports — plus optional AI helpers that run on your machine.

Everything you see in this manual was written from the program itself: every window, every field, every button.

---

## Contents

1. [What is SupportOS+?](#1-what-is-supportos)
2. [Before you start — what you need](#2-before-you-start--what-you-need)
3. [Installing SupportOS+](#3-installing-supportos)
4. [The first launch](#4-the-first-launch)
5. [Getting around the app](#5-getting-around-the-app)
6. [Connecting Help Scout (the one required setup)](#6-connecting-help-scout-the-one-required-setup)
7. [Where your data lives, backups, and deleting data](#7-where-your-data-lives-backups-and-deleting-data)
8. [The pages — a complete guide](#8-the-pages--a-complete-guide)
9. [AI features in plain words](#9-ai-features-in-plain-words)
10. [How syncing works](#10-how-syncing-works)
11. [Security and privacy](#11-security-and-privacy)
12. [Troubleshooting — "why doesn't it…?"](#12-troubleshooting--why-doesnt-it)
13. [For advanced users — environment variables](#13-for-advanced-users--environment-variables)
14. [Glossary — words this manual uses](#14-glossary--words-this-manual-uses)
15. [Appendix — every "Are you sure?" dialog in the app](#15-appendix--every-are-you-sure-dialog-in-the-app)

---

## 1. What is SupportOS+?

SupportOS+ is a **single window** that sits on your Linux desktop and works with your **Help Scout** account. Inside you get:

- an **Inbox** for answering customer emails,
- **Live chats** for your website's Beacon chat widget,
- **Customers** and **Organizations** address books,
- a **Knowledge** library (your own documents + your Help Scout Help Center),
- **Reports**, **Issue Radar**, **Incidents** and **SLA** tracking,
- **Automation** rules, **Outreach** campaigns, **Custom Objects** (a mini database),
- an **AI Center** that controls optional AI helpers running on your own computer,
- and **Diagnostics / Sync Health** screens that show exactly what the program is doing.

Three principles the program follows:

1. **Your data stays with you.** Everything is stored in one folder on your computer. The program never sends analytics or "telemetry" anywhere.
2. **The app starts empty.** It has no sample or demo content. After you connect Help Scout, it pulls in **only your real history**.
3. **You stay in charge.** Dangerous actions always ask "Are you sure?" first, and AI features are switched off by default for anything that writes to customers (automatic notes, automatic drafts, automatic sending are all **off** until you turn them on).

SupportOS+ is a **mirror**: most things you see are a local copy of what is in Help Scout. When you reply or change something, the app saves it immediately on your computer and then delivers it to Help Scout in the background (within seconds), showing you a status line so you always know whether it arrived.

---

## 2. Before you start — what you need

| You need | Details |
|---|---|
| A computer with **64-bit Linux** | Debian, Ubuntu, Linux Mint, and similar distributions are supported. The program is built for x86_64 processors (any normal PC or laptop from the last ~15 years). |
| A **Help Scout account with API access** | The API (the connection the app uses) requires a **paid** Help Scout plan. The Free plan has no API access and cannot connect. You sign in at helpscout.com as usual — the app uses its own connection, not your browser. |
| Permission to create a **custom app** in Help Scout | This is how you get the "keys" the app asks for (see [Section 6](#6-connecting-help-scout-the-one-required-setup)). Normally done by an account admin. |
| *(Optional)* **LM Studio** for AI features | A free program that runs AI on your own computer. Without it, SupportOS+ still works perfectly — you simply don't get the AI helpers. See [Section 9](#9-ai-features-in-plain-words). |
| *(Optional)* A small **tunnel tool** for live chats and ratings | Needed only if you want website Beacon chats and customer satisfaction ratings to arrive instantly. See [Section 6.6](#66-live-chats--ratings--the-webhook-relay-optional-but-recommended). |

You do **not** need: Docker, a server, a database program, programming tools, or any paid subscription beyond your Help Scout plan.

---

## 3. Installing SupportOS+

Both installers come from the GitHub releases page of the project:
`https://github.com/kimpearce888/SupportOS-Plus/releases` — look for the newest version (for this manual: **v1.8.0**).

You will see two files:

| File | Best for |
|---|---|
| `supportos-plus_1.8.0_amd64.deb` | Debian, Ubuntu, Linux Mint, Pop!_OS, elementary and other Debian-family systems. Installs like a normal program and adds "SupportOS+" to your applications menu. |
| `SupportOS-Plus-1.8.0-x86_64.AppImage` | Any other Linux. One single file — no installation, just run it. |

### Option A — install the .deb (recommended on Debian/Ubuntu/Mint)

1. Download `supportos-plus_1.8.0_amd64.deb`.
2. Double-click the downloaded file. Your software installer opens (called "Software Install", "Package Installer" or similar). Click **Install** and enter your password if asked.
   - The installer pulls in everything the program needs (its listed system dependencies) automatically — you never have to install anything by hand.
   - Comfortable with the terminal instead? `sudo apt install ./supportos-plus_1.8.0_amd64.deb` does the same.
3. After installing, find **SupportOS+** in your applications menu and click it.

### Option B — run the AppImage (no installation)

1. Download `SupportOS-Plus-1.8.0-x86_64.AppImage` into any folder (for example `Applications` in your home folder).
2. Allow it to run: right-click the file → **Properties** → **Permissions** → switch on **Allow executing file as program** (wording varies slightly by system).
3. Double-click the file. The program starts.

If your Linux cannot run AppImages at all, use the `.deb` instead — both contain the same program.

### Optional: check the download wasn't damaged

Every release includes a `SHA256SUMS.txt` file — a "fingerprint" list. On most systems you can verify it in a terminal with `sha256sum -c SHA256SUMS.txt` in your download folder. If you prefer not to use a terminal, you can skip this step; it is only a double-check.

### Upgrading from an older version

Simply install the new `.deb` over the old one, or run the new AppImage. **Your data is not inside the program** — it lives in a separate folder (see [Section 7](#7-where-your-data-lives-backups-and-deleting-data)), so upgrading never erases your conversations, settings, or backups.

### Uninstalling

Remove the program the normal way (Ubuntu Software → Installed → SupportOS+ → Remove, or your package manager). Removing the program does **not** delete your data folder — delete that too if you want everything gone (see [Section 7](#7-where-your-data-lives-backups-and-deleting-data)).

---

## 4. The first launch

Start SupportOS+ from the menu (or by double-clicking the AppImage). A window titled **SupportOS+** opens, about the size of a small laptop screen (1180 × 760 pixels — you can resize it like any window).

Two things to know on day one:

- **The app starts deliberately empty.** The welcome page says it directly: *"Connect your Help Scout account in Settings to pull your real conversations. The app starts empty: no sample content, no stand-ins — only your real history."*
- **Only one copy can run at a time.** If you start a second copy while one is open, the second one refuses to start with the message *"another SupportOS+ instance is already running"*. This protects your data — close the first window first if you want to restart.

If something prevents the program from starting at all (for example a broken database), it shows a dialog titled **"SupportOS+ failed to start"** with a short reason, and writes the details to a log file (see [Troubleshooting](#12-troubleshooting--why-doesnt-it)).

---

## 5. Getting around the app

### The sidebar (left edge)

A narrow bar with the brand **"SupportOS+"** and its version (for example `v1.8.0`), a search row, and all pages grouped under five headings:

| Group | Pages |
|---|---|
| **Workspace** | Dashboard, Inbox, Live chats, Operations |
| **Customers & knowledge** | Customers, Organizations, Knowledge |
| **Insights** | Reports, Issue Radar, Incidents |
| **Automation & AI** | Custom Objects, Automation, AI Center, Outreach |
| **System** | Notifications, Search, Support Graph, Connectors, Diagnostics, Sync Health, Settings, Onboarding, Shell |

Click a page name to open it. The current page is highlighted.

At the very bottom of the sidebar is a theme row — **"switch to dark theme"** (moon) or **"switch to light theme"** (sun). Your choice is remembered.

### The header (top right)

- A sun/moon button — same theme switch.
- A **"Search  ⌘K"** button — opens the **command palette** (see below).
- A small **freshness pill** showing your sync status, for example:
  - **"syncing…"** (blue) — a sync is running right now.
  - **"never synced"** (grey) — no sync has happened yet.
  - **"synced 45s ago"** (green) — all good; turns **amber** after 30 minutes and **red** after that.
  - Clicking the pill opens the **Sync Health** page.

### The command palette (Ctrl+K)

Press **Ctrl+K** (or click either Search control) and a small search window appears. Type a few letters — the hint reads *"search tickets, customers, knowledge, issues…"* — and it lists up to 12 matches from your conversations, customers, knowledge articles and issues, each labelled with where it comes from (`tickets`, `customers`, `knowledge`, `issues`).

- **↑ / ↓** move the selection, **Enter** (or a click) opens the hit in the right page, **Esc** closes.

### Toast messages (bottom-right corner)

Small cards appear in the bottom-right corner to confirm what happened in the background — for example **"Reply delivered to Help Scout"**, **"Sync finished: 12 record(s) updated"**, or **"Up to date — nothing new from Help Scout"**. They disappear by themselves: success/info after 4 seconds, warnings after 6, errors after 10.

### "Are you sure?" dialogs

Every destructive action (deleting a rule, a document, a segment, clearing logs, disabling AI, discarding campaign sends…) opens a centered confirmation dialog. **Nothing happens until you click the red confirm button.** Cancel, the X, or Escape always leaves everything untouched. All confirmation texts are listed in the [Appendix](#15-appendix--every-are-you-sure-dialog-in-the-app).

### Keyboard shortcuts (the complete list)

| Keys | Where | What it does |
|---|---|---|
| **Ctrl+K** (⌘K on Mac keyboards) | anywhere | open/close the command palette |
| **Esc** | palette / confirm dialog | close / cancel |
| **↑ / ↓ + Enter** | palette | pick and open a result |
| **Ctrl+Enter** | Inbox composer | send the reply or note |
| **@** then type | Inbox composer | opens the **@ mentions** popup; **↑/↓** choose, **Enter/Tab** or click completes |

---

## 6. Connecting Help Scout (the one required setup)

This is the only setup the app truly needs. Do it once and everything else fills in automatically. The whole flow lives on **Settings → Help Scout**.

### 6.1 Get your two "keys" from Help Scout

SupportOS+ talks to Help Scout using a **custom app** — Help Scout's name for a personal API key pair.

1. Sign in to **helpscout.com** in your browser.
2. Click your profile picture (top right) → **Apps & Integrations** (in newer plans: **Your Profile → Apps & Integrations**).
3. Find **Custom Apps** and create one. Help Scout shows you two values:
   - **Client ID** — a long public identifier,
   - **Client Secret** — a long private password. Treat it like a password: don't email it, don't share it.
4. Remember: the **Free plan has no API access** — creating a working custom app needs a paid plan.

> **What about the "redirect URI"?** The Settings page shows one (by default `http://localhost:3000/oauth/callback`). **You can ignore it.** It is only needed for the optional browser authorization flow (Section 6.5). The recommended connection method needs no browser at all.

### 6.2 Enter the credentials in SupportOS+

Open **Settings → Help Scout** and find the card **"Help Scout credentials"** (its subtitle: *"Stored encrypted on this machine. Save them and the app connects immediately…"*). Four fields:

| Field on screen | What to type | Where the value comes from |
|---|---|---|
| **client id:** | Your Help Scout custom app **Client ID** | Help Scout → Profile → Apps & Integrations → your custom app (Section 6.1) |
| **client secret:** | Your custom app **Client Secret** | Same place. Shown as dots while typing. Leave blank to keep a value you saved earlier. |
| **webhook secret:** | The webhook secret Help Scout generates when a webhook is created | See Section 6.6 — you can also save credentials now and add this later. Shown as dots. |
| **docs api key:** | A **Docs API key** — only needed for syncing your Help Scout **Help Center** articles into the Knowledge page | Help Scout → your Profile → **Security & Access → Docs API** → create a key. Optional; without it everything else still works. |

Tick **"show secrets"** if you want to check what you typed. Next to each field the app shows where the value came from: **"not set"**, **"environment"** (set by an advanced environment variable — wins over what you type here), or **"app settings"**.

Press **"save credentials"**. Three things happen immediately, with no restart:

1. The values are stored **encrypted** on your computer.
2. The **Connection Doctor** runs automatically and shows five check results (Section 6.7).
3. The **first sync starts by itself** — the app pulls your Help Scout history (on big accounts this can take an hour or more; you can keep working, and **Sync Health** shows progress).

### 6.3 Connect with client credentials (recommended)

If credentials are saved but the status badge still says **"credentials saved"**, press the primary button **"connect with client credentials (recommended)"**. This verifies your id/secret directly with Help Scout and stores the access token — **no browser, no redirect URL**. Success message: *"connected with client credentials — tokens stored. The sync pulls your account automatically."* The status badge changes to **"connected"**.

> If the button is missing, the connection is already live — the badge **"connected"** (green) means you are done.

### 6.4 The status badges

- **"connected"** (green) — a live access token exists. Everything works.
- **"credentials saved"** — values are stored but not yet verified; press the connect button.
- **"not configured"** — nothing saved yet.

The button **"disconnect"** (only shown while connected) removes the stored tokens; message: *"Disconnected. Local data is fully preserved."* Your local history stays and can still be browsed offline.

### 6.5 Optional: the browser authorization flow

Most users never need this. If your setup requires it, expand **"advanced: browser authorization flow"** on the same card, register the displayed **redirect uri** as the callback URL of your custom app in Help Scout **exactly, character for character**, then press **"open the authorization page in the browser"**, approve in Help Scout, and leave SupportOS+ running — it catches the redirect itself and stores the token. If this flow fails, use the client-credentials button instead; the app's own hint says exactly that.

### 6.6 Live chats & ratings — the webhook relay (optional, but recommended)

Some things cannot be "pulled" from Help Scout at all — they are **pushed**:

- **Beacon live chats** (Help Scout has no list endpoint for chats — the webhook *is* the data path),
- **customer satisfaction ratings**,
- and instant notifications for nearly everything else (without them, the app still polls every 30 s–1 min, so email conversations still arrive, just a bit less instantly).

For pushes to reach your desk, Help Scout needs a publicly reachable address that forwards to your computer. SupportOS+ listens **only on your own machine** (127.0.0.1) for safety — so you put a tiny "tunnel" in the middle. On the same Settings tab, the card **"Webhook relay & health"** explains it and gives you everything:

1. Start a tunnel from a terminal (copy one of these lines from the app's own hint, replacing `PORT` with the port shown at the top of the Help Scout tab):
   - `cloudflared tunnel --url http://127.0.0.1:PORT`
   - `ngrok http 127.0.0.1:PORT`
   - `ssh -N -R 80:127.0.0.1:PORT serveo.net`
2. The tool prints a public URL (e.g. `https://something.trycloudflare.com`). Type it into **"public relay URL:"** and press **"save relay URL"**. The app answers: *"relay URL saved — the next health check registers the hook there"* — it creates/updates the webhook on Help Scout for you.
3. Press **"check now"** any time to test. A badge shows the result: **"healthy"** (green), **"reregistered"**, **"failed"**, or **"not configured"**. The app also re-checks by itself about every 6 hours and re-registers the hook if Help Scout silently dropped it.
4. *(Optional, for the curious)*: you can prove the chain from outside — the app's hint suggests `curl -X POST <relay>/webhook -d '{}'`, which must answer **401** (that's the signature check working: strangers get rejected).

**The webhook secret** (field 6.2): create it in Help Scout's webhook settings when you add a webhook; the app verifies every incoming message against it. Note the small print shown after saving: Help Scout caps webhook secrets at **40 characters** — if yours is longer, only the first 40 are used.

### 6.7 The Connection Doctor — the five checks

On **Settings → Help Scout**, the card **"Connection Doctor"** runs five checks in order and shows `✓ ok`, `! warning` or `✗ failed` for each, in plain words:

1. **"reach the Help Scout API"** — is helpscout.net reachable from your machine?
2. **"sign in with client credentials"** — are your Client ID/Secret accepted?
3. **"read the connected user"** — is the account user allowed to use the API?
4. **"list mailboxes"** — can the app see your mailboxes?
5. **"rate-limit budget"** — how many requests per minute does your account allow?

Each failure comes with a remedy. The two most common:

- *Sign-in failed* → check that Client ID and Secret match exactly (no stray spaces) and the app is enabled; and remember the **Free plan has no API access — custom apps need a paid plan**.
- *User read failed* → the user may be a **Light User** (no API access) or the app's scopes are wrong — reconnect with a full user's credentials.

Press **"run the checks"** any time; it also runs automatically after every credential save.

### 6.8 Clearing stored credentials

The button **"clear stored"** removes all four stored values (tokens are untouched) — the app keeps working from its local mirror. Useful when you want to switch accounts.

---

## 7. Where your data lives, backups, and deleting data

### The data folder

All your data lives in **one folder**:

```
~/.local/share/supportos-plus
```

(`~` means your home folder; if your system uses a custom data location, the Settings → Vector index tab shows the real path of the store next to the database.)

Inside that folder:

| Item | What it is |
|---|---|
| `supportos-plus.db` | The database — every conversation, customer, article, report, setting. |
| `secrets.key` | The encryption key for stored credentials. **Do not delete it** — without it, saved passwords cannot be read back (see [Troubleshooting](#12-troubleshooting--why-doesnt-it)). |
| `vectors/vectors.db` | The AI search index (only used by optional AI features). |
| `backups/` | Automatic and manual backups (Section 7.1). |
| `attachments/` | Files downloaded from conversations. |
| `knowledge-import/` | The drop-box for importing your own documents (Section 8.7). |
| `logs/fatal.log` | Written only if the program ever crashes or fails to start. |
| `instance.lock` | The "already running" lock — never edit. |

### 7.1 Backups — Settings → Backups & export

- **Automatic**: the app saves a full backup every **24 hours** and keeps the **newest 20**. No action needed from you.
- **Manual backup**: press **"create backup"** — a message shows the file path, e.g. `backup /home/you/.local/share/supportos-plus/backups/supportos-backup-2026-10-27-….db`.
- **Restore**: every backup in the list has a **"restore"** button. Restoring replaces your current database with the backup — the app asks twice: first the button, then a red confirmation: *"Restore <path>? The current database will be REPLACED by this backup (verified first). Restart SupportOS+ afterwards to use it."* Press **"really restore"** only if you are sure, then restart the app. Backups are verified for integrity before anything is touched, and a broken/foreign backup file is refused.
- **Exports**: **"export JSON"** (all data) and **"export conversations CSV"** (a spreadsheet) write files that you can open elsewhere; the message shows the path.
- Backup files are permission-protected (readable only by you) and their sidecar settings file never contains passwords or tokens.

### 7.2 Moving to another computer

Install SupportOS+ on the new machine, copy the whole data folder (see above) into the same place, and start the app. Everything — database, settings, credentials (with `secrets.key`) — comes along.

### 7.3 Deleting your workspace data

On **Settings → Help Scout**, the card **"Workspace data"** has a red **"delete all workspace data"** button. It deletes every conversation, customer, thread, rating, knowledge document, incident, report and AI record — and arms a fresh first sync. It **keeps**: credentials, settings, business hours, automation rules, saved views and replies, custom object types. It asks twice (the second button reads **"really delete everything"**), then reports how many rows it removed and tells you to restart the app.

## 8. The pages — a complete guide

Every page, in sidebar order. Each entry starts with what the page is *for*, then how to use it, then every field and button.

---

### 8.1 Dashboard

**What it's for** — your support numbers at a glance: how much work arrived, how fast the team answers, how happy customers are. A subtitle states the promise: *"Local metrics · sources labeled Help Scout / local / AI-derived everywhere."*

**How to use it**

- Top-right: **"Sync now"** pulls fresh data from Help Scout immediately (a small note shows "syncing…", "a sync is already running", or "re-read the local mirror (no engine)").
- Three filter rows of clickable chips narrow every number on the page:
  - **Range**: "7 days", "30 days" *(default)*, "90 days", "1 year".
  - **Channel**: "All channels" *(default)*, "Email", "Chat (Beacon)".
  - **Mailboxes**: "All" plus one chip per mailbox — click several to combine; "All" clears the selection.
- Everything below updates instantly.

**The 12 metric cards** — each shows one number:

| Card | Meaning |
|---|---|
| **New** | conversations created in the selected range |
| **Active** | conversations currently waiting on your team |
| **Pending** | conversations currently waiting on the customer |
| **Closed** | conversations closed during the range |
| **Unassigned** | active/pending conversations with no assignee |
| **Backlog (7d stale)** | active/pending conversations untouched for 7+ days |
| **Replies sent** | replies your team published in the range |
| **First response avg** | average minutes until the first reply ("min") |
| **Great ratings / Okay ratings / Not-good ratings** | the three Help Scout satisfaction ratings received in the range |
| **Resolution avg** | average minutes from first message to close ("min") |

Below the cards: a **"New conversations per day"** line chart, breakdown cards **"By mailbox"**, **"By tag"** (top 12 tags), **"By channel"**, **"By agent"**, **"By team"**, a **"Channel metrics"** table (channel, count, first resp, resolution), a **"Mailbox comparison"** table, and an **"Issue radar"** panel (short preview of the Issue Radar page, with red **"high"** / amber **"medium"** alerts).

**Common questions**

- *Nothing shows / cards say 0* — either no data is in range, Help Scout isn't connected yet, or the first sync hasn't finished. Empty chart text: *"no conversations in range / Nothing was created in the selected period."*
- *Can I change a number here?* No — this page only reads. The single button is "Sync now".

---

### 8.2 Inbox

**What it's for** — the main work area: read customer conversations, reply, add private notes, change status, assign teammates. Everything you send is delivered to Help Scout in the background, with a status line telling you exactly where it is.

**The list (left side)**

- Filter chips: **"All"** *(default)*, **"Active"**, **"Pending"**, **"Closed"**.
- Dropdowns: **mailbox** ("All mailboxes" + your mailbox names), **channel** ("All channels" / "Email" / "Chat"), **tag** ("Any tag" + tag names) — take effect when you press **"apply"**.
- Search row: a text box (*"search conversations…"*) and a number box (**"#778"** or just "778") — press **"search"**.
- Header buttons: **"reset filters"** clears everything; **"Sync now"** pulls new/updated conversations (automatic syncing also runs — every 30 s while busy, backing off when quiet).
- The list shows 50 conversations per page with **"‹ prev / page n of m / next ›"** buttons. Each row: `#number subject`, a status pill (**Active** blue, **Pending** amber, **Closed** grey, **Spam** red), customer · mailbox, a message preview and the time.

**Reading a conversation (right side)**

- Title `#{number} {subject}` + status badge; below: customer name \<email\> · mailbox · priority.
- Refresh icon — *"refresh this conversation from Help Scout"*.
- Sparkles icon — opens the **Copilot** panel (see below).
- **Triage chips** (if AI analysis is on): small labelled chips such as **Intent**, **Urgency**, **Frustration cues**, **Escalation signal**, **Issue cluster**. Hover any chip to see its source and confidence; **click a chip to correct it** — a row appears with buttons like `bug_report`, `billing`, `high`… plus **"unknown (clear)"** and **"cancel"**. Your correction is stored as human-confirmed.
- **"Know before you reply"** card: up to three durable facts the AI extracted about this customer from past conversations.

**Acting on it — the action card**

- **status:** four chips — `active`, `pending`, `closed`, `spam`. Clicking applies the status locally and queues it for Help Scout (green line: *"... — sending to Help Scout"*). There is no separate "close" button — `closed` *is* closing.
- **assign:** pick a person (or "unassigned") in the dropdown, press **"assign"**. Team assignments show as `team: {name}`.

**Replying — the composer**

- Two mode chips: **"Reply"** (to the customer) and **"Internal note"** (team-only). The note box hints: *"Type an internal note (visible only to your team) — @ to mention (Ctrl+Enter sends)"*.
- Reply mode adds **"Cc (comma-separated emails)"** and **"Bcc (comma-separated emails)"** boxes.
- **@ mentions**: type `@` and a few letters → a popup headed **"@ mentions"** lists people and teams (up to 8). **↑/↓** to choose, **Enter/Tab** or click to insert. Mentioning a teammate in a note creates a notification for them.
- **send** button or **Ctrl+Enter**. Guards, in order — your text is never lost:
  - *"Composer is empty."*
  - *"AI evaluation mode is ON: no notes, replies or status changes are sent."* (a special testing switch; see [Troubleshooting](#12-troubleshooting--why-doesnt-it))
  - *"Help Scout is not connected. Remote actions are disabled - connect Help Scout in Settings first."*
  - *"This exact reply was already sent (duplicate-send protection)."*
- On success: *"Reply accepted — delivering to Help Scout (watch the status line)"* (or *"Internal note added — posting to Help Scout…"*). The status line below the composer then tracks the delivery: **"Reply: queued for delivery…" → "Reply: sending to Help Scout…" → "Reply delivered to Help Scout ✓"** — or **"Reply failed: …"** in red. A background toast confirms too.
- After a reply, the conversation's status follows the **"status after reply:"** setting (default: **pending** — waiting for the customer; you can choose "open (await agent)" or "keep current status" in Settings → Sync & AI).
- If you switch conversations with unsent text, the draft is **stashed for that conversation** and restored when you come back.

**AI helpers in the composer (all optional, need AI set up — Section 9)**

- **"Review draft"** — runs 9 advisory coaching checks on your text; results appear in a **"Draft coaching"** card (flagged/pass rows; subtitle: *"advisory — never blocks sending"*).
- **"Shorten" / "Expand" / "Warmer"** — rewrite your text with the configured AI model. The result replaces your text **only if you haven't typed while it ran** (*"…your text was kept."* otherwise).
- **"Translate"** — translate your text into one of 15 languages (a dropdown next to the chip; Spanish is the default target).
- **Copilot panel** (sparkles icon): a local summary of what the customer needs, open questions, history, **"Similar past issues"** with match scores and how they were resolved, relevant help articles, and a **"Suggested reply"** with **"Insert into reply"**. With a chat model configured you also get **"Draft with LM Studio"**, **"Insert LM Studio draft"** and **"Discard draft"** (recording feedback so the AI improves).
- **"Ask the assistant"** — a chat with an assistant that can look things up across your workspace (22 read-only tools). Starter chips pre-fill good questions, e.g. **"What is this customer asking?"** or **"What should I check before replying?"**.

**Post-resolution QA** — appears for closed conversations: an honest scorecard ("Back-and-forth messages", "Repeated information", "Messages after close", "Handoffs", "First response (min)", "Resolution (min)") plus an optional **"Run AI review"**.

**Common questions**

- *Nothing is in the list* — if Help Scout isn't connected the page says so: *"Help Scout is not connected — nothing can sync yet. Add your app credentials in Settings (Help Scout tab), press connect, and this inbox fills automatically."*
- *Where did Operations tiles go?* If you arrived from an Operations tile, a banner **"Operations drill-down: …"** filters the list — click its X to clear it.
- *Can I merge or snooze conversations?* Not in this app — status changes (including `closed` and `spam`) and assignment are the tools available.

---

### 8.3 Live chats

**What it's for** — answering website **Beacon** chats while they are open. Chats are not fetched from a list; they appear here the moment Help Scout pushes a webhook event (the subtitle explains: *"Beacon chats arrive through the five beacon.chat.* webhook events — Help Scout has no list endpoint, the webhook IS the data path."*). That's why the [webhook relay](#66-live-chats--ratings--the-webhook-relay-optional-but-recommended) matters.

**How to use it**

- Chips **"open chats"** *(default)* and **"show ended chats"**.
- Click a chat in the left list. The right card, **"Chat with {name or 'a visitor'}"**, shows the message history (who said what, when) and a state badge: **"open"** (green) or **"ended"** (grey).
- Type in **"reply to the visitor…"** and press **"send"**.
  - If no agent is assigned yet, the page warns: *"no agent is assigned — sending accepts this chat automatically"* — your first message claims the chat.
  - If another agent holds it, sending is blocked: *"held by {name} — only the assigned agent can message; Help Scout answers 400"*.
  - After sending: *"message queued for delivery … the platform's agent.replied echo lands it in the mirror"* — your message appears in the history a moment later.
- When a chat ends it becomes a normal conversation; the page tells you which: *"this chat ended — it became conversation #{number}"*.

**Common questions**

- *"No live chats yet" / "Chats appear here the moment the beacon.chat.created webhook lands — connect the webhook relay (Settings) and they will arrive on their own."* — set up Section 6.6; without it, chats cannot arrive at all.
- *Can I start a chat from here?* No — chats begin on your website's Beacon widget.

---

### 8.4 Operations

**What it's for** — the manager's overview. Subtitle: *"16 tiles cover the team's current workload, SLA risk, and system health. Click a tile to filter the inbox."* Three tabs: **"Tiles"**, **"Workload"**, **"Library"**.

**Tiles tab** — 16 tiles, each a count in a severity colour (red = critical, amber = warning, blue = info):

| Tile | What it counts | Clickable? |
|---|---|---|
| Unassigned | active conversations with no agent | ✔ opens Inbox |
| Needs first response | no agent has replied yet | ✔ |
| Customer waiting | customer sent the last message | ✔ |
| Waiting over threshold | customer waiting ≥ the threshold (4 hours by default) | ✔ |
| Urgent | conversations marked urgent | ✔ |
| SLA at risk | at risk of breaching their SLA | — |
| SLA breached | SLA already breached (the only red/critical tile) | — |
| High effort | elevated customer-effort signals | ✔ |
| Repeated issue | linked to a recurring issue | ✔ |
| Known issue | matched to a known issue | ✔ |
| AI escalation | escalated by the AI | — |
| Issue spike | clustered around a spiking issue | — |
| Automation approvals | rules awaiting your review | — |
| Failed jobs | background jobs that ran out of retries | — |
| Sync problems | sync runs that failed | — |
| Campaign activity | active outreach campaigns | — |

The hint above the tiles shows when the snapshot was generated and the waiting threshold in minutes.

**Workload tab**

- **"Team Workload"** table — per agent: **Agent / Open / Waiting / Capacity / Load / Pressure** and **+/−** buttons. The +/− buttons change that agent's capacity by 1 (between 1 and 500) and save immediately. (Note: in this version the percentage sits under the "Load" header and the weighted number under "Pressure" — the headers are swapped relative to the numbers; the values themselves are correct.)
- **"Suggested Assignees"** — for unassigned conversations, the agents with the lowest load are suggested.

**Library tab** — per mailbox:

- **Saved replies**: press **"new saved reply"**, fill **name** (*"name (required by the platform)"*) and **text (HTML)**, press **"save to Help Scout"** — it's written to Help Scout and appears when the sync echoes back. **"edit"** and **"delete"** work the same way; deleting asks *"Delete saved reply?"* first.
- **Workflows**: mirrored from Help Scout; a **"run"** button appears for manual workflows — type the **"conversation #:"** (e.g. `778`) first. The platform validates the run; rejections show up in Diagnostics.

---

### 8.5 Customers

**What it's for** — the customer directory and profile pages.

- **Search**: box *"Search customers by name or email…"* + **"search"** (or press Enter). An empty search lists everyone. 50 per page with prev/next.
- Results table: **name, email, conversations, last contact** + **"open"** per row.
- **Profile page** (after "open"): contact details at the top (name, email, phone, company, last activity); then:
  - **"Support health"** — open conversations, not-good ratings (90 days), SLA breaches.
  - **"Event mix"** — bars showing what kinds of events this customer generated.
  - **"Interaction profile"** (marked **AI**) — how this customer communicates, with a badge showing whether it came from rules (**"heuristic"**) or AI; below it a **"Support approach"** block recommending how to handle them.
  - **"Timeline"** — up to 100 recent events; **"Support milestones"** — SLA breaches, high-effort moments; **"Conversations"** — their 20 most recent tickets with status badges.
- **"← back to search"** returns to your results.
- **"Sync now"** (top right) pulls fresh customer data.

**Common questions**

- *Can I edit a customer here?* No — the profile is read-only; edits happen in Help Scout and sync back.
- *Where is the do-not-contact list?* On the **Outreach** page, tab **"Do not contact"** (Section 8.14).

---

### 8.6 Organizations

**What it's for** — grouping customers by company (matched by email domain). Subtitle: *"customer organizations, their domains, and their members"*.

- **Filter by name…** box + **"filter"** button.
- Click an organization chip (`{name} · {n} customers · {m} conversations`) to see its domains and members on the right.
- **"New organization"**: type a **Name** and **domains** (placeholder *"acme.com, acme.io"* — comma-separated), press **"create organization"**. Without a name you get *"name required"*.
- **"Sync now"** pulls fresh organization data.

---

### 8.7 Knowledge

**What it's for** — the help desk's memory: your own reference documents plus your Help Scout **Help Center**, used by search and the AI. Five tabs: **"Documents"**, **"Sources"**, **"Freshness"**, **"Help center"**, **"Article drafts"**.

**Documents tab** — three ways to add documents:

1. **Import files**: open the section **"Import documents"**. The app watches a folder called `knowledge-import` inside its data folder (the page prints the exact path). Copy files there — supported types: **.md .txt .csv .json .html .pdf .docx** — then press **"Refresh"** and **"Import"** beside each file. (For safety, files can only be imported from that folder.)
2. **Paste a document**: fields **"Title"**, **"Visibility"** (**"internal-only (never in customer drafts)"** — default — or **"customer-safe (may support customer drafts)"**), **"Source name"** (optional; defaults to "Manual import"), and **"Content (Markdown)"**. Press **"Import document"**.
3. **Help Center articles**: sync your Help Scout Help Center (see below).

Every document is split into searchable chunks (the table shows the count). Row buttons: **"reviewed"** and **"verify"** mark the document as checked by a human; **"delete"** asks *"Delete knowledge document?"* and removes it and its search chunks. There is no editing of an existing document — re-import a corrected copy instead.

**Sources tab** — where documents came from (name, kind, visibility, count, created).

**Freshness tab** — documents grouped by review state: **stale**, **unreviewed_long**, **unverified**, **low_usage** — so you know what to re-check.

**Help center tab**

- Press **"Sync Help Center now"** (top right) to pull collections and articles from Help Scout's Docs API.
- Metric cards: **"collections"**, **"articles"** (published/draft/internal), **"views"**.
- Filter by **"collection:"** and **"status:"** ("Any"/"published"/"draft"/"internal") + **"apply"**; search box *"Search help center…"* + **"search"**. Rows have a **"read"** button that opens the full article.
- A **"Knowledge gaps (tracked)"** card lists searches that returned **no hits** — evidence of what your documentation is missing.
- If no Docs key is set, the page tells you exactly what to do: *"Help Center sync needs a Docs API key (Help Scout Profile → Security & Access → Docs API). Add it in Settings → Help Scout, then press 'Sync Help Center now'."*

**Article drafts tab** — the AI-article pipeline, with a human gate:

1. A tracked gap appears in the dropdown **"Select a gap…"** → press **"Draft article"**. The AI writes an article grounded in your best real resolutions, **with citations**.
2. The draft lands here as **"awaiting review"**. Open it, read it (citations listed with excerpts).
3. Press **"Approve"** (message: *"Draft approved. Publishing needs the explicit publish action below."*) or **"Reject"**.
4. Only after approving does **"Publish to Help Scout (Docs API)"** appear: pick a collection and press **"Publish to Help Center"** — the app uploads, publishes, and re-syncs to verify. Statuses you'll see: **"published + verified"**, **"published"**, **"draft in Help Scout"**, or red **"publish failed"**.

**Nothing ever publishes without a human pressing those buttons.**

---

### 8.8 Reports

**What it's for** — the scoreboard. Subtitle: *"local analytics over the selected range"*. A global range row (**"7 days" / "30 days" *(default)* / "90 days" / "1 year"**) and **"Sync now"** apply to every tab. Eleven tabs:

| Tab | What you get |
|---|---|
| **Overview** | 8 metric cards (New, Closed, Replies sent, Backlog, the three ratings, Unassigned) + a "New conversations per day" line chart. |
| **SLA** | A bar chart of first-response averages per mailbox and a table (in range, met/missed, averages, oldest waiting). Warning card lists mailboxes without configured business hours. |
| **Why contacting** | Counts of why customers wrote in. |
| **Top questions** | The questions customers ask most. |
| **Quality** | Post-resolution QA aggregates plus the 25 most recent closures with their scores. |
| **Issue radar** | The AI alert list (same data as the Issue Radar page). |
| **Digest** | AI-written weekly summaries. Press **"Generate digest now"** or wait for the scheduled one. Each digest states honestly whether it has a narrative (**"facts + narrative"**, **"facts only (no model routed)"**, **"model failed (facts kept)"**) — the numbers are always real SQL facts. Search box filters the archive. |
| **Happiness** | Great/Okay/Not-good counts, a CSAT percentage, and the ratings table. An honesty note explains ratings arrive via the satisfaction webhook, so *"an empty table means none arrived, not zero happened."* |
| **Help Scout** | Fetch **32 official Help Scout reports** via the Reports API: pick one in the dropdown, press **"Fetch from Help Scout"**. A clear banner tells you if your plan doesn't include it (Plus/Pro plans only) — a 403 answer is recorded and reported, not hidden. |
| **Builder** | Your own reports (below). |
| **Insights** | Health facts, QA effectiveness, customer-effort ("friction") scores with a **"Recompute (last 100 closed)"** button, and **Graph nodes** you can add for the Support Graph. |

**Builder** — custom reports in four steps:

1. Pick a **"metric:"** (21 choices, e.g. "Conversations", "Avg first response time (minutes)", "Campaign reply rate") and **"grouped by:"** (14 choices: "No grouping (total)", "Day", "Week", "Month", "Mailbox", "Channel", "Tag", "Assignee", "Team", "Status", "Priority", "Custom state", "Response state", "Issue").
2. Optionally tick **"compare with previous period"**, choose a **"sort:"** chip, and set **"from (YYYY-MM-DD):"**, **"to:"**, **"limit:"**.
3. Optionally fill any of the ten filter boxes (**channel**, **statuses (csv)**, **tags any (csv)**, **tags none (csv)**, **min priority**, **attribute key/value**, **state key**, **mailbox ids (csv)**, **assignee ids (csv)**) — blank means unfiltered; csv means comma-separated.
4. Press **"Run report"**. The result is a table (group, value, Δ vs previous). Save configurations with a name via **"Save"** (+ the **"save as:"** box); **"Saved reports"** lists them with **"load"** and **"delete"** (delete asks first).

*Note:* there is no CSV export button on this page — use Settings → Backups & export → "export conversations CSV" for spreadsheet data.

---

### 8.9 Issue Radar

**What it's for** — spotting problems that repeat across many conversations. Subtitle: *"AI-derived alerts, clusters, and known issues"*. Four tabs: **"Alerts"**, **"Clusters"**, **"Gaps"**, **"Known issues"**.

- Header buttons: **"Cluster now"** (groups the last 14 days of conversations into clusters — needs the Classify AI role), **"Reload clusters"**, **"Detect gaps now"** (finds "asked often, documented nowhere" patterns from the last 90 days — needs no AI model).
- **Alerts**: rows like `"Rising volume: Refund delays · 18 conversations"` with red **critical** / amber badges, plus an SLA card ("SLA: {n} breached · {n} at risk") listing the conversations.
- **Clusters**: cards with trend badges (**"rising"**, **"new"**) and the count of conversations/customers affected.
- **Gaps**: candidates with occurrence badges (`12×`), an explanation, and up to 5 example conversation numbers. **"approve"** or **"reject"** each candidate — approving only *marks* it (and decisions survive later rebuilds); approved gaps feed the Knowledge page's article drafting.
- **Known issues**: press **"New known issue"** to record a recurring problem: **title**, **status** (`investigating` / `identified` / `fix_in_progress` / `resolved`), **symptoms**, **workaround**, and optional **conversation ids** (comma-separated local conversation numbers — the `#` numbers from the Inbox; invalid ones are skipped and reported).

---

### 8.10 Incidents

**What it's for** — one big problem affecting many customers (an outage, a bad release) tracked as one item instead of 200 tickets.

- Press **"New Incident"**, fill **title**, **status** (`investigating` / `identified` / `fix_in_progress` / `monitoring` / `resolved`), **severity** (`sev1` most severe … `sev4`), **description**, and optionally link **conversation ids** (comma-separated). **"create incident"** assigns a code like `INC-003`.
- Filter the list with **status:**, **severity:**, **"open only"**, and the search box + **"apply"**.
- In the detail workspace: **"link conversation #: {id}" + "link"** to attach more conversations; **"add note"** keeps a timestamped history; cards show **Linked conversations**, **Affected customers**, **Notes**, and **Releases** when present.
- To resolve an incident, create it with status `resolved` (which stamps the resolution time) — the current version has no in-place status change button.

---

### 8.11 Custom Objects

**What it's for** — a mini database for anything the app doesn't track by itself (warranty claims, subscriptions, returned devices…). A **type** is the blank form you design; an **object** is one filled-in form.

- Press **"New Type"**: give a **name**, a **description**, and add fields with **"+ add field"**. Each field has a **key** (machine name), a **label** (shown name), a **type** — one of `text`, `long_text`, `number`, `date`, `select`, `boolean` — a **"required"** checkbox, and for `select` a comma-separated options list. Press **"create type"**.
- Select the type on the left, then fill the **"New object"** card: a **title** plus one box per field (numbers must be numbers; booleans accept true/false, yes/no, 1/0, on/off). Empty boxes are skipped. Press **"create object"**.
- **"search objects…"** + **"search"** filters the list (up to 50 shown).
- Deleting: objects and whole types have **"delete"** buttons with confirmation ("Delete object?" / "Delete custom type?" — deleting a type removes every record under it).
- This version creates and deletes only — there is no edit button for existing objects or types.

---

### 8.12 Automation

**What it's for** — if-this-then-that rules for conversations. Subtitle: *"Rules trigger actions when conversations match certain conditions. Actions that need review park in the approvals queue."*

**Creating a rule** — press **"New Rule"**:

1. **name:**
2. **trigger:** three chips — **"Status changed"** (with optional **from → to** status boxes; blank = any), **"Tag added"** (a **tag:** box), or **"SLA at risk"** (no parameters).
3. **action:** five chips — **"Assign"** (an **assignee id:** — the agent's numeric user id), **"Add tag"** (a **tag:**), **"Send note"** (a **note body:**), **"Change status"** (a **status:**), **"Set priority"** (a **priority:**).
4. **"Create Rule"** — the rule is created **enabled**.

A warning line reminds you: *"enabled rules act on conversations automatically — actions that need review park in the approvals queue."*

**Safety valves** — Assign and Change status actions **always** need approval; Set priority needs it when the priority is `urgent`; Add tag and Send note never do. Riskier actions therefore wait in the **"Pending approvals"** card, where each item shows the rule, the conversation, and the proposed action, with **"approve"** / **"reject"** buttons.

**Managing rules** — the table lists each rule with its **"when"** and **"then"** in plain words (e.g. `status changed from any to resolved` → `add tag 'done'`). Click the **enabled/disabled** chip to switch a rule on/off instantly; **"delete"** asks *"Delete automation rule?"* first. There is no edit button — delete and re-create instead.

---

### 8.13 AI Center

**What it's for** — the control room for everything AI. Six tabs: **"Health"**, **"Analytics"**, **"Attributes"**, **"Copilot"**, **"Usage"**, **"Jobs"**.

**Health tab**

- **"LM Studio settings"** card — a read-only snapshot: AI enabled?, provider, base url (default `http://127.0.0.1:1234`), chat/embedding models, timeout, concurrency. The hint: *"Start LM Studio locally (default http://127.0.0.1:1234) and configure the models in Settings → LM Studio."* Press **"test connection"** to see `testing…` → `connected · {n} model(s)` (green) or `offline · {error}` (red).
- **"AI provider record (ai_settings)"** — a fallback record (used when the Settings → LM Studio fields are empty): pick provider **"None"** / **"LM Studio"**, optionally set chat model, embedding model, embedding dim, base url, then **"Save provider record"**.
- **"Provider hub — endpoints"** — where AI endpoints live. Two are built in: **"LM Studio (local)"** (`http://127.0.0.1:1234/v1`) and **"Ollama (local, optional)"** (`http://127.0.0.1:11434/v1`). You can add cloud endpoints (bring-your-own-key, off by default) in the collapsible **"add endpoint (cloud BYOK — off by default per-feature)"** section: name, base url, api key (password box), and optional price-per-million-tokens (feeds the cost estimate). Per endpoint:
  - **"probe"** — lists the models once; shows a green `reachable — {n} model(s) · {ms} ms` or red `offline — …` light.
  - **"set key"** — stores an API key *"in the encrypted envelope — never in plaintext"* (an empty save deletes it).
  - **"remove"** — only for endpoints you added yourself.
- **"Model-role routing"** — the heart of the AI setup. Every AI task goes through one of five roles; assign an endpoint and a model name to each:

  | Role | What it does |
  |---|---|
  | `embed` | search embeddings — local by default; cloud is an explicit opt-in |
  | `classify` | triage, sentiment, topic, urgency — high volume, low stakes |
  | `summarize` | thread summaries, memory extraction, notes, digest |
  | `draft` | customer-facing drafts — the only role where cloud is recommended |
  | `agent` | the tool-calling copilot chat |

  Changing any routing automatically re-runs the "golden set" evaluation (a built-in quality test).
- **"Features & kill switches"** — 16 individual feature checkboxes (ticket_analysis, ai_note, draft, attributes, memory_extraction, issue_clustering, copilot, draft_coaching, translation, post_resolution_qa, interaction_intelligence, retrieval, golden_eval, gap_detection, article_drafting, weekly_digest). Each click saves immediately. Above them, the big switch: when AI is on, **"global kill switch: AI is ON — click to disable all AI"** asks *"Disable all AI?"* and turns everything off at once; when off, one click re-enables.
- **"Cost ledger — last 7 days"** — tokens, latency and estimated spend per feature per day (only meaningful for priced cloud endpoints; local LM Studio costs $0).

**Analytics tab** — read-only quality numbers: tickets analyzed, analysis success rate, drafts accepted/rejected, draft edit rate, verification warnings, unsupported claim rate, and the top **"Common failure patterns"**.

**Attributes tab** — the 14 customer-attribute categories the AI fills (Intent, Product, Feature, Issue type, Urgency, Frustration cues, Technical familiarity, Customer goal, Question count, Risk, Known issue, Issue cluster, Response style, Escalation signal) with their value distributions. The card's promise: *"A missing attribute reads as unknown — it is never fabricated. Attributes are local-only and never written to Help Scout."*

**Copilot tab** — chat with a read-only assistant that searches your whole workspace and cites its sources (up to six green source badges per answer). **"New chat"** starts a session; saved sessions appear as chips. Two honest gates if it can't run: *"Copilot is switched off"* (enable in Features) or *"No Agent role endpoint routed"* (assign one in Model-role routing).

**Usage tab** — per-feature token/cost tables for the last 30 days, **"Draft feedback (edit rate)"** (how often agents accepted/edited/rejected AI drafts), and **"Golden-set evals (9 seeded cases)"** with a **"Run golden set now"** button and pass/fail results.

**Jobs tab** — the last 100 AI runs with type, status (completed green / failed red / queued neutral), conversation, model and time.

---

### 8.14 Outreach

**What it's for** — bulk email campaigns to selected groups, plus the do-not-contact list. Three tabs: **"Campaigns"**, **"Segments"**, **"Do not contact"**.

**Campaigns tab**

1. First create a **segment** (below) — campaigns always send to a saved segment, never "the whole address book".
2. Press **"New Campaign"**: **name**, **subject**, **body** (hint: *"email body — {{first_name}} is substituted"*), a **mailbox id** (the numeric mailbox the replies belong to — hover: *"local mailbox id (see Settings)"*; the Business hours tab in Settings lists your mailboxes with their ids) and a **segment id** (the form lists your saved segments with their numbers: `saved segments: #3 Ticket owners, …`).
3. **"create campaign"** reports: `campaign created with {n} recipient(s)` — the recipient list is frozen at creation. New campaigns are **draft**.
4. Lifecycle buttons on each campaign card: **"queue"** (starts sending — one conversation is created per customer on Help Scout), **"pause"**, **"resume"**, **"cancel remaining"** (asks first; discards queued sends, never touches sent mail), **"retry failed"**. Status badges: **draft** (grey), **queued/running** (blue), **paused** (amber), **completed** (green), **cancelled** (red). A **"Campaign report"** shows total / sent / failed / skipped / replied / unknown / remaining.
5. People on the do-not-contact list are skipped automatically at send time.

**Segments tab** — press **"New Segment"**: a **name** and **criteria (JSON)**. The criteria are written as JSON in the shape `{"combinator":"all"|"any","conditions":[…],"exclude":[…]}`. This is the one place the app expects machine text; the manual's [Glossary](#14-glossary--words-this-manual-uses) explains the pieces, and the recognized vocabulary is:

- **condition kinds (12)**: `customer_property`, `contact`, `ticket`, `history`, `history_tag`, `organization_property`, `history_issue`, `incident_exposure`, `campaign_history` (received / replied / not_received), `support_health`, `custom_object_link`, `customer_event`, plus nested `group`s.
- **contact fields (10)**: name, email, email_domain, organization, job_title, location, background, has_email, has_phone, has_multiple_emails.
- **history metrics (6)**: ticket_count, open_count, closed_count, last_contact_within_days, first_contact_before_days, waited_over_hours_count.
- **support-health metrics (4)**: avg_rating, avg_effort_score, first_response_resolution_rate, high_friction_rate.
- **event kinds (8)**: signup, support_conversation, customer_message, campaign, campaign_reply, rating, incident_exposure, custom_object_event.
- **comparisons**: equals, not_equals, contains, starts_with, ends_with, gt, gte, lt, lte, between, before, after.

Segments can be deleted (**"Delete segment?"**) — campaigns that already targeted them keep their recipient snapshots.

**Do not contact tab** — type a **customer id** (the numeric id, e.g. from a customer profile) and an optional **reason** (defaults to "manual"), press **"add"**. Everyone on this list is excluded from every campaign. **"remove"** (with confirmation) makes them contactable again.

---

### 8.15 Notifications

**What it's for** — the in-app notification feed: @mentions, campaign replies, sync failures. Header shows **"{total} total · {unread} unread"** with a **"mark all read"** button. Tabs: **"All"**, **"Mentions"**, **"Unread"**. Each card shows its type badge, time, title and body, plus a **"mark read"** / **"unread"** toggle. New notifications arrive automatically (the app re-checks every ~15 seconds).

---

### 8.16 Search

**What it's for** — one search box across everything. Type in **"Search…"**, press Enter or **"search"**; **"clear"** resets. A **"scope:"** row filters results to **"All"**, **"Tickets"**, **"Knowledge"**, or **"Customers"**. Results show a badge per hit (`tickets` / `knowledge` / `customers`) with "why" badges: **"keyword match"**, **"semantic match"** (the AI meaning-based lane, when the vector index is enabled) or **"recent"**. The results header is honest about what ran: **"{n} hit(s) for "…" (semantic)"** plus a note explaining which lanes were used.

---

### 8.17 Support Graph

**What it's for** — seeing how things are connected: which customer owns which conversation, which agent handles what. Subtitle shows link counts. Search in **"Search the graph…"** (customers, conversations, organizations, users, or incidents by name, subject, or number), click a result chip, and the **"Neighborhood"** card lists its nearest links, each as `relation (origin) other-side` — e.g. `owner (human_local) Ada`. Up to 5 links per direction are shown; nodes with no links say so.

---

### 8.18 Connectors

**What it's for** — attaching outside data as read-only tables the AI tools may query (when you allow it). Subtitle: *"Attach external data sources (HTTP endpoints, CSV files inside the data directory) as read-only tables; the AI tools can query them when allowed."*

- Press **"New Connector"**: a **name**, **kind** (**"http"** or **"csv"**), then either a **file (data dir):** (the name of a CSV file you placed inside the app's data folder, e.g. `billing.csv`) or a **url:** (`https://…`), a **refresh seconds:** number (60–86400; default 3600 = hourly), and the **"AI tools may query"** checkbox. Press **"create"**.
- The table shows each connector's **health** (`ok` green / `stale` amber / `error` red), refresh method and last sync. Buttons per row: **"rows"** (browse the stored rows), **"refresh"** (re-fetch now), **"delete"** (asks first — stored rows stay in the mirror).

---

### 8.19 Diagnostics

**What it's for** — the honest record of everything the app did and everything that went wrong. Four cards:

1. **"Event log"** — every error, warning and info event from syncing, connecting and background jobs (up to 200 shown), each with level badge, time, service and message; filter chips **"all" / "errors" / "warnings" / "info"**; **"refresh"** and **"clear log"** (asks first).
2. **"Audit trail"** — who changed what, when: outbound writes, people edits, copilot runs, with before/after values (50 newest).
3. **"Sync reports"** — the outcome of every recent sync run: kind, state (**LIVE** green / **ERROR** red / other amber), records, errors, timing (25 newest).
4. **"Outbound deliveries"** — the dispatch lane for your replies/notes/status changes/assignments, with a summary line (`{n} delivered · {n} queued · {n} sending · {n} failed`) and, when needed, a table of the last 8 failed sends with their exact error text.

**When something feels wrong, start here** — then open Sync Health.

---

### 8.20 Sync Health

**What it's for** — the state of the Help Scout connection: is the engine running, when did it last succeed, what's queued. Header: *"engine state, checkpoints, and recent runs"*.

- A live progress line while a run executes: `progress: {stage} · resource {done}/{total} · {n} record(s) so far`.
- **"Engine"** card: state, **"last success:"** (or "(never)"), and the webhook queue line: `webhook queue: {n} total · {n} pending · {n} processed · {n} failed · {n} duplicates`. If a webhook self-check ran: `webhook health: {verdict} · checked {time} · {detail}`.
- Four buttons: **"Run initial sync"** (pull the whole history — use after connecting or when starting fresh), **"Run incremental sync"** (check for changes), **"Run reconcile"** (a thorough comparison pass), **"Cancel running sync"**.
- **"Checkpoints"** — one row per Help Scout resource type with processed/failed/retries counts (fills after the first successful sync).
- **"Recent runs"** — the 10 newest runs with their state and record counts.

---

### 8.21 Settings

The control panel, in six tabs. Every value is stored locally; nothing is sent anywhere.

**Tab "Sync & AI"**

- **"Synchronization"** card:
  - **"max quiet sync interval (minutes):"** — 1–1440, default **1**. The ceiling of the automatic sync timer (it polls every 30 s while the account is busy, then backs off by doubling toward this ceiling when quiet). Changing it re-arms the timers immediately — no restart.
  - **"api concurrency:"** — 1–8, default **2**. How many parallel requests hit the Help Scout API.
  - **"log level:"** — `debug` / `info` *(default)* / `warn` / `error`. Applied immediately on save.
  - **"agent language:"** — free text, default `en`. The language the AI agent works in.
  - **"status after reply:"** — `"pending (await customer)"` *(default)*, `"open (await agent)"`, or `"keep current status"`.
  - Press **"save settings"** (this tab saves only when you press it; message: `"settings saved"`).
- **"AI behaviour"** card — subtitle: *"safe defaults — review before enabling write actions"*. Eight switches (default in brackets):

  | Checkbox | Default | Meaning |
  |---|---|---|
  | AI enabled | ✔ on | master switch for AI features |
  | Automatic analysis | ✔ on | AI analyzes conversations on its own |
  | Automatic internal notes | ✘ off | AI writes notes by itself |
  | Automatic draft replies | ✘ off | AI pre-writes drafts |
  | Automatic reply sending | ✘ off | AI sends replies without a human click |
  | Automation rules | ✘ off | your automation rules run |
  | Automation write actions | ✘ off | rules may make changes, not just observe |
  | Attachment auto download | ✔ on | attachments download by themselves |

- **"About & updates"** — shows the running version and a **"check for updates"** button. It reads the public GitHub release feed **once, on click** — the caption says: *"The app never sends telemetry."* Result: *"up to date"*, *"an update is available"*, or *"running ahead of the latest published release"*.

**Tab "Help Scout"** — the connection cards from [Section 6](#6-connecting-help-scout-the-one-required-setup): connection status, connect buttons, Connection Doctor, credentials, webhook relay, and the **"Workspace data"** reset.

**Tab "LM Studio"** — three fields for the local AI gateway: **"base url:"** (placeholder `http://localhost:1234/v1`), **"chat model:"**, **"embedding model:"** — press **"save"**. (Test the connection from the AI Center page.)

**Tab "Vector index"** — the AI search index. Shows where the store lives (`<data folder>/vectors/vectors.db`) and one checkbox, **"vector index enabled"** (default on; saves immediately; *"applies on the next start"*). When off, searches fall back to keyword-only.

**Tab "Business hours"** — per-mailbox working hours used by SLA math. Press **"edit"** on a mailbox row: **timezone** (e.g. `UTC`, `Asia/Dhaka`), **days** (`0=Sun…6=Sat`, e.g. `1,2,3,4,5`), **start/end** times (sliders, minutes of the day — defaults 540/1020 = 09:00–17:00), **"first response target (min):"** (default 240) and **"resolution target (min):"** (default 1440). **"save hours"** / **"cancel"**. The SLA tab in Reports uses these numbers.

**Tab "Backups & export"** — see [Section 7.1](#71-backups--settings--backups--export).

---

### 8.22 Onboarding

**What it's for** — the first-run checklist. Header: **"Welcome to SupportOS+"** with the *"app starts empty… only your real history"* message. The **"Setup status"** card shows: the current step (`welcome`/`complete`), whether onboarding is marked complete, the number of conversations synced, whether **Help Scout credentials** are saved, and the **sync state** (`new` = nothing yet, `registered` = a sync ran, `receiving` = webhook events are arriving). Buttons: **"Run initial sync"** and **"Mark complete"**. Hints guide you: *"No Help Scout credentials yet — add them in Settings → Help Scout first."*

---

### 8.23 Shell

**What it's for** — a small technical status page: which database file is in use, the schema version, background event count, last event, and the sync-engine state (`running` / `idle` / `unavailable`). Also carries a theme-switch button. Nothing here needs changing; it's useful when reporting a problem.

---

## 9. AI features in plain words

### What the AI does

SupportOS+ can read your conversations and help you: analyze tickets, suggest replies, summarize threads, translate drafts, cluster issues, draft help articles, and answer questions about your workspace. **By default it only reads and suggests** — the write-for-you switches (automatic notes, automatic drafts, automatic sending, automation actions) are **off** until you enable them, and even then risky actions wait for approval.

### Where the AI runs

Out of the box, all AI runs **on your own computer** through **LM Studio** — a free desktop program (from lmstudio.ai) that runs AI models locally:

1. Download and install LM Studio on the same computer.
2. In LM Studio, download a chat model (e.g. a small instruct model) and — for the best search features — an embedding model.
3. Start LM Studio's local server (the **Developer** tab → *Start Server*; default address `http://127.0.0.1:1234`).
4. In SupportOS+: **Settings → LM Studio** — set the **chat model** and **embedding model** names exactly as LM Studio shows them — press **"save"**. Then check **AI Center → Health → "test connection"** until it shows `connected · {n} model(s)`.
5. In **AI Center → Health → "Model-role routing"**, assign your LM Studio endpoint to the roles you want (classify/summarize/draft/agent; `embed` stays local by default).

No LM Studio (or no model routed)? The app degrades honestly: pages that need a model show exactly which role is missing, and everything else keeps working. You can also add cloud endpoints (OpenAI-compatible, bring-your-own-key) in **AI Center → Provider hub** — the `draft` role is the only one where the app suggests cloud is worth it, and prices you enter feed the **Cost ledger** so you can see estimated spend.

### What never happens

- Customer attributes and AI analyses are **local-only** — never written back to Help Scout.
- AI drafts of help articles are **never published** without a human pressing Approve → Publish.
- Replies are **never sent automatically** unless you turn on "Automatic reply sending".
- The kill switch (AI Center → Features) stops **every** AI feature and endpoint call at once.

---

## 10. How syncing works

**The local mirror.** SupportOS+ keeps its own copy of your Help Scout data in its database and reads from that copy — that's why pages open instantly and why everything still works offline ("offline mode for remote actions — Help Scout is unreachable. Local data remains fully browsable").

**What gets pulled.** The **initial sync** pulls your history in order: account, users, teams, mailboxes, folders, tags, fields, property definitions, organizations, customers, saved replies, workflows, conversations, threads, docs collections and articles, user statuses. On large accounts this *"can take an hour or more"* — watch **Sync Health**. After that, **incremental** syncs re-check conversations and customers for changes, plus the reference lists.

**The automatic rhythm.** The app polls every **30 seconds while your account is busy** (things are changing), then backs off by doubling the delay up to the **"max quiet sync interval"** ceiling (default 1 minute). Every reply/note/status change you send triggers an immediate poll, so the mirror catches up right after your action.

**What only arrives by push (webhooks).** Beacon **live chats** and **satisfaction ratings** have no list to pull — they arrive only through webhook events (Section 6.6). Other events (new/updated conversations, customers, organizations, merges, notes…) arrive faster with webhooks than by polling.

**What you send.** Replies, notes, status changes, assignments, saved-reply edits, chat messages and article publishing are queued in a **delivery lane** and dispatched within seconds, with up to 3 retry attempts. The Inbox shows each item's exact state (queued → sending → delivered ✓ / failed), Diagnostics lists failures, and a toast confirms deliveries. If a send's outcome is ambiguous (e.g. the connection dropped mid-send), the app **does not blindly resend** — it tells you to check the conversation on the platform first, so customers never get duplicates.

**Rate limits.** Help Scout limits API requests per minute (the app assumes 150 unless told otherwise and adapts to the real headers). If you hit the limit, the app waits and retries automatically — *"Help Scout rate limit reached. The operation will be retried automatically when the limit resets."* — and the **"rate-limit budget"** check shows what's left.

**Fixing sync.** Manual buttons live on **Sync Health**: initial sync, incremental sync, reconcile (a thorough comparison), and cancel. Failures are listed in **Diagnostics**.

## 11. Security and privacy

- **Secrets are encrypted.** Your Help Scout Client Secret, webhook secret, Docs API key and access tokens are stored in an encrypted envelope (AES-256-GCM) sealed by a key file (`secrets.key`) created on first start with private permissions. Password boxes never show values on screen; the logs redact them.
- **The listener is loopback-only.** The small built-in server that receives webhooks and OAuth callbacks binds to `127.0.0.1` — your own machine only. Strangers on the internet cannot reach it directly; only your tunnel can, and every webhook is verified with a signature check (unsigned or wrongly signed deliveries are rejected).
- **Untrusted files are sandboxed.** PDF and ZIP attachments are parsed inside a Linux security sandbox (Landlock + seccomp) that forbids network access, file writes, and program execution.
- **No telemetry.** Nothing is phoned home. The only internet calls are: Help Scout's API (your data), the Docs API (your help center), the update check **you** click, the AI endpoint you configure (local LM Studio by default), and your tunnel. The update-check caption says it plainly: *"The app never sends telemetry."*
- **Backups are protected.** Backup files are readable only by your user, and their settings sidecar strips anything containing "token", "secret" or "password".
- **Deleting data really deletes it.** The workspace reset uses the database's secure-delete mode, which zeroes freed pages.
- **Duplicate protection.** Sending the exact same reply text twice to the same conversation is blocked on purpose.
- **One honest limit:** the main database (conversations, customers, message text) is **not** encrypted at rest — it's protected by your Linux user account and disk permissions. If full-disk encryption matters for your data, use your distribution's disk-encryption option.

---

## 12. Troubleshooting — "why doesn't it…?"

**The app won't start and says "another SupportOS+ instance is already running".**
A copy is already open (maybe on another desktop/workspace). Close it first. If you can't find it, the lock file `~/.local/share/supportos-plus/instance.lock` is released automatically when the program exits — after a crash it is safe to start again.

**The app shows "SupportOS+ failed to start" with another message.**
Details are written to `~/.local/share/supportos-plus/logs/fatal.log`. Common causes: the database file can't be opened (disk full? permissions?), or a background worker failed. Send that file when asking for help.

**Help Scout says "not connected" / the connection badge is grey.**
Open Settings → Help Scout. If credentials are saved, press **"connect with client credentials (recommended)"**. If sign-in fails, run the **Connection Doctor** — the failing step includes the remedy (wrong id/secret, Free plan without API access, Light User without API access).

**The app says the connection is no longer authenticated (401).**
The stored token expired or was revoked. The app tries to refresh automatically; if that fails: *"The Help Scout connection is no longer authenticated. Re-connect Help Scout in Settings, then retry. No changes were made."* — press connect again. Nothing was lost.

**Pages are empty even though I connected.**
The first sync may still be running (it can take an hour on big accounts). Watch **Sync Health** (the freshness pill in the header also shows the last sync). If the last sync shows errors, open **Diagnostics → Sync reports**.

**A send says "failed".**
Look at the red status line under the composer and **Diagnostics → Outbound deliveries** for the exact error. The app retries failed sends up to 3 times automatically; if it stops, fix the cause (usually connectivity) and send again — except after an *ambiguous* outcome, where the app deliberately refuses to auto-resend to protect your customer from duplicates. Check the conversation in Help Scout first, then resend if it truly didn't arrive.

**"AI evaluation mode is ON: no notes, replies or status changes are sent."**
The app is in a special testing switch that blocks all outgoing writes (it's an internal evaluation flag, not a normal setting). If you see it outside of a test, it was set in the settings table (`ai_evaluation_mode`) — clear it or ask the person who set up the app.

**No live chats / no ratings arrive.**
Those arrive only through webhooks. Check Settings → Help Scout → **"Webhook relay & health"**: is a relay URL saved, is your tunnel running, does **"check now"** say **"healthy"**? Also confirm the **webhook secret** matches the one in Help Scout (mismatches are rejected with 401 and logged in Diagnostics).

**Help Center (Knowledge → Help center) is empty.**
You need a Docs API key: Help Scout → Profile → **Security & Access → Docs API**, saved into **Settings → Help Scout → "docs api key:"**, then press **"Sync Help Center now"** on the Knowledge page.

**The freshness pill turned red.**
No successful sync for over 30 minutes. Open **Sync Health** for engine state and last success; press **"Run incremental sync"**; read **Diagnostics** for errors (network down? credentials rejected? rate limit?).

**"Help Scout rate limit reached…"**
Normal under heavy load — the app waits and retries by itself. If it happens constantly, raise nothing: the limiter already paces itself under the account limit. Just let it work.

**AI buttons are missing or say a role isn't routed.**
Install/start LM Studio, set the models in Settings → LM Studio, route roles in **AI Center → Model-role routing**, and make sure the feature is enabled in **Features & kill switches**. The exact missing role is always printed in the message.

**LM Studio shows "offline".**
LM Studio isn't running, its server isn't started, or the model names don't match. Open LM Studio → Developer → Start Server, then press **"test connection"** in AI Center.

**Stored credentials stopped working after I deleted/moved the data folder.**
The encryption key (`secrets.key`) must travel with the data folder. If it's lost, old secrets can't be decrypted (by design) — re-enter your credentials in Settings → Help Scout.

**The update check says "update check failed".**
The GitHub release feed couldn't be reached (offline, or a private repo with no public release). This check runs **only** when you click the button — it never blocks anything.

**Where are the logs?**
Normal operation logs go to the terminal output (JSON lines in release builds). Crash/failure details: `~/.local/share/supportos-plus/logs/fatal.log`. In-app: **Diagnostics**.

---

## 13. For advanced users — environment variables

You never need these. They exist for special setups (scripted installs, custom ports, moving the data folder). They **override** the values saved in the app.

| Variable | Default | Purpose |
|---|---|---|
| `SPP_DATA_DIR` | — | Use a different data folder entirely (portable installs). |
| `SPP_LOOPBACK_PORT` | port from redirect URI (3000) | Fixed port for the webhook/OAuth listener (falls back to a free port if busy). |
| `SPP_RENDERER` | `glow` | Set `wgpu` to try the alternate graphics backend. |
| `SPP_LOG_FORMAT` | JSON in release builds | Force `json`/pretty log format. |
| `LOG_LEVEL` | `info` | Boot log level (the saved `log level:` setting wins after boot). |
| `HELPSCOUT_CLIENT_ID` / `HELPSCOUT_CLIENT_SECRET` | empty | Custom-app credentials (override stored values). |
| `HELPSCOUT_REDIRECT_URI` | `http://localhost:3000/oauth/callback` | OAuth callback URL (browser flow only). |
| `HELPSCOUT_API_BASE` | `https://api.helpscout.net` | API host. |
| `HELPSCOUT_WEBHOOK_SECRET` | empty | Webhook signature secret. |
| `HELPSCOUT_DOCS_API_KEY` / `HELPSCOUT_DOCS_API_BASE` | empty / `https://docsapi.helpscout.net` | Docs (Help Center) API key and host. |
| `LMSTUDIO_BASE_URL` | `http://127.0.0.1:1234` | Local AI gateway address. |
| `LMSTUDIO_CHAT_MODEL` / `LMSTUDIO_EMBEDDING_MODEL` | unset | Pin model names. |
| `LMSTUDIO_TIMEOUT_MS` / `LMSTUDIO_CONCURRENCY` | `120000` / `2` | AI request timeout / parallelism. |
| `SYNC_INTERVAL_MINUTES` / `SYNC_API_CONCURRENCY` | `1` / `2` | Fallbacks for the Sync & AI settings. |
| `QDRANT_ENABLED` | `true` | On/off switch for the local vector search index (the Vector index tab is the friendly way). |

The program takes **no command-line arguments** — everything is configured in the app or with these variables.

---

## 14. Glossary — words this manual uses

| Word | Plain meaning |
|---|---|
| **API** | The official "door" a service offers so other programs can read/write data. |
| **Client ID / Client Secret** | A two-part key identifying your private connection to Help Scout. The Secret is a password — never share it. |
| **Docs API key** | A separate key, only for reading/writing your Help Scout Help Center. |
| **Webhook** | A push notification: Help Scout calls a web address the moment something happens. |
| **Webhook relay (tunnel)** | A tiny helper that forwards public internet pushes to your own machine (cloudflared, ngrok, serveo). |
| **Mirror / local mirror** | The app's own copy of your Help Scout data, stored on your computer. |
| **Sync (initial / incremental / reconcile)** | First full pull / quick change-check / thorough comparison pass. |
| **Beacon** | Help Scout's website chat widget; its chats appear on the Live chats page. |
| **Conversation / ticket** | One customer email thread (`#number` is its id in the app). |
| **Mailbox** | A Help Scout inbox (support@, billing@…). |
| **Saved reply** | A pre-written answer template, stored on Help Scout. |
| **Workflow** | A Help Scout-side automation; the app can trigger manual ones. |
| **SLA** | Service-level agreement — the response/resolution time targets you set per mailbox (Settings → Business hours). |
| **CSAT / rating** | Customer satisfaction: Great / Okay / Not good. |
| **Segment** | A saved rule that picks a group of customers for campaigns. |
| **Campaign** | A bulk email to a segment; each recipient gets their own conversation. |
| **Do not contact (DNC)** | The never-email-me list. |
| **Custom object / type** | Your own mini-database record / the form that defines it. |
| **Known issue / gap** | A documented recurring problem / a question customers ask that your docs don't answer. |
| **Incident** | One tracked outage affecting many conversations. |
| **AI attribute** | A local-only label the AI assigns (intent, urgency…). |
| **Model role** | Which AI model does which job (embed/classify/summarize/draft/agent). |
| **Endpoint** | An address where an AI model answers (LM Studio locally, or a cloud service). |
| **Golden set** | Nine built-in test cases that score your Classify model. |
| **Vector index / embeddings** | The meaning-based search index (optional; lives in `vectors/vectors.db`). |
| **FTS / keyword search** | Plain text search, always available. |
| **Copilot** | The local conversation helper panel in the Inbox. |
| **Agent (AI role)** | The tool-using assistant that can look things up for you. |
| **Kill switch** | One click that turns every AI feature off. |
| **OAuth** | The standard sign-in handshake behind "connect"; you'll rarely see it — client credentials are easier. |
| **Rate limit** | Help Scout's requests-per-minute cap; the app paces itself and retries politely. |
| **.deb / AppImage** | The two Linux package formats the app ships in (installable package / single runnable file). |
| **Checksum (SHA-256)** | A download fingerprint used to verify nothing was corrupted. |

---

## 15. Appendix — every "Are you sure?" dialog in the app

| When you press… | The dialog says |
|---|---|
| Delete an automation rule | **"Delete automation rule?"** — *"The rule "{name}" stops firing immediately. This cannot be undone."* → **"Delete rule"** |
| Delete a knowledge document | **"Delete knowledge document?"** — *"The document "{title}" and its embedded chunks are removed. Search and AI answers lose it immediately. This cannot be undone."* → **"Delete document"** |
| Delete a custom-object type | **"Delete custom type?"** — *"The type "{name}" and every record stored under it are removed. This cannot be undone."* → **"Delete type"** |
| Delete one custom object | **"Delete object?"** — *"The object "{name}" is removed from its type. This cannot be undone."* → **"Delete object"** |
| Delete a connector | **"Delete connector?"** — *"The connector "{name}" stops syncing immediately; its stored rows stay in the mirror. This cannot be undone."* → **"Delete connector"** |
| Delete an outreach segment | **"Delete segment?"** — *"The segment "{name}" is removed; campaigns that targeted it keep their recipient snapshots. This cannot be undone."* → **"Delete segment"** |
| Clear the diagnostics event log | **"Clear the event log?"** — *"All {count} stored diagnostics event(s) are deleted. This cannot be undone."* → **"Clear log"** |
| Delete a saved report | **"Delete saved report?"** — *"The saved report "{name}" is removed. This cannot be undone."* → **"Delete report"** |
| Remove a do-not-contact entry | **"Remove from the do-not-contact list?"** — *"Customer {id} becomes contactable by every campaign again."* → **"Remove"** |
| Cancel campaign sends | **"Cancel the remaining sends?"** — *"Every queued send of the campaign "{name}" is discarded. This cannot be undone."* → **"Cancel sends"** |
| The global AI kill switch | **"Disable all AI?"** — *"Every AI feature and every endpoint call is disabled at once. Re-enable it here when needed."* → **"Disable AI"** |
| Delete a saved reply | **"Delete saved reply?"** — *"The saved reply "{name}" is deleted from Help Scout. This cannot be undone."* → **"Delete reply"** |
| Restore a backup | *(two-step, on the Backups tab)* — *"Restore {path}? The current database will be REPLACED by this backup (verified first). Restart SupportOS+ afterwards to use it."* → **"really restore"** |
| Delete all workspace data | *(two-step, on the Help Scout tab)* — *"This cannot be undone. Everything listed above will be deleted."* → **"really delete everything"** |

Cancel, the X, or **Esc** always cancels safely.

---

*SupportOS+ 1.8.0 — this manual describes the program as built from its source code. Where the app shows exact texts, they are quoted as the program prints them.*



