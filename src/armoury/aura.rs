//! Aura 灯效：经内置 PowerShell 桥调用官方 `AsRogAuraService.ServiceMediator.1`。
//!
//! 为什么不用裸 FFI：本机实测 `IDispatch::Invoke` 直接调用会在 `oleaut32`
//! 内部访问冲突（`0xC0000005`），而 PowerShell 后期绑定（`Type.InvokeMember`）
//! 稳定可用；读取约 200 ms，写入约 10 s（服务内部做整组灯珠提交）。
//! 因此：
//! - 读：后台线程每 `poll_ms × 3` 采样一次；
//! - 写：**必须异步**，UI 只显示「下发中…」，完成后回传结果；
//! - 回写 XML 的 `<funcid>` 必须是 1（照抄 20 会被静默忽略）；
//! - 写前解除独占，写完还 1，否则 Armoury Crate 自己的 UI 会失联。

use super::{AuraDevice, AuraHealth, AuraSnapshot, LightMode};
use crate::armoury::xml;
use crate::config::ArmouryCfg;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// 桥脚本。写入配置目录后由 `powershell -File` 执行（避免命令行转义地狱）。
const BRIDGE: &str = r#"param(
    [string]$Out,
    [string]$Payload,
    [string]$Result,
    [string]$Device = "Group"
)
$ErrorActionPreference = "Stop"
try {
    $m = New-Object -ComObject "AsRogAuraService.ServiceMediator.1"
} catch {
    [System.IO.File]::WriteAllText("$Out.error", $_.Exception.Message, [System.Text.UTF8Encoding]::new($false))
    exit 3
}
if ($Out) {
    $enc = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText("$Out.devices", $m.get_QueryAllDevice(), $enc)
    [System.IO.File]::WriteAllText("$Out.caps", $m.get_QueryAllDeviceCap(), $enc)
    [System.IO.File]::WriteAllText("$Out.status", $m.get_QueryAllDeviceStatus(), $enc)
    [System.IO.File]::WriteAllText("$Out.excl", [string]$m.get_AuraExclusive_Status(), $enc)
}
if ($Payload) {
    $xml = [System.IO.File]::ReadAllText($Payload)
    $xml = [regex]::Replace($xml, '<funcid>\d+</funcid>', '<funcid>1</funcid>', 1)
    $excl = $m.get_AuraExclusive_Status()
    if ($excl -ne 0) { $null = $m.set_AuraExclusive_Status(0) }
    try {
        $null = $m.GetType().InvokeMember(
            "SetProfile",
            [System.Reflection.BindingFlags]::SetProperty,
            $null, $m, @($Device, $xml)
        )
        [System.IO.File]::WriteAllText($Result, "ok", [System.Text.UTF8Encoding]::new($false))
    } catch {
        [System.IO.File]::WriteAllText($Result, ("err: " + $_.Exception.Message), [System.Text.UTF8Encoding]::new($false))
    } finally {
        $null = $m.set_AuraExclusive_Status(1)
    }
}
"#;

/// 桥工作目录：`<config>/armoury`。
pub fn work_dir() -> PathBuf {
    crate::config::config_dir().join("armoury")
}

/// 确保桥脚本与工作目录存在（幂等，每进程只需一次）。
pub fn ensure_bridge() -> Result<PathBuf, String> {
    let dir = work_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建 {}：{e}", dir.display()))?;
    let script = dir.join("aura_bridge.ps1");
    let need_write = std::fs::read_to_string(&script)
        .map(|s| s != BRIDGE)
        .unwrap_or(true);
    if need_write {
        std::fs::write(&script, BRIDGE)
            .map_err(|e| format!("无法写入桥脚本 {}：{e}", script.display()))?;
    }
    Ok(script)
}

fn powershell() -> &'static str {
    "powershell"
}

