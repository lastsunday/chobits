use std::sync::Arc;

use api::config::Config;
use async_trait::async_trait;
use framework::logging::LogConfig;
use framework::signal::SignalHandler;
use tokio::runtime;
use tracing::info;

use crate::clap::{Args, update};

pub(crate) struct Server {
    pub(crate) server: Arc<api::server::Server>,
    pub(crate) _log_reload: framework::log::LogLevelReloadHandles,
    pub(crate) _flame_guard: (),
    pub(crate) _cap_state: Arc<framework::log::capture::State>,
}

impl Server {
    pub(crate) fn new(
        args: &Args,
        runtime: Option<&runtime::Handle>,
    ) -> Result<Arc<Self>, anyhow::Error> {
        let _runtime_guard = runtime.map(runtime::Handle::enter);

        let config_paths = args.config.clone().unwrap_or_default();

        let config = Config::load(&config_paths)
            .and_then(|raw| update(raw, args))
            .and_then(|raw| Config::new(&raw))?;

        let log_config = LogConfig::default();

        let (reload_handles, flame_guard, cap_state) =
            framework::logging::init(&log_config)?;
        let _flame_guard = flame_guard;

        config.check()?;

        info!(
            server_name = %config.server_name,
            "{}",
            framework::version(),
        );

        Ok(Arc::new(Self {
            server: Arc::new(api::server::Server::new(config, runtime.cloned())),
            _log_reload: reload_handles,
            _flame_guard,
            _cap_state: cap_state,
        }))
    }
}

#[async_trait]
impl SignalHandler for Server {
    async fn reload(&self) -> Result<(), anyhow::Error> {
        self.server.reload()
    }

    async fn shutdown(&self) -> Result<(), anyhow::Error> {
        self.server.shutdown()
    }

    async fn signal(&self, sig: &'static str) -> Result<(), anyhow::Error> {
        self.server.signal(sig)
    }
}
