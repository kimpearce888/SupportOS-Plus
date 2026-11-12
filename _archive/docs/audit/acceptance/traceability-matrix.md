# SupportOS+ R4 Traceability Matrix — RS-21 (Batch RS-3)

> **Contract:** DC v1.0 (`docs/audit/design-contract-v1.md`, FROZEN 2026-10-28).
> **Rule traced:** G-1 (a page = archetypes + components, no bespoke styling) and
> G-7 (anything untraceable is a registered exception — never silently approximated).
> **Scope:** every route of the app (`PageKey`, `crates/app-shell/src/pages/mod.rs`)
> plus the persistent shell chrome (DC §6).
>
> **Machine-checked:** `scripts/gate_traceability.py` (wired into CI) validates that
> (a) every DS-C/DS-A id cited below exists in the DC (one registered alias, below),
> (b) every `PageKey` route has a section here, (c) every cited golden-lock file
> exists under `crates/app-shell/tests/`.
>
> **Widget column** names the `design.rs` function the region renders through — the
> single styling path (R2 widget sets, RS-13/14/15). **Golden lock** names the
> kittest suite that diff-locks the REAL page (not a mock) over a seeded mirror at
> 1440/1024/800 × 900, light + dark, a11y check ON (diff-locked twice, RS-16..RS-20).

## Registered id alias (G-7 note, cited by the gate)

| Cited id | DC row | Note |
|----------|--------|------|
| DS-C-04 | §3 **DS-C-03** (StatTile · MetricTile sheet, MetricTile half) | The DC folded MetricTile into the DS-C-03 sheet; code comments and the lab registry keep the `DS-C-04` label for the horizontal tile half. The matrix cites it as **DS-C-04 → C-03**; the gate resolves the alias to the DC sheet. |

Folded helpers (DC §3 tail — no separate sheet): `pager`, `divider`, `day_divider`,
`quoted_history`, `file_chip`, kv rows (DS-C-18), bar_row (DS-C-23), selection rows,
`kbd` chip. They trace to the sheet named in their region row.

---

## 0. Shell chrome (DC §6) — persistent across all routes

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Sidebar nav groups, active = neutral fill + 500 | §6 (sidebar; NAV_GROUPS verbatim) | `app.rs` nav rows | SPP_SMOKE (harness exception) |
| Global search (⌘K palette) | §6 (stays in the sidebar) | `palette.rs` | SPP_SMOKE (harness exception) |
| Theme switch (light/dark, persisted) | §6 + §2 tokens | `design::install` + `dark_visuals/light_visuals` | SPP_SMOKE (harness exception) |
| Header outline act (theme toggle) | §3 DS-C-11 `outline` | `design::button` | SPP_SMOKE (harness exception) |
| Version label (sidebar footer) | §6 chrome (display only) | `app.rs` label | SPP_SMOKE (harness exception) |
| Shell page: Runtime card (five kv rows) | §3 DS-C-02 (+DS-C-18 kv) | `design::card_titled` | SPP_SMOKE (harness exception) |
| Ported-pages hint (stale-checklist tag) | §3 DS-C-15 `hint` | `design::hint_text` | SPP_SMOKE (harness exception) |

> Shell is bound to the live `App` shell state (db path, sync engine, theme store);
> no standalone kittest suite — the registered harness exception (RS-20h), verified
> by SPP_SMOKE's 24-frame paint gate on a fresh binary.

## 1. Dashboard — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_dashboard_page.rs |
| Stat grid | §3 DS-C-03 (StatTile) | `metric_card` | kittest_dashboard_page.rs |
| Health/insurance bars | §3 DS-C-23 | `bar_row` | kittest_dashboard_page.rs |
| Content cards (friction, effectiveness, graph nodes) | §3 DS-C-02 | `card`, `card_titled` | kittest_dashboard_page.rs |
| Status chips | §3 DS-C-06 | `chip` | kittest_dashboard_page.rs |
| Tables (num cells right, mono) | §3 DS-C-05 | `data_table`, `td_num` | kittest_dashboard_page.rs |
| Bounded-recompute act | §3 DS-C-11 (`outline`) | `button_with_icon` | kittest_dashboard_page.rs |
| Data-track receipts | §3 DS-C-15 `quirk` (C-1) | `quirk` | kittest_dashboard_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_dashboard_page.rs |
| Status → color | §5 DS.TONE | `tokens`, `tone_colors` | style_lint.rs |

