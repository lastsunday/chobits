use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

pub const KEY_DEVICE_ID: &str = "Device-Id";
pub const KEY_CLIENT_ID: &str = "Client-Id";
pub const KEY_USER_AGENT: &str = "User-Agent";

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({}))]
pub struct OtaParam {
    pub version: Option<u32>,
    pub language: Option<String>,
    /// 设备的闪存大小
    pub flash_size: Option<u64>,
    pub minimum_free_heap_size: Option<u64>,
    /// MAC地址（与 HTTP header 里的 device-id 一致）
    pub mac_address: Option<String>,
    /// 设备的芯片型号，例如 esp32s3
    pub chip_model_name: Option<String>,
    /// 设备的PSRAM大小
    pub psram_size: Option<u64>,
    /// ClientId（与 HTTP header 里的 client-id 一致）
    pub uuid: Option<String>,
    pub application: Application,
    /// 设备分区表，用于检查是否有足够的空间，用于下载固件
    pub partition_table: Option<Vec<Partition>>,
    pub ota: Option<Ota>,
    pub board: Board,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(description = "包含设备当前固件版本信息的对象")]
pub struct Application {
    pub name: Option<String>,
    /// 当前固件版本号
    pub version: String,
    pub compile_time: Option<String>,
    pub idf_version: Option<String>,
    /// 用于校验固件文件完整性Hash
    pub elf_sha256: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct Partition {
    pub label: String,
    #[serde(rename = "type")]
    pub mtype: u32,
    pub subtype: u32,
    pub address: u64,
    pub size: u64,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct Ota {
    pub label: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(description = "开发板类型与版本，以及所运行的环境")]
pub struct Board {
    #[serde(rename = "type")]
    /// 开发板类型
    pub mtype: String,
    /// 开发板SKU（与user-agent中的前面部分保持一致）
    pub name: Option<String>,
    /// 设备接入的 Wi-Fi 名字
    pub ssid: Option<String>,
    /// 设备接入的 Wi-Fi 信号强度
    pub rssi: Option<i32>,
    pub channel: Option<i32>,
    pub ip: Option<String>,
    pub mac: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, Default)]
pub struct OtaResult {
    pub activation: Option<Activation>,
    pub mqtt: Option<Mqtt>,
    pub websocket: Websocket,
    pub server_time: ServerTime,
    pub firmware: Option<Firmware>,
}

#[derive(Debug, Serialize, ToSchema, Default)]
#[schema(description = "设备需要激活")]
pub struct Activation {
    /// 激活码
    pub code: String,
    /// 屏幕显示消息
    pub message: String,
    pub challenge: String,
}

#[derive(Debug, Serialize, ToSchema, Default)]
#[schema(description = "MQTT协议服务器配置信息")]
pub struct Mqtt {
    pub endpoint: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
    pub publish_topic: String,
}

#[derive(Debug, Serialize, ToSchema, Default)]
#[schema(description = "Websocket协议服务器配置信息")]
pub struct Websocket {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema, Default)]
#[schema(description = "服务器时间信息（用于同步设备时间）")]
pub struct ServerTime {
    /// 当前时间戳
    pub timestamp: i64,
    /// 服务器时区
    pub timezone: String,
    /// 服务器时区偏移量
    pub timezone_offset: i32,
}

#[derive(Debug, Serialize, ToSchema, Default)]
#[schema(description = "最新版本固件信息")]
pub struct Firmware {
    /// 固件版本号
    pub version: String,
    /// 固件下载链接
    pub url: Option<String>,
}
