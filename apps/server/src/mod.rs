#[cfg(unix)]
use std::sync::atomic::Ordering;
use std::{error::Error, sync::Arc};

use tracing::info;

use crate::{clap::Args, server::Server};
mod clap;
mod restart;
mod runtime;
mod server;
mod signal;

pub fn run() -> Result<(), Box<dyn Error>> {
    let args = clap::parse();
    run_with_args(&args)
}

pub fn run_with_args(args: &Args) -> Result<(), Box<dyn Error>> {
    let runtime = runtime::new(args)?;
    let server = Server::new(args, Some(runtime.handle()))?;

    runtime.spawn(signal::signal(server.clone()));
    runtime.block_on(async_main(&server))?;
    runtime::shutdown(&server, runtime);

    #[cfg(unix)]
    if server.server.restarting.load(Ordering::Acquire) {
        restart::restart();
    }

    info!("Exit");
    Ok(())
}

#[tracing::instrument(
	name = "main",
	parent = None,
	skip_all,
	level = "info"
)]
async fn async_main(server: &Arc<Server>) -> Result<(), anyhow::Error> {
    let config = server.server.config.clone();
    api::start(config).await?;
    info!("Exit runtime");
    Ok(())
}
