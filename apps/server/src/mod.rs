#[cfg(unix)]
use std::sync::atomic::Ordering;
use std::{error::Error, sync::Arc};

use api::config::{
    asr::AsrConfig, audio::AudioConfig, database::DatabaseConfig, llm::LlmConfig,
    matrix::MatrixConfig, mcp::McpConfig, server::ServerConfig, session::SessionConfig,
    tts::TtsConfig, vad::VadConfig, ws::WsConfig,
};
use framework::config::auth::AuthConfig;
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
        path: config.vad_path.to_owned(),
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
        path: config.tts_path.to_owned(),
        reference_prompt_text: config.tts_reference_prompt_text.to_owned(),
        reference_prompt_wav_path: config.tts_reference_prompt_wav_path.to_owned(),
    });
    let asr_config = Arc::new(AsrConfig {
        model: config.asr_model.to_owned(),
        path: config.asr_path.to_owned(),
    });
    let llm_config = Arc::new(LlmConfig {
        model: config.llm_model.to_owned(),
        path: config.llm_path.to_owned(),
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
