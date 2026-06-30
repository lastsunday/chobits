#[derive(Clone)]
pub struct SentryConfig {
    pub sentry: bool,
    pub sentry_endpoint: Option<String>,
    pub sentry_send_server_name: bool,
    pub sentry_send_panic: bool,
    pub sentry_send_error: bool,
    pub sentry_traces_sample_rate: f32,
    pub sentry_attach_stacktrace: bool,
    pub server_name: String,
}

impl Default for SentryConfig {
    fn default() -> Self {
        Self {
            sentry: false,
            sentry_endpoint: None,
            sentry_send_server_name: false,
            sentry_send_panic: true,
            sentry_send_error: true,
            sentry_traces_sample_rate: 0.0,
            sentry_attach_stacktrace: false,
            server_name: String::new(),
        }
    }
}
