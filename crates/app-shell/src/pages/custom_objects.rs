//! Custom Objects page (port of the web `pages/custom_objects.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`): card
//! sections, chip type selectors, a form-grid creator and destructive
//! delete buttons — presentation only, all CRUD behavior intact.
//!
//! Two panes (web: type sidebar + object table): a type list / create
//! form (`custom_objects::{list_types,create_type,delete_type}`) and the
//! selected type's object table (`list_objects`, the
//! `/api/custom-objects` SQL) with the new-object form (`create_object`)
//! and delete. The report banner ports `custom_objects::report`
//! (`/api/custom-objects/report`).

use eframe::egui;
use spp_core::custom_objects::{FieldInput, LinkInput, TypeDetail};
use spp_core::db::DbHandle;

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// Field types offered by the type builder (web: the field type select).
///
/// Clean-audit item 27: this list is exactly the vocabulary the core's
/// `validate_properties` accepts (`custom_objects.rs`) — the builder used
/// to offer `checkbox` and `url`, which the core rejected as unknown
/// field types, so any type built with them could never hold an object.
/// `long_text` is included because the core accepts it and the builder
/// was previously the only way to add fields to a type.
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
}

/// A field row under construction in the type builder.
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

/// The interactive custom objects page state.
pub struct CustomObjectsPage {
    types: Vec<TypeDetail>,
    selected_type: Option<i64>,
    objects: Vec<serde_json::Value>,
    objects_total: i64,
    object_query: String,
    report: Option<serde_json::Value>,
    show_new_type: bool,
    new_type_name: String,
    new_type_description: String,
    new_fields: Vec<FieldDraft>,
    new_object_title: String,
    new_object_fields: Vec<(String, String)>,
    error: Option<String>,
}

