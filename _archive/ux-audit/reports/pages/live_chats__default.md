# UX/UI Audit — SupportOS+ "Live chats" (main content area only)

## 1. Content Inventory
- Page title "Live chats" (top-left of content area)
- Subtitle paragraph: "Beacon chats arrive through the five beacon.chat.* webhook events — Help Scout has no list endpoint, the webhook IS the data path."
- Top-right status text: "0 open · 0 ended"
- View-toggle row: label "view" with two tabs/links, "open chats" (active) and "show ended chats"
- Empty-state icon (paper-plane / beacon glyph), centered
- Empty-state heading "No live chats yet"
- Empty-state helper line: "Chats appear here the moment the beacon.chat.created webhook lands — connect the webhook relay (Settings) and they will arrive on their own."
- No CTA button, no list, no filters, no table.

## 2. Layout Defects
- Empty-state block sits far above vertical center; ~450px of dead space below it — vertically unbalanced.
- Subtitle runs as one long line (~770px) instead of wrapping/constraining measure; fine at 1920 but will overflow on narrower windows.
- Toggle row ("view / open chats / show ended chats") has loose, uneven spacing; label "view" is lowercase orphan text and misaligned with the tab baseline.
- "0 open · 0 ended" floats alone at top-right with weak visual connection to the content; not aligned to any grid edge clearly.
- Empty-state icon, heading, and caption are horizontally centered while the title/subtitle/toggles are left-aligned — two competing alignment systems in one viewport.
- The "show ended chats" affordance is not grayed/disambiguated from "open chats" — both look like plain text links.

## 3. Visual / Typography Defects
- Subtitle uses developer jargon ("beacon.chat.*", "webhook IS the data path") in title-case-adjacent prose; inconsistent tone vs. the friendlier empty-state copy below — two texts explain the same thing redundantly.
- Low hierarchy contrast: subtitle, tab labels, and counter are all small, similar gray text; nothing clearly secondary vs. interactive.
- Casing inconsistency: "view", "open chats", "show ended chats" all lowercase while the page title is Title Case and empty-state heading is Sentence case.
- Icon-only toggle/labels have no weight/color distinction for the active vs. inactive tab beyond subtle bolding of "open chats".

## 4. Component Defects
- The "open chats" / "show ended chats" tabs are text-only: no pill, underline, background, hover/selected affordance, or focus ring visible; tiny click targets.
- No primary CTA in the empty state (e.g., "Connect webhook relay" or "Open Settings") — the copy references Settings but provides no actionable affordance.
- "Settings" in helper copy is plain text, not a link/button.
- No count-chips or badges attached to the tabs despite the "0 open · 0 ended" data existing.

## 5. UX Defects
- Redundant explanation of webhooks appears twice (subtitle + empty state); the first explanation adds no value to a first-run user.
- Empty state has no next step: tells the user to connect the webhook relay but doesn't offer the action — dead end for onboarding.
- No connection-health feedback on this page: user cannot tell if the webhook relay is connected and simply idle vs. not configured (the "synced 8s ago" chip is off-audit in the top bar but the content area shows nothing).
- "show ended chats" toggle has no visible effect at 0 ended — no disabled/explanatory state.
- If relay were connected, empty state should say "listening for chats…" with live/pulse feedback rather than a static "not yet."

## 6. Accessibility
- "0 open · 0 ended" and subtitle are light gray on white — likely below 4.5:1 contrast.
- Tab pair is likely not exposed as tabs (no selected state, no role indication); screen readers get two ambiguous links.
- Empty-state icon is purely decorative but appears to be the primary visual anchor; fine, but heading level of "No live chats yet" vs. "Live chats" hierarchy unverified.
- Lowercase "view" label preceding tabs creates odd reading order: "view open chats show ended chats" reads as one run-on phrase.

## 7. Verdict
- [MAJOR] Empty state is a dead end — no CTA to the Settings/webhook relay it references.
- [MAJOR] No relay/connection status in content area; user can't distinguish "waiting" from "not configured."
- [MAJOR] Text-only tab toggle with no selected-state affordance and small click targets.
- [MINOR] Redundant webhook explanation in subtitle and empty state.
- [MINOR] Empty state vertically misplaced; large dead space below.
- [MINOR] Casing inconsistency across title, tabs, and labels.
- [MINOR] Likely low-contrast gray body/counter text.
- [MINOR] "Settings" mention is plain text, not a link.
- [POLISH] Mixed center/left alignment between empty-state block and header cluster.
- [POLISH] Counter "0 open · 0 ended" could be badges on the tabs instead of floating text.