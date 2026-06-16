#[cfg(unix)]
use std::sync::atomic::Ordering;
use std::{error::Error, sync::Arc, time::Duration};

use api::config::{
    asr::AsrConfig, audio::AudioConfig, database::DatabaseConfig, llm::LlmConfig,
    matrix::MatrixConfig, mcp::McpConfig, server::ServerConfig, session::SessionConfig,
    tts::TtsConfig, vad::VadConfig, ws::WsConfig,
};
use framework::config::auth::AuthConfig;
use tracing::info;

use crate::{
    clap::{Commands, ServeArgs},
    server::Server,
};
mod clap;
mod download;
mod server;

pub fn run() -> Result<(), Box<dyn Error>> {
    let cli = clap::parse();
    match &cli.command {
        Some(Commands::Download {
            category,
            model,
            variant,
            data_dir,
            quiet,
            mirror,
            overrides,
            write_checksums,
            config,
            wizard,
        }) => {
            if *wizard {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(download::run_wizard(data_dir, *quiet))
            } else {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(download::run(
                    category.as_deref(),
                    model.as_deref(),
                    variant.as_deref(),
                    data_dir,
                    *quiet,
                    mirror,
                    overrides.as_deref(),
                    *write_checksums,
                    config.as_ref(),
                ))
            }
        }
        Some(Commands::List { category, json }) => {
            download::list(category.as_deref(), *json);
            Ok(())
        }
        None => run_with_args(&cli.serve),
    }
}

pub fn run_with_args(args: &ServeArgs) -> Result<(), Box<dyn Error>> {
    framework::panic::init();
    framework::deadlock::spawn();

    let runtime = framework::runtime::build(&args.runtime_config())?;
    let server = Server::new(args, Some(runtime.handle()))?;

    runtime.spawn(framework::signal::handle_signals(server.clone()));
    runtime.block_on(async_main(&server))?;
    framework::runtime::shutdown(runtime, Duration::from_millis(10000));

    #[cfg(unix)]
    if server.server.restarting.load(Ordering::Acquire) {
        framework::utils::restart::restart_process();
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
    let server_config = Arc::new(ServerConfig {
        server_name: Some(config.server_name.to_owned()),
        address: Some(config.address.to_owned()),
        port: Some(config.port.to_owned()),
    });
    let database_config = Arc::new(DatabaseConfig {
        url: config.database_url.to_owned(),
    });
    let session_config = Arc::new(SessionConfig {
        close_connection_no_voice_time: config.session_close_connection_no_voice_time.to_owned(),
        silence_voice_timeout: config.session_silence_voice_timeout.to_owned(),
        system_prompt: config.session_system_prompt.to_owned(),
        max_prompt_len: config.session_max_prompt_len.to_owned(),
    });
    let mcp_config = Arc::new(McpConfig {
        uri_list: config.mcp_uri_list.to_owned(),
    });
    let vad_config = Arc::new(VadConfig {
        model: config.vad_model.to_owned(),
        variant: config.vad_variant.to_owned(),
        path: config.vad_path.to_owned().or_else(|| config.derive_vad_path()),
        num_threads: config.vad_num_threads,
    });
    let audio_config = Arc::new(AudioConfig {
        input_sample_rate: config.audio_input_sample_rate,
        input_frame_duration: config.audio_input_frame_duration,
        input_channel: config.audio_input_channel,
        output_sample_rate: config.audio_output_sample_rate,
        output_channel: config.audio_output_channel,
        output_frame_duration: config.audio_output_frame_duration,
    });
    let auth_config = Arc::new(AuthConfig {
        access_token_secret: config.auth_access_token_secret.to_owned(),
        access_token_expires_in: config.auth_access_token_expires_in,
        refresh_token_secret: config.auth_refresh_token_secret.to_owned(),
        refresh_token_expires_in: config.auth_refresh_token_expires_in,
        audience: config.auth_audience.to_owned(),
        issuer: config.auth_issuer.to_owned(),
        client_id: config.auth_client_id.to_owned(),
        client_secret: config.auth_client_secret.to_owned(),
    });
    let ws_config = Arc::new(WsConfig {
        schema: config.ws_schema.to_owned(),
    });
    let tts_config = Arc::new(TtsConfig {
        model: config.tts_model.to_owned(),
        variant: config.tts_variant.to_owned(),
        path: config.tts_path.to_owned().or_else(|| config.derive_tts_path()),
        reference_prompt_text: config.tts_reference_prompt_text.to_owned(),
        reference_prompt_wav_path: config.tts_reference_prompt_wav_path.to_owned(),
        options: config.tts_options.clone(),
    });
    let asr_config = Arc::new(AsrConfig {
        model: config.asr_model.to_owned(),
        variant: config.asr_variant.to_owned(),
        path: config.asr_path.to_owned().or_else(|| config.derive_asr_path()),
    });
    let llm_config = Arc::new(LlmConfig {
        model: config.llm_model.to_owned(),
        variant: config.llm_variant.to_owned(),
        path: config.llm_path.to_owned().or_else(|| config.derive_llm_path()),
    });
    let matrix_config = Arc::new(MatrixConfig {
        enable: config.matrix_enable,
        client_name: config.matrix_client_name.to_owned(),
        homeserver: config.matrix_homeserver.to_owned(),
        client_username: config.matrix_client_username.to_owned(),
        client_password: config.matrix_client_password.to_owned(),
    });
    api::start(
        server_config,
        database_config,
        session_config,
        mcp_config,
        vad_config,
        audio_config,
        auth_config,
        ws_config,
        tts_config,
        asr_config,
        llm_config,
        matrix_config,
    )
    .await?;
    info!("Exit runtime");
    Ok(())
}
