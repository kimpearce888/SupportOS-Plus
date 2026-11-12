# SupportOS+ Settings — Help Scout Tab (1920×1080)

## Overall Layout
- **Top bar** (right side): green pill badge "synced 1s ago", search box "Search ⌘K", and a small chat/bug icon button.
- **Left sidebar** (~215px, light gray): logo "S+" black rounded square + "SupportOS+", "v1.9.4", a search input "Search… ⌘K" (placeholder text beneath is clipped/overlapped by the nav — partially legible ghost text), then nav groups:
  - (top items mostly clipped behind search — one item is unreadable)
  - **Customers, Organizations, Knowledge**
  - "Insights": **Reports, Issue Radar, Incidents**
  - "Automation & AI": **Custom Objects, Automation, AI Center, Outreach**
  - "System": **Notifications, Search, Support Graph, Connectors, Diagnostics, Sync Health, Settings** (active — highlighted with white card), **Onboarding, Shell**
  - Divider, then "switch to dark theme" with icon.
- **Main content**: H1 "Settings", subtitle "local configuration, persisted in the settings table". Tab row: **Sync & AI | Help Scout (active, underlined) | LM Studio | Vector index | Business hours | Backups & export**.
- Below tabs: small line "loopback listener answering on :36117 (webhook ingress + OAuth callback)".

## Card 1 — "Help Scout connection"
- Description: "Connect with client credentials (recommended — no browser, no redirect URL) or the advanced browser flow"
- Row "credentials stored:" → green pill **connected**
- Row "redirect uri:" → monospace `http://localhost:3000/oauth/callback` (plain text, no copy button)
- Red **disconnect** button.

## Card 2 — "Connection Doctor"
- Description: "five checks: reach the API → sign in → read the user → list mailboxes → rate-limit budget"
- Button: **run the checks**. No results/status shown.

## Card 3 — "Help Scout credentials"
- Subtitle: "Stored encrypted on this machine. Save them and the app connects immediately — the Connection Doctor verifies the chain and the first sync starts by itself. Environment variables override these values."
- Form fields (label left, input right):
  - **client id:** empty, placeholder "Help Scout custom app — client ID"
  - **client secret:** placeholder "leave blank to keep the stored value"
  - **webhook secret:** placeholder "verifies incoming Help Scout webhooks"
  - **docs api key:** placeholder "optional — leave blank to keep the stored value"
- Checkbox **show secrets** (unchecked; nothing visible to reveal), black **save credentials** button, red **clear stored** button.
- Status strip of chips: `client id: environment` · `client secret: environment` · `webhook secret: not set` · `docs api key: environment` (environment chips green, "not set" gray).
- No actual secret strings visible — only placeholders/status chips; the redirect URI is the only URI value shown (not a secret).

## Card 4 — "Webhook relay & health" (partially cut off at viewport bottom)
- Description: "the listener stays loopback-only by design; a user-run relay bridges the NAT, and the health check re-registers the hook when the platform silently drops it"
- "public relay URL:" input, placeholder `https://your-relay → forwards to this app'…` (clipped)
- Below, a partially visible "…webhook URL" input and a dark **check now** button — cut off by the viewport edge; an ✕ close glyph floats bottom-right over the content.

## No Docs API section
- A "docs api key" field exists in the credentials card, but there is no separate Docs API section on this screen (possibly below the fold / cut off).

## Visual / UX Defects
1. **Narrow column wasting width**: all four cards sit in a ~600–980px column; on 1920px the right half of the screen is empty whitespace (cards even have inconsistent max widths — connection card ends ~847px, credentials card ~1340px, doctor card ~719px).
2. **Sidebar clipping**: text behind the sidebar search box appears truncated/overlapped (a half-visible nav item under "Search…").
3. **Clipped text**: relay-URL placeholder is truncated mid-URL (`this app'…`); the webhook relay card and its "check now" button are cut off at the bottom with no scroll affordance shown; a stray ✕ overlaps content bottom-right.
4. **Inconsistent spacing**: gaps between cards vary (~15–40px); buttons/labels spacing differs between cards; the redirect-uri row has a much wider label gap than the credentials form labels.
5. **Missing feedback**: "run the checks" has no last-run status, timestamps, or inline results; "show secrets" checkbox is present but secrets apparently stored as "environment" — unclear what it toggles; no copy-to-clipboard button on the redirect URI; "disconnect" has no confirmation.
6. **Confusing copy**: lowercase mixed-style labels ("credentials stored:", "redirect uri:") inconsistent with title-cased headings; "five checks:" sentence-fragment style; "bridges the NAT" is jargon-y; subtitle runs on as one long sentence; duplicate connection surfaces (client credentials vs browser flow vs relay) with no guidance on which applies.
7. **Alignment issues**: card headings/descriptions use inconsistent margins; the "synced 1s ago" pill, search, and icon crowd the top-right; status chips row uses mixed label/chip baselines.
8. **Hierarchy**: everything is similar-size small text; no visual grouping or error/success detail for "not set" webhook secret despite connected status.
