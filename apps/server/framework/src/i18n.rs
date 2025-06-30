use axum::http::{HeaderMap, HeaderValue};

pub fn get_locale(headers: &HeaderMap) -> String {
    headers
        .get("locale")
        .unwrap_or(&HeaderValue::from_static(""))
        .to_str()
        .unwrap_or_default()
        .to_string()
}
