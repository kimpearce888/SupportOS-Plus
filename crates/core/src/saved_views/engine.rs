use super::*;

impl<'a> ViewEngine<'a> {
    #[must_use]
    pub fn new(timezone: impl Into<String>) -> Self {
        Self {
            timezone: timezone.into(),
            now: None,
            resolve_sla: None,
        }
    }

    #[must_use]
    pub fn with_now(mut self, now_ms: i64) -> Self {
        self.now = Some(now_ms);
        self
    }

    #[must_use]
    pub fn with_sla_resolver(mut self, resolver: SlaResolver<'a>) -> Self {
        self.resolve_sla = Some(resolver);
        self
    }

    pub fn compile(
        &self,
        def: &ViewDefinition,
    ) -> std::result::Result<CompiledView, ViewCompileError> {
        let mut notes = Vec::new();
        let fragment =
            self.compile_nodes(&def.conditions, def.combinator == "all", &mut notes, 0)?;
        if fragment.sql.is_empty() {
            return Ok(CompiledView {
                where_sql: "1=1".to_string(),
                params: Vec::new(),
                notes,
            });
        }
        Ok(CompiledView {
            where_sql: fragment.sql,
            params: fragment.params,
            notes,
        })
    }

    fn compile_nodes(
        &self,
        nodes: &[ViewNode],
        intersect: bool,
        notes: &mut Vec<String>,
        depth: u32,
    ) -> std::result::Result<Fragment, ViewCompileError> {
        if depth > MAX_TREE_DEPTH {
            return Err(ViewCompileError::new(
                "Condition tree nesting deeper than supported (max 10).",
            ));
        }
        let mut fragments = Vec::with_capacity(nodes.len());
        for node in nodes {
            fragments.push(self.compile_node(node, notes, depth)?);
        }
        let non_empty: Vec<&Fragment> = fragments.iter().filter(|f| !f.sql.is_empty()).collect();
        if non_empty.is_empty() {
            return Ok(Fragment::empty());
        }
        let joiner = if intersect { " AND " } else { " OR " };
        let sql = non_empty
            .iter()
            .map(|f| format!("({})", f.sql))
            .collect::<Vec<_>>()
            .join(joiner);
        let mut params = Vec::new();
        for f in non_empty {
            params.extend(f.params.iter().cloned());
        }
        Ok(Fragment { sql, params })
    }

    fn compile_node(
        &self,
        node: &ViewNode,
        notes: &mut Vec<String>,
        depth: u32,
    ) -> std::result::Result<Fragment, ViewCompileError> {
        if let ViewNode::Group {
            combinator,
            children,
        } = node
        {
            if children.is_empty() {
                return Ok(Fragment::empty());
            }
            return self.compile_nodes(children, combinator == "all", notes, depth + 1);
        }
        self.compile_condition(node, notes)
    }

