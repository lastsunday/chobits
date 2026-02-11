pub mod asr;
pub mod audio;
pub mod llm;
pub mod logic;
pub mod matrix;
pub mod mcp;
pub mod tts;
pub mod vad;

use anyhow::Context;
use config::{Config, FileFormat};
use serde::Deserialize;
use std::sync::LazyLock;

static CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| AppConfig::load().expect("Failed to initialize config"));

#[derive(Debug, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_server_port")]
    pub server_port: Option<u16>,
    #[serde(default = "default_database_url")]
    pub database_url: Option<String>,
    #[serde(default = "default_auth_access_token_secret")]
    pub auth_access_token_secret: Option<String>,
    #[serde(default = "default_auth_access_token_expires_in")]
    pub auth_access_token_expires_in: Option<u64>,
    #[serde(default = "default_auth_refresh_token_secret")]
    pub auth_refresh_token_secret: Option<String>,
    #[serde(default = "default_auth_refresh_token_expires_in")]
    pub auth_refresh_token_expires_in: Option<u64>,
    #[serde(default = "default_auth_audience")]
    pub auth_audience: Option<String>,
    #[serde(default = "default_auth_issuer")]
    pub auth_issuer: Option<String>,
    #[serde(default = "default_auth_client_id")]
    pub auth_client_id: Option<String>,
    #[serde(default = "default_auth_client_secret")]
    pub auth_client_secret: Option<String>,
    #[serde(default = "default_ws_schema")]
    pub ws_schema: Option<String>,
    #[serde(default = "default_vad_path")]
    pub vad_path: Option<String>,
    #[serde(default = "default_vad_num_threads")]
    pub vad_num_threads: Option<i32>,
    #[serde(default = "default_tts_model")]
    pub tts_model: Option<TtsModel>,
    #[serde(default = "default_tts_path")]
    pub tts_path: Option<String>,
    //参照音频字幕
    #[serde(default = "default_tts_reference_prompt_text")]
    pub tts_reference_prompt_text: Option<String>,
    //参照音频路径
    #[serde(default = "default_tts_reference_prompt_wav_path")]
    pub tts_reference_prompt_wav_path: Option<String>,
    #[serde(default = "default_asr_path")]
    pub asr_path: Option<String>,
    #[serde(default = "default_llm_model")]
    pub llm_model: Option<LlmModel>,
    #[serde(default = "default_llm_path")]
    pub llm_path: Option<String>,
    #[serde(default = "default_audio_input_sample_rate")]
    pub audio_input_sample_rate: Option<u32>,
    #[serde(default = "default_audio_input_frame_duration")]
    pub audio_input_frame_duration: Option<u64>,
    #[serde(default = "default_audio_input_channel")]
    pub audio_input_channel: Option<u32>,
    #[serde(default = "default_audio_output_sample_rate")]
    pub audio_output_sample_rate: Option<u32>,
    #[serde(default = "default_audio_output_channel")]
    pub audio_output_channel: Option<u32>,
    #[serde(default = "default_audio_output_frame_duration")]
    pub audio_output_frame_duration: Option<u64>,
    #[serde(default = "default_logic_close_connection_no_voice_time")]
    pub logic_close_connection_no_voice_time: Option<i64>,
    #[serde(default = "default_logic_silence_voice_timeout")]
    pub logic_silence_voice_timeout: Option<i64>,
    #[serde(default = "default_logic_system_prompt")]
    pub logic_system_prompt: Option<String>,
    #[serde(default = "default_logic_max_prompt_len")]
    pub logic_max_prompt_len: Option<u64>,
    #[serde(default = "default_mcp_uri_list")]
    pub mcp_uri_list: Option<Vec<String>>,
    #[serde(default = "default_matrix_enable")]
    pub matrix_enable: Option<bool>,
    #[serde(default = "default_matrix_client_name")]
    pub matrix_client_name: Option<String>,
    #[serde(default = "default_matrix_homeserver")]
    pub matrix_homeserver: Option<String>,
    #[serde(default = "default_matrix_client_username")]
    pub matrix_client_username: Option<String>,
    #[serde(default = "default_matrix_client_password")]
    pub matrix_client_password: Option<String>,
}

fn default_server_port() -> Option<u16> {
    Some(3000)
}

fn default_database_url() -> Option<String> {
    Some(String::from("sqlite://db.sqlite?mode=rwc"))
}

