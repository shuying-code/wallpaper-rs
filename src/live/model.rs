//! Channel catalog adapted from `ysp-live` (`crates/ysp-core/src/model.rs`).
//! Upstream license: MIT.

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

use super::error::CoreError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Cctv,
    Satellite,
    #[default]
    Other,
}

impl ChannelKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cctv => "央视",
            Self::Satellite => "卫视",
            Self::Other => "其他",
        }
    }
}

impl<'de> Deserialize<'de> for ChannelKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.to_ascii_lowercase().as_str() {
            "cctv" | "yangshi" | "cctv_hd" => Self::Cctv,
            "satellite" | "weishi" | "local" => Self::Satellite,
            _ => Self::Other,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    #[serde(rename = "channelName", alias = "name")]
    pub name: String,

    #[serde(default)]
    pub pid: String,

    #[serde(rename = "streamId", alias = "stream_id", default)]
    pub stream_id: String,

    #[serde(rename = "channelType", alias = "kind", default)]
    pub kind: ChannelKind,

    #[serde(rename = "payType", alias = "pay_type", default)]
    pub pay_type: String,

    #[serde(rename = "isVip", alias = "is_vip", default)]
    pub is_vip: bool,

    #[serde(rename = "isLimitedFree", alias = "is_limited_free", default)]
    pub is_limited_free: bool,

    #[serde(rename = "is4K", alias = "is_4k", default)]
    pub is_4k: bool,

    #[serde(rename = "isScreenFlag", alias = "is_screen_supported", default)]
    pub is_screen_supported: bool,
}

impl Channel {
    pub fn page_url(&self) -> Result<String, CoreError> {
        validate_pid(&self.pid)?;
        Ok(format!("https://yangshipin.cn/tv/home?pid={}", self.pid))
    }

    pub fn requires_membership(&self) -> bool {
        self.is_vip || self.pay_type == "880"
    }

    pub fn badge(&self) -> Option<&'static str> {
        if self.is_limited_free {
            Some("限免")
        } else if self.requires_membership() {
            Some("VIP")
        } else {
            None
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.badge() {
            Some(badge) => write!(f, "{} ({badge})", self.name),
            None => f.write_str(&self.name),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelCatalog {
    pub channels: Vec<Channel>,
}

impl ChannelCatalog {
    pub fn from_json(input: &str) -> Result<Self, CoreError> {
        let catalog: Self = serde_json::from_str(input)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn bundled() -> Result<Self, CoreError> {
        Self::from_json(include_str!("../../assets/live/channels.json"))
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        if self.channels.is_empty() {
            return Err(CoreError::EmptyCatalog);
        }

        let mut seen = std::collections::HashSet::new();
        for channel in &self.channels {
            if channel.name.trim().is_empty() {
                return Err(CoreError::ChannelNameMissing);
            }
            validate_pid(&channel.pid)?;
            if !seen.insert(channel.pid.as_str()) {
                return Err(CoreError::DuplicatePid(channel.pid.clone()));
            }
        }
        Ok(())
    }

    pub fn cctv(&self) -> impl Iterator<Item = &Channel> {
        self.channels
            .iter()
            .filter(|channel| channel.kind == ChannelKind::Cctv)
    }

    pub fn satellite(&self) -> impl Iterator<Item = &Channel> {
        self.channels
            .iter()
            .filter(|channel| channel.kind == ChannelKind::Satellite)
    }

    pub fn find_pid(&self, pid: &str) -> Option<&Channel> {
        self.channels.iter().find(|channel| channel.pid == pid)
    }

    pub fn find_name(&self, name: &str) -> Option<&Channel> {
        let wanted = name.trim().to_ascii_lowercase();
        self.channels
            .iter()
            .find(|channel| channel.name.trim().to_ascii_lowercase() == wanted)
    }
}

pub fn validate_pid(pid: &str) -> Result<(), CoreError> {
    if pid.is_empty() || pid.len() > 32 || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CoreError::InvalidPid(pid.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_has_cctv_and_satellite_channels() {
        let catalog = ChannelCatalog::bundled().expect("bundled catalog must parse");
        assert_eq!(catalog.channels.len(), 64);
        assert_eq!(catalog.cctv().count(), 40);
        assert_eq!(catalog.satellite().count(), 24);
        assert_eq!(
            catalog
                .find_pid("600152137")
                .map(|channel| channel.name.as_str()),
            Some("天津卫视")
        );
    }

    #[test]
    fn channel_page_url_is_built_from_pid() {
        let catalog = ChannelCatalog::bundled().unwrap();
        let channel = catalog.find_name("cctv1").unwrap();
        assert_eq!(
            channel.page_url().unwrap(),
            "https://yangshipin.cn/tv/home?pid=600001859"
        );
    }

    #[test]
    fn duplicate_pid_is_rejected() {
        let json = r#"[
            {"channelName":"A","pid":"1","channelType":"yangshi"},
            {"channelName":"B","pid":"1","channelType":"weishi"}
        ]"#;
        assert!(matches!(
            ChannelCatalog::from_json(json),
            Err(CoreError::DuplicatePid(_))
        ));
    }
}