/// PowerShell 只用于后台 COM 采样/下发，绝不能让控制台窗口出现在用户桌面上。
fn powershell_command() -> std::process::Command {
    let mut cmd = std::process::Command::new(powershell());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// 一次同步读取（200 ms 级）。仅供后台线程调用。
pub fn sample(cfg: &ArmouryCfg) -> AuraSnapshot {
    let sampled = Some(std::time::Instant::now());
    if !cfg!(windows) {
        return AuraSnapshot {
            health: Some(AuraHealth::Unsupported(
                "Aura 灯效仅支持 Windows（LocalServer COM）",
            )),
            sampled,
            ..Default::default()
        };
    }
    let script = match ensure_bridge() {
        Ok(p) => p,
        Err(e) => {
            return AuraSnapshot {
                health: Some(AuraHealth::Missing(e)),
                sampled,
                ..Default::default()
            }
        }
    };
    let dir = work_dir();
    let out = dir.join(format!("snap-{}", std::process::id()));
    let out_s = out.to_string_lossy().to_string();
    let _ = std::fs::remove_file(format!("{out_s}.error"));
    let status = powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg("-Out")
        .arg(&out_s)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let read = |ext: &str| std::fs::read_to_string(format!("{out_s}.{ext}")).unwrap_or_default();
    let err = read("error");
    match status {
        Ok(s) if s.success() && err.is_empty() => {}
        _ => {
            let why = if err.trim().is_empty() {
                "powershell 调用失败（Aura 服务未安装或 COM 被拦截）".to_string()
            } else {
                err.trim().to_string()
            };
            return AuraSnapshot {
                health: Some(AuraHealth::Missing(why)),
                sampled,
                ..Default::default()
            };
        }
    }

    let devices = read("devices");
    let caps = read("caps");
    let status_xml = read("status");
    let excl = read("excl");

    let mut snap = AuraSnapshot {
        health: Some(AuraHealth::Ready),
        devices: parse_devices(&devices),
        modes: parse_modes(&caps),
        switchable_devices: parse_switchable_devices(&status_xml),
        exclusive: excl.trim().parse::<i32>().map(|v| v != 0).unwrap_or(true),
        sampled,
        ..Default::default()
    };
    snap.led_count = xml::text(&caps, "ledcount")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| snap.devices.iter().map(|d| d.leds).sum());
    let (mode_key, hsl, brightness, speed) = parse_active(&status_xml);
    snap.active_mode = mode_key.and_then(|k| {
        snap.modes
            .iter()
            .find(|m| m.key == k)
            .map(|m| m.name.clone())
    });
    snap.active_color = hsl.map(|(h, s, l)| {
        let (r, g, b) = hue2rgb(h, s, l);
        format!("#{r:02X}{g:02X}{b:02X}")
    });
    if hsl.is_some() {
        snap.brightness = brightness;
        snap.speed = speed;
    }
    // 落盘模板：下发时必须「读—改—写」，手搓全量 XML 会被服务拒绝。
    let _ = cache_status_template(&status_xml);
    // 让目标值与会话内的期望保持一致，避免 UI 上的滑杆每帧跳回。
    if snap.brightness == 0 {
        snap.brightness = cfg.lighting.brightness;
    }
    snap
}

/// 下发结果。
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyState {
    Idle,
    Running,
    Ok,
    Err(String),
}

/// 异步下发：Aura 提交要 ~10 秒，绝不能阻塞 UI 线程。
pub fn apply_async(cfg: ArmouryCfg) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        crate::report::thread_state("aura", "Aura 灯效下发", true);
        let res = apply(&cfg);
        crate::report::thread_state("aura", "Aura 灯效下发", false);
        let _ = tx.send(res);
    });
    rx
}

/// 同步下发（只在后台线程调用）。
pub fn apply(cfg: &ArmouryCfg) -> Result<(), String> {
    let script = ensure_bridge()?;
    let dir = work_dir();
    // 1) 读当前状态作为模板：手搓全量 XML 会被服务拒绝。
    let snap = sample(cfg);
    if snap.health.as_ref().map(|h| !h.ok()).unwrap_or(true) {
        return Err(snap
            .health
            .map(|h| h.summary())
            .unwrap_or_else(|| "Aura 服务不可用".to_string()));
    }
    let status_path = dir.join("status-template.xml");
    if !status_path.is_file() {
        return Err("缺少状态模板（请先在「灯效」页刷新一次）".to_string());
    }
    let status = std::fs::read_to_string(&status_path).map_err(|e| e.to_string())?;
    if status.trim().is_empty() {
        return Err("状态模板为空，拒绝下发".to_string());
    }
    let payload = build_payload(&status, cfg)?;
    let payload_path = dir.join("apply.xml");
    std::fs::write(&payload_path, payload.as_bytes()).map_err(|e| e.to_string())?;
    let result_path = dir.join("apply.txt");
    let _ = std::fs::remove_file(&result_path);

    let out = powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg("-Payload")
        .arg(&payload_path)
        .arg("-Result")
        .arg(&result_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let text = std::fs::read_to_string(&result_path).unwrap_or_default();
    match out {
        Ok(s) if s.success() && text.trim() == "ok" => Ok(()),
        _ => Err(if text.trim().is_empty() {
            "下发未返回结果（服务超时或拒绝）".to_string()
        } else {
            text.trim().to_string()
        }),
    }
}

/// 把最近一次采样得到的状态 XML 落盘，作为下发模板。
pub fn cache_status_template(snap_status: &str) -> Result<(), String> {
    let dir = work_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("status-template.xml"), snap_status.as_bytes())
        .map_err(|e| e.to_string())
}

