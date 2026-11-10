use eframe::egui;
use spp_core::custom_objects::{FieldInput, LinkInput, TypeDetail};
use spp_core::db::DbHandle;

use super::{ConfirmRequest, NavRequest, i64_field, opt_str_field, str_field};
use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// AUD-P2-11 (C3): objects page size — 50 per page (the core list caps at
/// 200; 50 matches the customers list and keeps the cards readable).
const OBJECTS_PAGE_SIZE: i64 = 50;

/// UX-16: "3 types" / "1 type" — proper pluralization for the page counts.
fn count_noun(count: i64, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    Text,
    LongText,
    Number,
    Date,
    Select,
    Boolean,
}

impl FieldType {
    const ALL: [Self; 6] = [
        Self::Text,
        Self::LongText,
        Self::Number,
        Self::Date,
        Self::Select,
        Self::Boolean,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::LongText => "long_text",
            Self::Number => "number",
            Self::Date => "date",
            Self::Select => "select",
            Self::Boolean => "boolean",
        }
    }

    /// AUD-P2-11 (C3): map a stored field type back into the builder's
    /// vocabulary (edit prefill). Legacy "checkbox" normalizes to its
    /// canonical boolean twin — the core validates both identically — so
    /// the builder only ever speaks the six canonical kinds.
    fn parse(field_type: &str) -> Self {
        match field_type {
            "long_text" => Self::LongText,
            "number" => Self::Number,
            "date" => Self::Date,
            "select" => Self::Select,
            "boolean" | "checkbox" => Self::Boolean,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone)]
struct FieldDraft {
    key: String,
    label: String,
    field_type: FieldType,
    required: bool,
    options: String,
}

