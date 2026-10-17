use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

pub fn api_base() -> String {
    #[cfg(target_arch = "wasm32")]
    if let Some(w) = web_sys::window() {
        if let Ok(Some(v)) = w.local_storage() {
            if let Ok(Some(base)) = v.get_item("spp.api_base") {
                return base.trim_end_matches('/').to_string();
            }
        }
    }
    "http://127.0.0.1:3000".to_string()
}

#[must_use]
pub fn url_for(path: &str) -> String {
    format!("{}{}", api_base(), path)
}

pub async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let window = web_sys::window().ok_or("no window")?;
    let url = url_for(path);
    let response = JsFuture::from(window.fetch_with_str(&url))
        .await
        .map_err(|e| format!("GET {path} failed: {e:?}"))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|_| format!("GET {path}: fetch did not return a Response"))?;
    if !response.ok() {
        return Err(format!("GET {path} -> HTTP {}", response.status()));
    }
    let text_promise = response
        .text()
        .map_err(|e| format!("GET {path}: body read failed: {e:?}"))?;
    let text = JsFuture::from(text_promise)
        .await
        .map_err(|e| format!("GET {path}: body read failed: {e:?}"))?
        .as_string()
        .ok_or_else(|| format!("GET {path}: body is not UTF-8 text"))?;
    serde_json::from_str(&text).map_err(|e| format!("GET {path}: invalid JSON: {e}"))
}

pub async fn post_json<T: DeserializeOwned>(
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<T, String> {
    request_json::<T>("POST", path, body).await
}

pub async fn patch_json<T: DeserializeOwned>(
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    request_json::<T>("PATCH", path, Some(body)).await
}

pub async fn put_json<T: DeserializeOwned>(
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    request_json::<T>("PUT", path, Some(body)).await
}

pub async fn delete_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    request_json::<T>("DELETE", path, None).await
}

pub async fn delete_json_with_body<T: DeserializeOwned>(
    path: &str,
    body: &serde_json::Value,
) -> Result<T, String> {
    request_json::<T>("DELETE", path, Some(body)).await
}

async fn request_json<T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<T, String> {
    use wasm_bindgen::JsValue;

    let window = web_sys::window().ok_or("no window")?;
    let url = url_for(path);
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    if let Some(body) = body {
        init.set_headers(&{
            let headers = web_sys::Headers::new()
                .map_err(|e| format!("{method} {path}: headers failed: {e:?}"))?;
            headers
                .set("Content-Type", "application/json")
                .map_err(|e| format!("{method} {path}: headers failed: {e:?}"))?;
            headers.into()
        });
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    let response = JsFuture::from(window.fetch_with_str_and_init(&url, &init))
        .await
        .map_err(|e| format!("{method} {path} failed: {e:?}"))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|_| format!("{method} {path}: fetch did not return a Response"))?;
    if !response.ok() {
        let status = response.status();
        if let Ok(text_promise) = response.text() {
            if let Ok(text) = JsFuture::from(text_promise).await {
                if let Some(text) = text.as_string() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
                            return Err(m.to_string());
                        }
                    }
                }
            }
        }
        return Err(format!("{method} {path} -> HTTP {status}"));
    }
    let text_promise = response
        .text()
        .map_err(|e| format!("{method} {path}: body read failed: {e:?}"))?;
    let text = JsFuture::from(text_promise)
        .await
        .map_err(|e| format!("{method} {path}: body read failed: {e:?}"))?
        .as_string()
        .ok_or_else(|| format!("{method} {path}: body is not UTF-8 text"))?;
    serde_json::from_str(&text).map_err(|e| format!("{method} {path}: invalid JSON: {e}"))
}
