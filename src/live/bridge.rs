//! Official-page bridge protocol adapted from `ysp-live`
//! (`crates/ysp-core/src/bridge.rs`). Upstream license: MIT.

use serde::Deserialize;
use std::fmt;
use url::Url;

use super::error::CoreError;

const MAX_PLAY_URL_LEN: usize = 8192;

#[derive(Clone, PartialEq, Eq)]
pub struct PlayUrl {
    raw: String,
    host: String,
    https: bool,
}

impl PlayUrl {
    pub fn parse(input: &str) -> Result<Self, CoreError> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(CoreError::EmptyPlayUrl);
        }
        if raw.len() > MAX_PLAY_URL_LEN {
            return Err(CoreError::PlayUrlTooLong);
        }

        let url = Url::parse(raw)?;
        let scheme = url.scheme().to_ascii_lowercase();
        if scheme != "https" && scheme != "http" {
            return Err(CoreError::UnsupportedScheme(scheme));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(CoreError::PlayUrlHasCredentials);
        }

        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or(CoreError::MissingHost)?
            .to_ascii_lowercase();
        if !url.path().to_ascii_lowercase().ends_with(".m3u8") {
            return Err(CoreError::NotHls);
        }

        Ok(Self {
            raw: raw.to_string(),
            host,
            https: scheme == "https",
        })
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn is_https(&self) -> bool {
        self.https
    }
}

