//! Online daily wallpaper (Bing daily image via `cn.bing.com`).
//!
//! `cn.bing.com` is reachable from mainland China and exposes a stable JSON API
//! at `/HPImageArchive.aspx`, returning the image URL plus attribution text
//! (title / copyright) — which powers the "了解有关此图片信息" info panel.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

const BING: &str = "https://cn.bing.com";

#[derive(Clone, Debug)]
pub struct OnlineImage {
    pub title: String,
    pub copyright: String,
    pub date: String,
    pub url: String,
    pub path: PathBuf,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(25))
        .user_agent("Mozilla/5.0 (wallpaper-rs)")
        .build()
}

/// Fetch the Bing daily image at offset `idx` (0 = today, 1 = yesterday, ...),
/// downloading it into `cache_dir`, and return metadata.
pub fn fetch(idx: u32, cache_dir: &Path) -> Result<OnlineImage, String> {
    let api = format!("{BING}/HPImageArchive.aspx?format=js&idx={idx}&n=1&mkt=zh-CN");
    let body = agent()
        .get(&api)
        .call()
        .map_err(|e| format!("请求失败：{e}"))?
        .into_string()
        .map_err(|e| format!("读取响应失败：{e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析失败：{e}"))?;
    let img = v["images"]
        .get(0)
        .ok_or_else(|| "接口未返回图片数据".to_string())?;

    let url_path = img["url"].as_str().unwrap_or("").to_string();
    if url_path.is_empty() {
        return Err("图片地址为空".to_string());
    }
    let full = if url_path.starts_with("http") {
        url_path
    } else {
        format!("{BING}{url_path}")
    };
    let title = img["title"].as_str().unwrap_or("今日壁纸").to_string();
    let copyright = img["copyright"].as_str().unwrap_or("").to_string();
    let date = img["startdate"].as_str().unwrap_or("").to_string();

    // Download bytes to cache.
    std::fs::create_dir_all(cache_dir).ok();
    let ext = full
        .rsplit('.')
        .next()
        .unwrap_or("jpg")
        .split('&')
        .next()
        .unwrap_or("jpg")
        .to_string();
    let stamp = if date.is_empty() {
        format!("idx{idx}")
    } else {
        format!("{date}-{idx}")
    };
    let path = cache_dir.join(format!("bing_{stamp}.{ext}"));

    let resp = agent()
        .get(&full)
        .set("Referer", "https://cn.bing.com/")
        .call()
        .map_err(|e| format!("下载失败：{e}"))?;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("下载中断：{e}"))?;
    if buf.len() < 1024 {
        return Err("下载内容过小，可能被拦截".to_string());
    }
    std::fs::write(&path, &buf).map_err(|e| format!("写入缓存失败：{e}"))?;

    Ok(OnlineImage {
        title,
        copyright,
        date,
        url: full,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_shape() {
        // Validate our JSON field expectations without network access.
        let sample = r#"{"images":[{"startdate":"20260101","url":"/th?id=OHR.Test_ZH-CN123_1920x1080.jpg","copyright":"测试版权","title":"测试标题"}]}"#;
        let v: serde_json::Value = serde_json::from_str(sample).unwrap();
        let img = v["images"].get(0).unwrap();
        assert_eq!(img["title"].as_str().unwrap(), "测试标题");
        assert_eq!(img["copyright"].as_str().unwrap(), "测试版权");
        assert!(img["url"].as_str().unwrap().contains("OHR"));
    }

    /// Live network test — ignored by default (no network in CI). Run with:
    /// `cargo test --release fetch_live_bing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fetch_live_bing() {
        let dir = std::env::temp_dir().join("wp_online_live");
        let img = fetch(0, &dir).expect("live Bing fetch should succeed");
        let md = std::fs::metadata(&img.path).expect("downloaded file exists");
        assert!(md.len() > 10_000, "image should be non-trivial size");
        println!(
            "LIVE OK title={:?} bytes={} url={}",
            img.title,
            md.len(),
            img.url
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
