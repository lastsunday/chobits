use axum::http::{HeaderMap, HeaderValue, header::ACCEPT_LANGUAGE};

pub fn get_locale(headers: &HeaderMap) -> String {
    headers
        .get(ACCEPT_LANGUAGE)
        .unwrap_or(&HeaderValue::from_static(""))
        .to_str()
        .unwrap_or_default()
        .to_string()
}
