use crate::i18n::I18nError;

pub const ERROR_AUTH_ACCOUNT_NOT_FOUND: I18nError = I18nError {
    code: 1001,
    key: "auth.account_or_password_not_correct",
};
pub const ERROR_CLIENT_ID_OR_CLINET_SECRET_INVALID: I18nError = I18nError {
    code: -1002,
    key: "auth.client_id_or_client_secret_invalid",
};
pub const ERROR_GRANT_TYPE_MUST_BE_REFERSH_TOKEN: I18nError = I18nError {
    code: -1003,
    key: "auth.grant_type_must_be_refresh_token",
};
pub const ERROR_ACCOUNT_NOT_FOUND: I18nError = I18nError {
    code: -1004,
    key: "auth.account_not_found",
};
pub const ERROR_OLD_PASSWORD_NOT_CORRECT: I18nError = I18nError {
    code: -1005,
    key: "auth.old_password_not_correct",
};
