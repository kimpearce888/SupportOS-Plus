use super::*;

impl<'a> SegmentEngine<'a> {
    pub(super) fn tickets_for_customer_matching(&self, cid: i64, t: &SegmentNode) -> Vec<Value> {
        let SegmentNode::Ticket {
            tags,
            tag_mode,
            statuses,
            created_within_days,
            modified_within_days,
            ..
        } = t
        else {
            return Vec::new();
        };
        let mut where_clauses = vec![
            "c.deleted_at IS NULL".to_string(),
            "c.customer_local_id = ?N_CID".to_string(),
        ];
        let mut p: SqlParams = vec![integer(cid)];
        let statuses: Vec<String> = statuses.iter().filter(|s| !s.is_empty()).cloned().collect();
        if !statuses.is_empty() {
            let ph = (0..statuses.len())
                .map(|i| format!("?N_ST{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            where_clauses.push(format!("c.status IN ({ph})"));
            p.extend(statuses.iter().map(|s| text(s)));
        }
        if let Some(days) = created_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', ?N_CD)"
                    .to_string(),
            );
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        if let Some(days) = modified_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_updated_at, c.updated_at, c.created_at)) >= julianday('now', ?N_MD)".to_string());
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        let tags_clean: Vec<String> = tags
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let tag_mode = if tag_mode.is_empty() {
            "any"
        } else {
            tag_mode.as_str()
        };
        if !tags_clean.is_empty() {
            let ph = (0..tags_clean.len())
                .map(|i| format!("?N_TG{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let clause = if tag_mode == "all" {
                format!(
                    "(SELECT COUNT(DISTINCT LOWER(tg.name)) FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph})) = {}",
                    tags_clean.len()
                )
            } else if tag_mode == "none" {
                format!(
                    "NOT EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                )
            } else {
                format!(
                    "EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                )
            };
            where_clauses.push(clause);
            p.extend(tags_clean.iter().map(|t| text(&lower(t))));
        }
        let sql = format!(
            "SELECT c.id, c.number, c.subject, c.status, COALESCE(c.remote_created_at, c.created_at),
                    (SELECT GROUP_CONCAT(tg.name) FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id) AS tags
               FROM conversations c WHERE {}
              ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC LIMIT 20",
            where_clauses.join(" AND "),
        );
        let sql = renumber_placeholders(&sql, &mut 0);
        let Ok(mut stmt) = self.conn.prepare(&sql) else {
            return Vec::new();
        };
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let rows = stmt
            .query_map(refs.as_slice(), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect::<Vec<_>>())
            .unwrap_or_default();
        rows.into_iter()
            .map(|(id, number, subject, status, created_at, tags)| json!({
                "conversationId": id,
                "number": number,
                "subject": subject,
                "status": status,
                "tags": tags.map(|t| t.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect::<Vec<_>>())
                    .unwrap_or_default(),
                "createdAt": created_at,
            }))
            .collect()
    }

    pub(super) fn collect_matching_tickets(&self, cid: i64, def: &SegmentDefinition) -> Vec<Value> {
        let mut seen: HashMap<i64, Value> = HashMap::new();
        fn walk(
            engine: &SegmentEngine<'_>,
            cid: i64,
            nodes: &[SegmentNode],
            seen: &mut HashMap<i64, Value>,
        ) {
            for n in nodes {
                match n {
                    SegmentNode::Group { children, .. } => walk(engine, cid, children, seen),
                    SegmentNode::Ticket { .. } => {
                        for t in engine.tickets_for_customer_matching(cid, n) {
                            let id = t
                                .get("conversationId")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            seen.entry(id).or_insert(t);
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(self, cid, &def.conditions, &mut seen);
        walk(self, cid, &def.exclude, &mut seen);
        let mut out: Vec<Value> = seen.into_values().collect();
        out.sort_by(|a, b| {
            let ka = a.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
            let kb = b.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
            kb.cmp(ka)
        });
        out
    }

    pub(super) fn eval_customer_property(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CustomerProperty {
            definition_id,
            op,
            value,
            value2,
            values,
            ..
        } = n
        else {
            return Vec::new();
        };
        let val = value.as_deref().unwrap_or("").trim().to_string();
        match op.as_str() {
            "is_empty" => self.ids(
                "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                 (SELECT cp.customer_id FROM customer_properties cp
                   WHERE cp.definition_id = ?1 AND cp.value IS NOT NULL AND cp.value <> '')",
                &[integer(*definition_id)],
            ),
            "is_not_empty" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value IS NOT NULL AND cp.value <> ''",
                &[integer(*definition_id)],
            ),
            "equals" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND LOWER(cp.value) = LOWER(?2)",
                &[integer(*definition_id), text(&val)],
            ),
            "not_equals" => self.ids(
                "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                 (SELECT cp.customer_id FROM customer_properties cp
                   WHERE cp.definition_id = ?1 AND LOWER(cp.value) = LOWER(?2))",
                &[integer(*definition_id), text(&val)],
            ),
            "contains" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\'",
                &[
                    integer(*definition_id),
                    text(&format!("%{}%", escape_like(&val))),
                ],
            ),
            "not_contains" => self.ids(
                "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                 (SELECT cp.customer_id FROM customer_properties cp
                   WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\')",
                &[
                    integer(*definition_id),
                    text(&format!("%{}%", escape_like(&val))),
                ],
            ),
            "starts_with" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\'",
                &[
                    integer(*definition_id),
                    text(&format!("{}%", escape_like(&val))),
                ],
            ),
            "ends_with" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\'",
                &[
                    integer(*definition_id),
                    text(&format!("%{}", escape_like(&val))),
                ],
            ),
            "gt" | "gte" | "lt" | "lte" => {
                let Some(num) = val.parse::<f64>().ok() else {
                    return Vec::new();
                };
                let op_sql = match op.as_str() {
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    _ => "<=",
                };
                self.ids(
                    &format!(
                        "SELECT cp.customer_id AS cid FROM customer_properties cp
                               WHERE cp.definition_id = ?1 AND CAST(cp.value AS REAL) {op_sql} ?2"
                    ),
                    &[integer(*definition_id), real(num)],
                )
            }
            "between" => {
                let (Some(a), b) = (
                    val.parse::<f64>().ok(),
                    value2.as_deref().unwrap_or("").to_string(),
                ) else {
                    return Vec::new();
                };
                let Some(b) = b.trim().parse::<f64>().ok() else {
                    return Vec::new();
                };
                self.ids(
                    "SELECT cp.customer_id AS cid FROM customer_properties cp
                      WHERE cp.definition_id = ?1 AND CAST(cp.value AS REAL) BETWEEN ?2 AND ?3",
                    &[integer(*definition_id), real(a.min(b)), real(a.max(b))],
                )
            }
            "before" => {
                if val.is_empty() {
                    return Vec::new();
                }
                self.ids(
                    "SELECT cp.customer_id AS cid FROM customer_properties cp
                      WHERE cp.definition_id = ?1 AND cp.value < ?2",
                    &[integer(*definition_id), text(&val)],
                )
            }
            "after" => {
                if val.is_empty() {
                    return Vec::new();
                }
                self.ids(
                    "SELECT cp.customer_id AS cid FROM customer_properties cp
                      WHERE cp.definition_id = ?1 AND cp.value > ?2",
                    &[integer(*definition_id), text(&val)],
                )
            }
            "is_any_of" => {
                let list: Vec<String> = values
                    .iter()
                    .map(|s| lower(s))
                    .filter(|s| !s.is_empty())
                    .collect();
                if list.is_empty() {
                    return Vec::new();
                }
                let mut p = vec![integer(*definition_id)];
                p.extend(list.iter().map(|s| text(s)));
                self.ids(
                    &format!(
                        "SELECT cp.customer_id AS cid FROM customer_properties cp
                               WHERE cp.definition_id = ?1 AND LOWER(cp.value) IN ({})",
                        placeholders(list.len())
                    ),
                    &p,
                )
            }
            "is_none_of" => {
                let list: Vec<String> = values
                    .iter()
                    .map(|s| lower(s))
                    .filter(|s| !s.is_empty())
                    .collect();
                if list.is_empty() {
                    return self.all_customer_ids();
                }
                let mut p = vec![integer(*definition_id)];
                p.extend(list.iter().map(|s| text(s)));
                self.ids(
                    &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                               (SELECT cp.customer_id FROM customer_properties cp
                                 WHERE cp.definition_id = ?1 AND LOWER(cp.value) IN ({}))",
                             placeholders(list.len())),
                    &p,
                )
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn eval_contact(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::Contact { field, op, value } = n else {
            return Vec::new();
        };
        let val = value.as_deref().unwrap_or("").trim().to_string();
        match field.as_str() {
            "has_email" | "has_phone" => {
                let table = if field == "has_email" {
                    "customer_emails"
                } else {
                    "customer_phones"
                };
                if op == "is_empty" {
                    self.ids(
                        &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL
                                   AND NOT EXISTS (SELECT 1 FROM {table} x WHERE x.customer_id = c.id)"),
                        &[],
                    )
                } else {
                    self.ids(
                        &format!("SELECT DISTINCT x.customer_id AS cid FROM {table} x
                                   JOIN customers c ON c.id = x.customer_id WHERE c.deleted_at IS NULL"),
                        &[],
                    )
                }
            }
            "has_multiple_emails" => self.ids(
                "SELECT ce.customer_id AS cid FROM customer_emails ce
                   JOIN customers c ON c.id = ce.customer_id WHERE c.deleted_at IS NULL
                  GROUP BY ce.customer_id HAVING COUNT(*) > 1",
                &[],
            ),
            "email" => {
                if op == "is_empty" || op == "is_not_empty" {
                    return if op == "is_empty" {
                        self.ids(
                            "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL
                              AND NOT EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.id)",
                            &[],
                        )
                    } else {
                        self.ids(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id WHERE c.deleted_at IS NULL",
                            &[],
                        )
                    };
                }
                if val.is_empty() {
                    return Vec::new();
                }
                match op.as_str() {
                    "equals" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND LOWER(ce.value) = LOWER(?1)",
                        &[text(&val)],
                    ),
                    "not_equals" => self.ids(
                        "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                           (SELECT ce.customer_id FROM customer_emails ce WHERE LOWER(ce.value) = LOWER(?1))",
                        &[text(&val)],
                    ),
                    "contains" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND ce.value LIKE ?1 ESCAPE '\\'",
                        &[text(&format!("%{}%", escape_like(&val)))],
                    ),
                    "starts_with" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND ce.value LIKE ?1 ESCAPE '\\'",
                        &[text(&format!("{}%", escape_like(&val)))],
                    ),
                    "ends_with" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND ce.value LIKE ?1 ESCAPE '\\'",
                        &[text(&format!("%{}", escape_like(&val)))],
                    ),
                    _ => Vec::new(),
                }
            }
            "email_domain" => {
                match op.as_str() {
                    "is_empty" => {
                        return self.ids(
                            "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                               (SELECT ce.customer_id FROM customer_emails ce)",
                            &[],
                        );
                    }
                    "is_not_empty" => {
                        return self.ids(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id
                              WHERE c.deleted_at IS NULL",
                            &[],
                        );
                    }
                    _ => {}
                }
                let domain = val.trim_start_matches('@').to_lowercase();
                if domain.is_empty() {
                    return Vec::new();
                }
                const DOMAIN_EXPR: &str = "LOWER(SUBSTR(ce.value, INSTR(ce.value, '@') + 1))";
                match op.as_str() {
                    "equals" => self.ids(
                        &format!(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id
                              WHERE c.deleted_at IS NULL AND {DOMAIN_EXPR} = ?1"
                        ),
                        &[text(&domain)],
                    ),
                    "not_equals" => self.ids(
                        "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                           (SELECT ce.customer_id FROM customer_emails ce
                             WHERE LOWER(SUBSTR(ce.value, INSTR(ce.value, '@') + 1)) = ?1)",
                        &[text(&domain)],
                    ),
                    "contains" => self.ids(
                        &format!(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id
                              WHERE c.deleted_at IS NULL AND {DOMAIN_EXPR} LIKE '%' || ?1 || '%' ESCAPE '\\'"
                        ),
                        &[text(&escape_like(&domain))],
                    ),
                    "starts_with" => self.ids(
                        &format!(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id
                              WHERE c.deleted_at IS NULL AND {DOMAIN_EXPR} LIKE ?1 || '%' ESCAPE '\\'"
                        ),
                        &[text(&escape_like(&domain))],
                    ),
                    "ends_with" => self.ids(
                        &format!(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id
                              WHERE c.deleted_at IS NULL AND {DOMAIN_EXPR} LIKE '%' || ?1 ESCAPE '\\'"
                        ),
                        &[text(&escape_like(&domain))],
                    ),
                    _ => Vec::new(),
                }
            }
            _ => {
                let expr = match field.as_str() {
                    "name" => "(COALESCE(c.first_name,'') || ' ' || COALESCE(c.last_name,''))",
                    "organization" => "o.name",
                    "job_title" => "c.job_title",
                    "location" => "c.location",
                    "background" => "c.background",
                    _ => return Vec::new(),
                };
                let from = "FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id";
                match op.as_str() {
                    "is_empty" => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND ({expr} IS NULL OR TRIM({expr}) = '')"),
                        &[],
                    ),
                    "is_not_empty" => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND ({expr} IS NOT NULL AND TRIM({expr}) <> '')"),
                        &[],
                    ),
                    "equals" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND LOWER({expr}) = LOWER(?1)"),
                        &[text(&val)],
                    ),
                    "not_equals" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND (LOWER({expr}) <> LOWER(?1) OR {expr} IS NULL)"),
                        &[text(&val)],
                    ),
                    "contains" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND {expr} LIKE ?1 ESCAPE '\\'"),
                        &[text(&format!("%{}%", escape_like(&val)))],
                    ),
                    "starts_with" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND {expr} LIKE ?1 ESCAPE '\\'"),
                        &[text(&format!("{}%", escape_like(&val)))],
                    ),
                    "ends_with" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND {expr} LIKE ?1 ESCAPE '\\'"),
                        &[text(&format!("%{}", escape_like(&val)))],
                    ),
                    _ => Vec::new(),
                }
            }
        }
    }

    pub(super) fn eval_ticket(&self, n: &SegmentNode, notes: &mut Vec<String>) -> Vec<i64> {
        let SegmentNode::Ticket {
            tags,
            tag_mode,
            statuses,
            mailbox_ids,
            assignee_ids,
            created_within_days,
            modified_within_days,
            number_min,
            number_max,
            ai_attribute,
            custom_fields,
            channel,
        } = n
        else {
            return Vec::new();
        };

        let mut where_clauses: Vec<String> = vec![
            "c.deleted_at IS NULL".to_string(),
            "c.customer_local_id IS NOT NULL".to_string(),
        ];
        let mut p: SqlParams = Vec::new();

        let statuses: Vec<String> = statuses.iter().filter(|s| !s.is_empty()).cloned().collect();
        if !statuses.is_empty() {
            where_clauses.push(format!("c.status IN ({})", placeholders(statuses.len())));
            p.extend(statuses.iter().map(|s| text(s)));
        }
        let mailboxes: Vec<i64> = mailbox_ids.iter().copied().filter(|id| *id > 0).collect();
        if !mailboxes.is_empty() {
            where_clauses.push(format!(
                "c.mailbox_local_id IN ({})",
                placeholders(mailboxes.len())
            ));
            p.extend(mailboxes.iter().map(|id| integer(*id)));
        }
        let assignees: Vec<i64> = assignee_ids.clone();
        if !assignees.is_empty() {
            let unassigned = assignees.contains(&-1);
            let listed: Vec<i64> = assignees.iter().copied().filter(|a| *a != -1).collect();
            if unassigned && !listed.is_empty() {
                where_clauses.push(format!(
                    "(c.assignee_local_id IS NULL OR c.assignee_local_id IN ({}))",
                    placeholders(listed.len())
                ));
                p.extend(listed.iter().map(|id| integer(*id)));
            } else if unassigned {
                where_clauses.push("c.assignee_local_id IS NULL".to_string());
            } else {
                where_clauses.push(format!(
                    "c.assignee_local_id IN ({})",
                    placeholders(listed.len())
                ));
                p.extend(listed.iter().map(|id| integer(*id)));
            }
        }
        if let Some(ch) = channel {
            let ch = ch.trim();
            if !ch.is_empty() {
                where_clauses.push("LOWER(COALESCE(c.source_type, '')) = LOWER(?N_CH)".to_string());
                p.push(text(ch));
            }
        }
        if let Some(days) = created_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', ?N_CD)"
                    .to_string(),
            );
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        if let Some(days) = modified_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_updated_at, c.updated_at, c.created_at)) >= julianday('now', ?N_MD)".to_string());
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        if let Some(nmin) = number_min.filter(|d| d.is_finite()) {
            where_clauses.push("c.number >= ?N_MIN".to_string());
            p.push(real(nmin));
        }
        if let Some(nmax) = number_max.filter(|d| d.is_finite()) {
            where_clauses.push("c.number <= ?N_MAX".to_string());
            p.push(real(nmax));
        }

        let tags_clean: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let tag_mode = if tag_mode.is_empty() {
            "any"
        } else {
            tag_mode.as_str()
        };
        if !tags_clean.is_empty() {
            let ph = placeholders(tags_clean.len());
            if tag_mode == "all" {
                where_clauses.push(format!(
                    "(SELECT COUNT(DISTINCT LOWER(tg.name)) FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph})) = {}",
                    tags_clean.len()
                ));
                p.extend(tags_clean.iter().map(|t| text(&lower(t))));
                if tags_clean.len() > 1 {
                    notes.push(format!(
                        "Tag mode ALL requires one single conversation carrying all {} tags (conversation-level intersection).",
                        tags_clean.len()));
                }
            } else if tag_mode == "none" {
                where_clauses.push(format!(
                    "NOT EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                ));
                p.extend(tags_clean.iter().map(|t| text(&lower(t))));
            } else {
                where_clauses.push(format!(
                    "EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                ));
                p.extend(tags_clean.iter().map(|t| text(&lower(t))));
            }
        }

        if let Some(aa) = ai_attribute {
            let Some(def) = crate::catalog::AiAttributeKey::parse(&aa.attribute) else {
                notes.push(format!(
                    "AI attribute '{}' is not in the catalog - condition matched no conversations.",
                    aa.attribute
                ));
                return Vec::new();
            };
            if aa.value.to_lowercase() == "unknown" {
                if aa.op != "equals" {
                    return Vec::new();
                }
                where_clauses.push(
                    "NOT EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                       AND a.attribute = ?N_AA AND a.superseded_at IS NULL)"
                        .to_string(),
                );
                p.push(text(&aa.attribute));
            } else if def.value_type() == crate::catalog::AttributeValueType::Number {
                let Ok(num) = aa.value.parse::<f64>() else {
                    return Vec::new();
                };
                let Some(op_sql) = (match aa.op.as_str() {
                    "gt" => Some(">"),
                    "gte" => Some(">="),
                    "lt" => Some("<"),
                    "lte" => Some("<="),
                    _ => None,
                }) else {
                    return Vec::new();
                };
                where_clauses.push(format!(
                    "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                       AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                       AND CAST(a.value AS REAL) {op_sql} ?N_AA2)"
                ));
                p.push(text(&aa.attribute));
                p.push(real(num));
            } else {
                match aa.op.as_str() {
                    "equals" => {
                        where_clauses.push(
                            "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND LOWER(a.value) = LOWER(?N_AA2))"
                                .to_string(),
                        );
                        p.push(text(&aa.attribute));
                        p.push(text(&aa.value));
                    }
                    "not_equals" => {
                        where_clauses.push(
                            "NOT EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND LOWER(a.value) = LOWER(?N_AA2))".to_string());
                        p.push(text(&aa.attribute));
                        p.push(text(&aa.value));
                    }
                    "contains" => {
                        where_clauses.push(
                            "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND a.value LIKE ?N_AA2 ESCAPE '\\')"
                                .to_string(),
                        );
                        p.push(text(&aa.attribute));
                        p.push(text(&format!("%{}%", escape_like(&aa.value))));
                    }
                    "gt" | "gte" | "lt" | "lte" => {
                        let vocab: Vec<String> =
                            def.values().iter().map(|v| v.to_string()).collect();
                        let Some(idx) = vocab.iter().position(|v| *v == aa.value) else {
                            return Vec::new();
                        };
                        let list: Vec<String> = if aa.op == "gt" || aa.op == "gte" {
                            vocab[if aa.op == "gt" { idx + 1 } else { idx }..].to_vec()
                        } else {
                            vocab[..if aa.op == "lt" { idx } else { idx + 1 }].to_vec()
                        };
                        if list.is_empty() {
                            return Vec::new();
                        }
                        where_clauses.push(format!(
                            "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND a.value IN ({}))",
                            placeholders(list.len())
                        ));
                        p.push(text(&aa.attribute));
                        p.extend(list.iter().map(|v| text(v)));
                    }
                    _ => return Vec::new(),
                }
            }
            notes.push(format!(
                "AI attribute '{}' {} '{}' evaluated over current local attribute rows (missing = unknown).",
                def.label(), aa.op, aa.value));
        }

        for cf in custom_fields {
            if cf.field_local_id <= 0 {
                continue;
            }
            let exists: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM inbox_fields WHERE id = ?1",
                    params![cf.field_local_id],
                    |r| r.get(0),
                )
                .ok();
            if exists.is_none() {
                notes.push(format!(
                    "Custom field #{} does not exist locally - condition matched no conversations.",
                    cf.field_local_id
                ));
                return Vec::new();
            }
            let val = cf.value.as_deref().unwrap_or("").trim().to_string();
            match cf.op.as_str() {
                "is_empty" => {
                    where_clauses.push(
                        "NOT EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND COALESCE(f.text_value, f.value) IS NOT NULL
                           AND COALESCE(f.text_value, f.value) <> '')".to_string());
                    p.push(integer(cf.field_local_id));
                }
                "is_not_empty" => {
                    where_clauses.push(
                        "EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND COALESCE(f.text_value, f.value) IS NOT NULL
                           AND COALESCE(f.text_value, f.value) <> '')"
                            .to_string(),
                    );
                    p.push(integer(cf.field_local_id));
                }
                "equals" => {
                    if val.is_empty() {
                        return Vec::new();
                    }
                    where_clauses.push(
                        "EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND LOWER(COALESCE(f.text_value, f.value)) = LOWER(?N_CF2))".to_string());
                    p.push(integer(cf.field_local_id));
                    p.push(text(&val));
                }
                "not_equals" => {
                    if val.is_empty() {
                        return Vec::new();
                    }
                    where_clauses.push(
                        "NOT EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND LOWER(COALESCE(f.text_value, f.value)) = LOWER(?N_CF2))".to_string());
                    p.push(integer(cf.field_local_id));
                    p.push(text(&val));
                }
                "contains" => {
                    if val.is_empty() {
                        return Vec::new();
                    }
                    where_clauses.push(
                        "EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND COALESCE(f.text_value, f.value) LIKE ?N_CF2 ESCAPE '\\')".to_string());
                    p.push(integer(cf.field_local_id));
                    p.push(text(&format!("%{}%", escape_like(&val))));
                }
                _ => return Vec::new(),
            }
        }

        let mut sql = format!(
            "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c WHERE {}",
            where_clauses.join(" AND ")
        );
        let mut counter = 0usize;
        sql = renumber_placeholders(&sql, &mut counter);
        self.ids(&sql, &p)
    }

    pub(super) fn eval_history(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::History { metric, op, value } = n else {
            return Vec::new();
        };
        if !value.is_finite() {
            return Vec::new();
        }
        let cmp = match op.as_str() {
            "gte" => ">=",
            "lte" => "<=",
            _ => "=",
        };
        let count_cond = |status_filter: &str| {
            format!(
                "(SELECT COUNT(*) FROM conversations cv WHERE cv.customer_local_id = c.id
                       AND cv.deleted_at IS NULL{status_filter}) {cmp} ?1"
            )
        };
        match metric.as_str() {
            "ticket_count" => self.ids(
                &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND {}",
                         count_cond("")),
                &[real(*value)],
            ),
            "open_count" => self.ids(
                &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND {}",
                         count_cond(" AND cv.status = 'active'")),
                &[real(*value)],
            ),
            "closed_count" => self.ids(
                &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND {}",
                         count_cond(" AND cv.status = 'closed'")),
                &[real(*value)],
            ),
            "last_contact_within_days" => self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN customers cu ON cu.id = c.customer_local_id
                  WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL
                    AND julianday(COALESCE(c.last_activity_at, c.remote_created_at, c.created_at))
                        >= julianday('now', ?1)",
                &[text(&format!("-{} days", value.max(0.0) as i64))],
            ),
            "first_contact_before_days" => self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN customers cu ON cu.id = c.customer_local_id
                  WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL
                    AND julianday(COALESCE(c.remote_created_at, c.created_at)) < julianday('now', ?1)",
                &[text(&format!("-{} days", value.max(0.0) as i64))],
            ),
            "waited_over_hours_count" => {
                let over = self.ids(
                    "SELECT c.customer_local_id AS cid FROM conversations c
                      WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL
                        AND julianday(COALESCE(c.closed_at, datetime('now')))
                            - julianday(COALESCE(c.last_customer_reply_at, c.first_customer_message_at,
                                                 c.remote_created_at, c.created_at)) > ?1",
                    &[real(value.max(0.0) / 24.0)],
                );
                match op.as_str() {
                    "gte" => over,
                    "lte" => {
                        let set: HashSet<i64> = over.into_iter().collect();
                        self.all_customer_ids().into_iter().filter(|id| !set.contains(id)).collect()
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn eval_history_tag(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::HistoryTag { tag, within_days } = n else {
            return Vec::new();
        };
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() {
            return Vec::new();
        }
        if let Some(days) = within_days.filter(|d| d.is_finite()) {
            self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN conversation_tags ct ON ct.conversation_id = c.id
                   JOIN tags tg ON tg.id = ct.tag_id
                   JOIN customers cu ON cu.id = c.customer_local_id
                 WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL AND LOWER(tg.name) = ?1
                   AND julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', ?2)",
                &[text(&tag), text(&format!("-{} days", days.max(0.0) as i64))],
            )
        } else {
            self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN conversation_tags ct ON ct.conversation_id = c.id
                   JOIN tags tg ON tg.id = ct.tag_id
                   JOIN customers cu ON cu.id = c.customer_local_id
                 WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL AND LOWER(tg.name) = ?1",
                &[text(&tag)],
            )
        }
    }

    pub(super) fn eval_organization_property(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::OrganizationProperty {
            field,
            definition_id,
            op,
            value,
            value2,
            values,
            ..
        } = n
        else {
            return Vec::new();
        };
        let val = value.as_deref().unwrap_or("").trim().to_string();
        if field.as_deref() == Some("name") || field.as_deref() == Some("domains") {
            let expr = if field.as_deref() == Some("name") {
                "o.name"
            } else {
                "o.domains"
            };
            match op.as_str() {
                "is_empty" => self.ids(
                    &format!("SELECT c.id AS cid FROM customers c
                               LEFT JOIN organizations o ON o.id = c.organization_id
                              WHERE c.deleted_at IS NULL AND ({expr} IS NULL OR TRIM({expr}) = '')"),
                    &[],
                ),
                "is_not_empty" => self.ids(
                    &format!("SELECT c.id AS cid FROM customers c
                               LEFT JOIN organizations o ON o.id = c.organization_id
                              WHERE c.deleted_at IS NULL AND ({expr} IS NOT NULL AND TRIM({expr}) <> '')"),
                    &[],
                ),
                "equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND LOWER({expr}) LIKE ?N_V"),
            &mut 0),

                        &[text(&format!("%{}%", escape_like(&lower(&val))))],
                    )
                }
                "not_equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   LEFT JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL
                                    AND ({expr} IS NULL OR LOWER({expr}) NOT LIKE ?N_V)"),
            &mut 0),

                        &[text(&format!("%{}%", escape_like(&lower(&val))))],
                    )
                }
                "contains" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND {expr} LIKE ?N_V ESCAPE '\\'"),
            &mut 0),

                        &[text(&format!("%{}%", escape_like(&val)))],
                    )
                }
                "starts_with" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND {expr} LIKE ?N_V ESCAPE '\\'"),
            &mut 0),

                        &[text(&format!("{}%", escape_like(&val)))],
                    )
                }
                "ends_with" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND {expr} LIKE ?N_V ESCAPE '\\'"),
            &mut 0),

                        &[text(&format!("%{}", escape_like(&val)))],
                    )
                }
                _ => Vec::new(),
            }
        } else {
            let Some(def_id) = definition_id.filter(|id| *id > 0) else {
                return Vec::new();
            };
            match op.as_str() {
                "is_empty" => self.ids(
                    "SELECT c.id AS cid FROM customers c
                      WHERE c.deleted_at IS NULL
                        AND (c.organization_id IS NULL OR c.organization_id NOT IN
                             (SELECT op.organization_id FROM organization_properties op
                               WHERE op.definition_id = ?1 AND op.value IS NOT NULL AND op.value <> ''))",
                    &[integer(def_id)],
                ),
                "is_not_empty" => self.ids(
                    "SELECT c.id AS cid FROM customers c
                       JOIN organization_properties op ON op.organization_id = c.organization_id
                      WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                        AND op.value IS NOT NULL AND op.value <> ''",
                    &[integer(def_id)],
                ),
                "equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        "SELECT c.id AS cid FROM customers c
                           JOIN organization_properties op ON op.organization_id = c.organization_id
                          WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                            AND LOWER(op.value) = LOWER(?2)",
                        &[integer(def_id), text(&val)],
                    )
                }
                "not_equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        "SELECT c.id AS cid FROM customers c
                          WHERE c.deleted_at IS NULL
                            AND (c.organization_id IS NULL OR c.organization_id NOT IN
                                 (SELECT op.organization_id FROM organization_properties op
                                   WHERE op.definition_id = ?1 AND LOWER(op.value) = LOWER(?2)))",
                        &[integer(def_id), text(&val)],
                    )
                }
                "contains" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        "SELECT c.id AS cid FROM customers c
                           JOIN organization_properties op ON op.organization_id = c.organization_id
                          WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                            AND op.value LIKE ?2 ESCAPE '\\'",
                        &[integer(def_id), text(&format!("%{}%", escape_like(&val)))],
                    )
                }
                "is_any_of" => {
                    let list: Vec<String> = values.iter().map(|s| lower(s)).filter(|s| !s.is_empty()).collect();
                    if list.is_empty() { return Vec::new(); }
                    let mut p = vec![integer(def_id)];
                    p.extend(list.iter().map(|s| text(s)));
                    self.ids(
                        &format!("SELECT c.id AS cid FROM customers c
                                    JOIN organization_properties op ON op.organization_id = c.organization_id
                                   WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                                     AND LOWER(op.value) IN ({})", placeholders(list.len())),
                        &p,
                    )
                }
                "is_none_of" => {
                    let list: Vec<String> = values.iter().map(|s| lower(s)).filter(|s| !s.is_empty()).collect();
                    if list.is_empty() { return self.all_customer_ids(); }
                    let mut p = vec![integer(def_id)];
                    p.extend(list.iter().map(|s| text(s)));
                    self.ids(
                        &format!("SELECT c.id AS cid FROM customers c
                                   WHERE c.deleted_at IS NULL
                                     AND (c.organization_id IS NULL OR c.organization_id NOT IN
                                          (SELECT op.organization_id FROM organization_properties op
                                            WHERE op.definition_id = ?1 AND LOWER(op.value) IN ({})))",
                                 placeholders(list.len())),
                        &p,
                    )
                }
                "gt" | "gte" | "lt" | "lte" => {
                    let (Some(a), b) = (val.parse::<f64>().ok(), value2.as_deref().unwrap_or("").to_string()) else {
                        return Vec::new();
                    };
                    let _ = b;
                    let op_sql = match op.as_str() {
                        "gt" => ">",
                        "gte" => ">=",
                        "lt" => "<",
                        _ => "<=",
                    };
                    self.ids(
                        &renumber_placeholders(
                            &format!("SELECT c.id AS cid FROM customers c
                                    JOIN organization_properties op ON op.organization_id = c.organization_id
                                   WHERE c.deleted_at IS NULL AND op.definition_id = ?N_D
                                     AND CAST(op.value AS REAL) {op_sql} ?N_V"),
                            &mut 0),
                        &[integer(def_id), real(a)],
                    )
                }
                _ => Vec::new(),
            }
        }
    }

    pub(super) fn eval_history_issue(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::HistoryIssue {
            issue_kind,
            issue_local_id,
            op,
            value,
        } = n
        else {
            return Vec::new();
        };
        if issue_kind != "cluster" && issue_kind != "known_issue" {
            return Vec::new();
        }
        if !value.is_finite() || *value < 0.0 {
            return Vec::new();
        }
        let (link_table, id_col) = if issue_kind == "cluster" {
            ("issue_cluster_conversations", "cluster_id")
        } else {
            ("known_issue_links", "known_issue_id")
        };
        let specific = issue_local_id.map(|id| id > 0).unwrap_or(false);
        let (issue_filter, mut p): (String, SqlParams) = if specific {
            (
                format!(" AND l.{id_col} = ?N_I"),
                vec![integer(issue_local_id.unwrap_or(0))],
            )
        } else {
            (String::new(), Vec::new())
        };
        p.push(real((*value).ceil().max(1.0)));
        let sql = renumber_placeholders(
            &format!(
                "SELECT c.customer_local_id AS cid FROM conversations c
                        JOIN {link_table} l ON l.conversation_id = c.id
                      WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL{issue_filter}
                      GROUP BY c.customer_local_id HAVING COUNT(DISTINCT c.id) >= ?N_V"
            ),
            &mut 0,
        );
        let matching = self.ids(&sql, &p);
        if op == "gte" {
            return matching;
        }
        let set: HashSet<i64> = matching.into_iter().collect();
        self.all_customer_ids()
            .into_iter()
            .filter(|id| !set.contains(id))
            .collect()
    }

    pub(super) fn eval_incident_exposure(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::IncidentExposure {
            incident_id,
            within_days,
        } = n
        else {
            return Vec::new();
        };
        let specific = incident_id.map(|id| id > 0).unwrap_or(false);
        let within = within_days.filter(|d| d.is_finite()).map(|d| d.max(0.0));
        let mut p: SqlParams = Vec::new();
        let incident_filter = if specific {
            p.push(integer(incident_id.unwrap_or(0)));
            " AND inc.id = ?N_I".to_string()
        } else {
            " AND inc.status <> 'resolved'".to_string()
        };
        let time_filter = if let Some(w) = within {
            p.push(text(&format!("-{} days", w as i64)));
            " AND julianday(COALESCE(c.remote_created_at, c.created_at, inc.created_at)) >= julianday('now', ?N_W)".to_string()
        } else {
            String::new()
        };
        let sql = renumber_placeholders(
            &format!("SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                        JOIN incident_conversations ic ON ic.conversation_id = c.id
                        JOIN incidents inc ON inc.id = ic.incident_id
                      WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL{incident_filter}{time_filter}"),
            &mut 0,
        );
        self.ids(&sql, &p)
    }

    pub(super) fn eval_campaign_history(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CampaignHistory {
            relation,
            campaign_id,
        } = n
        else {
            return Vec::new();
        };
        let specific = campaign_id.map(|id| id > 0).unwrap_or(false);
        let (campaign_filter, p): (String, SqlParams) = if specific {
            (
                " AND r.campaign_id = ?N_C".to_string(),
                vec![integer(campaign_id.unwrap_or(0))],
            )
        } else {
            (String::new(), Vec::new())
        };
        match relation.as_str() {
            "received" => self.ids(
                &renumber_placeholders(
                    &format!("SELECT DISTINCT r.customer_local_id AS cid FROM outreach_recipients r
                                JOIN customers c ON c.id = r.customer_local_id
                              WHERE c.deleted_at IS NULL AND r.sent_at IS NOT NULL{campaign_filter}"),
                    &mut 0),
                &p,
            ),
            "replied" => self.ids(
                &renumber_placeholders(
                    &format!("SELECT DISTINCT r.customer_local_id AS cid FROM outreach_recipients r
                                JOIN customers c ON c.id = r.customer_local_id
                              WHERE c.deleted_at IS NULL AND r.replied_at IS NOT NULL{campaign_filter}"),
                    &mut 0),
                &p,
            ),
            "not_received" => {
                let received = self.ids(
                    &renumber_placeholders(
                        &format!("SELECT DISTINCT r.customer_local_id AS cid FROM outreach_recipients r WHERE 1=1{campaign_filter}"),
                        &mut 0),
                    &p,
                );
                let set: HashSet<i64> = received.into_iter().collect();
                self.all_customer_ids().into_iter().filter(|id| !set.contains(id)).collect()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn eval_support_health(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::SupportHealth { metric, op, value } = n else {
            return Vec::new();
        };
        if !value.is_finite() {
            return Vec::new();
        }
        let cmp = if op == "gte" { ">=" } else { "<=" };
        match metric.as_str() {
            "avg_rating" => self.ids(
                &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND
                             (SELECT AVG(CASE ra.rating WHEN 'great' THEN 5.0 WHEN 'okay' THEN 3.0 WHEN 'not-good' THEN 1.0 ELSE NULL END)
                              FROM ratings ra WHERE ra.customer_local_id = c.id) {cmp} ?N_V"),
            &mut 0),

                &[real(*value)],
            ),
            "avg_effort_score" | "first_response_resolution_rate" | "high_friction_rate" => {
                let effort = metric == "avg_effort_score";
                let resolved = metric == "first_response_resolution_rate";
                let convs: Vec<(i64, String)> = self
                    .conn
                    .prepare("SELECT c.id, c.status FROM conversations c
                               WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL")
                    .and_then(|mut s| {
                        let rows = s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                        Ok(rows.filter_map(|x| x.ok()).collect::<Vec<_>>())
                    })
                    .unwrap_or_default();
                let mut acc: HashMap<i64, (f64, usize)> = HashMap::new();
                for (conv_id, status) in convs {
                    let threads = crate::quality::published_threads(self.conn, conv_id);
                    let Some(customer_id) = self
                        .conn
                        .query_row(
                            "SELECT customer_local_id FROM conversations WHERE id = ?1",
                            params![conv_id],
                            |r| r.get::<_, Option<i64>>(0),
                        )
                        .ok()
                        .flatten()
                    else { continue };
                    let out = crate::quality::derive_outcome(
                        &threads, &status, (conv_id, 0, None));
                    let sample: Option<f64> = if effort {
                        out.effort_score
                    } else if resolved {
                        Some(out.resolved_after_first.map(|v| v as f64).unwrap_or(0.0))
                    } else {
                        Some(if out.friction == "high" { 1.0 } else { 0.0 })
                    };
                    if let Some(v) = sample {
                        let e = acc.entry(customer_id).or_insert((0.0, 0));
                        e.0 += v;
                        e.1 += 1;
                    }
                }
                acc.into_iter()
                    .filter_map(|(cid, (sum, count))| {
                        if count == 0 { return None; }
                        let avg = sum / count as f64;
                        let pass = if op == "gte" { avg >= *value } else { avg <= *value };
                        pass.then_some(cid)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn eval_custom_object_link(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CustomObjectLink { type_id } = n else {
            return Vec::new();
        };
        let specific = type_id.map(|id| id > 0).unwrap_or(false);
        let (join, filter, p): (String, String, SqlParams) = if specific {
            (
                "JOIN custom_object_types t ON t.id = o.type_id AND t.deleted_at IS NULL"
                    .to_string(),
                " AND t.id = ?N_T".to_string(),
                vec![integer(type_id.unwrap_or(0))],
            )
        } else {
            (String::new(), String::new(), Vec::new())
        };
        let sql = renumber_placeholders(
            &format!(
                "SELECT DISTINCT l.target_local_id AS cid FROM custom_object_links l
                        JOIN custom_objects o ON o.id = l.object_id AND o.deleted_at IS NULL
                        {join}
                      WHERE l.target_kind = 'customer'{filter}"
            ),
            &mut 0,
        );
        self.ids(&sql, &p)
    }

    pub(super) fn eval_customer_event(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CustomerEvent {
            event_kind,
            within_days,
        } = n
        else {
            return Vec::new();
        };
        let within = within_days.filter(|d| d.is_finite()).map(|d| d.max(0.0));
        let mut p: SqlParams = vec![text(event_kind)];
        let time_filter = if let Some(w) = within {
            p.push(text(&format!("-{} days", w as i64)));
            " AND COALESCE(julianday(e.occurred_at), julianday(e.created_at)) >= julianday('now', ?N_W)".to_string()
        } else {
            String::new()
        };
        let sql = renumber_placeholders(
            &format!(
                "SELECT DISTINCT e.customer_local_id AS cid FROM customer_events e
                        JOIN customers c ON c.id = e.customer_local_id
                      WHERE c.deleted_at IS NULL AND e.event_kind = ?N_K{time_filter}"
            ),
            &mut 0,
        );
        self.ids(&sql, &p)
    }
}