## 2. Reports (+ Builder) — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_reports_page.rs |
| Grouped tab rail (3 groups / 11 labels) | §3 DS-C-09 `grouped` | `tab_groups` | kittest_reports_page.rs |
| SLA first-response bar list | §3 DS-C-23 | `bar_row` | kittest_reports_page.rs |
| Metric cards | §3 DS-C-03 | `metric_card` | kittest_reports_page.rs |
| Striped tables (Happiness/digest) | §3 DS-C-05 | `data_table`, `td_num` | kittest_reports_page.rs |
| Builder form (metric/dimension/filters) | §3 DS-C-12/13 | `form_grid`, `field_label` | kittest_reports_page.rs |
| Builder saved rows (load outline / delete danger) | §3 DS-C-11 + A-5 armed rule | `button`, `button_with_icon` | kittest_reports_page.rs |
| Radar severity receipt | §3 DS-C-15 `quirk` (C-1) | `quirk` | kittest_reports_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_reports_page.rs |

## 3. IssueRadar — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_issue_radar_page.rs |
| Tab rail (same four labels) | §3 DS-C-09 | `tabs` | kittest_issue_radar_page.rs |
| Alerts cards + badge rows | §3 DS-C-02 / DS-C-06 | `card_titled`, `badge` | kittest_issue_radar_page.rs |
| SLA rows (breached Err / at-risk Warn) | §3 DS-C-15 `callout` roles | `callout` | kittest_issue_radar_page.rs |
| Gap approve / reject (armed two-click) | §3 DS-C-11 + DS-C-16 armed rule (A-5) | `button`, `armed_danger_button` | kittest_issue_radar_page.rs |
| Severity receipt | §3 DS-C-15 `quirk` (C-1) | `quirk` | kittest_issue_radar_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_issue_radar_page.rs |

## 4. CustomObjects — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + count pill | §3 DS-C-01 (counter pill) | `page_header`, `page_counter` | kittest_custom_objects_page.rs |
| Object pane head (strong title, count right) | §3 DS-C-02 (header row) | `card_titled` | kittest_custom_objects_page.rs |
| Filter toolbar | §3 DS-C-10 | `filter_bar` | kittest_custom_objects_page.rs |
| Object cards (kv anatomy: muted key, mono value) | §3 DS-C-02 (+DS-C-18) | `card` | kittest_custom_objects_page.rs |
| 50-row-cap honesty note | §3 DS-C-15 `hint` | `hint_text` | kittest_custom_objects_page.rs |
| New-Type form | §3 DS-C-12/13 | `form_grid`, `field_label` | kittest_custom_objects_page.rs |
| Empty state (lab copy) | §3 DS-C-14 | `empty_state` | kittest_custom_objects_page.rs |

## 5. Outreach — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_outreach_page.rs |
| Chip rail | §3 DS-C-09 | `tabs` (rail) | kittest_outreach_page.rs |
| Campaign cards (lifecycle acts left / danger+outline right) | §3 DS-C-02 + DS-C-11 | `card`, `button_with_icon` | kittest_outreach_page.rs |
| CAMP badges (completed → success, others neutral, raw token visible) | §3 DS-C-06 via §5 DS.TONE (C-1) | `badge` | kittest_outreach_page.rs |
| Campaign report warn receipt | §3 DS-C-15 `callout(.warn)` (C-1) | `callout` | kittest_outreach_page.rs |
| Segment cards (strong id — name, criteria mono line) | §3 DS-C-02 (+DS-C-18) | `card` | kittest_outreach_page.rs |
| DNC rows (reason/ts/remove right) | §3 DS-C-02 | `card` rows | kittest_outreach_page.rs |
| New-Campaign form | §3 DS-C-12/13 | `form_grid`, `field_label` | kittest_outreach_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_outreach_page.rs |

## 6. Automation — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_automation_page.rs |
| Approval rows (human-readable action badge first, raw JSON collapse) | §3 DS-C-02 + DS-C-06 + C-1 receipt | `card_titled`, `badge`, `quirk` | kittest_automation_page.rs |
| Approve act (one-click outline) | §3 DS-C-11 `outline` | `button_with_icon` | kittest_automation_page.rs |
| Rule table | §3 DS-C-05 | `data_table` | kittest_automation_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_automation_page.rs |

