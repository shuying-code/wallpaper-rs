//! Online daily wallpaper (Bing daily image via `cn.bing.com`).
//!
//! `cn.bing.com` is reachable from mainland China and exposes a stable JSON API
//! at `/HPImageArchive.aspx`, returning the image URL plus attribution text
//! (title / copyright) — which powers the "了解有关此图片信息" info panel.

use std::io::Read; // 引入依赖
use std::path::{Path, PathBuf}; // 引入依赖
use std::time::Duration; // 引入依赖

const BING: &str = "https://cn.bing.com"; // 常量 BING

#[derive(Clone, Debug)] // 语句
pub struct OnlineImage { // 结构体 OnlineImage
    pub title: String, // 字段 title
    pub copyright: String, // 字段 copyright
    pub date: String, // 字段 date
    pub url: String, // 字段 url
    pub path: PathBuf, // 字段 path
} // 块结束

fn agent() -> ureq::Agent { // 函数 agent
    ureq::AgentBuilder::new() // 调用 new
        .timeout_connect(Duration::from_secs(8)) // 语句
        .timeout_read(Duration::from_secs(25)) // 语句
        .user_agent("Mozilla/5.0 (wallpaper-rs)") // 语句
        .build() // 语句
} // 块结束

/// Fetch the Bing daily image at offset `idx` (0 = today, 1 = yesterday, ...),
/// downloading it into `cache_dir`, and return metadata.
pub fn fetch(idx: u32, cache_dir: &Path) -> Result<OnlineImage, String> { // 函数 fetch
    let api = format!("{BING}/HPImageArchive.aspx?format=js&idx={idx}&n=1&mkt=zh-CN"); // 变量 api
    let body = agent() // 变量 body
        .get(&api) // 语句
        .call() // 语句
        .map_err(|e| format!("请求失败：{e}"))? // 语句
        .into_string() // 语句
        .map_err(|e| format!("读取响应失败：{e}"))?; // 语句
    let v: serde_json::Value = // 变量 v
        serde_json::from_str(&body).map_err(|e| format!("解析失败：{e}"))?; // 调用 from_str
    let img = v["images"] // 变量 img
        .get(0) // 语句
        .ok_or_else(|| "接口未返回图片数据".to_string())?; // 语句

    let url_path = img["url"].as_str().unwrap_or("").to_string(); // 变量 url_path
    if url_path.is_empty() { // 条件分支
        return Err("图片地址为空".to_string()); // 返回
    } // 块结束
    let full = if url_path.starts_with("http") { // 变量 full
        url_path // 语句
    } else { // 语句
        format!("{BING}{url_path}") // 语句
    }; // 块结束
    let title = img["title"].as_str().unwrap_or("今日壁纸").to_string(); // 变量 title
    let copyright = img["copyright"].as_str().unwrap_or("").to_string(); // 变量 copyright
    let date = img["startdate"].as_str().unwrap_or("").to_string(); // 变量 date

    // Download bytes to cache.
    std::fs::create_dir_all(cache_dir).ok(); // 调用 create_dir_all
    let ext = full // 变量 ext
        .rsplit('.') // 语句
        .next() // 语句
        .unwrap_or("jpg") // 语句
        .split('&') // 语句
        .next() // 语句
        .unwrap_or("jpg") // 语句
        .to_string(); // 语句
    let stamp = if date.is_empty() { // 变量 stamp
        format!("idx{idx}") // 语句
    } else { // 语句
        format!("{date}-{idx}") // 语句
    }; // 块结束
    let path = cache_dir.join(format!("bing_{stamp}.{ext}")); // 变量 path

    let resp = agent() // 变量 resp
        .get(&full) // 语句
        .set("Referer", "https://cn.bing.com/") // 语句
        .call() // 语句
        .map_err(|e| format!("下载失败：{e}"))?; // 语句
    let mut reader = resp.into_reader(); // 变量 reader
    let mut buf = Vec::new(); // 变量 buf
    reader // 语句
        .read_to_end(&mut buf) // 语句
        .map_err(|e| format!("下载中断：{e}"))?; // 语句
    if buf.len() < 1024 { // 条件分支
        return Err("下载内容过小，可能被拦截".to_string()); // 返回
    } // 块结束
    std::fs::write(&path, &buf).map_err(|e| format!("写入缓存失败：{e}"))?; // 调用 write

    Ok(OnlineImage { // 调用 Ok
        title, // 语句
        copyright, // 语句
        date, // 语句
        url: full, // 字段 url
        path, // 语句
    }) // 语句
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn parses_api_shape() { // 函数 parses_api_shape
        // Validate our JSON field expectations without network access.
        let sample = r#"{"images":[{"startdate":"20260101","url":"/th?id=OHR.Test_ZH-CN123_1920x1080.jpg","copyright":"测试版权","title":"测试标题"}]}"#; // 变量 sample
        let v: serde_json::Value = serde_json::from_str(sample).unwrap(); // 变量 v
        let img = v["images"].get(0).unwrap(); // 变量 img
        assert_eq!(img["title"].as_str().unwrap(), "测试标题"); // 语句
        assert_eq!(img["copyright"].as_str().unwrap(), "测试版权"); // 语句
        assert!(img["url"].as_str().unwrap().contains("OHR")); // 语句
    } // 块结束

    /// Live network test — ignored by default (no network in CI). Run with:
    /// `cargo test --release fetch_live_bing -- --ignored --nocapture`
    #[test] // 语句
    #[ignore] // 语句
    fn fetch_live_bing() { // 函数 fetch_live_bing
        let dir = std::env::temp_dir().join("wp_online_live"); // 变量 dir
        let img = fetch(0, &dir).expect("live Bing fetch should succeed"); // 变量 img
        let md = std::fs::metadata(&img.path).expect("downloaded file exists"); // 变量 md
        assert!(md.len() > 10_000, "image should be non-trivial size"); // 语句
        println!( // 语句
            "LIVE OK title={:?} bytes={} url={}", // 语句
            img.title, // 语句
            md.len(), // 语句
            img.url // 语句
        ); // 块结束
        let _ = std::fs::remove_dir_all(&dir); // 变量 _
    } // 块结束
} // 块结束
