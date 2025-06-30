use axum::http::HeaderMap;
use framework::{error::ApiError, i18n::get_locale};
use serde::Serialize;

pub fn t(key: &str, headers: &HeaderMap) -> String {
    String::from(t!(key, locale = get_locale(headers).as_str()))
}

#[derive(Debug, Serialize)]
pub struct I18nError<'a> {
    pub code: i32,
    pub key: &'a str,
}

impl<'a> I18nError<'a> {
    pub fn new(code: i32, key: &'a str) -> Self {
        Self { code, key }
    }

    pub fn gen_api_error(&self, headers: &HeaderMap) -> ApiError {
        ApiError::Biz(self.code, t(self.key, headers))
    }
}
