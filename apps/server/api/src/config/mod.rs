pub mod asr;
pub mod audio;
pub mod llm;
pub mod logic;
pub mod mcp;
pub mod tts;
pub mod vad;

use anyhow::Context;
use config::{Config, FileFormat};
use framework::config::{ServerConfig, auth::AuthConfig, database::DatabaseConfig};
use serde::Deserialize;
use std::sync::LazyLock;

use crate::config::{
    asr::AsrConfig, audio::AudioConfig, llm::LlmConfig, logic::LogicConfig, mcp::McpConfig,
    tts::TtsConfig, vad::VadConfig,
};

static CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| AppConfig::load().expect("Failed to initialize config"));

#[derive(Debug, Default, Deserialize)]
pub struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    auth: AuthConfig,
    vad: VadConfig,
    tts: TtsConfig,
    asr: AsrConfig,
    llm: LlmConfig,
    audio: AudioConfig,
    logic: LogicConfig,
    mcp: McpConfig,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        match Config::builder()
            .add_source(
                config::File::with_name("application")
                    .format(FileFormat::Yaml)
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
                let config = Self::new();
                tracing::info!("{:#?}", config);
                Ok(config)
            }
        }
    }

    pub fn new() -> Self {
        Self {
            server: ServerConfig::new(),
            database: DatabaseConfig::new(),
            auth: AuthConfig::new(),
            vad: VadConfig::new(),
            tts: TtsConfig::new(),
            asr: AsrConfig::new(),
            llm: LlmConfig::new(),
            audio: AudioConfig::new(),
            logic: LogicConfig::new(),
            mcp: McpConfig::new(),
        }
    }

    pub fn server(&self) -> &ServerConfig {
        &self.server
    }

    pub fn database(&self) -> &DatabaseConfig {
        &self.database
    }

    pub fn auth(&self) -> &AuthConfig {
        &self.auth
    }

    pub fn vad(&self) -> &VadConfig {
        &self.vad
    }

    pub fn tts(&self) -> &TtsConfig {
        &self.tts
    }

    pub fn asr(&self) -> &AsrConfig {
        &self.asr
    }

    pub fn llm(&self) -> &LlmConfig {
        &self.llm
    }

    pub fn audio(&self) -> &AudioConfig {
        &self.audio
    }

    pub fn logic(&self) -> &LogicConfig {
        &self.logic
    }

    pub fn mcp(&self) -> &McpConfig {
        &self.mcp
    }
}

pub fn get() -> &'static AppConfig {
    &CONFIG
}
