use std::sync::Arc;

use tracing_subscriber::{EnvFilter, Layer, Registry, fmt, layer::SubscriberExt, reload};

use crate::log::{ConsoleFormat, ConsoleWriter, LogLevelReloadHandles, capture, fmt_span};

#[cfg(feature = "perf")]
use tracing_flame::FlameLayer;

#[cfg(feature = "perf")]
pub(crate) type TracingFlameGuard =
	Option<tracing_flame::FlushGuard<std::io::BufWriter<std::fs::File>>>;
#[cfg(not(feature = "perf"))]
pub(crate) type TracingFlameGuard = ();

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
			sentry_filter: "".into(),
			#[cfg(feature = "otlp")]
			otlp_filter: "".into(),
			#[cfg(feature = "otlp")]
			allow_otlp: false,
			#[cfg(feature = "otlp")]
			otlp_protocol: "http".into(),
			#[cfg(feature = "perf")]
			tracing_flame: false,
			#[cfg(feature = "perf")]
			tracing_flame_filter: "".into(),
			#[cfg(feature = "perf")]
			tracing_flame_output_path: PathBuf::from("tracing.flame"),
			#[cfg(feature = "console")]
			tokio_console: false,
		}
	}
}

#[allow(unused_variables)]
pub fn init(
	config: &LogConfig,
) -> Result<(LogLevelReloadHandles, TracingFlameGuard, Arc<capture::State>), anyhow::Error> {
	let reload_handles = LogLevelReloadHandles::default();

	let console_span_events = fmt_span::from_str(&config.log_span_events).unwrap_or(
		tracing_subscriber::fmt::format::FmtSpan::CLOSE,
	);

	let console_filter = EnvFilter::builder()
		.with_regex(config.log_filter_regex)
		.parse(&config.log)
		.map_err(|e| anyhow::anyhow!("Config(log, \"{e}.\")"))?;

	let console_layer = fmt::Layer::new()
		.with_span_events(console_span_events)
		.event_format(ConsoleFormat::new(config.log_thread_ids, config.log_colors))
		.fmt_fields(ConsoleFormat::new(config.log_thread_ids, config.log_colors))
		.with_writer(ConsoleWriter::new());

	let (console_reload_filter, console_reload_handle) =
		reload::Layer::new(console_filter.clone());

	reload_handles.add("console", Box::new(console_reload_handle));

	let cap_state = Arc::new(capture::State::new());
	let cap_layer = capture::Layer::new(&cap_state);

	let subscriber = Registry::default()
		.with(console_layer.with_filter(console_reload_filter))
		.with(cap_layer);

	#[cfg(all(target_family = "unix", feature = "journald"))]
	if config.log_to_journald {
		println!("Initialising journald logging");
		if let Err(e) = init_journald_logging(config) {
			eprintln!("Failed to initialize journald logging: {e}");
		}
	}

	#[cfg(feature = "sentry")]
	let subscriber = {
		let sentry_filter = EnvFilter::try_new(&config.sentry_filter)
			.map_err(|e| anyhow::anyhow!("Config(sentry_filter, \"{e}.\")"))?;

		let sentry_layer = sentry_tracing::layer();
		let (sentry_reload_filter, sentry_reload_handle) = reload::Layer::new(sentry_filter);

		reload_handles.add("sentry", Box::new(sentry_reload_handle));
		subscriber.with(sentry_layer.with_filter(sentry_reload_filter))
	};

	#[cfg(feature = "otlp")]
	let subscriber = {
		let otlp_filter = EnvFilter::try_new(&config.otlp_filter)
			.map_err(|e| anyhow::anyhow!("Config(otlp_filter, \"{e}.\")"))?;

		let otlp_layer = config.allow_otlp.then(|| {
			opentelemetry::global::set_text_map_propagator(
				opentelemetry_sdk::propagation::TraceContextPropagator::new(),
			);

			let exporter = match config.otlp_protocol.as_str() {
				| "grpc" => opentelemetry_otlp::SpanExporter::builder()
					.with_tonic()
					.with_protocol(opentelemetry_otlp::Protocol::Grpc)
					.build()
					.expect("Failed to create OTLP gRPC exporter"),
				| "http" => opentelemetry_otlp::SpanExporter::builder()
					.with_http()
					.build()
					.expect("Failed to create OTLP HTTP exporter"),
				| protocol => {
					tracing::warn!(
						"Invalid OTLP protocol '{protocol}', falling back to HTTP. Valid options are \
						 'http' or 'grpc'."
					);
					opentelemetry_otlp::SpanExporter::builder()
						.with_http()
						.build()
						.expect("Failed to create OTLP HTTP exporter")
				},
			};

			let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
				.with_batch_exporter(exporter)
				.build();

			let tracer = provider.tracer(crate::info::name());

			let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

			let (otlp_reload_filter, otlp_reload_handle) =
				reload::Layer::new(otlp_filter.clone());
			reload_handles.add("otlp", Box::new(otlp_reload_handle));

			Some(telemetry.with_filter(otlp_reload_filter))
		});

		subscriber.with(otlp_layer)
	};

	#[cfg(feature = "perf")]
	let (subscriber, flame_guard) = {
		let (flame_layer, flame_guard) = if config.tracing_flame {
			let flame_filter = EnvFilter::try_new(&config.tracing_flame_filter)
				.map_err(|e| anyhow::anyhow!("Config(tracing_flame_filter, \"{e}.\")"))?;

			let (flame_layer, flame_guard) =
				FlameLayer::with_file(&config.tracing_flame_output_path)
					.map_err(|e| anyhow::anyhow!("Config(tracing_flame_output_path, \"{e}.\")"))?;

			let flame_layer = flame_layer
				.with_empty_samples(false)
				.with_filter(flame_filter);

			(Some(flame_layer), Some(flame_guard))
		} else {
			(None, None)
		};

		let subscriber = subscriber.with(flame_layer);
		(subscriber, flame_guard)
	};

	#[cfg(not(feature = "perf"))]
	#[cfg_attr(not(feature = "perf"), allow(clippy::let_unit_value))]
	let flame_guard = ();

	let ret = (reload_handles, flame_guard, cap_state);

	let (console_enabled, console_disabled_reason) = tokio_console_enabled(config);
	#[cfg(all(feature = "console", feature = "tokio_unstable"))]
	if console_enabled {
		let console_layer = console_subscriber::ConsoleLayer::builder()
			.with_default_env()
			.spawn();

		set_global_default(subscriber.with(console_layer));
		return Ok(ret);
	}

	set_global_default(subscriber);

	if !console_enabled && !console_disabled_reason.is_empty() {
		tracing::warn!("{}", console_disabled_reason);
	}

	Ok(ret)
}

