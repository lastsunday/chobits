pub mod asr;
pub mod audio;
pub mod check;
pub mod database;
pub mod llm;
pub mod manager;
pub mod matrix;
pub mod mcp;
pub mod server;
pub mod session;
pub mod tts;
pub mod vad;
pub mod ws;

use anyhow::Error;
use chobits_macros::config_example_generator;
use either::Either::{self, Left, Right};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::{Deserialize, de::IgnoredAny};
use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    result::Result,
};

pub use self::{check::check, manager::Manager};

const DEPRECATED_KEYS: &[&str] = &[];

/// All the config options for chobits.
#[allow(clippy::struct_excessive_bools)]
#[allow(rustdoc::broken_intra_doc_links, rustdoc::bare_urls)]
#[derive(Clone, Debug, Deserialize)]
#[config_example_generator(
    filename = "application-example.toml",
    section = "global",
    undocumented = "# This item is undocumented. Please contribute documentation for it.",
    header = r#"### chobits Configuration
###
### THIS FILE IS GENERATED. CHANGES/CONTRIBUTIONS IN THE REPO WILL BE
### OVERWRITTEN!
###
### You should rename this file before configuring your server. Changes to
### documentation and defaults can be contributed in source code at
### src/config/mod.rs. This file is generated when building.
###
### Any values pre-populated are the default values for said config option.
###
### At the minimum, you MUST edit all the config options to your environment
### that say "YOU NEED TO EDIT THIS".
###
"#,
    ignore = "config_paths catchall"
)]
pub struct Config {
    /// default: localhost.localdomain
    #[serde(default = "default_server_name")]
    pub server_name: String,

    /// The default address (IPv4 or IPv6) continuwuity will listen on.
    ///
    /// If you are using Docker or a container NAT networking setup, this must
    /// be "0.0.0.0".
    ///
    ///
    /// default: 127.0.0.1
    #[serde(default = "default_address")]
    pub address: ListeningAddr,

    /// The port(s) continuwuity will listen on.
    ///
    /// For reverse proxying, see:
    /// https://continuwuity.org/deploying/generic.html#setting-up-the-reverse-proxy
    ///
    /// If you are using Docker, don't change this, you'll need to map an
    /// external port to this.
    ///
    /// default: 3000
    #[serde(default = "default_port")]
    pub port: ListeningPort,

    /// default: sqlite://db.sqlite?mode=rwc
    #[serde(default = "default_database_url")]
    pub database_url: Option<String>,

    /// default: QLjJTeVblAlM47de
    #[serde(default = "default_auth_access_token_secret")]
    pub auth_access_token_secret: Option<String>,

    /// default: 28800
    #[serde(default = "default_auth_access_token_expires_in")]
    pub auth_access_token_expires_in: Option<u64>,

    /// default: N8lI0uitNzJl6vYK
    #[serde(default = "default_auth_refresh_token_secret")]
    pub auth_refresh_token_secret: Option<String>,

    /// default: 15897600
    #[serde(default = "default_auth_refresh_token_expires_in")]
    pub auth_refresh_token_expires_in: Option<u64>,

    /// default: audience
    #[serde(default = "default_auth_audience")]
    pub auth_audience: Option<String>,

    /// default: issuer
    #[serde(default = "default_auth_issuer")]
    pub auth_issuer: Option<String>,

    /// default: d1aicsr57dijo7h963ig
    #[serde(default = "default_auth_client_id")]
    pub auth_client_id: Option<String>,

    /// default: ujTgh2lEQYy0PXhK
    #[serde(default = "default_auth_client_secret")]
    pub auth_client_secret: Option<String>,

    /// default: ws
    #[serde(default = "default_ws_schema")]
    pub ws_schema: Option<String>,

    /// default: data/vad/model/onnx-community/silero-vad/
    #[serde(default = "default_vad_path")]
    pub vad_path: Option<String>,

    /// default: 4
    #[serde(default = "default_vad_num_threads")]
    pub vad_num_threads: Option<i32>,

    /// default: voxcpm
    #[serde(default = "default_tts_model")]
    pub tts_model: Option<TtsModel>,

    /// default: data/tts/model/openbmb/VoxCPM-0.5B/
    #[serde(default = "default_tts_path")]
    pub tts_path: Option<String>,

    /// default: 一定被灰太狼给吃了，我已经为他准备好了花圈了
    #[serde(default = "default_tts_reference_prompt_text")]
    pub tts_reference_prompt_text: Option<String>,

    /// default: file://data/tts/reference/voice_05.wav
    #[serde(default = "default_tts_reference_prompt_wav_path")]
    pub tts_reference_prompt_wav_path: Option<String>,

    /// default: data/asr/model/openai/whisper-small/
    #[serde(default = "default_asr_path")]
    pub asr_path: Option<String>,

    /// default: qwen3
    #[serde(default = "default_llm_model")]
    pub llm_model: Option<LlmModel>,

    /// default: data/llm/model/unsloth/Qwen3-1.7B-GGUF/
    #[serde(default = "default_llm_path")]
    pub llm_path: Option<String>,

    /// default: 16000
    #[serde(default = "default_audio_input_sample_rate")]
    pub audio_input_sample_rate: Option<u32>,

    /// default: 60
    #[serde(default = "default_audio_input_frame_duration")]
    pub audio_input_frame_duration: Option<u64>,