## 7. Customers — DS-A-01 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + counter pill | §3 DS-C-01 | `page_header`, `page_counter` | kittest_list_pages.rs |
| Filter bar (one primary apply) | §3 DS-C-10 | `filter_bar` | kittest_list_pages.rs |
| Customer table (row `open` act) | §3 DS-C-05 | `data_table`, `td_num` | kittest_list_pages.rs |
| Pagination | DC §3 folded helper (pager) | `pager` | kittest_list_pages.rs |
| Detail panel (content = DS-A-03) | §3 DS-C-17 + §4 DS-A-03 | `detail_panel` | kittest_list_pages.rs (detail golden) |
| Timeline bars / avatars | §3 DS-C-23 / DS-C-19 | `bar_row`, `avatar` | kittest_list_pages.rs |
| Empty regions (why + next step) | §3 DS-C-14 (slim inline) | `empty_state_slim` | kittest_list_pages.rs |

## 8. Organizations — DS-A-01 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_list_pages.rs |
| Filter bar | §3 DS-C-10 | `filter_bar` | kittest_list_pages.rs |
| Org table | §3 DS-C-05 | `data_table` | kittest_list_pages.rs |
| Detail panel (DS-A-03 composition; members store-shape fix RS-17a) | §3 DS-C-17 + §4 DS-A-03 | `detail_panel` | kittest_list_pages.rs (detail golden) |
| Member avatars | §3 DS-C-19 | `avatar` | kittest_list_pages.rs |
| Empty regions | §3 DS-C-14 | `empty_state_slim` | kittest_list_pages.rs |