impl FieldDraft {
    fn to_input(&self) -> Result<FieldInput, String> {
        if self.key.trim().is_empty() {
            return Err("field key required".into());
        }
        let options = self
            .options
            .split(',')
            .map(str::trim)
            .filter(|o| !o.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if self.field_type == FieldType::Select && options.is_empty() {
            return Err(format!(
                "select field '{}' needs at least one option",
                self.key
            ));
        }
        Ok(FieldInput {
            key: self.key.trim().to_string(),
            label: if self.label.trim().is_empty() {
                self.key.trim().to_string()
            } else {
                self.label.trim().to_string()
            },
            field_type: self.field_type.as_str().to_string(),
            required: self.required,
            options: if self.field_type == FieldType::Select {
                Some(options)
            } else {
                None
            },
        })
    }
}

/// AUD-P2-11 (C3): per-object add-link mini form state (target kind + row
/// id + optional note) — the card-level lane that finally lets a link exist.
#[derive(Debug, Clone)]
struct LinkDraft {
    kind: String,
    id: String,
    note: String,
}

impl Default for LinkDraft {
    fn default() -> Self {
        Self {
            kind: spp_core::custom_objects::LINK_TARGET_KINDS[0].to_string(),
            id: String::new(),
            note: String::new(),
        }
    }
}

pub struct CustomObjectsPage {
    types: Vec<TypeDetail>,
    types_loaded: bool,
    selected_type: Option<i64>,
    objects: Vec<serde_json::Value>,
    objects_loaded: bool,
    objects_total: i64,
    // AUD-P2-11 (C3): real pagination — the "showing first N of M — the
    // list has no pager" hint is gone.
    objects_page: i32,
    object_query: String,
    // AUD-P2-11 (C3): links per object (labels already resolved by the
    // core), loaded with the object list — one read per reload, not per frame.
    object_links: std::collections::HashMap<i64, Vec<serde_json::Value>>,
    // AUD-P2-11 (C3): add-link mini form state per object card.
    link_drafts: std::collections::HashMap<i64, LinkDraft>,
    report: Option<serde_json::Value>,
    show_new_type: bool,
    // AUD-P2-11 (C3): the type builder doubles as the edit form.
    edit_type_id: Option<i64>,
    new_type_name: String,
    new_type_description: String,
    new_fields: Vec<FieldDraft>,
    new_object_title: String,
    new_object_fields: Vec<(String, String)>,
    // AUD-P2-11 (C3): the new-object form's link row — create_object used
    // to pass a hard-coded `Vec::new()` while the header advertised "N links".
    new_object_link_kind: String,
    new_object_link_id: String,
    new_object_link_note: String,
    // AUD-P2-11 (C3): the object form doubles as the edit form.
    edit_object_id: Option<i64>,
    error: Option<String>,
}

impl CustomObjectsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            types_loaded: false,
            selected_type: None,
            objects: Vec::new(),
            objects_loaded: false,
            objects_total: 0,
            objects_page: 0,
            object_query: String::new(),
            object_links: std::collections::HashMap::new(),
            link_drafts: std::collections::HashMap::new(),
            report: None,
            show_new_type: false,
            edit_type_id: None,
            new_type_name: String::new(),
            new_type_description: String::new(),
            new_fields: vec![FieldDraft {
                key: String::new(),
                label: String::new(),
                field_type: FieldType::Text,
                required: false,
                options: String::new(),
            }],
            new_object_title: String::new(),
            new_object_fields: Vec::new(),
            new_object_link_kind: spp_core::custom_objects::LINK_TARGET_KINDS[0].to_string(),
            new_object_link_id: String::new(),
            new_object_link_note: String::new(),
            edit_object_id: None,
            error: None,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.types.clear();
        self.types_loaded = false;
        self.objects.clear();
        self.objects_loaded = false;
        self.report = None;
        // AUD-P2-11 (C3): the pager and the link caches reset with the rest.
        self.objects_page = 0;
        self.object_links.clear();
        self.link_drafts.clear();
    }

    fn reload_types(&mut self, db: &DbHandle) {
        let types = db.with_read(spp_core::custom_objects::list_types);
        match types {
            Ok(types) => {
                self.types = types;
                self.error = None;
                if let Some(id) = self.selected_type
                    && !self.types.iter().any(|t| t.id == id)
                {
                    self.selected_type = None;
                    self.objects.clear();
                    self.objects_loaded = false;
                    self.new_object_fields.clear();
                    // AUD-P2-11 (C3): the selected type vanished — the
                    // in-progress object edit and link state go with it.
                    self.cancel_object_edit();
                    self.object_links.clear();
                    self.link_drafts.clear();
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.report = db.with_read(spp_core::custom_objects::report).ok();
        self.types_loaded = true;
    }

    fn reload_objects(&mut self, db: &DbHandle) {
        let Some(type_id) = self.selected_type else {
            return;
        };
        let query = self.object_query.trim().to_string();
        // AUD-P2-11 (C3): the pager's page feeds the offset — page * 50.
        let offset =
            u32::try_from(i64::from(self.objects_page.max(0)) * OBJECTS_PAGE_SIZE).unwrap_or(0);
        let result = db.with_read(|conn| -> spp_core::Result<_> {
            let (objects, total) = spp_core::custom_objects::list_objects(
                conn,
                Some(type_id),
                if query.is_empty() {
                    None
                } else {
                    Some(query.as_str())
                },
                u32::try_from(OBJECTS_PAGE_SIZE).unwrap_or(50),
                offset,
            )?;
            // AUD-P2-11 (C3): the objects' links ride along (labels already
            // resolved by the core) so the cards never query per frame.
            let mut links = std::collections::HashMap::new();
            for object in &objects {
                let id = i64_field(object, "id");
                links.insert(id, spp_core::custom_objects::list_object_links(conn, id)?);
            }
            Ok((objects, total, links))
        });
        match result {
            Ok((objects, total, links)) => {
                if objects.is_empty() && self.objects_page > 0 && total > 0 {
                    // AUD-P2-11 (C3): a page past the end (the last object
                    // on the last page was deleted, say) falls back to the
                    // last page instead of showing an empty list.
                    let last = ((total + OBJECTS_PAGE_SIZE - 1) / OBJECTS_PAGE_SIZE - 1).max(0);
                    self.objects_page = i32::try_from(last).unwrap_or(0);
                    return self.reload_objects(db);
                }
                self.objects = objects;
                self.objects_total = total;
                self.object_links = links;
                // keep only the mini-form drafts of the objects on screen
                self.link_drafts
                    .retain(|id, _| self.object_links.contains_key(id));
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.objects_loaded = true;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.types_loaded {
            self.reload_types(db);
        }
        if self.selected_type.is_some() && !self.objects_loaded {
            self.reload_objects(db);
        }
    }

    fn select_type(&mut self, db: &DbHandle, id: i64) {
        self.selected_type = Some(id);
        self.objects.clear();
        self.objects_loaded = false;
        // AUD-P2-11 (C3): a fresh type starts on page 1 — and any
        // in-progress object edit belonged to the previous type.
        self.objects_page = 0;
        self.cancel_object_edit();
        self.reload_objects(db);
        self.rebuild_new_object_fields();
    }

    /// AUD-P2-11 (C3): the object form's typed inputs track the selected
    /// type's field set (also refreshed after a type edit changes it).
    fn rebuild_new_object_fields(&mut self) {
        if let Some(t) = self
            .selected_type
            .and_then(|id| self.types.iter().find(|t| t.id == id))
        {
            self.new_object_fields = t
                .fields
                .iter()
                .map(|f| (f.key.clone(), String::new()))
                .collect();
        }
    }

    fn create_type(&mut self, db: &DbHandle) {
        let name = self.new_type_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("type name required".into());
            return;
        }
        let mut fields = Vec::new();
        for draft in &self.new_fields {
            match draft.to_input() {
                Ok(input) => fields.push(input),
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        let description = self.new_type_description.trim().to_string();
        let result = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                &name,
                if description.is_empty() {
                    None
                } else {
                    Some(description.as_str())
                },
                &fields,
            )
        });
        match result {
            Ok((id, _slug)) => {
                self.show_new_type = false;
                self.new_type_name.clear();
                self.new_type_description.clear();
                self.reload_types(db);
                self.select_type(db, id);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub(crate) fn delete_type(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::custom_objects::delete_type(conn, id));
        match &result {
            Ok(_) => {
                if self.selected_type == Some(id) {
                    self.selected_type = None;
                    self.objects.clear();
                    self.objects_loaded = false;
                    self.new_object_fields.clear();
                    // AUD-P2-11 (C3): link state for the deleted type's
                    // objects goes with them.
                    self.object_links.clear();
                    self.link_drafts.clear();
                    self.cancel_object_edit();
                }
                // AUD-P2-11 (C3): the deleted type cannot be saved anymore.
                if self.edit_type_id == Some(id) {
                    self.edit_type_id = None;
                }
                self.reload_types(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        result.map(|_| ())
    }

    fn create_object(&mut self, db: &DbHandle) {
        let Some(type_id) = self.selected_type else {
            return;
        };
        let title = self.new_object_title.trim().to_string();
        if title.is_empty() {
            self.error = Some("object title required".into());
            return;
        }
        let Some(properties) = self.collect_form_properties(type_id) else {
            return;
        };
        // AUD-P2-11 (C3): the form's link row finally feeds real
        // LinkInputs — `create_object` used to pass a hard-coded
        // `Vec::new()` while the header counted and advertised "N links".
        let Some(links) = self.collect_form_links() else {
            return;
        };
        let result = db.with_write(|conn| {
            spp_core::custom_objects::create_object(conn, type_id, &title, &properties, &links)
        });
        match result {
            Ok(_) => {
                self.new_object_title.clear();
                for field in &mut self.new_object_fields {
                    field.1.clear();
                }
                self.clear_new_object_link_form();
                self.reload_objects(db);
                self.reload_types(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// AUD-P2-11 (C3): the form's typed inputs → a property map (shared by
    /// create and edit so both paths coerce identically). None + an error
    /// in the slot when a value cannot coerce.
    fn collect_form_properties(
        &mut self,
        type_id: i64,
    ) -> Option<serde_json::Map<String, serde_json::Value>> {
        let field_types: Vec<(String, String)> = self
            .types
            .iter()
            .find(|t| t.id == type_id)
            .map(|t| {
                t.fields
                    .iter()
                    .map(|f| (f.key.clone(), f.field_type.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut properties = serde_json::Map::new();
        for (key, value) in &self.new_object_fields {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let field_type = field_types
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, t)| t.as_str())
                .unwrap_or("text");
            match coerce_property_value(field_type, value) {
                Ok(v) => {
                    properties.insert(key.clone(), v);
                }
                Err(e) => {
                    self.error = Some(e);
                    return None;
                }
            }
        }
        Some(properties)
    }

    /// AUD-P2-11 (C3): the new-object form's link row → LinkInputs. An
    /// empty target id means "no link at creation"; a non-numeric id is
    /// refused before any write.
    fn collect_form_links(&mut self) -> Option<Vec<LinkInput>> {
        let raw = self.new_object_link_id.trim().to_string();
        if raw.is_empty() {
            return Some(Vec::new());
        }
        let Some(target) = raw.parse::<i64>().ok() else {
            self.error = Some(format!("link target id must be a number, got '{raw}'"));
            return None;
        };
        let note = self.new_object_link_note.trim().to_string();
        Some(vec![LinkInput {
            target_kind: self.new_object_link_kind.clone(),
            target_local_id: target,
            note: if note.is_empty() { None } else { Some(note) },
        }])
    }

    fn clear_new_object_link_form(&mut self) {
        self.new_object_link_id.clear();
        self.new_object_link_note.clear();
        self.new_object_link_kind = spp_core::custom_objects::LINK_TARGET_KINDS[0].to_string();
    }

    /// AUD-P2-11 (C3): prefill the object form from a stored row (the
    /// connectors `start_edit` idiom, applied to objects).
    fn start_object_edit(&mut self, object: &serde_json::Value) {
        let Some(type_id) = self.selected_type else {
            return;
        };
        self.edit_object_id = Some(i64_field(object, "id"));
        self.error = None;
        self.new_object_title = str_field(object, "title");
        let props = object
            .get("properties")
            .and_then(|p| p.as_object())
            .cloned()
            .unwrap_or_default();
        self.new_object_fields = self
            .types
            .iter()
            .find(|t| t.id == type_id)
            .map(|t| {
                t.fields
                    .iter()
                    .map(|f| {
                        (
                            f.key.clone(),
                            props
                                .get(&f.key)
                                .map(property_to_form_string)
                                .unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    /// AUD-P2-11 (C3): save the object edit (title + properties, validated
    /// by the core against the type's current fields).
    fn save_object_edit(&mut self, db: &DbHandle) {
        let Some(object_id) = self.edit_object_id else {
            return;
        };
        let Some(type_id) = self.selected_type else {
            return;
        };
        let title = self.new_object_title.trim().to_string();
        if title.is_empty() {
            self.error = Some("object title required".into());
            return;
        }
        let Some(properties) = self.collect_form_properties(type_id) else {
            return;
        };
        let result = db.with_write(|conn| {
            spp_core::custom_objects::update_object(conn, object_id, &title, &properties)
        });
        match result {
            Ok(()) => {
                self.cancel_object_edit();
                self.reload_objects(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// AUD-P2-11 (C3): leave edit mode and hand the form back to "New object".
    fn cancel_object_edit(&mut self) {
        self.edit_object_id = None;
        self.new_object_title.clear();
        for field in &mut self.new_object_fields {
            field.1.clear();
        }
    }

    /// AUD-P2-11 (C3): attach a link from an object card's mini form.
    fn add_object_link(&mut self, db: &DbHandle, object_id: i64) {
        let Some(draft) = self.link_drafts.get(&object_id) else {
            return;
        };
        let raw = draft.id.trim().to_string();
        let Some(target) = raw.parse::<i64>().ok() else {
            self.error = Some(format!("link target id must be a number, got '{raw}'"));
            return;
        };
        let note = draft.note.trim().to_string();
        let link = LinkInput {
            target_kind: draft.kind.clone(),
            target_local_id: target,
            note: if note.is_empty() { None } else { Some(note) },
        };
        let result =
            db.with_write(|conn| spp_core::custom_objects::add_link(conn, object_id, &link));
        match result {
            Ok(_) => {
                // the card's form starts fresh for the next link
                if let Some(draft) = self.link_drafts.get_mut(&object_id) {
                    draft.id.clear();
                    draft.note.clear();
                }
                self.reload_objects(db);
                self.reload_types(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// AUD-P2-11 (C3): remove a link (the removable chip on the card).
    fn remove_object_link(
        &mut self,
        db: &DbHandle,
        object_id: i64,
        target_kind: &str,
        target_local_id: i64,
    ) {
        let result = db.with_write(|conn| {
            spp_core::custom_objects::remove_link(conn, object_id, target_kind, target_local_id)
        });
        if let Err(e) = &result {
            self.error = Some(e.to_string());
        }
        self.reload_objects(db);
        self.reload_types(db);
    }

    /// AUD-P2-11 (C3): prefill the type builder from an existing type (the
    /// connectors `start_edit` idiom, applied to types).
    fn start_type_edit(&mut self, t: &TypeDetail) {
        self.edit_type_id = Some(t.id);
        self.show_new_type = true;
        self.error = None;
        self.new_type_name = t.name.clone();
        self.new_type_description = t.description.clone().unwrap_or_default();
        self.new_fields = t
            .fields
            .iter()
            .map(|f| FieldDraft {
                key: f.key.clone(),
                label: f.label.clone(),
                field_type: FieldType::parse(&f.field_type),
                required: f.required,
                options: f.options.clone().unwrap_or_default().join(", "),
            })
            .collect();
    }

    /// AUD-P2-11 (C3): leave type-edit mode with the form wiped.
    fn cancel_type_edit(&mut self) {
        self.edit_type_id = None;
        self.show_new_type = false;
        self.error = None;
        self.new_type_name.clear();
        self.new_type_description.clear();
        self.new_fields = vec![FieldDraft {
            key: String::new(),
            label: String::new(),
            field_type: FieldType::Text,
            required: false,
            options: String::new(),
        }];
    }

    /// AUD-P2-11 (C3): save the type edit — the core refuses to drop a
    /// field still set on objects; that refusal lands in the error slot
    /// verbatim so the user can fix the form instead of losing data.
    fn save_type_edit(&mut self, db: &DbHandle) {
        let Some(type_id) = self.edit_type_id else {
            return;
        };
        let name = self.new_type_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("type name required".into());
            return;
        }
        let mut fields = Vec::new();
        for draft in &self.new_fields {
            match draft.to_input() {
                Ok(input) => fields.push(input),
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        let description = self.new_type_description.trim().to_string();
        let result = db.with_write(|conn| {
            spp_core::custom_objects::update_type(
                conn,
                type_id,
                &name,
                if description.is_empty() {
                    None
                } else {
                    Some(description.as_str())
                },
                &fields,
            )
        });
        match result {
            Ok(()) => {
                let was_selected = self.selected_type == Some(type_id);
                self.cancel_type_edit();
                self.reload_types(db);
                if was_selected {
                    // AUD-P2-11 (C3): the selected type's field set just
                    // changed — rebuild the object form and drop any
                    // in-progress object edit built on the old fields.
                    self.cancel_object_edit();
                    self.rebuild_new_object_fields();
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub(crate) fn delete_object(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::custom_objects::delete_object(conn, id));
        if let Err(e) = &result {
            self.error = Some(e.to_string());
        }
        // AUD-P2-11 (C3): the confirm flow can remove the object being
        // edited — drop the stale edit state instead of saving into a
        // deleted row.
        if self.edit_object_id == Some(id) {
            self.cancel_object_edit();
        }
        self.reload_objects(db);
        self.reload_types(db);
        result.map(|_| ())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        let subtitle = self.report.as_ref().map(|report| {
            let type_count = report
                .get("types")
                .and_then(|t| t.as_array())
                .map_or(0, std::vec::Vec::len);
            // UX-16: real pluralization — "0 type(s) · …" read as a debug
            // string, and screen readers announce it letter by letter.
            let objects = i64_field(report, "total_objects");
            let links = i64_field(report, "total_links");
            format!(
                "{} · {} · {}",
                count_noun(i64::try_from(type_count).unwrap_or(0), "type"),
                count_noun(objects, "object"),
                count_noun(links, "link")
            )
        });
        design::page_header(ui, "Custom Objects", subtitle.as_deref(), |ui| {
            if design::button_with_icon(ui, Icon::Plus, "New Type", ButtonVariant::Primary)
                .clicked()
            {
                self.show_new_type = !self.show_new_type;
            }
            design::page_counter(ui, &format!("{} types", self.types.len()));
        });
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        if self.show_new_type {
            self.new_type_ui(ui, db);
            ui.add_space(10.0);
        }

        let mut nav: Option<NavRequest> = None;
        ui.columns(2, |columns| {
            let types = self.types.clone();
            let mut picked: Option<i64> = None;
            let mut deleted_type: Option<(i64, String)> = None;
            // AUD-P2-11 (C3): types were create-only — Edit opens the
            // builder prefilled.
            let mut edit_type: Option<TypeDetail> = None;
            egui::ScrollArea::vertical()
                .id_salt("co-types")
                .show(&mut columns[0], |ui| {
                    design::section_title(ui, "Types");
                    ui.add_space(4.0);
                    for t in &types {
                        ui.horizontal(|ui| {
                            if design::chip(
                                ui,
                                self.selected_type == Some(t.id),
                                &format!("{} ({})", t.name, t.object_count),
                            )
                            .clicked()
                            {
                                picked = Some(t.id);
                            }
                            if design::button(ui, "Edit", ButtonVariant::Outline).clicked() {
                                edit_type = Some(t.clone());
                            }
                            if design::button(ui, "Delete", ButtonVariant::Destructive).clicked() {
                                deleted_type = Some((t.id, t.name.clone()));
                            }
                        });
                        if let Some(desc) = &t.description {
                            design::hint_text(ui, desc);
                        }
                    }
                    if types.is_empty() {
                        // UX-16: the empty state carries the "create a type"
                        // action instead of only naming it in prose.
                        if design::empty_state_cta(
                            ui,
                            Icon::Boxes,
                            "No types yet",
                            "A type defines the fields your custom objects carry — \
                             create one to start storing objects.",
                            "New Type",
                        ) {
                            self.show_new_type = true;
                        }
                    }
                });
            if let Some(id) = picked {
                self.select_type(db, id);
            }
            if let Some(t) = edit_type {
                self.start_type_edit(&t);
            }
            if let Some((id, name)) = deleted_type {
                nav = Some(NavRequest::Confirm(ConfirmRequest::delete_custom_type(
                    id, &name,
                )));
            }

            if nav.is_none() {
                nav = self.objects_ui(&mut columns[1], db);
            }
        });
        nav
    }

    fn objects_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        let Some(type_id) = self.selected_type else {
            // UX-16: don't tell the user to "pick a type on the left" when
            // no types exist — that instruction was impossible to follow.
            if self.types.is_empty() {
                design::empty_state(
                    ui,
                    Icon::Boxes,
                    "Nothing to show yet",
                    "Objects appear here once a type is created and selected.",
                );
            } else {
                design::empty_state(
                    ui,
                    Icon::Boxes,
                    "Select a type",
                    "Pick a type on the left to see its objects.",
                );
            }
            return None;
        };
        let t = self.types.iter().find(|t| t.id == type_id)?;
        ui.horizontal(|ui| {
            design::section_title(ui, &format!("{} objects", t.name));
            design::page_counter(ui, &self.objects_total.to_string());
        });
        // AUD-P2-11 (C3): the self-admitted "the list has no pager" hint is
        // gone — the honest range line stays, the pager below the list
        // moves through it.
        let shown = self.objects.len() as i64;
        if self.objects_total > shown {
            let from = i64::from(self.objects_page.max(0)) * OBJECTS_PAGE_SIZE + 1;
            design::hint_text(
                ui,
                &format!(
                    "showing {from}–{} of {}",
                    from - 1 + shown,
                    self.objects_total
                ),
            );
        }
        ui.add_space(4.0);
        let apply = design::filter_bar(
            ui,
            |ui| {
                let mut q = self.object_query.clone();
                let response = ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut q).hint_text("search objects…"),
                );
                if response.changed() {
                    self.object_query = q;
                }
            },
            "Search",
        );
        if apply.clicked() {
            // AUD-P2-11 (C3): a new search restarts from page 1.
            self.objects_page = 0;
            self.reload_objects(db);
        }
        ui.add_space(6.0);

        let objects = self.objects.clone();
        let mut deleted: Option<(i64, String)> = None;
        // AUD-P2-11 (C3): deferred card actions — collected while the
        // widgets render, applied after (the page's existing borrow-safe
        // pick/delete order).
        let mut edit_object: Option<serde_json::Value> = None;
        let mut link_object: Option<i64> = None;
        let mut unlink: Option<(i64, String, i64)> = None;
        egui::ScrollArea::vertical()
            .id_salt("co-objects")
            .show(ui, |ui| {
                let tokens = design::tokens(ui);
                for object in &objects {
                    let object_id = i64_field(object, "id");
                    // AUD-P2-11 (C3): per-card state cloned out so the
                    // widgets edit locals and write back once, card-scoped.
                    let mut draft = self
                        .link_drafts
                        .get(&object_id)
                        .cloned()
                        .unwrap_or_default();
                    let links = self
                        .object_links
                        .get(&object_id)
                        .cloned()
                        .unwrap_or_default();
                    ui.push_id(format!("co-object-{object_id}"), |ui| {
                        design::card(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(str_field(object, "title"))
                                        .size(13.5)
                                        .strong()
                                        .color(tokens.foreground),
                                );
                                if design::button(ui, "Edit", ButtonVariant::Outline).clicked() {
                                    edit_object = Some(object.clone());
                                }
                                if design::button(ui, "Delete", ButtonVariant::Destructive)
                                    .clicked()
                                {
                                    deleted =
                                        Some((i64_field(object, "id"), str_field(object, "title")));
                                }
                            });
                            if let Some(props) =
                                object.get("properties").and_then(|p| p.as_object())
                            {
                                for (key, value) in props {
                                    let rendered = match value {
                                        serde_json::Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                    ui.horizontal(|ui| {
                                        design::field_label(ui, &format!("{key}:"));
                                        ui.monospace(
                                            egui::RichText::new(rendered).color(tokens.foreground),
                                        );
                                    });
                                }
                            }
                            // AUD-P2-11 (C3): the links row the header has
                            // always counted — chips remove (the hover says
                            // so), the mini form below adds.
                            ui.horizontal_wrapped(|ui| {
                                design::field_label(ui, "Links:");
                                if links.is_empty() {
                                    design::hint_text(ui, "none yet");
                                }
                                for link in &links {
                                    let kind = str_field(link, "target_kind");
                                    let target = i64_field(link, "target_local_id");
                                    let chip_text = match opt_str_field(link, "note") {
                                        Some(note) => format!(
                                            "{} · {} ({note})",
                                            kind,
                                            str_field(link, "label")
                                        ),
                                        None => {
                                            format!("{} · {}", kind, str_field(link, "label"))
                                        }
                                    };
                                    if design::chip(ui, false, &chip_text)
                                        .on_hover_text(format!(
                                            "click to remove this link — {kind} #{target}"
                                        ))
                                        .clicked()
                                    {
                                        unlink = Some((object_id, kind, target));
                                    }
                                }
                            });
                            // AUD-P2-11 (C3): the add-link mini form — kind
                            // select over the core's LINK_TARGET_KINDS,
                            // numeric target id, optional note.
                            ui.horizontal(|ui| {
                                let selected_kind = draft.kind.clone();
                                design::select(
                                    ui,
                                    &format!("co-link-kind-{object_id}"),
                                    &selected_kind,
                                    130.0,
                                    |ui| {
                                        for kind in spp_core::custom_objects::LINK_TARGET_KINDS {
                                            ui.selectable_value(
                                                &mut draft.kind,
                                                kind.to_string(),
                                                kind,
                                            );
                                        }
                                    },
                                )
                                .on_hover_text("what to link this object to");
                                ui.add_sized(
                                    [80.0, 28.0],
                                    egui::TextEdit::singleline(&mut draft.id)
                                        .hint_text("target id"),
                                )
                                .on_hover_text(
                                    "the row id of the customer, organization, conversation…",
                                );
                                ui.add_sized(
                                    [120.0, 28.0],
                                    egui::TextEdit::singleline(&mut draft.note)
                                        .hint_text("note (optional)"),
                                );
                                if design::button(ui, "Link", ButtonVariant::Secondary).clicked() {
                                    link_object = Some(object_id);
                                }
                            });
                        });
                    });
                    self.link_drafts.insert(object_id, draft);
                }
                if objects.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Boxes,
                        "No objects",
                        "Create one below or adjust the search — objects store \
                         per-customer properties used by the graph and AI answers.",
                    );
                }
            });
        // AUD-P2-11 (C3): the pager the audit flagged — ‹ Prev / Next ›
        // under the list, offset = page * 50 on reload_objects.
        let pages = u32::try_from(
            ((self.objects_total + OBJECTS_PAGE_SIZE - 1) / OBJECTS_PAGE_SIZE).max(1),
        )
        .unwrap_or(1);
        let delta = design::pager(
            ui,
            u32::try_from(self.objects_page.max(0)).unwrap_or(0),
            pages,
        );
        if delta != 0 {
            let pages_i = i32::try_from(pages).unwrap_or(i32::MAX);
            if let Some(next) = self.objects_page.checked_add(delta)
                && next >= 0
                && next < pages_i
            {
                self.objects_page = next;
                self.reload_objects(db);
            }
        }
        if let Some(object_id) = link_object {
            self.add_object_link(db, object_id);
        }
        if let Some((object_id, kind, target)) = unlink {
            self.remove_object_link(db, object_id, &kind, target);
        }
        if let Some(object) = edit_object {
            self.start_object_edit(&object);
        }
        let nav = deleted.map(|(id, title)| {
            NavRequest::Confirm(ConfirmRequest::delete_custom_object(id, &title))
        });
        ui.add_space(10.0);

        // AUD-P2-11 (C3): the form doubles as the object edit form
        // (connectors idiom) — shared labels switch, Cancel appears.
        let editing_object = self.edit_object_id.is_some();
        let keys: Vec<String> = self
            .new_object_fields
            .iter()
            .map(|(k, _)| k.clone())
            .collect();
        let has_fields = !keys.is_empty();
        let mut link_kind = self.new_object_link_kind.clone();
        let mut link_id = self.new_object_link_id.clone();
        let mut link_note = self.new_object_link_note.clone();
        design::card_titled(
            ui,
            if editing_object {
                "Edit object"
            } else {
                "New object"
            },
            None,
            |ui| {
                let title_label = design::field_label(ui, "Title");
                ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut self.new_object_title)
                        .hint_text("object title"),
                )
                .labelled_by(title_label.id);
                for (idx, key) in keys.iter().enumerate() {
                    let label = design::field_label(ui, &format!("{key}:"));
                    ui.add_sized(
                        [180.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_object_fields[idx].1),
                    )
                    .labelled_by(label.id);
                }
                if !editing_object {
                    // AUD-P2-11 (C3): the link row that replaces the
                    // hard-coded `Vec::new()` — one link can land at
                    // creation time; more can be added on the card above.
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        design::field_label(ui, "Link to:");
                        let selected_link_kind = link_kind.clone();
                        design::select(
                            ui,
                            "co-new-object-link-kind",
                            &selected_link_kind,
                            150.0,
                            |ui| {
                                for kind in spp_core::custom_objects::LINK_TARGET_KINDS {
                                    ui.selectable_value(&mut link_kind, kind.to_string(), kind);
                                }
                            },
                        )
                        .on_hover_text("what to link the new object to");
                        ui.add_sized(
                            [90.0, 28.0],
                            egui::TextEdit::singleline(&mut link_id).hint_text("target id"),
                        )
                        .on_hover_text(
                            "the row id to link — e.g. the customer id; leave empty for no link",
                        );
                        ui.add_sized(
                            [140.0, 28.0],
                            egui::TextEdit::singleline(&mut link_note).hint_text("note (optional)"),
                        );
                    });
                }
                if has_fields || editing_object {
                    if design::button(
                        ui,
                        if editing_object {
                            "Save Changes"
                        } else {
                            "Create Object"
                        },
                        ButtonVariant::Primary,
                    )
                    .clicked()
                    {
                        if editing_object {
                            self.save_object_edit(db);
                        } else {
                            self.create_object(db);
                        }
                    }
                    if editing_object
                        && design::button(ui, "Cancel", ButtonVariant::Outline).clicked()
                    {
                        self.cancel_object_edit();
                    }
                }
            },
        );
        self.new_object_link_kind = link_kind;
        self.new_object_link_id = link_id;
        self.new_object_link_note = link_note;
        nav
    }

    fn new_type_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        // AUD-P2-11 (C3): the builder doubles as the type edit form
        // (connectors idiom) — shared labels switch, Cancel appears.
        let editing = self.edit_type_id.is_some();
        design::card(ui, |ui| {
            design::section_title(ui, if editing { "Edit Type" } else { "New Type" });
            design::form_grid(ui, "co-new-type", |ui| {
                let name_label = design::field_label(ui, "Name");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_type_name).hint_text("type name"),
                )
                .labelled_by(name_label.id);
                ui.end_row();
                let desc_label = design::field_label(ui, "Description");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_type_description)
                        .hint_text("what this type collects"),
                )
                .labelled_by(desc_label.id);
                ui.end_row();
            });
            ui.add_space(6.0);
            design::divider(ui);
            ui.add_space(6.0);
            design::section_title(ui, "Fields");
            let tokens = design::tokens(ui);
            let mut remove: Option<usize> = None;
            for (idx, draft) in self.new_fields.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.monospace(
                        egui::RichText::new(format!("{}. {}", idx + 1, draft.key))
                            .color(tokens.foreground),
                    );
                    if design::button(ui, "Remove", ButtonVariant::Destructive).clicked() {
                        remove = Some(idx);
                    }
                });
            }
            if let Some(idx) = remove {
                self.new_fields.remove(idx);
            }
            ui.horizontal(|ui| {
                if design::button(ui, "+ add field", ButtonVariant::Outline).clicked() {
                    self.new_fields.push(FieldDraft {
                        key: String::new(),
                        label: String::new(),
                        field_type: FieldType::Text,
                        required: false,
                        options: String::new(),
                    });
                }
            });
            for (idx, draft) in self.new_fields.clone().iter().enumerate() {
                let mut key = draft.key.clone();
                let mut label = draft.label.clone();
                let mut options = draft.options.clone();
                ui.push_id(format!("co-field-{idx}"), |ui| {
                    egui::Grid::new(format!("co-field-grid-{idx}"))
                        .num_columns(5)
                        .show(ui, |ui| {
                            let key_label = design::field_label(ui, "Key");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut key).hint_text("key"),
                            )
                            .labelled_by(key_label.id);
                            let label_label = design::field_label(ui, "Label");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut label).hint_text("label"),
                            )
                            .labelled_by(label_label.id);
                            design::field_label(ui, "Type");
                            ui.horizontal(|ui| {
                                for field_type in FieldType::ALL {
                                    if design::chip(
                                        ui,
                                        self.new_fields[idx].field_type == field_type,
                                        field_type.as_str(),
                                    )
                                    .clicked()
                                    {
                                        self.new_fields[idx].field_type = field_type;
                                    }
                                }
                            });
                            ui.checkbox(&mut self.new_fields[idx].required, "Required");
                            if self.new_fields[idx].field_type == FieldType::Select {
                                ui.add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut options),
                                )
                                .on_hover_text("comma-separated options")
                                .widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::TextInput,
                                        ui.is_enabled(),
                                        "select options (comma-separated)",
                                    )
                                });
                            }
                            ui.end_row();
                        });
                });
                self.new_fields[idx].key = key;
                self.new_fields[idx].label = label;
                self.new_fields[idx].options = options;
            }
            if design::button(
                ui,
                if editing {
                    "Save Changes"
                } else {
                    "Create Type"
                },
                ButtonVariant::Primary,
            )
            .clicked()
            {
                if editing {
                    self.save_type_edit(db);
                } else {
                    self.create_type(db);
                }
            }
            if editing && design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                self.cancel_type_edit();
            }
        });
    }
}

impl Default for CustomObjectsPage {
    fn default() -> Self {
        Self::new()
    }
}

fn coerce_property_value(
    field_type: &str,
    value: &str,
) -> std::result::Result<serde_json::Value, String> {
    match field_type {
        "number" => {
            if let Ok(i) = value.parse::<i64>() {
                Ok(serde_json::Value::from(i))
            } else if let Ok(f) = value.parse::<f64>()
                && f.is_finite()
            {
                Ok(serde_json::Value::from(f))
            } else {
                Err(format!("'{value}' is not a valid number"))
            }
        }
        "boolean" | "checkbox" => match value.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(serde_json::Value::from(true)),
            "false" | "0" | "no" | "off" => Ok(serde_json::Value::from(false)),
            _ => Err(format!("'{value}' is not a boolean (true/false)")),
        },
        _ => Ok(serde_json::Value::from(value)),
    }
}