    fn compile_condition(
        &self,
        cond: &ViewNode,
        notes: &mut Vec<String>,
    ) -> std::result::Result<Fragment, ViewCompileError> {
        match cond {
            ViewNode::Group { .. } => unreachable!("groups handled in compile_node"),
            ViewNode::Status { statuses } => {
                if statuses.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!("c.status IN ({})", placeholders(statuses.len()).join(",")),
                    params: statuses.iter().map(|s| text(s)).collect(),
                })
            }
            ViewNode::Assignee {
                assignee_local_ids,
                include_unassigned,
            } => {
                let mut parts: Vec<String> = Vec::new();
                let mut params = SqlParams::new();
                if !assignee_local_ids.is_empty() {
                    parts.push(format!(
                        "c.assignee_local_id IN ({})",
                        placeholders(assignee_local_ids.len()).join(",")
                    ));
                    params.extend(assignee_local_ids.iter().map(|id| integer(*id)));
                }
                if *include_unassigned {
                    parts.push("c.assignee_local_id IS NULL".to_string());
                }
                if parts.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: parts.join(" OR "),
                    params,
                })
            }
            ViewNode::Team { team_local_ids } => {
                if team_local_ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.assigned_team_local_id IN ({})",
                        placeholders(team_local_ids.len()).join(",")
                    ),
                    params: team_local_ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::Mailbox { mailbox_local_ids } => {
                if mailbox_local_ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.mailbox_local_id IN ({})",
                        placeholders(mailbox_local_ids.len()).join(",")
                    ),
                    params: mailbox_local_ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::Channel { channels } => {
                if channels.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!("c.type IN ({})", placeholders(channels.len()).join(",")),
                    params: channels.iter().map(|c| text(c)).collect(),
                })
            }
            ViewNode::Tags { tags, mode } => {
                // Audit item 119: the Rust side folded with Unicode rules
                // (`str::to_lowercase`) while SQL `LOWER` folds ASCII
                // only — a tag stored as "ELECTRONICS" (or any name with
                // a non-ASCII letter) produced a parameter the SQL side
                // could never match. Both sides now fold ASCII-only, so
                // the filter agrees with the stored collation: ASCII
                // tags match case-insensitively, non-ASCII names compare
                // symmetrically on both sides.
                let lowered: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();
                if lowered.is_empty() {
                    return Ok(Fragment::empty());
                }
                let tag_exists = |names: &[String]| Fragment {
                    sql: format!(
                        "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id AND LOWER(t.name) IN ({}))",
                        placeholders(names.len()).join(",")
                    ),
                    params: names.iter().map(|n| text(n)).collect(),
                };
                if mode == "any" {
                    return Ok(tag_exists(&lowered));
                }
                if mode == "none" {
                    let any = tag_exists(&lowered);
                    return Ok(Fragment {
                        sql: format!("NOT {}", any.sql),
                        params: any.params,
                    });
                }
                let parts = lowered
                    .iter()
                    .map(|_| "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id AND LOWER(t.name) = ?)")
                    .collect::<Vec<_>>();
                Ok(Fragment {
                    sql: parts.join(" AND "),
                    params: lowered.iter().map(|n| text(n)).collect(),
                })
            }
            ViewNode::CustomField {
                field_local_id,
                op,
                value,
            } => {
                let base = "cf.field_id = ?";
                match op.as_str() {
                    "is_empty" => Ok(Fragment {
                        sql: format!(
                            "NOT EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value IS NOT NULL AND cf.value <> '')"
                        ),
                        params: vec![integer(*field_local_id)],
                    }),
                    "is_not_empty" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value IS NOT NULL AND cf.value <> '')"
                        ),
                        params: vec![integer(*field_local_id)],
                    }),
                    "equals" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND LOWER(cf.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*field_local_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: format!(
                            "NOT EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND LOWER(cf.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*field_local_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "contains" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value LIKE ? ESCAPE '\\')"
                        ),
                        params: vec![
                            integer(*field_local_id),
                            text(&format!(
                                "%{}%",
                                escape_like(value.as_deref().unwrap_or(""))
                            )),
                        ],
                    }),
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported custom_field operator: {other}"
                    ))),
                }
            }
            ViewNode::CustomerProperty {
                definition_id,
                op,
                value,
            } => {
                let def = "cp.definition_id = ?";
                let cust_join = "cp.customer_id = c.customer_local_id";
                match op.as_str() {
                    "is_empty" => Ok(Fragment {
                        sql: format!(
                            "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value IS NOT NULL AND cp.value <> '')"
                        ),
                        params: vec![integer(*definition_id)],
                    }),
                    "is_not_empty" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value IS NOT NULL AND cp.value <> '')"
                        ),
                        params: vec![integer(*definition_id)],
                    }),
                    "equals" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND LOWER(cp.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*definition_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: format!(
                            "NOT EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND LOWER(cp.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*definition_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "contains" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value LIKE ? ESCAPE '\\')"
                        ),
                        params: vec![
                            integer(*definition_id),
                            text(&format!(
                                "%{}%",
                                escape_like(value.as_deref().unwrap_or(""))
                            )),
                        ],
                    }),
                    "gt" | "gte" | "lt" | "lte" => {
                        let n = value
                            .as_deref()
                            .and_then(|v| v.parse::<f64>().ok())
                            .filter(|n| n.is_finite());
                        let Some(n) = n else {
                            return Err(ViewCompileError::new(
                                "customer_property numeric operator requires a numeric value.",
                            ));
                        };
                        let op_sql = match op.as_str() {
                            "gt" => ">",
                            "gte" => ">=",
                            "lt" => "<",
                            _ => "<=",
                        };
                        // Audit item 122: `CAST(cp.value AS REAL)` turns
                        // any non-numeric text into 0.0 — "N/A", "blue",
                        // "yes" all satisfied `lt`/`lte` comparisons
                        // against positive thresholds. Only values that
                        // are actually numeric participate now: at least
                        // one digit, and nothing outside a number's
                        // character set (sign, digits, decimal point,
                        // exponent).
                        Ok(Fragment {
                            sql: format!(
                                "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} \
                                  AND TRIM(cp.value) GLOB '*[0-9]*' \
                                  AND TRIM(cp.value) NOT GLOB '*[^0-9.+-eE]*' \
                                  AND CAST(cp.value AS REAL) {op_sql} ?)"
                            ),
                            params: vec![integer(*definition_id), real(n)],
                        })
                    }
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported customer_property operator: {other}"
                    ))),
                }
            }
            ViewNode::CustomerText { field, op, value } => {
                let v = value.as_deref().unwrap_or("").trim();
                match field.as_str() {
                    "name" => {
                        let name_expr =
                            "TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, ''))";
                        match op.as_str() {
                            "is_empty" => Ok(Fragment {
                                sql: format!("c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} <> '')"),
                                params: Vec::new(),
                            }),
                            "is_not_empty" => Ok(Fragment {
                                sql: format!("EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} <> '')"),
                                params: Vec::new(),
                            }),
                            "contains" => Ok(Fragment {
                                sql: format!("EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} LIKE ? ESCAPE '\\')"),
                                params: vec![text(&format!("%{}%", escape_like(v)))],
                            }),
                            "equals" => Ok(Fragment {
                                sql: format!("EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} = ?)"),
                                params: vec![text(v)],
                            }),
                            "not_contains" => Ok(Fragment {
                                sql: format!("c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} LIKE ? ESCAPE '\\')"),
                                params: vec![text(&format!("%{}%", escape_like(v)))],
                            }),
                            other => Err(ViewCompileError::new(format!(
                                "Unsupported customer_text operator: {other}"
                            ))),
                        }
                    }
                    "email" => match op.as_str() {
                        "is_empty" => Ok(Fragment {
                            sql: "NOT EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id)".to_string(),
                            params: Vec::new(),
                        }),
                        "is_not_empty" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id)".to_string(),
                            params: Vec::new(),
                        }),
                        "contains" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id AND ce.value LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        "equals" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id AND ce.value = ?)".to_string(),
                            params: vec![text(v)],
                        }),
                        "not_contains" => Ok(Fragment {
                            sql: "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id AND ce.value LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        other => Err(ViewCompileError::new(format!(
                            "Unsupported customer_text operator: {other}"
                        ))),
                    },
                    "organization" => match op.as_str() {
                        "is_empty" => Ok(Fragment {
                            sql: "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND cu.organization_id IS NOT NULL AND EXISTS (SELECT 1 FROM organizations o WHERE o.id = cu.organization_id AND o.name <> ''))".to_string(),
                            params: Vec::new(),
                        }),
                        "is_not_empty" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name <> '')".to_string(),
                            params: Vec::new(),
                        }),
                        "contains" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        "equals" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name = ?)".to_string(),
                            params: vec![text(v)],
                        }),
                        "not_contains" => Ok(Fragment {
                            sql: "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        other => Err(ViewCompileError::new(format!(
                            "Unsupported customer_text operator: {other}"
                        ))),
                    },
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported customer_text field: {other}"
                    ))),
                }
            }
            ViewNode::DateActivity {
                activity_field,
                mode,
                from,
                to,
                from_time,
                to_time,
            } => {
                let range = resolve_date_range(&DateRangeInput {
                    mode,
                    timezone: &self.timezone,
                    from: from.as_deref(),
                    to: to.as_deref(),
                    from_time: from_time.as_deref(),
                    to_time: to_time.as_deref(),
                    now: self.now,
                });
                let Some(range) = range else {
                    return Err(ViewCompileError::new(format!(
                        "Date filter '{mode}' requires valid from/to dates (YYYY-MM-DD)."
                    )));
                };
                let Some(column) = activity_field_column(activity_field) else {
                    return Err(ViewCompileError::new(format!(
                        "Unknown activity field: {activity_field}"
                    )));
                };
                notes.push(format!(
                    "Date filter '{}' resolved to {} ({} to {}, {} boundaries, {}).",
                    activity_field,
                    range.label,
                    range.from,
                    range.to,
                    range.kind.as_str(),
                    self.timezone
                ));
                Ok(Fragment {
                    sql: format!("{column} IS NOT NULL AND {column} >= ? AND {column} < ?"),
                    params: vec![text(&range.from), text(&range.to)],
                })
            }
            ViewNode::ResponseState { states } => {
                if states.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "({RESPONSE_STATE_SQL}) IN ({})",
                        placeholders(states.len()).join(",")
                    ),
                    params: states.iter().map(|s| text(s)).collect(),
                })
            }
            ViewNode::ResponseAge {
                metric,
                op,
                minutes,
            } => {
                let Some((metric_sql, _label)) = age_metric_sql(metric) else {
                    return Err(ViewCompileError::new(format!(
                        "Unsupported response_age metric: {metric}"
                    )));
                };
                let op_sql = match op.as_str() {
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    _ => "<=",
                };
                Ok(Fragment {
                    sql: format!("{metric_sql} {op_sql} ?"),
                    params: vec![real(*minutes)],
                })
            }
            ViewNode::Sla { states, negate } => {
                let Some(resolve) = &self.resolve_sla else {
                    return Err(ViewCompileError::new(
                        "SLA conditions require the SLA service.",
                    ));
                };
                let queried: Vec<&str> = states.iter().map(String::as_str).collect();
                let ids = resolve(&queried);
                notes.push(format!(
                    "SLA state evaluated live against current mailbox business-hours ({} conversation(s) currently {}).",
                    ids.len(),
                    states.join("/")
                ));
                if ids.is_empty() {
                    return Ok(Fragment {
                        sql: if *negate { "1=1" } else { "1=0" }.to_string(),
                        params: Vec::new(),
                    });
                }
                let in_list = placeholders(ids.len()).join(",");
                Ok(Fragment {
                    sql: if *negate {
                        format!("c.id NOT IN ({in_list})")
                    } else {
                        format!("c.id IN ({in_list})")
                    },
                    params: ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::Priority { priorities } => {
                if priorities.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.supportos_priority IN ({})",
                        placeholders(priorities.len()).join(",")
                    ),
                    params: priorities.iter().map(|p| text(p)).collect(),
                })
            }
            ViewNode::TicketState {
                state_ids,
                include_no_state,
            } => {
                let mut parts: Vec<String> = Vec::new();
                let mut params = SqlParams::new();
                if !state_ids.is_empty() {
                    parts.push(format!(
                        "c.supportos_state_id IN ({})",
                        placeholders(state_ids.len()).join(",")
                    ));
                    params.extend(state_ids.iter().map(|id| integer(*id)));
                }
                if *include_no_state {
                    parts.push("c.supportos_state_id IS NULL".to_string());
                }
                if parts.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: parts.join(" OR "),
                    params,
                })
            }
            ViewNode::KnownIssue {
                any,
                known_issue_ids,
            } => {
                if *any {
                    return Ok(Fragment {
                        sql: "EXISTS (SELECT 1 FROM known_issue_links kic WHERE kic.conversation_id = c.id)".to_string(),
                        params: Vec::new(),
                    });
                }
                let Some(ids) = known_issue_ids else {
                    return Ok(Fragment::empty());
                };
                if ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "EXISTS (SELECT 1 FROM known_issue_links kic WHERE kic.conversation_id = c.id AND kic.known_issue_id IN ({}))",
                        placeholders(ids.len()).join(",")
                    ),
                    params: ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::AiAnalyzed { analyzed } => Ok(Fragment {
                sql: if *analyzed {
                    "EXISTS (SELECT 1 FROM ai_runs ar WHERE ar.conversation_id = c.id AND ar.type = 'ticket_analysis' AND ar.status = 'completed')".to_string()
                } else {
                    "NOT EXISTS (SELECT 1 FROM ai_runs ar WHERE ar.conversation_id = c.id AND ar.type = 'ticket_analysis' AND ar.status = 'completed')".to_string()
                },
                params: Vec::new(),
            }),
            ViewNode::InteractionSignal {
                dimension,
                value,
                negate,
            } => {
                let frag = Fragment {
                    sql: "EXISTS (SELECT 1 FROM client_current_signals s, json_each(s.signals_json) je WHERE s.conversation_id = c.id AND json_extract(je.value, '$.dimension') = ? AND json_extract(je.value, '$.value') = ?)".to_string(),
                    params: vec![text(dimension), text(value)],
                };
                Ok(Fragment {
                    sql: if *negate {
                        format!("NOT {}", frag.sql)
                    } else {
                        frag.sql
                    },
                    params: frag.params,
                })
            }
            ViewNode::AiAttribute {
                attribute,
                op,
                value,
            } => {
                let Some(def) = crate::catalog::AiAttributeKey::parse(attribute) else {
                    return Err(ViewCompileError::new(format!(
                        "Unknown AI attribute: {attribute}"
                    )));
                };
                let attr = attribute.as_str();
                if value.to_lowercase() == "unknown" {
                    if op != "equals" {
                        return Err(ViewCompileError::new(
                            "Only 'equals unknown' is supported for the unknown value.",
                        ));
                    }
                    return Ok(Fragment {
                        sql: "NOT EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL)".to_string(),
                        params: vec![text(attr)],
                    });
                }
                if def.value_type() == crate::catalog::AttributeValueType::Number {
                    let Ok(n) = value.parse::<f64>() else {
                        return Err(ViewCompileError::new(
                            "ai_attribute numeric operator requires a numeric value.",
                        ));
                    };
                    if !n.is_finite() {
                        return Err(ViewCompileError::new(
                            "ai_attribute numeric operator requires a numeric value.",
                        ));
                    }
                    let op_sql = match op.as_str() {
                        "gt" => ">",
                        "gte" => ">=",
                        "lt" => "<",
                        "lte" => "<=",
                        _ => {
                            return Err(ViewCompileError::new(format!(
                                "ai_attribute (number) does not support operator {op}."
                            )));
                        }
                    };
                    return Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND CAST(aa.value AS REAL) {op_sql} ?)"
                        ),
                        params: vec![text(attr), real(n)],
                    });
                }
                match op.as_str() {
                    "equals" => Ok(Fragment {
                        sql: "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND LOWER(aa.value) = LOWER(?))".to_string(),
                        params: vec![text(attr), text(value)],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: "NOT EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND LOWER(aa.value) = LOWER(?))".to_string(),
                        params: vec![text(attr), text(value)],
                    }),
                    "contains" => Ok(Fragment {
                        sql: "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND aa.value LIKE ? ESCAPE '\\')".to_string(),
                        params: vec![text(attr), text(&format!("%{}%", escape_like(value)))],
                    }),
                    "not_contains" => Ok(Fragment {
                        sql: "NOT EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND aa.value LIKE ? ESCAPE '\\')".to_string(),
                        params: vec![text(attr), text(&format!("%{}%", escape_like(value)))],
                    }),
                    "gt" | "gte" | "lt" | "lte" => {
                        let vocab = def.values();
                        let Some(idx) = vocab.iter().position(|v| *v == value) else {
                            return Err(ViewCompileError::new(format!(
                                "ai_attribute '{attr}' comparison requires a value from its vocabulary ({}).",
                                vocab.join(", ")
                            )));
                        };
                        let bounds: Vec<&str> = match op.as_str() {
                            "gt" => vocab[idx + 1..].to_vec(),
                            "gte" => vocab[idx..].to_vec(),
                            "lt" => vocab[..idx].to_vec(),
                            _ => vocab[..idx + 1].to_vec(),
                        };
                        if bounds.is_empty() {
                            return Ok(Fragment {
                                sql: "1=0".to_string(),
                                params: Vec::new(),
                            });
                        }
                        let mut params = vec![text(attr)];
                        params.extend(bounds.iter().map(|b| text(b)));
                        Ok(Fragment {
                            sql: format!(
                                "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND aa.value IN ({}))",
                                placeholders(bounds.len()).join(",")
                            ),
                            params,
                        })
                    }
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported ai_attribute operator: {other}"
                    ))),
                }
            }
            ViewNode::Unread { unread } => Ok(Fragment {
                sql: "c.is_unread = ?".to_string(),
                params: vec![integer(i64::from(*unread))],
            }),
            ViewNode::Snoozed { snoozed } => Ok(Fragment {
                sql: if *snoozed {
                    "c.snoozed_until IS NOT NULL AND c.snoozed_until > datetime('now')".to_string()
                } else {
                    "c.snoozed_until IS NULL OR c.snoozed_until <= datetime('now')".to_string()
                },
                params: Vec::new(),
            }),
            ViewNode::Customer { customer_local_ids } => {
                if customer_local_ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.customer_local_id IN ({})",
                        placeholders(customer_local_ids.len()).join(",")
                    ),
                    params: customer_local_ids.iter().map(|id| integer(*id)).collect(),
                })
            }
        }
    }
}