## 9. Incidents — DS-A-01 (§4, R3 wave-B pilot)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + counter pill ('[counter] [New Incident]', lab order) | §3 DS-C-01 | `page_header`, `page_counter` | kittest_incidents_page.rs |
| Filter card (combos + checkbox refetch on change; text keeps explicit apply) | §3 DS-C-10 | `filter_bar` | kittest_incidents_page.rs |
| Incident chip list (code strong / title / severity+status badges / mono count / open) | §3 DS-C-05 | `data_table`, `badge` | kittest_incidents_page.rs |
| Detail panel (DS-A-03 record head) | §3 DS-C-17 + §4 DS-A-03 | `detail_panel` | kittest_incidents_page.rs |
| New-Incident form | §3 DS-C-12/13 | `form_grid`, `form_row` | kittest_incidents_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state_slim` | kittest_incidents_page.rs |

## 10. Knowledge — DS-A-02 block stack over the DS-C-09 rail (§4; RS-18 wave B)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_knowledge_page.rs |
| Underline tab rail (five labels, rail divider) | §3 DS-C-09 | `tabs` | kittest_knowledge_page.rs |
| Index metric cards | §3 DS-C-03 | `metric_card` | kittest_knowledge_page.rs |
| Documents table | §3 DS-C-05 | `data_table`, `td_num` | kittest_knowledge_page.rs |
| Scope chips | §3 DS-C-06 | `chip` | kittest_knowledge_page.rs |
| Import form | §3 DS-C-12/13 | `form_grid`, `field_label` | kittest_knowledge_page.rs |
| Article status line | §3 DS-C-15 `status-line` | `success_text` | kittest_knowledge_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_knowledge_page.rs |

## 11. LiveChats — DS-A-04 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + counter pill | §3 DS-C-01 | `page_header`, `page_counter` | kittest_live_chats_page.rs |
| Chat threads as role-typed message cards | §3 DS-C-20 | `message_card_with` | kittest_live_chats_page.rs |
| Filter chips | §3 DS-C-06 | `chip` | kittest_live_chats_page.rs |
| Context callout / send status line | §3 DS-C-15 roles | `callout`, `status_line` | kittest_live_chats_page.rs |
| Empty state | §3 DS-C-14 | `empty_state` | kittest_live_chats_page.rs |

## 12. Inbox — DS-A-05 (§4; R3 pilot, G3)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + counter pill | §3 DS-C-01 | `page_header`, `page_counter` | kittest_inbox_page.rs |
| Views rail (172px, surface-2, count per item) | §4 DS-A-05 | `view_item` | kittest_inbox_page.rs |
| Conversation list head + filter | §3 DS-C-10 | `filter_bar` | kittest_inbox_page.rs |
| List rows (unread dot, name, 2-line preview, time) | §4 DS-A-05 + DS-C-06 dot | `view_item`, `status_dot` (tone) | kittest_inbox_page.rs |
| Reader head (avatar row, assignee) | §3 DS-C-19 | `avatar`, `assignee` | kittest_inbox_page.rs |
| Message cards (role-typed, day dividers, quoted collapse, attachments) | §3 DS-C-20 | `message_card_with`, `day_divider`, `quoted_history`, `file_chip` | kittest_inbox_page.rs |
| Composer (Reply/Note, chips, draft flag) | §3 DS-C-21 | `composer` | kittest_inbox_page.rs |
| Copilot panel (suggest/similar/chat, AI badge) | §3 DS-C-22 | `cop_suggest`, `cop_chat`, `badge` | kittest_inbox_page.rs + kittest_inbox_copilot.rs (open state, 1625px, light+dark) |
| Triage chips + status | §3 DS-C-06 via §5 | `chip`, `badge` | kittest_inbox_page.rs |
| Pagination | §3 folded helper | `pager` | kittest_inbox_page.rs |
| Empty panes (slim) | §3 DS-C-14 | `empty_state_slim` | kittest_inbox_page.rs |

## 13. Operations — DS-A-06 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_operations_page.rs |
| Banded metric tiles (3px tone rail; drill) | §3 DS-C-03 (MetricTile half; cited DS-C-04 → C-03) | `metric_tile` | kittest_operations_page.rs |
| Tab rail | §3 DS-C-09 | `tabs` | kittest_operations_page.rs |
| Jobs table (num cells) | §3 DS-C-05 | `data_table`, `td_num` | kittest_operations_page.rs |
| Filter chips | §3 DS-C-06 | `chip` | kittest_operations_page.rs |
| Load/Pressure presentation receipt | §3 DS-C-15 `quirk` (C-1) | `quirk` | kittest_operations_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_operations_page.rs |

## 14. AiCenter — DS-A-06 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_ai_center_page.rs |
| Tab rail (grouped variant) | §3 DS-C-09 | `tab_groups` | kittest_ai_center_page.rs |
| Provider panel / routing form (endpoint-name combo) | §3 DS-C-12/13 + C-1 receipt | `form_grid`, `field_label`, `quirk` | kittest_ai_center_page.rs |
| Usage/cost tables | §3 DS-C-05 | `data_table`, `td_num` | kittest_ai_center_page.rs |
| Provider status badges | §3 DS-C-06 | `badge` | kittest_ai_center_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_ai_center_page.rs |

## 15. Diagnostics — DS-A-06 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_diagnostics_page.rs |
| Level filter chips + refresh/clear acts right | §3 DS-C-10/06 + DS-C-11 | `chip`, `button` | kittest_diagnostics_page.rs |
| Event rows (muted ts, level dot, service badge, message) | §3 DS-C-24 ev-row anatomy + DS-C-06 dot | `table` rows, `status_dot`, `badge` | kittest_diagnostics_page.rs |
| Audit trail (actor/ai AI-badge tone) | §3 DS-C-06 via §5 (`ai` tone) | `badge` | kittest_diagnostics_page.rs |
| Sync-state badges (SYNC map) | §3 DS-C-06 via §5 | `badge` | kittest_diagnostics_page.rs |

## 16. SyncHealth — DS-A-06 (§4; register B-2 closure)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_sync_health_page.rs |
| Engine hero band (mono state + forensic note + status badge) | §3 DS-C-24 (B-2 closure) | `hero` | kittest_sync_health_page.rs |
| Action toolbar (run acts left / destructive cancel right) | §3 DS-C-11 | `button` | kittest_sync_health_page.rs |
| Snapshot tables | §3 DS-C-05 | `data_table`, `td_num` | kittest_sync_health_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_sync_health_page.rs |

## 17. Search — DS-A-06 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + toolbar (lost-focus + Enter kept; clear = text act) | §3 DS-C-01 + DS-C-11 `text` | `page_header`, `button` | kittest_search_page.rs |
| Scope chips | §3 DS-C-06 | `chip` | kittest_search_page.rs |
| Hit cards (scope badge + mono strong title + muted subtitle + why chips + snippet hint) | §3 DS-C-02 | `card` | kittest_search_page.rs |
| Empty state (Try-"export" CTA over the real FTS engine) | §3 DS-C-14 with CTA | `empty_state_cta` | kittest_search_page.rs |

## 18. SupportGraph — DS-A-02 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + result-count pill | §3 DS-C-01 | `page_header`, `page_counter` | kittest_support_graph_page.rs |
| Result rows (full-width selectable, live-chats pattern) | §3 DS-C-05 row anatomy (selection list — folded helper) | `card_titled` rows | kittest_support_graph_page.rs |
| Neighborhood edges (DS.row pairs: relation strong left, origin mono right) | §3 DS-C-02 (+DS-C-18 kv) | `card_titled` | kittest_support_graph_page.rs |
| Empty state | §3 DS-C-14 | `empty_state` | kittest_support_graph_page.rs |

## 19. Settings — DS-A-04 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_settings_page.rs |
| Tab rail below header (six labels, same state machine) | §3 DS-C-09 | `tabs` | kittest_settings_page.rs |
| Form sections (FormGrid + field labels; every control `labelled_by` a11y) | §3 DS-C-12/13 | `form_grid`, `field_label` | kittest_settings_page.rs |
| Danger zones (danger Card naming, honest sub) | §3 DS-C-02 `danger` | `card_titled` | kittest_settings_page.rs |
| Destructive arms (two-step restore, purge) | §3 DS-C-16 armed rule (A-5) | `armed_danger_button` rule (shell confirm) | kittest_settings_page.rs |
| Status feedback | §3 DS-C-15 `status-line` | `success_text`, `error_text` | kittest_settings_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_settings_page.rs |

## 20. Connectors — DS-A-04 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head + count pill beside New Connector primary | §3 DS-C-01 | `page_header`, `page_counter` | kittest_connectors_page.rs |
| Connector cards (health = dot + badge pair; rows/refresh/delete → refresh+delete per A-5 receipt) | §3 DS-C-02 + DS-C-06 + C-1 receipt | `card`, `status_dot`, `badge`, `quirk` | kittest_connectors_page.rs |
| New-Connector form (every input bound) | §3 DS-C-12/13 | `form_grid`, `field_label` | kittest_connectors_page.rs |
| Empty regions | §3 DS-C-14 | `empty_state` | kittest_connectors_page.rs |

## 21. Notifications — DS-A-04 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_notifications_page.rs |
| Tab rail (Mentions tab interaction golden) | §3 DS-C-09 | `tabs` | kittest_notifications_page.rs |
| Mark-all-read act (text button) | §3 DS-C-11 `text` | `button` | kittest_notifications_page.rs |
| Notification cards + real unread state dot (`ds-udot`) | §3 DS-C-02 + DS-C-06 | `card`, `status_dot` | kittest_notifications_page.rs |
| Raw type badge + rel-time (raw UTC in hover title — C-1 receipt) | §3 DS-C-06 via §5 + `quirk` | `badge`, `quirk` | kittest_notifications_page.rs |
| Empty state (lab copy) | §3 DS-C-14 | `empty_state` | kittest_notifications_page.rs |

## 22. Onboarding — DS-A-04 (§4)

| Screen region | DC trace | Widget | Golden lock |
|---|---|---|---|
| Page head | §3 DS-C-01 | `page_header` | kittest_onboarding_page.rs |
| Setup status card (DS.row anatomy: okFlag dot + strong key left, mono value right) | §3 DS-C-02 (+DS-C-18 kv, DS-C-06 dot) | `card_titled`, `status_dot` | kittest_onboarding_page.rs |
| Hints | §3 DS-C-15 `hint` | `hint_text` | kittest_onboarding_page.rs |

## Coverage reconciliation (gate-enforced)

- Routes (`PageKey`): 23 — all carry a section above (Shell covered by §0 + the
  Shell route's own row in the kittest exception note).
- Golden locks: 21 page suites + the widget suite + the copilot-state suite
  + style lint = the 24-suite / 175-test (173 kittest + 2 style lint) /
  171-golden set under `crates/app-shell/tests/` (RS-4-8: the copilot-open
  state class — never locked before the G4 rejection — is now
  `kittest_inbox_copilot.rs`, light + dark at the owner's live aspect).
- Exceptions in force: **E-1** (blur shadows → hairline strokes; motion only where
  egui animates — DC §7), **C-1** receipts (footnoted data-layer quirks, never faked
  fixed), **C-2** labels (data-track items marked mocked pending the async track).
