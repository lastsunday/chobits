#[cfg(feature = "perf")]
use std::path::PathBuf;

#[derive(Clone)]
pub struct LogConfig {
	pub log_thread_ids: bool,
	pub log_colors: bool,
	pub log: String,
	pub log_filter_regex: bool,
	pub log_span_events: String,
	pub log_to_journald: bool,
	pub journald_identifier: Option<String>,
	#[cfg(feature = "sentry")]
	pub sentry_filter: String,
	#[cfg(feature = "otlp")]
	pub otlp_filter: String,
	#[cfg(feature = "otlp")]
	pub allow_otlp: bool,
	#[cfg(feature = "otlp")]
	pub otlp_protocol: String,
	#[cfg(feature = "perf")]
	pub tracing_flame: bool,
	#[cfg(feature = "perf")]
	pub tracing_flame_filter: String,
	#[cfg(feature = "perf")]
	pub tracing_flame_output_path: PathBuf,
	#[cfg(feature = "console")]
	pub tokio_console: bool,
}

impl Default for LogConfig {
	fn default() -> Self {
		Self {
			log_thread_ids: false,
			log_colors: true,
			log: "info".into(),
			log_filter_regex: false,
			log_span_events: "CLOSE".into(),
			log_to_journald: false,
			journald_identifier: None,
			#[cfg(feature = "sentry")]
			sentry_filter: String::new(),
			#[cfg(feature = "otlp")]
			otlp_filter: String::new(),
			#[cfg(feature = "otlp")]
			allow_otlp: false,
			#[cfg(feature = "otlp")]
			otlp_protocol: "http".into(),
			#[cfg(feature = "perf")]
			tracing_flame: false,
			#[cfg(feature = "perf")]
			tracing_flame_filter: String::new(),
			#[cfg(feature = "perf")]
			tracing_flame_output_path: PathBuf::from("tracing.flame"),
			#[cfg(feature = "console")]
			tokio_console: false,
		}
	}
}