impl fmt::Debug for PlayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlayUrl")
            .field("host", &self.host)
            .field("https", &self.https)
            .field("query", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StreamFormat {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_encrypt")]
    pub encrypt: Option<i64>,
    #[serde(rename = "fn", default)]
    pub fn_name: Option<String>,
    #[serde(default)]
    pub fnname: Option<String>,
    #[serde(default)]
    pub defnname: Option<String>,
    #[serde(default)]
    pub defnrate: Option<String>,
}

impl StreamFormat {
    pub fn label(&self) -> String {
        self.fnname
            .clone()
            .or_else(|| self.defnname.clone())
            .unwrap_or_else(|| {
                self.fn_name
                    .clone()
                    .unwrap_or_else(|| "未知清晰度".to_string())
            })
    }
}

fn deserialize_encrypt<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(raw.and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveInfo {
    pub play_url: PlayUrl,
    pub definition: Option<String>,
    pub formats: Vec<StreamFormat>,
    pub live_pid: Option<String>,
    pub encrypt: Option<i64>,
}

impl LiveInfo {
    pub fn encryption(&self) -> Option<i64> {
        self.encrypt
            .or_else(|| self.formats.iter().find_map(|format| format.encrypt))
    }

    /// A non-zero upstream encryption flag means the raw HLS video cannot be
    /// decoded by mpv. Those channels must stay on the official player.
    pub fn encrypted(&self) -> bool {
        self.encryption().is_some_and(|value| value != 0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackStatus {
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub current_time: Option<f64>,
    pub backup: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BridgeEvent {
    LiveInfo(LiveInfo),
    Playback(PlaybackStatus),
    Ready {
        url: Option<String>,
    },
    Error {
        code: Option<String>,
        message: String,
    },
    Ignored {
        kind: String,
    },
}

impl BridgeEvent {
    pub fn from_ipc(raw: &str) -> Result<Self, CoreError> {
        let value: serde_json::Value = serde_json::from_str(raw)?;
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or(CoreError::MissingField("type"))?;

        match kind {
            "live_info" => {
                let playurl = value
                    .get("playurl")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(CoreError::MissingField("playurl"))?;
                let play_url = PlayUrl::parse(playurl)?;
                let formats = value
                    .get("formats")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();

                let encrypt = value.get("encrypt").and_then(|raw| {
                    raw.as_i64()
                        .or_else(|| raw.as_str().and_then(|text| text.parse().ok()))
                });

                Ok(Self::LiveInfo(LiveInfo {
                    play_url,
                    definition: value
                        .get("defn")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    formats,
                    live_pid: value
                        .get("livepid")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    encrypt,
                }))
            }
            "bridge_ready" => Ok(Self::Ready {
                url: value
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            }),
            "error" => Ok(Self::Error {
                code: value
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                message: value
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("鉴权桥未知错误")
                    .to_string(),
            }),
            "playing" | "buffering" | "video_size" | "video_progress" | "playback_error" => {
                Ok(Self::Playback(PlaybackStatus {
                    kind: kind.to_string(),
                    width: value
                        .get("width")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u32,
                    height: value
                        .get("height")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u32,
                    current_time: value.get("currentTime").and_then(serde_json::Value::as_f64),
                    backup: value
                        .get("backup")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    message: value
                        .get("message")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                }))
            }
            other => Err(CoreError::UnknownBridgeEvent(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_realistic_bridge_message() {
        let raw = r#"{
            "type":"live_info",
            "playurl":"https://live.example.ysp.cctv.cn/opaque/2024078203_web.m3u8?from=player&svrtime=1&pid=600001859",
            "defn":"fhd",
            "livepid":"600001859",
            "formats":[{"id":2024078203,"fn":"fhd","fnname":"蓝光 1080P","defnrate":"1080P"}]
        }"#;
        let event = BridgeEvent::from_ipc(raw).unwrap();
        let BridgeEvent::LiveInfo(info) = event else {
            panic!("expected live_info");
        };
        assert_eq!(info.play_url.host(), "live.example.ysp.cctv.cn");
        assert_eq!(info.definition.as_deref(), Some("fhd"));
        assert_eq!(info.formats[0].label(), "蓝光 1080P");
    }

    #[test]
    fn rejects_non_hls_and_unsafe_urls() {
        assert!(matches!(
            PlayUrl::parse("javascript:alert(1)"),
            Err(CoreError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            PlayUrl::parse("file:///etc/passwd.m3u8"),
            Err(CoreError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            PlayUrl::parse("https://example.com/video.mp4"),
            Err(CoreError::NotHls)
        ));
    }

    #[test]
    fn debug_redacts_query_and_path_secrets() {
        let url =
            PlayUrl::parse("https://cdn.example.com/secret-token/channel.m3u8?ysign=super-secret")
                .unwrap();
        let debug = format!("{url:?}");
        assert!(debug.contains("cdn.example.com"));
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("secret-token"));
    }

    #[test]
    fn optional_playback_events_are_parsed() {
        for kind in [
            "playing",
            "buffering",
            "video_size",
            "video_progress",
            "playback_error",
        ] {
            let raw = format!(r#"{{"type":"{kind}","width":1280,"height":720,"currentTime":3.5}}"#);
            let event = BridgeEvent::from_ipc(&raw).unwrap();
            let BridgeEvent::Playback(status) = event else {
                panic!("expected playback status for {kind}");
            };
            assert_eq!(status.kind, kind);
            assert_eq!(status.width, 1280);
            assert_eq!(status.height, 720);
            assert_eq!(status.current_time, Some(3.5));
        }
    }

    #[test]
    fn parses_encryption_flag_from_live_info() {
        let raw = r#"{
            "type":"live_info",
            "playurl":"https://live.example.ysp.cctv.cn/opaque/channel.m3u8?pid=600002521",
            "defn":"fhd",
            "livepid":"600002521",
            "encrypt":2,
            "formats":[{"id":1,"fn":"fhd","fnname":"蓝光 1080P","encrypt":2}]
        }"#;
        let BridgeEvent::LiveInfo(info) = BridgeEvent::from_ipc(raw).unwrap() else {
            panic!("expected live_info");
        };
        assert_eq!(info.encryption(), Some(2));
        assert!(info.encrypted());
    }

    #[test]
    fn encryption_falls_back_to_format_flags() {
        let raw = r#"{
            "type":"live_info",
            "playurl":"https://live.example.ysp.cctv.cn/opaque/channel.m3u8?pid=600002521",
            "defn":"fhd",
            "formats":[{"id":1,"fn":"fhd","fnname":"蓝光 1080P","encrypt":"2"}]
        }"#;
        let BridgeEvent::LiveInfo(info) = BridgeEvent::from_ipc(raw).unwrap() else {
            panic!("expected live_info");
        };
        assert_eq!(info.encryption(), Some(2));
        assert!(info.encrypted());
    }

    #[test]
    fn clear_stream_is_not_encrypted() {
        let raw = r#"{
            "type":"live_info",
            "playurl":"https://live.example.ysp.cctv.cn/opaque/channel.m3u8?pid=600152137",
            "defn":"fhd",
            "encrypt":0,
            "formats":[{"id":1,"fn":"fhd","fnname":"蓝光 1080P","encrypt":0}]
        }"#;
        let BridgeEvent::LiveInfo(info) = BridgeEvent::from_ipc(raw).unwrap() else {
            panic!("expected live_info");
        };
        assert_eq!(info.encryption(), Some(0));
        assert!(!info.encrypted());
    }
}