impl CustomObjectsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            selected_type: None,
            objects: Vec::new(),
            objects_total: 0,
            object_query: String::new(),
            report: None,
            show_new_type: false,
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
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.types.clear();
        self.objects.clear();
        self.report = None;
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
                    self.new_object_fields.clear();
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.report = db.with_read(spp_core::custom_objects::report).ok();
    }

    fn reload_objects(&mut self, db: &DbHandle) {
        let Some(type_id) = self.selected_type else {
            return;
        };
        let query = self.object_query.trim().to_string();
        let result = db.with_read(|conn| {
            spp_core::custom_objects::list_objects(
                conn,
                Some(type_id),
                if query.is_empty() {
                    None
                } else {
                    Some(query.as_str())
                },
                50,
                0,
            )
        });
        match result {
            Ok((objects, total)) => {
                self.objects = objects;
                self.objects_total = total;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.types.is_empty() {
            self.reload_types(db);
        }
        if self.selected_type.is_some() && self.objects.is_empty() {
            self.reload_objects(db);
        }
    }

    fn select_type(&mut self, db: &DbHandle, id: i64) {
        self.selected_type = Some(id);
        self.objects.clear();
        self.reload_objects(db);
        // Rebuild the new-object form fields from the type definition.
        if let Some(t) = self.types.iter().find(|t| t.id == id) {
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

    /// Delete a custom-object type (runs on shell confirmation only —
    /// clean-audit item 140). Returns the core outcome for the shell's
    /// toast.
    pub fn delete_type(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::custom_objects::delete_type(conn, id));
        match &result {
            Ok(_) => {
                if self.selected_type == Some(id) {
                    self.selected_type = None;
                    self.objects.clear();
                    self.new_object_fields.clear();
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
        // Clean-audit item 26: the form used to serialise every field
        // value as a JSON string, so number and boolean fields always
        // failed the core's type validation ("Invalid number", …) and
        // object creation never succeeded. Coerce each raw input against
        // the selected type's own field definitions.
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
                    return;
                }
            }
        }
        let links: Vec<LinkInput> = Vec::new();
        let result = db.with_write(|conn| {
            spp_core::custom_objects::create_object(conn, type_id, &title, &properties, &links)
        });
        match result {
            Ok(_) => {
                self.new_object_title.clear();
                for field in &mut self.new_object_fields {
                    field.1.clear();
                }
                self.reload_objects(db);
                self.reload_types(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Delete one custom-object record (runs on shell confirmation only
    /// — clean-audit item 140). Returns the core outcome for the toast.
    pub fn delete_object(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::custom_objects::delete_object(conn, id));
        if let Err(e) = &result {
            self.error = Some(e.to_string());
        }
        self.reload_objects(db);
        self.reload_types(db);
        result.map(|_| ())
    }

    /// Render the page. Returns a confirm request when the user asked to
    /// delete a type or an object (clean-audit item 140 — the deletes run
    /// only after the shell dialog confirms).
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        let subtitle = self.report.as_ref().map(|report| {
            let type_count = report
                .get("types")
                .and_then(|t| t.as_array())
                .map_or(0, std::vec::Vec::len);
            format!(
                "{} type(s) · {} object(s) · {} link(s)",
                type_count,
                i64_field(report, "total_objects"),
                i64_field(report, "total_links")
            )
        });
        design::page_header(ui, "Custom Objects", subtitle.as_deref(), |ui| {
            if design::button_with_icon(ui, Icon::Plus, "New Type", ButtonVariant::Primary)
                .clicked()
            {
                self.show_new_type = !self.show_new_type;
            }
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
                            if design::button(ui, "delete", ButtonVariant::Destructive).clicked() {
                                deleted_type = Some((t.id, t.name.clone()));
                            }
                        });
                        if let Some(desc) = &t.description {
                            design::hint_text(ui, desc);
                        }
                    }
                    if types.is_empty() {
                        design::empty_state(
                            ui,
                            Icon::Boxes,
                            "No types yet",
                            "Create a type to start storing objects.",
                        );
                    }
                });
            if let Some(id) = picked {
                self.select_type(db, id);
            }
            // Clean-audit item 140: arming the dialog replaces the
            // unconfirmed delete.
            if let Some((id, name)) = deleted_type {
                nav = Some(NavRequest::Confirm(ConfirmRequest::delete_custom_type(
                    id, &name,
                )));
            }

            // The type-delete request wins over the object pane's.
            if nav.is_none() {
                nav = self.objects_ui(&mut columns[1], db);
            }
        });
        nav
    }

    fn objects_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        let Some(type_id) = self.selected_type else {
            design::empty_state(
                ui,
                Icon::Boxes,
                "Select a type",
                "Pick a type on the left to see its objects.",
            );
            return None;
        };
        let t = self.types.iter().find(|t| t.id == type_id)?;
        design::section_title(ui, &format!("{} objects ({})", t.name, self.objects_total));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut q = self.object_query.clone();
            let response = ui.add_sized(
                [220.0, 28.0],
                egui::TextEdit::singleline(&mut q).hint_text("search objects…"),
            );
            if response.changed() {
                self.object_query = q;
            }
            if design::button_with_icon(ui, Icon::Search, "search", ButtonVariant::Primary)
                .clicked()
            {
                self.reload_objects(db);
            }
        });
        ui.add_space(6.0);

        let objects = self.objects.clone();
        let mut deleted: Option<(i64, String)> = None;
        egui::ScrollArea::vertical()
            .id_salt("co-objects")
            .show(ui, |ui| {
                let tokens = design::tokens(ui);
                for object in &objects {
                    design::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(str_field(object, "title"))
                                    .size(13.5)
                                    .strong()
                                    .color(tokens.foreground),
                            );
                            if design::button(ui, "delete", ButtonVariant::Destructive).clicked() {
                                deleted =
                                    Some((i64_field(object, "id"), str_field(object, "title")));
                            }
                        });
                        if let Some(props) = object.get("properties").and_then(|p| p.as_object()) {
                            for (key, value) in props {
                                // Item 26 follow-through: number and
                                // boolean properties are real scalars now,
                                // so render them with their JSON form
                                // instead of the string-only path that
                                // showed them blank.
                                let rendered = match value {
                                    serde_json::Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                };
                                ui.monospace(
                                    egui::RichText::new(format!("{key}: {rendered}"))
                                        .color(tokens.foreground),
                                );
                            }
                        }
                    });
                }
                if objects.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Boxes,
                        "No objects",
                        "Create one below or adjust the search.",
                    );
                }
            });
        // Clean-audit item 140: arming the dialog replaces the
        // unconfirmed delete.
        let nav = deleted.map(|(id, title)| {
            NavRequest::Confirm(ConfirmRequest::delete_custom_object(id, &title))
        });
        ui.add_space(10.0);

        let keys: Vec<String> = self
            .new_object_fields
            .iter()
            .map(|(k, _)| k.clone())
            .collect();
        let has_fields = !keys.is_empty();
        design::card_titled(ui, "New object", None, |ui| {
            ui.add_sized(
                [ui.available_width(), 28.0],
                egui::TextEdit::singleline(&mut self.new_object_title).hint_text("object title"),
            );
            for (idx, key) in keys.iter().enumerate() {
                ui.horizontal(|ui| {
                    design::field_label(ui, &format!("{key}:"));
                    ui.add_sized(
                        [180.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_object_fields[idx].1),
                    );
                });
            }
            if has_fields && design::button(ui, "create object", ButtonVariant::Primary).clicked() {
                self.create_object(db);
            }
        });
        nav
    }

    fn new_type_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card(ui, |ui| {
            design::section_title(ui, "New Type");
            design::form_grid(ui, "co-new-type", |ui| {
                design::field_label(ui, "name:");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_type_name).hint_text("type name"),
                );
                ui.end_row();
                design::field_label(ui, "description:");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_type_description)
                        .hint_text("what this type collects"),
                );
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
                    if design::button(ui, "remove", ButtonVariant::Destructive).clicked() {
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
                            design::field_label(ui, "key:");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut key).hint_text("key"),
                            );
                            design::field_label(ui, "label:");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut label).hint_text("label"),
                            );
                            design::field_label(ui, "type:");
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
                            ui.checkbox(&mut self.new_fields[idx].required, "required");
                            if self.new_fields[idx].field_type == FieldType::Select {
                                ui.add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut options),
                                )
                                .on_hover_text("comma-separated options");
                            }
                            ui.end_row();
                        });
                });
                self.new_fields[idx].key = key;
                self.new_fields[idx].label = label;
                self.new_fields[idx].options = options;
            }
            if design::button(ui, "create type", ButtonVariant::Primary).clicked() {
                self.create_type(db);
            }
        });
    }
}

