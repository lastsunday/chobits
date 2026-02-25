use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct MatrixConfig {
    pub enable: Option<bool>,
    pub client_name: Option<String>,
    pub homeserver: Option<String>,
    pub client_username: Option<String>,
    pub client_password: Option<String>,
}