pub fn is_windows() -> bool {
    cfg!(windows)
}

#[allow(dead_code)]
fn _unused(_p: &Path) {}

// ---- 解析 -----------------------------------------------------------------

fn parse_devices(body: &str) -> Vec<AuraDevice> {
    xml::nodes(body, "device")
        .into_iter()
        .filter(|n| xml::child_text(body, n, "type").is_some())
        .map(|n| AuraDevice {
            kind: xml::child_text(body, &n, "type").unwrap_or_default(),
            model: xml::child_text(body, &n, "model").unwrap_or_default(),
            manufacture: xml::child_text(body, &n, "manufacture").unwrap_or_default(),
            leds: xml::child_text(body, &n, "count")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            width: xml::child_text(body, &n, "width")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            height: xml::child_text(body, &n, "height")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            index: xml::child_text(body, &n, "index")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        })
        .collect()
}

fn parse_modes(body: &str) -> Vec<LightMode> {
    let mut seen = std::collections::BTreeSet::new();
    xml::nodes(body, "mode")
        .into_iter()
        .filter_map(|n| {
            let key = xml::key_of(&n)?;
            // S0 / Non-S0 两个场景各有一份 mode 列表，去重后才是真实模式数。
            if !seen.insert(key) {
                return None;
            }
            let name = xml::child_text(body, &n, "name").unwrap_or_default();
            if name.is_empty() {
                return None;
            }
            Some(LightMode {
                key,
                name,
                can_color: xml::child_text(body, &n, "cansetcolor").as_deref() == Some("1"),
                can_speed: xml::child_text(body, &n, "cansetspeed").as_deref() == Some("1"),
                can_direction: xml::child_text(body, &n, "cansetdirection").as_deref() == Some("1"),
            })
        })
        .collect()
}

/// Group S0 中实际带 LED 节点的设备（可被逐设备熄灭）。
fn parse_switchable_devices(status: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let Some(group) = xml::nodes(status, "device")
        .into_iter()
        .find(|n| xml::attr(&n.tag, "key").as_deref() == Some("Group"))
    else {
        return out;
    };
    let group_body = &status[group.body_start..group.body_end];
    let Some(scene) = xml::nodes(group_body, "scene")
        .into_iter()
        .find(|n| matches!(xml::attr(&n.tag, "key").as_deref(), Some("S0") | Some("0")))
    else {
        return out;
    };
    let scene_body = &group_body[scene.body_start..scene.body_end];
    let mut seen: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for led in xml::nodes(scene_body, "led") {
        let Some(name) = xml::child_text(scene_body, &led, "DeviceName") else {
            continue;
        };
        let index = seen.entry(name.clone()).or_insert(0);
        out.insert(AuraDevice::key_for(&name, *index));
        *index += 1;
    }
    out
}