/// AUD-P2-11 (C3): a stored property value back into form text so an edit
/// prefills exactly what was saved — numbers and booleans round-trip
/// through `coerce_property_value`.
fn property_to_form_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    fn field(
        key: &str,
        field_type: &str,
        required: bool,
        options: Option<Vec<String>>,
    ) -> FieldInput {
        FieldInput {
            key: key.into(),
            label: key.into(),
            field_type: field_type.into(),
            required,
            options,
        }
    }

    #[test]
    fn create_type_and_objects_round_trip() {
        let db = fixture(Vec::new());
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        assert!(page.types.is_empty());

        page.new_type_name = "Warranty Claims".into();
        page.new_fields = vec![FieldDraft {
            key: "sku".into(),
            label: "SKU".into(),
            field_type: FieldType::Text,
            required: true,
            options: String::new(),
        }];
        page.create_type(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.types.len(), 1);
        let t = &page.types[0];
        assert_eq!(t.name, "Warranty Claims");
        assert_eq!(t.slug, "warranty-claims");
        assert_eq!(t.fields.len(), 1);
        assert_eq!(t.fields[0].key, "sku");
        assert!(t.fields[0].required);
        assert_eq!(page.selected_type, Some(t.id));

        page.new_object_title = "Broken hinge".into();
        page.new_object_fields = vec![("sku".into(), "A-100".into())];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.objects_total, 1);
        assert_eq!(str_field(&page.objects[0], "title"), "Broken hinge");
        assert_eq!(page.types[0].object_count, 1);
    }

    #[test]
    fn required_fields_are_enforced_on_object_create() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Rmas",
                None,
                &[field("reason", "text", true, None)],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.selected_type = Some(type_id);
        page.new_object_title = "No reason".into();
        page.new_object_fields = vec![("reason".into(), String::new())];
        page.create_object(&db);
        assert!(
            page.error.is_some(),
            "required field must reject the object"
        );
        assert_eq!(page.objects_total, 0);
    }

    #[test]
    fn select_fields_need_options() {
        let mut draft = FieldDraft {
            key: "stage".into(),
            label: "Stage".into(),
            field_type: FieldType::Select,
            required: false,
            options: String::new(),
        };
        assert!(draft.to_input().is_err());
        draft.options = "open, closed".into();
        let input = draft.to_input().expect("with options");
        assert_eq!(
            input.options.expect("options"),
            vec!["open".to_string(), "closed".to_string()]
        );
    }

    #[test]
    fn field_draft_requires_a_key() {
        let draft = FieldDraft {
            key: "  ".into(),
            label: String::new(),
            field_type: FieldType::Text,
            required: false,
            options: String::new(),
        };
        let error = draft.to_input().unwrap_err();
        assert!(error.contains("key required"));
    }
    #[test]
    fn field_type_builder_offers_exactly_the_core_vocabulary() {
        let offered: Vec<&str> = FieldType::ALL.iter().map(|f| f.as_str()).collect();
        assert_eq!(
            offered,
            vec!["text", "long_text", "number", "date", "select", "boolean"],
            "the chips must match spp_core::custom_objects::validate_properties"
        );
        for name in ["checkbox", "url"] {
            assert!(
                !offered.contains(&name),
                "the core rejects {name} as an unknown field type — the builder must not offer it"
            );
        }
    }

    #[test]
    fn number_and_boolean_form_inputs_become_real_json_scalars() {
        let db = fixture(Vec::new());
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);

        page.new_type_name = "Inventory".into();
        page.new_fields = vec![
            FieldDraft {
                key: "sku".into(),
                label: "SKU".into(),
                field_type: FieldType::Text,
                required: false,
                options: String::new(),
            },
            FieldDraft {
                key: "qty".into(),
                label: "Qty".into(),
                field_type: FieldType::Number,
                required: false,
                options: String::new(),
            },
            FieldDraft {
                key: "in_stock".into(),
                label: "In stock".into(),
                field_type: FieldType::Boolean,
                required: false,
                options: String::new(),
            },
            FieldDraft {
                key: "unit_cost".into(),
                label: "Unit cost".into(),
                field_type: FieldType::Number,
                required: false,
                options: String::new(),
            },
            FieldDraft {
                key: "stage".into(),
                label: "Stage".into(),
                field_type: FieldType::Select,
                required: false,
                options: "live, eol".into(),
            },
        ];
        page.create_type(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        page.new_object_title = "Widget".into();
        assert_eq!(page.new_object_fields.len(), 5);
        for (key, value) in page.new_object_fields.iter_mut() {
            *value = match key.as_str() {
                "sku" => "A-100".into(),
                "qty" => "42".into(),
                "in_stock" => "true".into(),
                "unit_cost" => "1.5".into(),
                "stage" => "live".into(),
                other => unreachable!("unexpected field {other}"),
            };
        }
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.objects_total, 1);

        let props = &page.objects[0]["properties"];
        assert!(
            props["qty"].is_number(),
            "qty must be a JSON number, got {}",
            props["qty"]
        );
        assert_eq!(props["qty"].as_i64(), Some(42));
        assert_eq!(props["unit_cost"].as_f64(), Some(1.5));
        assert_eq!(props["in_stock"].as_bool(), Some(true));
        assert_eq!(props["sku"], serde_json::json!("A-100"));
        assert_eq!(props["stage"], serde_json::json!("live"));
    }

    #[test]
    fn number_form_input_rejects_garbage_with_a_clear_error() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Rmas",
                None,
                &[field("count", "number", false, None)],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);

        page.new_object_title = "Bad".into();
        page.new_object_fields = vec![("count".into(), "not-a-number".into())];
        page.create_object(&db);
        let error = page.error.as_deref().unwrap_or_default().to_string();
        assert!(
            error.contains("not a valid number"),
            "the form must explain the bad number: {error}"
        );
        assert_eq!(page.objects_total, 0);

        page.error = None;
        page.new_object_fields = vec![("count".into(), "2.25".into())];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.objects_total, 1);
        assert_eq!(page.objects[0]["properties"]["count"].as_f64(), Some(2.25));
    }

    #[test]
    fn boolean_form_inputs_accept_common_truth_words() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Flags",
                None,
                &[field("done", "boolean", false, None)],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);

        for (input, expected) in [("yes", true), ("no", false), ("TRUE", true), ("0", false)] {
            page.new_object_title = format!("Flag {input}");
            page.new_object_fields = vec![("done".into(), input.into())];
            page.create_object(&db);
            assert!(page.error.is_none(), "{input} must parse: {:?}", page.error);
            assert_eq!(
                page.objects[0]["properties"]["done"].as_bool(),
                Some(expected),
                "{input} must coerce to {expected}"
            );
        }

        page.new_object_title = "Broken".into();
        page.new_object_fields = vec![("done".into(), "maybe".into())];
        page.create_object(&db);
        let error = page.error.as_deref().unwrap_or_default().to_string();
        assert!(
            error.contains("not a boolean"),
            "the form must explain the bad boolean: {error}"
        );
    }

    #[test]
    fn legacy_checkbox_and_url_fields_work_from_the_form() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Legacy Builder Types",
                None,
                &[
                    field("shipped", "checkbox", false, None),
                    field("docs", "url", false, None),
                ],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);

        page.new_object_title = "Legacy".into();
        page.new_object_fields = vec![
            ("shipped".into(), "true".into()),
            ("docs".into(), "https://example.com/x".into()),
        ];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.objects_total, 1);
        assert_eq!(
            page.objects[0]["properties"]["shipped"].as_bool(),
            Some(true)
        );
        assert_eq!(
            page.objects[0]["properties"]["docs"],
            serde_json::json!("https://example.com/x")
        );
    }

    #[test]
    fn ensure_loaded_latches_and_invalidate_refetches() {
        let db = fixture(Vec::new());
        let mut page = CustomObjectsPage::new();
        assert!(!page.types_loaded);
        page.ensure_loaded(&db);
        assert!(page.types_loaded);
        assert!(page.types.is_empty(), "no types seeded yet");

        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Latched",
                None,
                &[field("reason", "text", false, None)],
            )
            .expect("type")
        });

        page.error = Some("sentinel".into());
        page.ensure_loaded(&db);
        assert!(
            page.types.is_empty(),
            "latch holds: ensure_loaded did not refetch (pre-fix this re-ran reload_types every frame on an empty mirror)"
        );
        assert_eq!(
            page.error.as_deref(),
            Some("sentinel"),
            "the reload side effect (error clear) did not run"
        );

        page.invalidate();
        assert!(!page.types_loaded);
        page.ensure_loaded(&db);
        assert!(page.types_loaded);
        assert_eq!(page.types.len(), 1, "invalidate refetches");
        assert!(page.error.is_none(), "the refetch cleared the error");

        page.select_type(&db, type_id);
        assert!(page.objects_loaded);
        assert!(page.objects.is_empty(), "a legitimately empty object list");
        page.error = Some("sentinel".into());
        page.ensure_loaded(&db);
        assert_eq!(
            page.error.as_deref(),
            Some("sentinel"),
            "the empty-objects gate latches too (pre-fix this re-ran reload_objects every frame)"
        );
    }

    // AUD-P2-11 (C3): the links lifecycle the page advertised but never
    // had — add via the card's mini form, list with resolved labels,
    // remove via the chip action.
    #[test]
    fn audp2_link_add_list_remove_round_trip() {
        // fixture() seeds customers 9 (Kim Pearce) and 10 (Ada Customer).
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Warranty",
                None,
                &[field("sku", "text", false, None)],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);
        page.new_object_title = "Broken hinge".into();
        page.new_object_fields = vec![("sku".into(), "A-100".into())];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        let object_id = i64_field(&page.objects[0], "id");

        // add through the card's mini form state
        {
            let draft = page.link_drafts.entry(object_id).or_default();
            draft.kind = "customer".into();
            draft.id = "9".into();
            draft.note = "warranty sender".into();
        }
        page.add_object_link(&db, object_id);
        assert!(page.error.is_none(), "{:?}", page.error);
        let links = page.object_links.get(&object_id).expect("links loaded");
        assert_eq!(links.len(), 1);
        assert_eq!(str_field(&links[0], "label"), "Kim Pearce");
        assert_eq!(str_field(&links[0], "target_kind"), "customer");
        assert_eq!(i64_field(&links[0], "target_local_id"), 9);
        // the header's "N links" count picked it up
        assert_eq!(
            i64_field(page.report.as_ref().expect("report"), "total_links"),
            1
        );
        // the card's form cleared after the successful add
        assert!(page.link_drafts[&object_id].id.is_empty());

        // a non-numeric target id is refused before any write
        page.link_drafts.get_mut(&object_id).unwrap().id = "not-a-number".into();
        page.add_object_link(&db, object_id);
        assert!(
            page.error
                .as_deref()
                .unwrap_or_default()
                .contains("must be a number"),
            "the form must explain the bad id: {:?}",
            page.error
        );

        // a nonexistent target surfaces the core's validation error
        page.link_drafts.get_mut(&object_id).unwrap().id = "999".into();
        page.add_object_link(&db, object_id);
        let error = page.error.as_deref().unwrap_or_default().to_string();
        assert!(
            error.contains("does not exist"),
            "the core refusal must reach the error slot: {error}"
        );
        assert_eq!(
            page.object_links[&object_id].len(),
            1,
            "no second link landed"
        );

        // remove → gone, header count back to zero
        page.remove_object_link(&db, object_id, "customer", 9);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(
            page.object_links.get(&object_id).is_some_and(Vec::is_empty),
            "the removed link is gone from the card's row"
        );
        assert_eq!(
            i64_field(page.report.as_ref().expect("report"), "total_links"),
            0
        );
    }

    // AUD-P2-11 (C3): the new-object form's link row — create_object used
    // to pass a hard-coded `Vec::new()`.
    #[test]
    fn audp2_create_object_with_link_from_the_form() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Warranty",
                None,
                &[field("sku", "text", false, None)],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);
        page.new_object_title = "Cracked screen".into();
        page.new_object_fields = vec![("sku".into(), "B-2".into())];
        page.new_object_link_kind = "customer".into();
        page.new_object_link_id = "9".into();
        page.new_object_link_note = "reported by".into();
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        let object_id = i64_field(&page.objects[0], "id");
        let links = page.object_links.get(&object_id).expect("creation link");
        assert_eq!(links.len(), 1);
        assert_eq!(str_field(&links[0], "label"), "Kim Pearce");
        // the form cleared after the create
        assert!(page.new_object_link_id.is_empty());

        // a garbage target id stops the create before any write
        page.new_object_title = "Never lands".into();
        page.new_object_link_id = "nine".into();
        page.create_object(&db);
        assert!(
            page.error
                .as_deref()
                .unwrap_or_default()
                .contains("must be a number"),
            "the form must explain the bad id: {:?}",
            page.error
        );
        assert_eq!(page.objects_total, 1, "no second object was created");
    }

    // AUD-P2-11 (C3): objects were create-only — the form now prefills
    // from the stored row and saves through update_object.
    #[test]
    fn audp2_object_edit_round_trip() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Inventory",
                None,
                &[
                    field("sku", "text", false, None),
                    field("qty", "number", false, None),
                    field("in_stock", "boolean", false, None),
                ],
            )
            .expect("type")
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);
        page.new_object_title = "Widget".into();
        page.new_object_fields = vec![
            ("sku".into(), "A-100".into()),
            ("qty".into(), "42".into()),
            ("in_stock".into(), "true".into()),
        ];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        // the edit prefills title + typed properties
        let object = page.objects[0].clone();
        let object_id = i64_field(&object, "id");
        page.start_object_edit(&object);
        assert_eq!(page.edit_object_id, Some(object_id));
        assert_eq!(page.new_object_title, "Widget");
        assert_eq!(
            page.new_object_fields,
            vec![
                ("sku".into(), "A-100".into()),
                ("qty".into(), "42".into()),
                ("in_stock".into(), "true".into()),
            ],
            "stored scalars round-trip back into form text"
        );

        page.new_object_title = "Widget v2".into();
        page.new_object_fields = vec![
            ("sku".into(), "A-100".into()),
            ("qty".into(), "7".into()),
            ("in_stock".into(), "false".into()),
        ];
        page.save_object_edit(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(page.edit_object_id.is_none(), "edit mode closes on save");
        assert_eq!(page.objects_total, 1);
        let updated = &page.objects[0];
        assert_eq!(str_field(updated, "title"), "Widget v2");
        assert_eq!(updated["properties"]["qty"].as_i64(), Some(7));
        assert_eq!(updated["properties"]["in_stock"].as_bool(), Some(false));

        // cancel returns the form to create mode without writing
        page.start_object_edit(&page.objects[0].clone());
        page.new_object_title = "Scratch".into();
        page.cancel_object_edit();
        assert!(page.edit_object_id.is_none());
        assert!(page.new_object_title.is_empty());
        assert_eq!(
            str_field(&page.objects[0], "title"),
            "Widget v2",
            "cancel wrote nothing"
        );
    }

    // AUD-P2-11 (C3): types were create-only — the builder now prefills,
    // saves through update_type, and shows the core's refusal when a
    // dropped field is still set on objects.
    #[test]
    fn audp2_type_edit_round_trip_and_in_use_field_refusal() {
        let db = fixture(Vec::new());
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.new_type_name = "Warranty Claims".into();
        page.new_fields = vec![FieldDraft {
            key: "sku".into(),
            label: "SKU".into(),
            field_type: FieldType::Text,
            required: true,
            options: String::new(),
        }];
        page.create_type(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        let type_id = page.types[0].id;

        page.new_object_title = "Broken hinge".into();
        page.new_object_fields = vec![("sku".into(), "A-100".into())];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        // the builder prefills from the stored type
        let t = page
            .types
            .iter()
            .find(|t| t.id == type_id)
            .cloned()
            .unwrap();
        page.start_type_edit(&t);
        assert_eq!(page.edit_type_id, Some(type_id));
        assert_eq!(page.new_type_name, "Warranty Claims");
        assert_eq!(page.new_fields.len(), 1);
        assert_eq!(page.new_fields[0].key, "sku");
        assert!(page.new_fields[0].required);

        // dropping a field still set on an object is refused, honestly
        page.new_fields.clear();
        page.save_type_edit(&db);
        let error = page.error.as_deref().unwrap_or_default().to_string();
        assert!(
            error.contains("still set on 1 object"),
            "the core refusal must reach the page's error slot: {error}"
        );
        assert!(
            page.edit_type_id.is_some(),
            "the edit form stays open on refusal"
        );
        let still = page
            .types
            .iter()
            .find(|t| t.id == type_id)
            .cloned()
            .unwrap();
        assert_eq!(still.fields.len(), 1, "the type is untouched on refusal");

        // a real edit: rename + add a field
        page.new_type_name = "Warranty Claims v2".into();
        page.new_fields = vec![
            FieldDraft {
                key: "sku".into(),
                label: "SKU".into(),
                field_type: FieldType::Text,
                required: true,
                options: String::new(),
            },
            FieldDraft {
                key: "qty".into(),
                label: "Qty".into(),
                field_type: FieldType::Number,
                required: false,
                options: String::new(),
            },
        ];
        page.save_type_edit(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(page.edit_type_id.is_none(), "edit mode closes on save");
        let updated = page
            .types
            .iter()
            .find(|t| t.id == type_id)
            .cloned()
            .unwrap();
        assert_eq!(updated.name, "Warranty Claims v2");
        assert_eq!(updated.fields.len(), 2);
        // the slug is identity — it stays
        assert_eq!(updated.slug, "warranty-claims");
        // the selected type's object form picked up the new field set
        assert_eq!(page.selected_type, Some(type_id));
        assert_eq!(
            page.new_object_fields
                .iter()
                .map(|(k, _)| k.as_str())
                .collect::<Vec<_>>(),
            vec!["sku", "qty"]
        );
        // the object survived the type edit
        assert_eq!(page.objects_total, 1);
        assert_eq!(str_field(&page.objects[0], "title"), "Broken hinge");
    }

    // AUD-P2-11 (C3): the pager the page self-admitted it lacked — page 2
    // must load different objects, and the page state must reset.
    #[test]
    fn audp2_objects_pager_loads_page_two() {
        let db = fixture(Vec::new());
        let (type_id, _) = db.with_write(|conn| {
            spp_core::custom_objects::create_type(
                conn,
                "Bulk",
                None,
                &[field("n", "number", false, None)],
            )
            .expect("type")
        });
        // 55 objects → two pages at 50 per page
        db.with_write(|conn| {
            for i in 1..=55 {
                let mut props = serde_json::Map::new();
                props.insert("n".into(), serde_json::json!(i));
                spp_core::custom_objects::create_object(
                    conn,
                    type_id,
                    &format!("Object {i:03}"),
                    &props,
                    &[],
                )
                .expect("object");
            }
        });
        let mut page = CustomObjectsPage::new();
        page.reload_types(&db);
        page.select_type(&db, type_id);
        assert_eq!(page.objects_total, 55, "the total counts every object");
        assert_eq!(page.objects.len(), 50, "page 1 holds the page size");
        assert_eq!(page.objects_page, 0);
        let mut seen: std::collections::HashSet<i64> =
            page.objects.iter().map(|o| i64_field(o, "id")).collect();
        assert_eq!(seen.len(), 50, "no duplicates on page 1");

        // page 2 loads the rest — different objects
        page.objects_page = 1;
        page.reload_objects(&db);
        assert_eq!(page.objects.len(), 5, "page 2 holds the rest");
        assert_eq!(page.objects_total, 55);
        for object in &page.objects {
            assert!(
                seen.insert(i64_field(object, "id")),
                "no object appears on two pages"
            );
        }

        // a page past the end falls back to the last page
        page.objects_page = 9;
        page.reload_objects(&db);
        assert_eq!(
            page.objects_page, 1,
            "the past-the-end page clamped to the last page"
        );
        assert_eq!(page.objects.len(), 5);

        // re-selecting a type restarts at page 1
        page.objects_page = 1;
        page.select_type(&db, type_id);
        assert_eq!(page.objects_page, 0, "selecting a type restarts at page 1");
        assert_eq!(page.objects.len(), 50);
    }
}
