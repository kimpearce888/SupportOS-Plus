use serde_json::Value;

pub(crate) fn json_str(v: &Value, key: &str) -> Option<String> {
    v[key].as_str().map(str::to_string)
}

pub(crate) fn json_string(v: &Value, key: &str) -> String {
    json_str(v, key).unwrap_or_default()
}