/// 当前生效模式 + 第一颗灯珠的 HSL + 亮度 + 速度。
fn parse_active(status: &str) -> (Option<u32>, Option<(f32, f32, f32)>, u8, u8) {
    let group = xml::nodes(status, "device")
        .into_iter()
        .find(|n| xml::attr(&n.tag, "key").as_deref() == Some("Group"));
    let Some(group) = group else {
        return (None, None, 0, 0);
    };
    let body = &status[group.body_start..group.body_end];
    let mode_key = xml::nodes(body, "mode")
        .into_iter()
        .next()
        .and_then(|n| xml::key_of(&n));
    // 只取 S0（正常开机态）场景：Non-S0 是关机 / 休眠态，颜色常全 0。
    // 取错场景会让预览条与当前色显示成纯黑（实测于本机）。
    let scene_body = xml::nodes(body, "scene")
        .into_iter()
        .find(|n| xml::attr(&n.tag, "key").as_deref() == Some("S0"))
        .map(|n| &body[n.body_start..n.body_end])
        .unwrap_or(body);
    let led = xml::nodes(scene_body, "led").into_iter().next();
    let mut hsl = None;
    let mut speed = 0u8;
    if let Some(led) = led {
        let led_body = &scene_body[led.body_start..led.body_end];
        let h = xml::text(led_body, "hue").and_then(|v| v.parse::<f32>().ok());
        let s = xml::text(led_body, "saturation").and_then(|v| v.parse::<f32>().ok());
        let l = xml::text(led_body, "lightness").and_then(|v| v.parse::<f32>().ok());
        if let (Some(h), Some(s), Some(l)) = (h, s, l) {
            hsl = Some((h, s, l));
        }
        speed = xml::text(led_body, "speed")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v.clamp(0.0, 100.0) as u8)
            .unwrap_or(0);
    }
    let brightness = hsl
        .map(|(_, _, l)| (l * 100.0).clamp(0.0, 100.0) as u8)
        .unwrap_or(0);
    (mode_key, hsl, brightness, speed)
}

pub fn hue2rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m) * 255.0).clamp(0.0, 255.0).round() as u8;
    (f(r), f(g), f(b))
}

pub fn rgb2hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, l)
}

/// 一处文本替换 `(start, end, replacement)`。
type Edit = (usize, usize, String);

fn apply_edits(src: &str, mut edits: Vec<Edit>) -> String {
    // 从后往前改，避免前面的偏移量失效。
    edits.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = src.to_string();
    for (start, end, text) in edits {
        if start <= end && end <= out.len() {
            out.replace_range(start..end, &text);
        }
    }
    out
}

/// 把起始标签里的 `key="N"` 换掉。
fn retag_key(tag: &str, key: u32) -> String {
    for quote in ['"', '\''] {
        let pat = format!("key={quote}");
        if let Some(i) = tag.find(&pat) {
            let rest = &tag[i + pat.len()..];
            if let Some(j) = rest.find(quote) {
                return format!("{}{}{}", &tag[..i + pat.len()], key, &rest[j..]);
            }
        }
    }
    tag.to_string()
}