#[cfg(all(target_family = "unix", feature = "journald"))]
fn init_journald_logging(config: &LogConfig) -> Result<(), anyhow::Error> {
	use tracing_journald::Layer as JournaldLayer;

	let journald_filter =
		EnvFilter::try_new(&config.log).map_err(|e| anyhow::anyhow!("Config(log, \"{e}.\")"))?;

	let mut journald_layer = JournaldLayer::new()
		.map_err(|e| anyhow::anyhow!("Config(journald, \"Failed to initialize journald layer: {e}.\")"))?;

	if let Some(ref identifier) = config.journald_identifier {
		journald_layer = journald_layer.with_syslog_identifier(identifier.to_owned());
	}

	let journald_subscriber =
		Registry::default().with(journald_layer.with_filter(journald_filter));

	let _guard = tracing::subscriber::set_default(journald_subscriber);

	Ok(())
}

fn tokio_console_enabled(config: &LogConfig) -> (bool, &'static str) {
	#[cfg(all(feature = "console", feature = "tokio_unstable"))]
	{
		if !config.tokio_console {
			return (false, "tokio console is available but disabled by the configuration.");
		}

		(true, "")
	}

	#[cfg(not(all(feature = "console", feature = "tokio_unstable")))]
	{
		let _ = config;
		(false, "")
	}
}

fn set_global_default<S>(subscriber: S)
where
	S: tracing::Subscriber + Send + Sync + 'static,
{
	tracing::subscriber::set_global_default(subscriber)
		.expect("the global default tracing subscriber failed to be initialized");
}
