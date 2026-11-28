//! Error type adapted from `ysp-live` (`crates/ysp-core/src/error.rs`).
//! Upstream license: MIT.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL 解析失败: {0}")]
    Url(#[from] url::ParseError),

    #[error("播放地址为空")]
    EmptyPlayUrl,

    #[error("播放地址协议不受支持: {0}")]
    UnsupportedScheme(String),

    #[error("播放地址缺少主机名")]
    MissingHost,

    #[error("播放地址长度超过限制")]
    PlayUrlTooLong,

    #[error("播放地址包含用户凭据")]
    PlayUrlHasCredentials,

    #[error("播放地址不是 HLS(.m3u8)")]
    NotHls,

    #[error("桥消息缺少字段: {0}")]
    MissingField(&'static str),

    #[error("未知桥消息类型: {0}")]
    UnknownBridgeEvent(String),

    #[error("频道数据缺少名称")]
    ChannelNameMissing,

    #[error("频道 PID 非法: {0}")]
    InvalidPid(String),

    #[error("频道 PID 重复: {0}")]
    DuplicatePid(String),

    #[error("日期必须为 yyyyMMdd")]
    InvalidDate,

    #[error("频道目录为空")]
    EmptyCatalog,
}
