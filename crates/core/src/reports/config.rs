use super::*;
pub(crate) fn config_to_json(config: &ReportBuilderConfig) -> serde_json::Value {
    let filters = {
        let mut f = serde_json::Map::new();
        if let Some(v) = &config.filters.mailbox_local_ids {
            f.insert("mailboxLocalIds".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.channel {
            f.insert("channel".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.tags_any {
            f.insert("tagsAny".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.tags_none {
            f.insert("tagsNone".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.statuses {
            f.insert("statuses".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.assignee_local_ids {
            f.insert("assigneeLocalIds".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.min_priority {
            f.insert("minPriority".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.attribute_key {
            f.insert("attributeKey".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.attribute_value {
            f.insert("attributeValue".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.state_key {
            f.insert("stateKey".into(), serde_json::json!(v));
        }
        f
    };
    let mut v = serde_json::json!({
        "metric": config.metric.as_str(),
        "dimension": config.dimension.as_str(),
        "dateFrom": config.date_from,
        "dateTo": config.date_to,
        "comparison": match config.comparison {
            ReportComparison::None => "none",
            ReportComparison::PreviousPeriod => "previous_period",
        },
        "sort": match config.sort {
            ReportSort::MetricDesc => "metric_desc",
            ReportSort::MetricAsc => "metric_asc",
            ReportSort::DimensionAsc => "dimension_asc",
        },
    });
    if !filters.is_empty() {
        v["filters"] = serde_json::Value::Object(filters);
    }
    if let Some(limit) = config.limit {
        v["limit"] = serde_json::json!(limit);
    }
    v
}
pub fn parse_report_config(
    body: &serde_json::Value,
) -> std::result::Result<ReportBuilderConfig, Vec<(String, String)>> {
    parse_report_config_inner(body).map_err(|mut issues| {
        issues.truncate(10);
        issues
    })
}

fn enum_error(variants: &[&str], received: &str) -> String {
    let quoted: Vec<String> = variants.iter().map(|v| format!("'{v}'")).collect();
    format!(
        "Invalid enum value. Expected {}, received '{received}'",
        quoted.join(" | ")
    )
}

fn metric_variants() -> Vec<&'static str> {
    ReportMetricKey::ALL.iter().map(|k| k.as_str()).collect()
}

fn dimension_variants() -> Vec<&'static str> {
    ReportDimensionKey::ALL.iter().map(|k| k.as_str()).collect()
}

fn parse_report_config_inner(
    body: &serde_json::Value,
) -> std::result::Result<ReportBuilderConfig, Vec<(String, String)>> {
    let mut issues: Vec<(String, String)> = Vec::new();

    let metric = body.get("metric").and_then(|v| v.as_str());
    let metric = match metric {
        None => {
            issues.push(("metric".to_string(), "Required".to_string()));
            None
        }
        Some(m) => match ReportMetricKey::ALL.iter().find(|k| k.as_str() == m) {
            Some(k) => Some(*k),
            None => {
                issues.push(("metric".to_string(), enum_error(&metric_variants(), m)));
                None
            }
        },
    };

    let dimension = body.get("dimension").and_then(|v| v.as_str());
    let dimension = match dimension {
        None => {
            issues.push(("dimension".to_string(), "Required".to_string()));
            None
        }
        Some(d) => match ReportDimensionKey::ALL.iter().find(|k| k.as_str() == d) {
            Some(k) => Some(*k),
            None => {
                issues.push((
                    "dimension".to_string(),
                    enum_error(&dimension_variants(), d),
                ));
                None
            }
        },
    };

    let date_from = match body.get("dateFrom") {
        None | Some(serde_json::Value::Null) => {
            issues.push(("dateFrom".to_string(), "Required".to_string()));
            String::new()
        }
        Some(serde_json::Value::String(s)) => {
            let shape_ok = s.len() == 10
                && s.as_bytes().iter().enumerate().all(|(i, b)| match i {
                    4 | 7 => *b == b'-',
                    _ => b.is_ascii_digit(),
                });
            if shape_ok {
                s.clone()
            } else {
                issues.push(("dateFrom".to_string(), "Invalid".to_string()));
                String::new()
            }
        }
        Some(_) => {
            issues.push((
                "dateFrom".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            String::new()
        }
    };

    let date_to = match body.get("dateTo") {
        None | Some(serde_json::Value::Null) => {
            issues.push(("dateTo".to_string(), "Required".to_string()));
            String::new()
        }
        Some(serde_json::Value::String(s)) => {
            let shape_ok = s.len() == 10
                && s.as_bytes().iter().enumerate().all(|(i, b)| match i {
                    4 | 7 => *b == b'-',
                    _ => b.is_ascii_digit(),
                });
            if shape_ok {
                s.clone()
            } else {
                issues.push(("dateTo".to_string(), "Invalid".to_string()));
                String::new()
            }
        }
        Some(_) => {
            issues.push((
                "dateTo".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            String::new()
        }
    };

    let comparison = match body.get("comparison") {
        None | Some(serde_json::Value::Null) => ReportComparison::None,
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "none" => ReportComparison::None,
            "previous_period" => ReportComparison::PreviousPeriod,
            _ => {
                issues.push((
                    "comparison".to_string(),
                    enum_error(&["none", "previous_period"], s),
                ));
                ReportComparison::None
            }
        },
        Some(_) => {
            issues.push((
                "comparison".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            ReportComparison::None
        }
    };

    let sort = match body.get("sort") {
        None | Some(serde_json::Value::Null) => ReportSort::MetricDesc,
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "metric_desc" => ReportSort::MetricDesc,
            "metric_asc" => ReportSort::MetricAsc,
            "dimension_asc" => ReportSort::DimensionAsc,
            _ => {
                issues.push((
                    "sort".to_string(),
                    enum_error(&["metric_desc", "metric_asc", "dimension_asc"], s),
                ));
                ReportSort::MetricDesc
            }
        },
        Some(_) => {
            issues.push((
                "sort".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            ReportSort::MetricDesc
        }
    };

    let limit = match body.get("limit") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => match v.as_i64() {
            Some(n) if (1..=200).contains(&n) => Some(n),
            Some(n) if n < 1 => {
                issues.push((
                    "limit".to_string(),
                    "Number must be greater than or equal to 1".to_string(),
                ));
                None
            }
            Some(_) => {
                issues.push((
                    "limit".to_string(),
                    "Number must be less than or equal to 200".to_string(),
                ));
                None
            }
            None => {
                issues.push((
                    "limit".to_string(),
                    "Expected number, received non-number".to_string(),
                ));
                None
            }
        },
    };

    let filters = match body.get("filters") {
        None => ReportFilters::default(),
        Some(serde_json::Value::Object(f)) => parse_report_filters(f, &mut issues),
        Some(_) => {
            issues.push((
                "filters".to_string(),
                "Expected object, received non-object".to_string(),
            ));
            ReportFilters::default()
        }
    };

    match (metric, dimension) {
        (Some(metric), Some(dimension)) if issues.is_empty() => Ok(ReportBuilderConfig {
            metric,
            dimension,
            date_from,
            date_to,
            comparison,
            filters,
            sort,
            limit,
        }),
        _ => Err(issues),
    }
}

fn parse_report_filters(
    f: &serde_json::Map<String, serde_json::Value>,
    issues: &mut Vec<(String, String)>,
) -> ReportFilters {
    let mut out = ReportFilters::default();

    for key in ["mailboxLocalIds", "assigneeLocalIds"] {
        if let Some(v) = f.get(key) {
            match v.as_array() {
                Some(arr) if arr.len() > 20 => {
                    issues.push((
                        format!("filters.{key}"),
                        "Array must contain at most 20 item(s)".to_string(),
                    ));
                }
                Some(arr) => {
                    let mut ids = Vec::new();
                    let mut ok = true;
                    for item in arr {
                        match item.as_i64() {
                            Some(n) if n > 0 => ids.push(n),
                            _ => {
                                issues.push((
                                    format!("filters.{key}"),
                                    "Expected positive integer, received non-integer".to_string(),
                                ));
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        if key == "mailboxLocalIds" {
                            out.mailbox_local_ids = Some(ids);
                        } else {
                            out.assignee_local_ids = Some(ids);
                        }
                    }
                }
                None => {
                    issues.push((
                        format!("filters.{key}"),
                        "Expected array, received non-array".to_string(),
                    ));
                }
            }
        }
    }

    if let Some(v) = f.get("channel") {
        match v {
            serde_json::Value::Null => {}
            serde_json::Value::String(s) if s.len() <= 40 => {
                out.channel = Some(s.clone());
            }
            serde_json::Value::String(_) => {
                issues.push((
                    "filters.channel".to_string(),
                    "String must contain at most 40 character(s)".to_string(),
                ));
            }
            _ => {
                issues.push((
                    "filters.channel".to_string(),
                    "Expected string, received non-string".to_string(),
                ));
            }
        }
    }

    let string_lists = [
        ("tagsAny", 10usize, 80usize, "tagsAny"),
        ("tagsNone", 10, 80, "tagsNone"),
        ("statuses", 10, 40, "statuses"),
    ];
    for (key, max_items, max_len, field) in string_lists {
        if let Some(v) = f.get(key) {
            match v.as_array() {
                Some(arr) if arr.len() > max_items => {
                    issues.push((
                        format!("filters.{field}"),
                        format!("Array must contain at most {max_items} item(s)"),
                    ));
                }
                Some(arr) => {
                    let mut items = Vec::new();
                    let mut ok = true;
                    for item in arr {
                        match item.as_str() {
                            Some(s) if !s.is_empty() && s.len() <= max_len => {
                                items.push(s.to_string());
                            }
                            Some("") => {
                                issues.push((
                                    format!("filters.{field}"),
                                    "String must contain at least 1 character(s)".to_string(),
                                ));
                                ok = false;
                                break;
                            }
                            Some(_) => {
                                issues.push((
                                    format!("filters.{field}"),
                                    format!("String must contain at most {max_len} character(s)"),
                                ));
                                ok = false;
                                break;
                            }
                            None => {
                                issues.push((
                                    format!("filters.{field}"),
                                    "Expected string, received non-string".to_string(),
                                ));
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        match field {
                            "tagsAny" => out.tags_any = Some(items),
                            "tagsNone" => out.tags_none = Some(items),
                            _ => out.statuses = Some(items),
                        }
                    }
                }
                None => {
                    issues.push((
                        format!("filters.{field}"),
                        "Expected array, received non-array".to_string(),
                    ));
                }
            }
        }
    }

    if let Some(v) = f.get("minPriority") {
        match v {
            serde_json::Value::Null => {}
            serde_json::Value::String(s)
                if ["low", "normal", "high", "urgent"].contains(&s.as_str()) =>
            {
                out.min_priority = Some(s.clone());
            }
            serde_json::Value::String(s) => {
                issues.push((
                    "filters.minPriority".to_string(),
                    enum_error(&["low", "normal", "high", "urgent"], s),
                ));
            }
            _ => {
                issues.push((
                    "filters.minPriority".to_string(),
                    "Expected string, received non-string".to_string(),
                ));
            }
        }
    }

    let nullable_strings = [
        ("attributeKey", 60usize, "attribute_key"),
        ("attributeValue", 200, "attribute_value"),
        ("stateKey", 80, "state_key"),
    ];
    for (key, max_len, field) in nullable_strings {
        if let Some(v) = f.get(key) {
            match v {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) if s.len() <= max_len => match field {
                    "attribute_key" => out.attribute_key = Some(s.clone()),
                    "attribute_value" => out.attribute_value = Some(s.clone()),
                    _ => out.state_key = Some(s.clone()),
                },
                serde_json::Value::String(_) => {
                    issues.push((
                        format!("filters.{key}"),
                        format!("String must contain at most {max_len} character(s)"),
                    ));
                }
                _ => {
                    issues.push((
                        format!("filters.{key}"),
                        "Expected string, received non-string".to_string(),
                    ));
                }
            }
        }
    }

    out
}