/// 以查询结果为模板改目标字段。返回的 XML 已把 `<funcid>` 置 1。
pub fn build_payload(status: &str, cfg: &ArmouryCfg) -> Result<String, String> {
    let mut edits: Vec<Edit> = Vec::new();

    let group = xml::nodes(status, "device")
        .into_iter()
        .find(|n| xml::attr(&n.tag, "key").as_deref() == Some("Group"))
        .ok_or_else(|| "状态 XML 缺少 Group 节点".to_string())?;
    let group_body = &status[group.body_start..group.body_end];

    // 1) S0 场景下第一个 `mode key` 改成目标模式。
    // 2) S0 场景下每颗灯珠的 HSL 与速度（只改已存在的标签，不新增字段）。
    //    逐设备开关也只作用于 Group S0：关闭的设备写 0，其他设备保持当前效果。
    //    Non-S0 是关机 / 休眠态，不能拿来做正常灯效模板。
    let scenes = xml::nodes(group_body, "scene");
    let scene = scenes
        .iter()
        .find(|n| matches!(xml::attr(&n.tag, "key").as_deref(), Some("S0") | Some("0")));
    if let Some(scene) = scene {
        let scene_body = &group_body[scene.body_start..scene.body_end];
        if let Some(mode) = xml::nodes(scene_body, "mode").into_iter().next() {
            let tag_start = group.body_start + scene.body_start + mode.body_start - mode.tag.len();
            let tag_end = group.body_start + scene.body_start + mode.body_start;
            edits.push((tag_start, tag_end, retag_key(&mode.tag, cfg.lighting.mode)));
        }

        let (h, s, l) = {
            let c = crate::ui_kit::parse_hex(&cfg.lighting.color);
            rgb2hsl(c.r(), c.g(), c.b())
        };
        let mut seen: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        for led in xml::nodes(scene_body, "led") {
            let led_start = group.body_start + scene.body_start + led.body_start;
            let led_body = &scene_body[led.body_start..led.body_end];
            let device = xml::text(led_body, "DeviceName").unwrap_or_default();
            let index = seen.entry(device.clone()).or_insert(0);
            let key = AuraDevice::key_for(&device, *index);
            *index += 1;
            let on = cfg.lighting.device_enabled(&key);
            let values = if on {
                [
                    ("hue", format!("{h:.6}")),
                    ("saturation", format!("{s:.6}")),
                    ("lightness", format!("{l:.6}")),
                    ("hashue2", format!("{h:.6}")),
                    ("hue2", format!("{h:.6}")),
                    ("speed", format!("{:.0}", cfg.lighting.speed as f32)),
                ]
            } else {
                [
                    ("hue", "0.000000".to_string()),
                    ("saturation", "0.000000".to_string()),
                    ("lightness", "0.000000".to_string()),
                    ("hashue2", "0.000000".to_string()),
                    ("hue2", "0.000000".to_string()),
                    ("speed", "0".to_string()),
                ]
            };
            for (tag, value) in values {
                if let Some(node) = xml::nodes(led_body, tag).into_iter().next() {
                    edits.push((
                        led_start + node.body_start,
                        led_start + node.body_end,
                        value,
                    ));
                }
            }
        }
    }

    Ok(xml::force_funcid_one(&apply_edits(status, edits)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = r#"<root><funcid>20</funcid>
<device key="Group"><isenabled>0</isenabled>
<scene key="S0"><state>S0</state><mode key="1">
<led key="0"><DeviceName>Mainboard_Master</DeviceName><hue>0.500000</hue>
<saturation>1.000000</saturation><lightness>0.500000</lightness><speed>0</speed></led>
<color_type>Plain</color_type></mode></scene></device></root>"#;

    #[test]
    fn payload_forces_funcid_and_sets_mode() {
        let mut cfg = ArmouryCfg::default();
        cfg.lighting.mode = 5;
        cfg.lighting.color = "#FF0000".to_string();
        let p = build_payload(STATUS, &cfg).expect("payload");
        assert!(p.contains("<funcid>1</funcid>"), "funcid 必须为 1: {p}");
        assert!(p.contains(r#"<mode key="5">"#), "模式应被改写: {p}");
        assert!(p.contains("<hue>0.000000</hue>"), "红色 hue=0: {p}");
        assert!(p.contains("<saturation>1.000000</saturation>"), "{p}");
    }

    #[test]
    fn payload_keeps_unknown_fields_untouched() {
        let cfg = ArmouryCfg::default();
        let p = build_payload(STATUS, &cfg).expect("payload");
        // 读—改—写：模板里的其余字段必须原样保留，否则服务会拒绝。
        assert!(p.contains("<color_type>Plain</color_type>"));
        assert!(p.contains("<DeviceName>Mainboard_Master</DeviceName>"));
        assert!(p.contains(r#"<scene key="S0">"#));
    }

    #[test]
    fn color_roundtrip_is_stable() {
        let cases = [
            (0xFFu8, 0x00u8, 0x00u8),
            (0x2F, 0x4B, 0xFF),
            (0x0E, 0x9F, 0x6E),
            (0x00, 0x00, 0x00),
        ];
        for (r, g, b) in cases {
            let (h, s, l) = rgb2hsl(r, g, b);
            let (r2, g2, b2) = hue2rgb(h, s, l);
            assert!(
                (r as i32 - r2 as i32).abs() <= 1
                    && (g as i32 - g2 as i32).abs() <= 1
                    && (b as i32 - b2 as i32).abs() <= 1,
                "{r},{g},{b} -> {r2},{g2},{b2}"
            );
        }
    }

    #[test]
    fn missing_group_is_an_error_not_a_silent_write() {
        let cfg = ArmouryCfg::default();
        assert!(build_payload("<root></root>", &cfg).is_err());
    }

    #[test]
    fn per_device_switch_only_dims_that_device() {
        let status = r#"<root><funcid>1</funcid><device key="Group">
<scene key="S0"><mode key="1">
<led key="0"><DeviceName>Mainboard_Master</DeviceName><hue>0.1</hue><saturation>0.2</saturation><lightness>0.3</lightness><speed>4</speed></led>
<led key="1"><DeviceName>AddressableStrip</DeviceName><hue>0.1</hue><saturation>0.2</saturation><lightness>0.3</lightness><speed>4</speed></led>
</mode></scene></device></root>"#;
        let mut cfg = ArmouryCfg::default();
        cfg.lighting.set_device_enabled("AddressableStrip#0", false);
        let p = build_payload(status, &cfg).expect("payload");

        let first = p.find("Mainboard_Master").unwrap();
        let second = p.find("AddressableStrip").unwrap();
        let mainboard = &p[first..second];
        let strip = &p[second..];
        assert!(mainboard.contains("<lightness>"), "主板灯光应保留: {p}");
        assert!(
            !mainboard.contains("<lightness>0.000000</lightness>"),
            "主板不应被熄灭: {p}"
        );
        assert!(
            strip.contains("<lightness>0.000000</lightness>"),
            "ARGB 设备应被熄灭: {p}"
        );
    }

    #[test]
    fn per_device_switch_leaves_non_s0_scene_untouched() {
        let status = r#"<root><funcid>1</funcid><device key="Group">
<scene key="S0"><mode key="1">
<led key="0"><DeviceName>AddressableStrip</DeviceName><hue>0.1</hue><saturation>0.2</saturation><lightness>0.3</lightness><speed>4</speed></led>
</mode></scene>
<scene key="Non-S0"><mode key="5">
<led key="0"><DeviceName>Other</DeviceName><hue>0.4</hue><saturation>0.5</saturation><lightness>0.6</lightness><speed>7</speed></led>
</mode></scene></device></root>"#;
        let mut cfg = ArmouryCfg::default();
        cfg.lighting.set_device_enabled("AddressableStrip#0", false);
        cfg.lighting.set_device_enabled("Other#0", false);
        let p = build_payload(status, &cfg).expect("payload");

        let other = p.find("Other").expect("Non-S0 LED 必须保留");
        let tail = &p[other..];
        assert!(tail.contains("<hue>0.4</hue>"), "Non-S0 不应被改写: {p}");
        assert!(
            tail.contains("<saturation>0.5</saturation>"),
            "Non-S0 不应被改写: {p}"
        );
        assert!(
            tail.contains("<lightness>0.6</lightness>"),
            "Non-S0 不应被熄灭: {p}"
        );
        assert!(tail.contains("<speed>7</speed>"), "Non-S0 速度应保留: {p}");
        assert!(
            p.contains(r#"<scene key="Non-S0"><mode key="5">"#),
            "Non-S0 的模式不应被改写: {p}"
        );
    }

    #[test]
    fn switchable_devices_follow_group_s0_leds() {
        let status = r#"<root><device key="Group"><scene key="S0"><mode key="1">
<led><DeviceName>Mainboard_Master</DeviceName></led>
<led><DeviceName>AddressableStrip</DeviceName></led>
<led><DeviceName>AddressableStrip</DeviceName></led>
</mode></scene><scene key="Non-S0"><mode key="5"><led><DeviceName>Other</DeviceName></led></mode></scene></device></root>"#;
        let keys = parse_switchable_devices(status);
        assert!(keys.contains("Mainboard_Master#0"));
        assert!(keys.contains("AddressableStrip#0"));
        assert!(keys.contains("AddressableStrip#1"));
        assert!(!keys.contains("Other#0"));
    }

    /// 真实服务集成测试：默认忽略，用 `cargo test --lib aura -- --ignored` 手动跑。
    #[test]
    #[ignore]
    fn live_read_aura_service() {
        let cfg = ArmouryCfg::default();
        let snap = sample(&cfg);
        match &snap.health {
            Some(AuraHealth::Ready) => {
                assert!(!snap.devices.is_empty(), "应读到至少一台灯效设备");
                assert!(!snap.modes.is_empty(), "应读到灯效模式");
                assert!(snap.led_count > 0, "应读到灯珠总数");
                println!(
                    "设备 {} 台 / 灯珠 {} / 模式 {} / 当前 {}",
                    snap.devices.len(),
                    snap.led_count,
                    snap.modes.len(),
                    snap.active_mode.clone().unwrap_or_default()
                );
            }
            other => panic!("Aura 服务不可用：{other:?}"),
        }
    }
}