impl Default for CustomObjectsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Coerce one trimmed form input into the JSON scalar the core's
/// `validate_properties` expects for that field type (clean-audit item 26):
/// numbers arrive as JSON numbers, booleans as JSON bools, everything
/// else as strings. The legacy `checkbox`/`url` field names coerce through
/// their canonical equivalents (`boolean`/`text`) so types created before
/// item 27 keep accepting objects from this form.
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
        // text, long_text, url (legacy), date, select and unknown fields
        // stay strings — the core's own validation rejects bad dates and
        // off-option selects with a precise message.
        _ => Ok(serde_json::Value::from(value)),
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
    /// Audit item 27: the builder's vocabulary is exactly what the core's
    /// `validate_properties` accepts — the phantom `checkbox`/`url` chips
    /// are gone, and the previously unreachable `long_text` is offered.
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

    /// Audit item 26: the form used to serialise every value as a JSON
    /// string, so number and boolean fields always failed the core's type
    /// validation and object creation never succeeded. Number inputs now
    /// arrive as JSON numbers and boolean inputs as JSON bools.
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

    /// Item 26: garbage in a number field is a clear form error instead of
    /// a failed platform-side object create.
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

        // The same field accepts a decimal once corrected.
        page.error = None;
        page.new_object_fields = vec![("count".into(), "2.25".into())];
        page.create_object(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.objects_total, 1);
        assert_eq!(page.objects[0]["properties"]["count"].as_f64(), Some(2.25));
    }

    /// Item 26: boolean fields accept the obvious truthy/falsy words, and
    /// anything else is a form error.
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

    /// Item 27 (legacy): types created back when the builder offered the
    /// phantom `checkbox`/`url` names keep working through this form — the
    /// page coerces them via their canonical equivalents and the core's
    /// alias validation accepts the values.
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
}