    /// default: 1
    #[serde(default = "default_audio_input_channel")]
    pub audio_input_channel: Option<u32>,

    /// default: 16000
    #[serde(default = "default_audio_output_sample_rate")]
    pub audio_output_sample_rate: Option<u32>,

    /// default: 1
    #[serde(default = "default_audio_output_channel")]
    pub audio_output_channel: Option<u32>,

    /// default: 60
    #[serde(default = "default_audio_output_frame_duration")]
    pub audio_output_frame_duration: Option<u64>,

    /// default: 30000
    #[serde(default = "default_session_close_connection_no_voice_time")]
    pub session_close_connection_no_voice_time: Option<i64>,

    /// default: 1200
    #[serde(default = "default_session_silence_voice_timeout")]
    pub session_silence_voice_timeout: Option<i64>,

    /// default: 你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。
    #[serde(default = "default_session_system_prompt")]
    pub session_system_prompt: Option<String>,

    /// default: 3000
    #[serde(default = "default_session_max_prompt_len")]
    pub session_max_prompt_len: Option<u64>,

    /// default: ["http://127.0.0.1:3000/mcp"]
    #[serde(default = "default_mcp_uri_list")]
    pub mcp_uri_list: Option<Vec<String>>,

    /// default: false
    #[serde(default = "default_matrix_enable")]
    pub matrix_enable: Option<bool>,

    /// default: chobits
    #[serde(default = "default_matrix_client_name")]
    pub matrix_client_name: Option<String>,

    /// default: http://127.0.0.1:8008
    #[serde(default = "default_matrix_homeserver")]
    pub matrix_homeserver: Option<String>,

    /// default: @chobits:localhost.localdomain
    #[serde(default = "default_matrix_client_username")]
    pub matrix_client_username: Option<String>,

    /// default:
    #[serde(default = "default_matrix_client_password")]
    pub matrix_client_password: Option<String>,

    #[serde(flatten)]
    #[allow(clippy::zero_sized_map_values)]
    // this is a catchall, the map shouldn't be zero at runtime
    catchall: BTreeMap<String, IgnoredAny>,
}

fn default_server_name() -> String {
    String::from("localhost")
}

fn default_address() -> ListeningAddr {
    ListeningAddr {
        addrs: Right(vec![Ipv4Addr::LOCALHOST.into(), Ipv6Addr::LOCALHOST.into()]),
    }
}

fn default_port() -> ListeningPort {
    ListeningPort { ports: Left(3000) }
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

fn default_session_close_connection_no_voice_time() -> Option<i64> {
    Some(30000)
}

fn default_session_silence_voice_timeout() -> Option<i64> {
    Some(1200)
}

fn default_session_system_prompt() -> Option<String> {
    Some(String::from(
        "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。",
    ))
}

fn default_session_max_prompt_len() -> Option<u64> {
    Some(3000)
}

fn default_mcp_uri_list() -> Option<Vec<String>> {
    Some(vec![String::from("http://127.0.0.1:3000/mcp")])
}

fn default_ws_schema() -> Option<String> {
    Some(String::from("ws"))
}

fn default_vad_path() -> Option<String> {
    Some(String::from("data/vad/model/onnx-community/silero-vad/"))
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

impl Config {
    /// Pre-initialize config
    pub fn load(paths: &[PathBuf]) -> std::result::Result<Figment, Error> {
        let envs = [Env::var("CHOBITS_CONFIG")];
        let mut config = envs
            .into_iter()
            .flatten()
            .map(Toml::file)
            .chain(paths.iter().cloned().map(Toml::file))
            .fold(Figment::new(), |config, file| config.merge(file.nested()))
            .merge(Env::prefixed("CHOBITS_").global().split("__"));

        config = config.join(("config_paths", paths));

        Ok(config)
    }

    /// Finalize config
    pub fn new(raw_config: &Figment) -> Result<Self, Error> {
        let config = raw_config.extract::<Self>().map_err(|e| {
            anyhow::anyhow!("There was a problem with your configuration file: {e}")
        })?;

        // don't start if we're listening on both UNIX sockets and TCP at same time
        check::is_dual_listening(raw_config)?;

        Ok(config)
    }

    #[must_use]
    pub fn get_bind_addrs(&self) -> Vec<SocketAddr> {
        let mut addrs = Vec::with_capacity(
            self.get_bind_hosts()
                .len()
                .saturating_mul(self.get_bind_ports().len()),
        );
        for host in &self.get_bind_hosts() {
            for port in &self.get_bind_ports() {
                addrs.push(SocketAddr::new(*host, *port));
            }
        }

        addrs
    }

    fn get_bind_hosts(&self) -> Vec<IpAddr> {
        match &self.address.addrs {
            Left(addr) => vec![*addr],
            Right(addrs) => addrs.clone(),
        }
    }

    fn get_bind_ports(&self) -> Vec<u16> {
        match &self.port.ports {
            Left(port) => vec![*port],
            Right(ports) => ports.clone(),
        }
    }

    pub fn check(&self) -> Result<(), Error> {
        check(self)
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct ListeningPort {
    #[serde(with = "either::serde_untagged")]
    pub ports: Either<u16, Vec<u16>>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct ListeningAddr {
    #[serde(with = "either::serde_untagged")]
    pub addrs: Either<IpAddr, Vec<IpAddr>>,
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
