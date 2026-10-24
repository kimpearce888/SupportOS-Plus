# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — batch 36 (opened 2026-10-25)

The owner resolved the three parked decisions on 2026-10-25 with an
explicit "wire it" for each; the items moved here from `plan.md` per
Rule 2 before implementation began. This is the final parked tail of
the v1.7.0 clean-audit queue (three items, under the ten-item cap).

56. [med|gap] Entire report-builder/dashboard surface computed but never
    rendered (32 endpoints) — reports.rs:1110+ — owner decision: WIRE.
    Scope: the report builder rendered as a Reports-page Builder tab
    (catalog-driven metric/dimension form, date range + comparison +
    sort + limit, the filter set, run via run_builder_report with
    parse_report_config/config_to_json backing saved-report CRUD through
    list_saved_reports/save_report/delete_saved_report); the dashboard
    surface rendered as an Insights tab (get_health_facts over
    get_dashboard_metrics, get_effectiveness_report, the friction stack
    with a bounded recompute action feeding compute_friction and reading
    count_high_friction/friction_findings, the graph_nodes surface via
    list/add/get/count_graph_nodes + get_graph_neighbors); and
    record_timeline_event/get_timeline rendered on the Customers page as
    the per-customer support-milestone timeline with real writers
    (persisted SLA breaches and high-friction recomputes).

150. [low|gap] intelligence_features ~70% dead (SLA-breach + knowledge-gap
    tracking never gain rows) — owner decision: WIRE. Scope: a workers
    intelligence sweep that persists sla_breaches from the live
    sla::sla_alerts evaluation (deduped per conversation + breach kind)
    with customer_timeline milestones, honors set_sla_config/
    get_sla_config thresholds as the coarse wall-clock tracker for
    mailboxes the live business-hours system does not monitor, records
    record_knowledge_gap on zero-hit knowledge searches with
    list_knowledge_gaps surfaced on the Knowledge page, stamps
    update_doc_freshness on knowledge ingest, and marks/prunes
    knowledge_doc_freshness stale state on the sweep with
    count_stale_docs surfaced.

151. [low|gap] ai_settings write API dead (provider_kind inert plumbing)
    — ai_center.rs:135-179 — owner decision: WIRE. Scope: the AI Center
    Health tab gains a provider panel that writes set_provider_kind /
    set_chat_model / set_embedding_model / set_base_url over the
    ai_settings row — the pre-hub fallback layer
    ai_pipeline::backend_from_settings and
    embeddings::lm_studio_embedding_settings already consume — with
    ai_features_enabled surfaced as the provider-ready indicator
    (Settings → LM Studio remains the primary editor for the
    settings-key layer).
