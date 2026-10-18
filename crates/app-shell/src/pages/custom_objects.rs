//! Custom Objects page (port of the web `pages/custom_objects.rs`).
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

use super::{i64_field, str_field};

/// Field types offered by the type builder (web: the field type select).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    Text,
    Number,
    Date,
    Select,
    Checkbox,
    Url,
}

impl FieldType {
    const ALL: [Self; 6] = [
        Self::Text,
        Self::Number,
        Self::Date,
        Self::Select,
        Self::Checkbox,
        Self::Url,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Date => "date",
            Self::Select => "select",
            Self::Checkbox => "checkbox",
            Self::Url => "url",
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

    fn delete_type(&mut self, db: &DbHandle, id: i64) {
        let result = db.with_write(|conn| spp_core::custom_objects::delete_type(conn, id));
        match result {
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
        let mut properties = serde_json::Map::new();
        for (key, value) in &self.new_object_fields {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            properties.insert(key.clone(), serde_json::Value::String(value.to_string()));
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

    fn delete_object(&mut self, db: &DbHandle, id: i64) {
        let result = db.with_write(|conn| spp_core::custom_objects::delete_object(conn, id));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload_objects(db);
        self.reload_types(db);
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        ui.heading("Custom Objects");
        if let Some(report) = &self.report {
            let type_count = report
                .get("types")
                .and_then(|t| t.as_array())
                .map_or(0, std::vec::Vec::len);
            ui.weak(format!(
                "{} type(s) · {} object(s) · {} link(s)",
                type_count,
                i64_field(report, "total_objects"),
                i64_field(report, "total_links")
            ));
        }
        ui.horizontal(|ui| {
            if ui.button("New Type").clicked() {
                self.show_new_type = !self.show_new_type;
            }
        });
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
        }
        ui.separator();

        if self.show_new_type {
            self.new_type_ui(ui, db);
            ui.separator();
        }

        ui.columns(2, |columns| {
            let types = self.types.clone();
            let mut picked: Option<i64> = None;
            let mut deleted_type: Option<i64> = None;
            egui::ScrollArea::vertical()
                .id_salt("co-types")
                .show(&mut columns[0], |ui| {
                    ui.strong("Types");
                    for t in &types {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.selected_type == Some(t.id),
                                    format!("{} ({})", t.name, t.object_count),
                                )
                                .clicked()
                            {
                                picked = Some(t.id);
                            }
                            if ui.button("delete").clicked() {
                                deleted_type = Some(t.id);
                            }
                        });
                        if let Some(desc) = &t.description {
                            ui.weak(format!("   {desc}"));
                        }
                    }
                    if types.is_empty() {
                        ui.weak("(no types yet — create one)");
                    }
                });
            if let Some(id) = picked {
                self.select_type(db, id);
            }
            if let Some(id) = deleted_type {
                self.delete_type(db, id);
            }

            self.objects_ui(&mut columns[1], db);
        });
    }

    fn objects_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(type_id) = self.selected_type else {
            ui.weak("Select a type to see its objects.");
            return;
        };
        let Some(t) = self.types.iter().find(|t| t.id == type_id) else {
            return;
        };
        ui.strong(format!("{} objects ({})", t.name, self.objects_total));
        ui.horizontal(|ui| {
            let mut q = self.object_query.clone();
            let response = ui.text_edit_singleline(&mut q);
            if response.changed() {
                self.object_query = q;
            }
            if ui.button("search").clicked() {
                self.reload_objects(db);
            }
        });
        ui.separator();

        let objects = self.objects.clone();
        let mut deleted: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("co-objects")
            .show(ui, |ui| {
                for object in &objects {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(str_field(object, "title"));
                            if ui.button("delete").clicked() {
                                deleted = Some(i64_field(object, "id"));
                            }
                        });
                        if let Some(props) = object.get("properties").and_then(|p| p.as_object()) {
                            for (key, value) in props {
                                ui.monospace(format!(
                                    "{key}: {}",
                                    value.as_str().unwrap_or_default()
                                ));
                            }
                        }
                    });
                }
                if objects.is_empty() {
                    ui.weak("(no objects)");
                }
            });
        if let Some(id) = deleted {
            self.delete_object(db, id);
        }
        ui.separator();

        ui.strong("New object");
        ui.text_edit_singleline(&mut self.new_object_title);
        let keys: Vec<String> = self
            .new_object_fields
            .iter()
            .map(|(k, _)| k.clone())
            .collect();
        for (idx, key) in keys.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.monospace(format!("{key}:"));
                ui.text_edit_singleline(&mut self.new_object_fields[idx].1);
            });
        }
        if !self.new_object_fields.is_empty() && ui.button("create object").clicked() {
            self.create_object(db);
        }
    }

    fn new_type_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("New Type");
        egui::Grid::new("co-new-type")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("name:");
                ui.text_edit_singleline(&mut self.new_type_name);
                ui.end_row();
                ui.label("description:");
                ui.text_edit_singleline(&mut self.new_type_description);
                ui.end_row();
            });
        ui.separator();
        ui.strong("Fields");
        let mut remove: Option<usize> = None;
        for (idx, draft) in self.new_fields.clone().iter().enumerate() {
            ui.horizontal(|ui| {
                ui.monospace(format!("{}. {}", idx + 1, draft.key));
                if ui.button("remove").clicked() {
                    remove = Some(idx);
                }
            });
        }
        if let Some(idx) = remove {
            self.new_fields.remove(idx);
        }
        ui.horizontal(|ui| {
            if ui.button("+ add field").clicked() {
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
                        ui.label("key:");
                        ui.text_edit_singleline(&mut key);
                        ui.label("label:");
                        ui.text_edit_singleline(&mut label);
                        ui.label("type:");
                        ui.horizontal(|ui| {
                            for field_type in FieldType::ALL {
                                if ui
                                    .selectable_label(
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
                            ui.text_edit_singleline(&mut options)
                                .on_hover_text("comma-separated options");
                        }
                        ui.end_row();
                    });
            });
            self.new_fields[idx].key = key;
            self.new_fields[idx].label = label;
            self.new_fields[idx].options = options;
        }
        if ui.button("create type").clicked() {
            self.create_type(db);
        }
    }
}

impl Default for CustomObjectsPage {
    fn default() -> Self {
        Self::new()
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
}
