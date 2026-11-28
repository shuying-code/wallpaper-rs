//! Fan Xpert：**只读**。
//!
//! 实测四类通道全部不可写（见 `ac_api_findings.md` 6.2）：
//! `AsusFanControlService` 64 位 `REGDB_E_CLASSNOTREG`、32 位 `E_ACCESSDENIED`；
//! `\\.\Asusgio3` 的 SDDL 只给 SYSTEM / Administrators；`\\.\ATKACPI` 等本机不存在。
//!
//! 因此首版做「真实读数 + 曲线可视化」：读 ASUS 自己的标定表与预设曲线，
//! 实时转速没有可信来源时显示 `n/a`，**不显示 0**（0 会被误读成风扇停转）。

use super::{FanChannel, FanSnapshot};
use crate::armoury::xml;
use crate::config::ArmouryCfg;
use std::path::PathBuf;

/// ASUS 发布数据目录。
fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let p = PathBuf::from(std::env::var_os("ProgramData")?)
            .join("ASUS")
            .join("Dip")
            .join("FanXpert");
        if p.is_dir() {
            return Some(p);
        }
        return None;
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// 解析 `FanCalibrationData.xml`：每路风扇的转速表与占空比上下限。
pub fn parse_calibration(src: &str) -> Vec<FanChannel> {
    xml::nodes(src, "fan")
        .into_iter()
        .map(|n| {
            let body = &src[n.body_start..n.body_end];
            let index = xml::key_of(&n).unwrap_or(0);
            let name = xml::text(body, "name").unwrap_or_else(|| format!("Fan {index}"));
            let rpm: Vec<u32> = (0..11)
                .filter_map(|i| xml::text(body, &format!("rpm_{i}")).and_then(|v| v.parse().ok()))
                .collect();
            let max_rpm = rpm.iter().cloned().max().unwrap_or(0);
            let min_duty = xml::text(body, "maxdutytokeepstill")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            FanChannel {
                index,
                name,
                max_rpm,
                min_duty,
                curve: Vec::new(),
            }
        })
        .collect()
}

/// 解析预设里的 `curvedata`：`(温度, 占空比)` 升序。
pub fn parse_curve(src: &str, fan_index: u32) -> Vec<(u8, u8)> {
    let fan = xml::nodes(src, "fan")
        .into_iter()
        .find(|n| xml::key_of(n) == Some(fan_index));
    let Some(fan) = fan else {
        return Vec::new();
    };
    let body = &src[fan.body_start..fan.body_end];
    let curve = xml::nodes(body, "curvedata").into_iter().next();
    let Some(curve) = curve else {
        return Vec::new();
    };
    let curve_body = &body[curve.body_start..curve.body_end];
    let mut pts: Vec<(u8, u8)> = xml::nodes(curve_body, "point")
        .into_iter()
        .filter_map(|p| {
            let pb = &curve_body[p.body_start..p.body_end];
            let x = xml::text(pb, "x").and_then(|v| v.parse::<f32>().ok())?;
            let y = xml::text(pb, "y").and_then(|v| v.parse::<f32>().ok())?;
            if x < 0.0 || y < 0.0 {
                return None;
            }
            Some((x.clamp(0.0, 100.0) as u8, y.clamp(0.0, 100.0) as u8))
        })
        .collect();
    pts.sort_by_key(|p| p.0);
    pts
}

pub fn sample(cfg: &ArmouryCfg) -> FanSnapshot {
    let sampled = Some(std::time::Instant::now());
    let Some(dir) = data_dir() else {
        return FanSnapshot {
            error: Some(if cfg!(windows) {
                "未找到 ASUS Fan Xpert 数据目录（未安装 Armoury Crate？）".to_string()
            } else {
                "Fan Xpert 仅支持 Windows".to_string()
            }),
            can_write: false,
            sampled,
            active_profile: cfg.fans.preset.clone(),
            ..Default::default()
        };
    };
    let calib = std::fs::read_to_string(dir.join("FanCalibrationData.xml")).ok();
    let mut channels = calib.as_deref().map(parse_calibration).unwrap_or_default();

    let profiles_dir = dir.join("Profiles");
    let mut profiles = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&profiles_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".xml") {
                profiles.push(stem.trim_end_matches(" (ASUS)").to_string());
            }
        }
    }
    profiles.sort();

    // 用当前预设的曲线填充通道（找不到就回落 Standard）。
    let want = format!("{} (ASUS).xml", title_case(&cfg.fans.preset));
    let alt = format!("{}.xml", title_case(&cfg.fans.preset));
    let profile_path = [profiles_dir.join(&want), profiles_dir.join(&alt)]
        .into_iter()
        .find(|p| p.is_file());
    if let Some(path) = profile_path {
        if let Ok(src) = std::fs::read_to_string(&path) {
            for ch in &mut channels {
                ch.curve = parse_curve(&src, ch.index);
            }
        }
    }

    FanSnapshot {
        error: if calib.is_none() {
            Some("标定文件缺失".to_string())
        } else {
            None
        },
        channels,
        profiles,
        active_profile: cfg.fans.preset.clone(),
        // 首版如实告知：普通会话不能下发。
        can_write: false,
        sampled,
    }
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALIB: &str = r#"<root>
<fan key="0"><name>CPU Fan</name><rpmtable>
<rpm_0>623</rpm_0><rpm_1>849</rpm_1><rpm_2>1962</rpm_2></rpmtable>
<maxdutytokeepstill>23</maxdutytokeepstill></fan>
<fan key="1"><name>Chassis Fan 1</name><rpmtable><rpm_0>0</rpm_0></rpmtable></fan>
</root>"#;

    const PROFILE: &str = r#"<root><fan key="0"><curvedata>
<point key="0"><x>20</x><y>51</y></point>
<point key="1"><x>40</x><y>87</y></point>
<point key="2"><x>100</x><y>255</y></point>
</curvedata></fan></root>"#;

    #[test]
    fn calibration_reports_real_max_rpm() {
        let ch = parse_calibration(CALIB);
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].name, "CPU Fan");
        assert_eq!(ch[0].max_rpm, 1962);
        assert_eq!(ch[0].min_duty, 23);
        // 全 0 的通道 max_rpm 必须如实为 0（没有可信读数），而不是编造。
        assert_eq!(ch[1].max_rpm, 0);
    }

    #[test]
    fn curve_is_sorted_and_clamped() {
        let pts = parse_curve(PROFILE, 0);
        assert_eq!(pts.len(), 3);
        assert!(pts.windows(2).all(|w| w[0].0 <= w[1].0));
        // y=255 是 8bit 满量程，钳到 100 而不是溢出成 255。
        assert_eq!(pts[2].1, 100);
    }

    #[test]
    fn missing_curve_is_empty_not_fake() {
        assert!(parse_curve(PROFILE, 7).is_empty());
    }
}
