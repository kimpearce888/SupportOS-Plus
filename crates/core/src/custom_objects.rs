use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};

use crate::error::{Error, Result};

fn valid_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    let rest: usize = chars.count();
    rest <= 58
        && key[1..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub(crate) fn slugify(name: &str) -> String {
    let lowered = name.trim().to_lowercase();
    let mut slug = String::new();
    let mut pending_dash = false;
    for c in lowered.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(c);
        } else {
            pending_dash = true;
        }
    }
    if slug.is_empty() {
        "type".to_string()
    } else {
        slug
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct FieldInput {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub options: Option<Vec<String>>,
}

impl FieldInput {
    fn validate(&self) -> Result<()> {
        if !valid_field_key(&self.key) {
            return Err(Error::Validation(format!(
                "Field key \"{}\" must be lowercase snake_case",
                self.key
            )));
        }
        if self.field_type == "select" && self.options.as_ref().is_none_or(Vec::is_empty) {
            return Err(Error::Validation(format!(
                "Select field \"{}\" needs at least one option",
                self.key
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LinkInput {
    pub target_kind: String,
    pub target_local_id: i64,
    pub note: Option<String>,
}

pub fn create_type(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    fields: &[FieldInput],
) -> Result<(i64, String)> {
    let slug = slugify(name);
    let slug_ok = {
        let mut chars = slug.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
            && slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && slug.len() <= 59
    };
    if !slug_ok {
        return Err(Error::Validation(
            "Type name must start with a letter and contain only letters, digits and spaces"
                .to_string(),
        ));
    }
    let dup: Option<i64> = conn
        .query_row(
            "SELECT id FROM custom_object_types WHERE slug = ?1 AND deleted_at IS NULL",
            params![slug],
            |r| r.get(0),
        )
        .ok();
    if dup.is_some() {
        return Err(Error::Validation(format!(
            "A type with slug \"{slug}\" already exists"
        )));
    }
    let mut keys = std::collections::HashSet::new();
    for f in fields {
        f.validate()?;
        if !keys.insert(f.key.clone()) {
            return Err(Error::Validation(format!(
                "Duplicate field key \"{}\"",
                f.key
            )));
        }
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO custom_object_types (name, slug, description) VALUES (?1, ?2, ?3)",
        params![name.trim(), slug, description],
    )?;
    let type_id = tx.last_insert_rowid();
    for (order, f) in fields.iter().enumerate() {
        insert_field(&tx, type_id, f, order)?;
    }
    tx.commit()?;
    Ok((type_id, slug))
}

fn insert_field(conn: &Connection, type_id: i64, f: &FieldInput, order: usize) -> Result<()> {
    let options = f
        .options
        .as_ref()
        .map(|o| serde_json::to_string(o).unwrap_or_else(|_| "[]".to_string()));
    conn.execute(
        "INSERT INTO custom_object_fields
            (type_id, name, label, field_type, required, options_json, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            type_id,
            f.key,
            f.label,
            f.field_type,
            i64::from(f.required),
            options,
            (order + 1) as i64
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct TypeDetail {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub provenance: String,
    pub fields: Vec<FieldDef>,
    pub object_count: i64,
}

impl TypeDetail {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "slug": self.slug,
            "description": self.description,
            "deleted_at": null,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "provenance": self.provenance,
            "fields": self.fields.iter().map(field_def_json).collect::<Vec<_>>(),
            "object_count": self.object_count,
        })
    }
}

fn field_def_json(f: &FieldDef) -> Value {
    json!({
        "key": f.key,
        "label": f.label,
        "fieldType": f.field_type,
        "required": f.required,
        "options": f.options,
    })
}

fn load_fields(conn: &Connection, type_id: i64) -> Result<Vec<FieldDef>> {
    let mut stmt = conn.prepare(
        "SELECT name, label, field_type, required, options_json
           FROM custom_object_fields
          WHERE type_id = ?1
          ORDER BY sort_order, id",
    )?;
    let fields = stmt
        .query_map(params![type_id], |r| {
            let key: String = r.get(0)?;
            let label: String = r.get(1)?;
            let field_type: String = r.get(2)?;
            let required = r.get::<_, i64>(3)? != 0;
            let options: Option<String> = r.get(4)?;
            let options = options.and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok());
            Ok(FieldDef {
                key,
                label,
                field_type,
                required,
                options,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(fields)
}

struct TypeRow {
    id: i64,
    name: String,
    slug: String,
    description: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    provenance: String,
}

fn row_to_type_detail(conn: &Connection, row: TypeRow) -> Result<TypeDetail> {
    let fields = load_fields(conn, row.id)?;
    let object_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM custom_objects WHERE type_id = ?1 AND deleted_at IS NULL",
        params![row.id],
        |r| r.get(0),
    )?;
    Ok(TypeDetail {
        id: row.id,
        name: row.name,
        slug: row.slug,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
        provenance: row.provenance,
        fields,
        object_count,
    })
}

pub fn list_types(conn: &Connection) -> Result<Vec<TypeDetail>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, slug, description, created_at, updated_at, provenance
           FROM custom_object_types
          WHERE deleted_at IS NULL
          ORDER BY name",
    )?;
    let rows: Vec<TypeRow> = stmt
        .query_map([], |r| {
            Ok(TypeRow {
                id: r.get(0)?,
                name: r.get(1)?,
                slug: r.get(2)?,
                description: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
                provenance: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|row| row_to_type_detail(conn, row))
        .collect()
}

pub(crate) fn get_type(conn: &Connection, type_id: i64) -> Result<Option<TypeDetail>> {
    let row = conn
        .query_row(
            "SELECT id, name, slug, description, created_at, updated_at, provenance
               FROM custom_object_types
              WHERE id = ?1 AND deleted_at IS NULL",
            params![type_id],
            |r| {
                Ok(TypeRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    slug: r.get(2)?,
                    description: r.get(3)?,
                    created_at: r.get(4)?,
                    updated_at: r.get(5)?,
                    provenance: r.get(6)?,
                })
            },
        )
        .ok();
    match row {
        Some(row) => row_to_type_detail(conn, row).map(Some),
        None => Ok(None),
    }
}

pub fn delete_type(conn: &Connection, type_id: i64) -> Result<bool> {
    let Some(existing) = get_type(conn, type_id)? else {
        return Ok(false);
    };
    if existing.object_count > 0 {
        return Err(Error::Validation(format!(
            "Type \"{}\" still has {} object(s); delete or move them first",
            existing.name, existing.object_count
        )));
    }
    conn.execute(
        "DELETE FROM custom_object_types WHERE id = ?1",
        params![type_id],
    )?;
    Ok(true)
}

pub fn validate_properties(
    fields: &[FieldDef],
    input: &Map<String, Value>,
) -> std::result::Result<Map<String, Value>, String> {
    let mut clean = Map::new();
    let defs: std::collections::HashMap<&str, &FieldDef> =
        fields.iter().map(|f| (f.key.as_str(), f)).collect();
    for (key, value) in input {
        let Some(def) = defs.get(key.as_str()) else {
            return Err(format!("Unrecognized key: \"{key}\""));
        };
        let value = match value {
            Value::Null => {
                if def.required {
                    return Err(format!("{key}: Required"));
                }
                continue;
            }
            v => v,
        };
        let ok = match def.field_type.as_str() {
            "text" => value.as_str().is_some_and(|s| s.chars().count() <= 300),
            "long_text" => value.as_str().is_some_and(|s| s.chars().count() <= 5000),
            "number" => value.as_f64().is_some_and(f64::is_finite),
            "date" => value.as_str().is_some_and(valid_iso_date),
            "boolean" => value.is_boolean(),
            "checkbox" => value.is_boolean(),
            "url" => value.as_str().is_some_and(|s| s.chars().count() <= 300),
            "select" => value.as_str().is_some_and(|s| {
                def.options
                    .as_ref()
                    .is_some_and(|o| o.iter().any(|opt| opt == s))
            }),
            other => return Err(format!("{key}: Unknown field type {other}")),
        };
        if !ok {
            return Err(format!("{key}: Invalid {}", def.field_type));
        }
        clean.insert(key.clone(), value.clone());
    }
    for def in fields {
        if def.required && !clean.contains_key(&def.key) {
            return Err(format!("{}: Required", def.key));
        }
    }
    Ok(clean)
}

fn valid_iso_date(s: &str) -> bool {
    let date: String = s.chars().take(10).collect();
    if s.chars().count() < 10 {
        return false;
    }
    let mut parts = date.split('-');
    let (y, m, d) = (
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
    );
    if parts.next().is_some() {
        return false;
    }
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return false;
    }
    if !y.bytes().all(|b| b.is_ascii_digit())
        || !m.bytes().all(|b| b.is_ascii_digit())
        || !d.bytes().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let m_v: u32 = m.parse().unwrap_or(0);
    let d_v: u32 = d.parse().unwrap_or(0);
    if !(1..=12).contains(&m_v) || !(1..=31).contains(&d_v) {
        return false;
    }
    if s.len() == 10 {
        return true;
    }
    let rest = &s[10..];
    let rest = match rest.strip_prefix('T') {
        Some(r) => r,
        None => return false,
    };
    if rest.is_empty() {
        return false;
    }
    let (time, zone) = match rest.rfind(['Z', '+', '-']) {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    if time.is_empty()
        || !time
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b':' || b == b'.')
    {
        return false;
    }
    if zone == "Z" {
        return true;
    }
    if zone.is_empty() {
        return false;
    }
    let zone_body = &zone[1..];
    let digits = zone_body.replace(':', "");
    (zone_body.len() == 5 && zone_body.as_bytes()[2] == b':')
        || (digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()))
}

fn build_search_text(title: &str, properties: &Map<String, Value>) -> String {
    let mut parts = vec![title.to_string()];
    for v in properties.values() {
        match v {
            Value::String(s) => parts.push(s.clone()),
            Value::Number(n) => parts.push(n.to_string()),
            Value::Bool(b) => parts.push(b.to_string()),
            _ => {}
        }
    }
    let joined = parts.join(" ");
    joined.chars().take(5000).collect()
}

pub fn create_object(
    conn: &Connection,
    type_id: i64,
    title: &str,
    properties: &Map<String, Value>,
    links: &[LinkInput],
) -> Result<i64> {
    let type_detail =
        get_type(conn, type_id)?.ok_or_else(|| Error::Validation("Type not found".to_string()))?;
    let clean = validate_properties(&type_detail.fields, properties).map_err(Error::Validation)?;
    let search_text = build_search_text(title, &clean);
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO custom_objects (type_id, title, data_json, search_text)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            type_id,
            title,
            serde_json::to_string(&Value::Object(clean)).unwrap_or_else(|_| "{}".into()),
            search_text
        ],
    )?;
    let object_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO fts_custom_objects (title, search_text, object_id) VALUES (?1, ?2, ?3)",
        params![title, search_text, object_id],
    )?;
    for link in links {
        insert_link(&tx, object_id, link)?;
    }
    tx.commit()?;
    Ok(object_id)
}

pub fn delete_object(conn: &Connection, object_id: i64) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "UPDATE custom_objects SET deleted_at = datetime('now')
          WHERE id = ?1 AND deleted_at IS NULL",
        params![object_id],
    )?;
    tx.execute(
        "DELETE FROM fts_custom_objects WHERE object_id = ?1",
        params![object_id],
    )?;
    tx.commit()?;
    Ok(changed > 0)
}

fn parse_properties(stored: Option<String>) -> Map<String, Value> {
    stored
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

pub fn list_objects(
    conn: &Connection,
    type_id: Option<i64>,
    query: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<(Vec<Value>, i64)> {
    let limit = limit.clamp(1, 200) as i64;
    let offset = offset as i64;
    let trimmed = query.map(str::trim).filter(|q| !q.is_empty());
    let mut where_sql = String::from("o.deleted_at IS NULL");
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(tid) = type_id.filter(|v| *v > 0) {
        where_sql.push_str(" AND o.type_id = ?");
        binds.push(Box::new(tid));
    }
    let like = trimmed.map(|q| {
        format!(
            "%{}%",
            q.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    if let Some(l) = &like {
        where_sql.push_str(" AND (o.title LIKE ? ESCAPE '\\' OR o.search_text LIKE ? ESCAPE '\\')");
        binds.push(Box::new(l.clone()));
        binds.push(Box::new(l.clone()));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let select_sql = format!(
        "SELECT o.id, o.type_id, o.title, o.data_json, o.created_at, o.updated_at, o.provenance
           FROM custom_objects o
          WHERE {where_sql}
          ORDER BY o.updated_at DESC, o.id DESC
          LIMIT {limit} OFFSET {offset}"
    );
    let to_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<Value> {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "type_id": r.get::<_, i64>(1)?,
            "title": r.get::<_, String>(2)?,
            "properties": parse_properties(r.get(3)?),
            "created_at": r.get::<_, Option<String>>(4)?,
            "updated_at": r.get::<_, Option<String>>(5)?,
            "provenance": r.get::<_, String>(6)?,
        }))
    };
    let objects: Vec<Value> = conn
        .prepare(&select_sql)?
        .query_map(rusqlite::params_from_iter(refs.iter()), to_row)?
        .filter_map(|r| r.ok())
        .collect();
    let count_sql = format!("SELECT COUNT(*) FROM custom_objects o WHERE {where_sql}");
    let total: i64 = conn.query_row(&count_sql, rusqlite::params_from_iter(refs.iter()), |r| {
        r.get(0)
    })?;
    Ok((objects, total))
}

fn insert_link(conn: &Connection, object_id: i64, link: &LinkInput) -> Result<()> {
    if !link_target_exists(conn, &link.target_kind, link.target_local_id)? {
        return Err(Error::Validation(format!(
            "{} #{} does not exist",
            link.target_kind, link.target_local_id
        )));
    }
    conn.execute(
        "INSERT OR IGNORE INTO custom_object_links
            (object_id, target_kind, target_local_id, note)
         VALUES (?1, ?2, ?3, ?4)",
        params![object_id, link.target_kind, link.target_local_id, link.note],
    )?;
    Ok(())
}

fn link_target_exists(conn: &Connection, kind: &str, id: i64) -> Result<bool> {
    let sql = match kind {
        "customer" => "SELECT 1 FROM customers WHERE id = ?1 AND deleted_at IS NULL",
        "organization" => "SELECT 1 FROM organizations WHERE id = ?1 AND deleted_at IS NULL",
        "conversation" => "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
        "known_issue" => "SELECT 1 FROM known_issues WHERE id = ?1",
        "incident" => "SELECT 1 FROM incidents WHERE id = ?1",
        "campaign" => "SELECT 1 FROM outreach_campaigns WHERE id = ?1",
        _ => return Ok(false),
    };
    Ok(conn.query_row(sql, params![id], |_| Ok(())).is_ok())
}

pub fn report(conn: &Connection) -> Result<Value> {
    let types = list_types(conn)?;
    let mut out = Vec::with_capacity(types.len());
    for t in &types {
        let mut stmt = conn.prepare(
            "SELECT l.target_kind, COUNT(*)
               FROM custom_object_links l
               JOIN custom_objects o ON o.id = l.object_id AND o.deleted_at IS NULL
              WHERE o.type_id = ?1
              GROUP BY l.target_kind",
        )?;
        let rows: Vec<(String, i64)> = stmt
            .query_map(params![t.id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        let mut link_counts = Map::new();
        for (kind, n) in rows {
            link_counts.insert(kind, json!(n));
        }
        out.push(json!({
            "id": t.id,
            "name": t.name,
            "slug": t.slug,
            "object_count": t.object_count,
            "link_counts": Value::Object(link_counts),
        }));
    }
    let total_objects: i64 = conn.query_row(
        "SELECT COUNT(*) FROM custom_objects WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    let total_links: i64 =
        conn.query_row("SELECT COUNT(*) FROM custom_object_links", [], |r| r.get(0))?;
    Ok(json!({
        "types": out,
        "total_objects": total_objects,
        "total_links": total_links,
    }))
}

#[cfg(test)]
mod batch21_audit27_tests {
    use super::*;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn field(key: &str, field_type: &str) -> FieldInput {
        FieldInput {
            key: key.into(),
            label: key.into(),
            field_type: field_type.into(),
            required: false,
            options: None,
        }
    }

    #[test]
    fn audit27_legacy_checkbox_and_url_field_types_accept_objects() {
        let conn = conn();
        let (type_id, _) = create_type(
            &conn,
            "Legacy Builder Types",
            None,
            &[field("shipped", "checkbox"), field("docs", "url")],
        )
        .expect("type with legacy field names");

        let mut properties = Map::new();
        properties.insert("shipped".into(), Value::Bool(true));
        properties.insert("docs".into(), Value::String("https://example.com/x".into()));
        let object_id =
            create_object(&conn, type_id, "LGTM", &properties, &[]).expect("legacy fields work");
        assert!(object_id > 0);
    }

    #[test]
    fn audit27_legacy_aliases_still_reject_wrong_value_types() {
        let conn = conn();
        let (type_id, _) = create_type(
            &conn,
            "Legacy Strict",
            None,
            &[field("shipped", "checkbox")],
        )
        .expect("type");

        let mut properties = Map::new();
        properties.insert("shipped".into(), Value::String("yes".into()));
        let err = create_object(&conn, type_id, "Nope", &properties, &[])
            .expect_err("a string is not a checkbox value");
        assert!(
            err.to_string().contains("Invalid checkbox"),
            "the error must name the field: {err}"
        );
    }

    #[test]
    fn audit27_unknown_field_types_are_still_rejected() {
        let conn = conn();
        let (type_id, _) =
            create_type(&conn, "Mystery", None, &[field("vibe", "telepathy")]).expect("type");

        let mut properties = Map::new();
        properties.insert("vibe".into(), Value::String("strong".into()));
        let err = create_object(&conn, type_id, "Nope", &properties, &[])
            .expect_err("unknown field type must be refused");
        assert!(
            err.to_string().contains("Unknown field type telepathy"),
            "the error must name the type: {err}"
        );
    }
}

#[cfg(test)]
#[cfg(test)]
mod batch31_tests {
    use super::*;

    #[test]
    fn valid_iso_date_survives_multibyte_prefixes() {
        assert!(valid_iso_date("2024-01-15"));
        assert!(!valid_iso_date("2024-1-5"));
        assert!(!valid_iso_date("2024/01/15"));
        assert!(
            !valid_iso_date("日日日日2024-01-15"),
            "rejects, never panics"
        );
        assert!(
            valid_iso_date("2024-01-15T10:00:00Z"),
            "trailing time ignored"
        );
    }
}