fn default_auth_access_token_secret() -> Option<String> {
    Some(String::from("QLjJTeVblAlM47de"))
}

fn default_auth_access_token_expires_in() -> Option<u64> {
    Some(28800)
}

fn default_auth_refresh_token_secret() -> Option<String> {
    Some(String::from("N8lI0uitNzJl6vYK"))
}

fn default_auth_refresh_token_expires_in() -> Option<u64> {
    Some(15897600)
}

fn default_auth_audience() -> Option<String> {
    Some(String::from("audience"))
}

fn default_auth_issuer() -> Option<String> {
    Some(String::from("issuer"))
}

fn default_auth_client_id() -> Option<String> {
    Some(String::from("d1aicsr57dijo7h963ig"))
}

fn default_auth_client_secret() -> Option<String> {
    Some(String::from("ujTgh2lEQYy0PXhK"))
}

fn default_vad_path() -> Option<String> {
    Some(String::from("data/vad/model/onnx-community/silero-vad/"))
}

fn default_tts_model() -> Option<TtsModel> {
    Some(TtsModel::Voxcpm)
}

fn default_tts_path() -> Option<String> {
    Some(String::from("data/tts/model/openbmb/VoxCPM-0.5B/"))
}

fn default_tts_reference_prompt_text() -> Option<String> {
    Some(String::from("一定被灰太狼给吃了，我已经为他准备好了花圈了"))
}

fn default_tts_reference_prompt_wav_path() -> Option<String> {
    Some(String::from("file://data/tts/reference/voice_05.wav"))
}

fn default_asr_path() -> Option<String> {
    Some(String::from("data/asr/model/openai/whisper-small/"))
}

fn default_llm_model() -> Option<LlmModel> {
    Some(LlmModel::Qwen3)
}

fn default_llm_path() -> Option<String> {
    Some(String::from("data/llm/model/unsloth/Qwen3-1.7B-GGUF/"))
}

fn default_audio_input_sample_rate() -> Option<u32> {
    Some(16000)
}

fn default_audio_input_frame_duration() -> Option<u64> {
    Some(60_u64)
}

fn default_audio_input_channel() -> Option<u32> {
    Some(1)
}

fn default_audio_output_sample_rate() -> Option<u32> {
    Some(16000)
}

fn default_audio_output_channel() -> Option<u32> {
    Some(1)
}

fn default_audio_output_frame_duration() -> Option<u64> {
    Some(60_u64)
}

fn default_logic_close_connection_no_voice_time() -> Option<i64> {
    Some(30000)
}

fn default_logic_silence_voice_timeout() -> Option<i64> {
    Some(1200)
}

fn default_logic_system_prompt() -> Option<String> {
    Some(String::from(
        "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。",
    ))
}

fn default_logic_max_prompt_len() -> Option<u64> {
    Some(3000)
}

fn default_mcp_uri_list() -> Option<Vec<String>> {
    Some(vec![String::from("http://127.0.0.1:3000/mcp")])
}

fn default_ws_schema() -> Option<String> {
    Some(String::from("ws"))
}

fn default_vad_num_threads() -> Option<i32> {
    Some(4)
}

fn default_matrix_enable() -> Option<bool> {
    Some(false)
}

fn default_matrix_client_name() -> Option<String> {
    Some(String::from("chobits"))
}

fn default_matrix_homeserver() -> Option<String> {
    Some(String::from("http://127.0.0.1:8008"))
}

fn default_matrix_client_username() -> Option<String> {
    Some(String::from("@chobits:localhost.localdomain"))
}

fn default_matrix_client_password() -> Option<String> {
    None
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        match Config::builder()
            .add_source(
                config::File::with_name("application")
                    .format(FileFormat::Toml)
                    .required(false),
            )
            .add_source(
                config::Environment::with_prefix("APP")
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(","),
            )
            .build()
            .with_context(|| anyhow::anyhow!("Failed to load config"))?
            .try_deserialize()
        {
            Ok(config) => {
                tracing::info!("Load config file successfully");
                tracing::info!("{:#?}", config);
                Ok(config)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load config file,using default config,error = {:?}",
                    e
                );
                let config = Self::default();
                tracing::info!("{:#?}", config);
                Ok(config)
            }
        }
    }
}

pub fn get() -> &'static AppConfig {
    &CONFIG
}

#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TtsModel {
    #[default]
    Kokoro,
    Voxcpm,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmModel {
    #[default]
    Qwen3,
    MiniCPM4,
}
