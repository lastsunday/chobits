use crate::i18n::I18nError;

pub const ERROR_OTA_LACK_DEVICE_ID: I18nError = I18nError {
    code: 2001,
    key: "ota.lack_device_id",
};

pub const ERROR_OTA_LACK_CLIENT_ID: I18nError = I18nError {
    code: 2002,
    key: "ota.lack_client_id",
};

pub const ERROR_OTA_LACK_USER_AGENT: I18nError = I18nError {
    code: 2003,
    key: "ota.lack_user_agent",
};
