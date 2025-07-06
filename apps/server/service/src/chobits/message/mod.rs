use serde::{Deserialize, Serialize};

pub mod hello;
pub mod listen;
pub mod tts;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    #[serde(rename = "type")]
    pub mtype: String,
}

#[derive(Debug, Clone)]
pub enum Transport {
    Websocket,
}

impl<'de> Deserialize<'de> for Transport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == r#"websocket"# {
            Ok(Self::Websocket)
        } else {
            Err(serde::de::Error::custom("Expected websocket for transport"))
        }
    }
}

impl Serialize for Transport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::Websocket => r#"websocket"#,
        })
    }
}

#[derive(Debug, Clone)]
pub enum AudioFormat {
    Opus,
}

impl<'de> Deserialize<'de> for AudioFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == r#"opus"# {
            Ok(Self::Opus)
        } else {
            Err(serde::de::Error::custom("Expected opus for audio format"))
        }
    }
}

impl Serialize for AudioFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::Opus => r#"opus"#,
        })
    }
}
