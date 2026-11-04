//! 网上壁纸库客户端 —— 微软必应每日壁纸 4K（`bing.ioliu.cn`，数据源 `cn.bing.com`）。
//!
//! 该站是 SvelteKit 应用，真实接口为：
//! - **列表/搜索**：`GET /api/wallpapers?page=1&pageSize=24[&q=关键词]`。
//! - **反爬挑战**：命中 429(`x-bf-challenge: refresh`) 时先 `GET /api/public/challenge`
//!   取 `{nonce, token}` 与 `bf_vid` Cookie，再带 `x-bf-nonce` / `x-bf-token` /
//!   `x-bf-ts` 头重试。
//! - **原图**：`GET https://cn.bing.com/th?id=<urlbase>_UHD.jpg`（3840x2160）。
//! - **缩略图**：`GET https://cn.bing.com/th?id=<urlbase>_1920x1080.jpg&w=800&h=450`。
//! - **详情页**：`GET /wallpapers/{slug}`。

use crate::websrc::{download_to, ext_of, fnv, urlencode, Category, Full, Item, Page, SRC_BING};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const BASE: &str = "https://bing.ioliu.cn";
pub const REFERER: &str = "https://bing.ioliu.cn/";
/// 图片由微软官方 CDN 提供。
pub const CDN: &str = "https://cn.bing.com";
pub const PAGESIZE: u32 = 24;

/// 必应每日壁纸没有分类，仅「每日更新」一档（搜索框可用关键词）。
pub const CATEGORIES: &[Category] = &[Category {
    name: "每日更新",
    path: "",
}];

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

struct Challenge {
    cookie: String,
    nonce: String,
    token: String,
}

fn challenge(agent: &ureq::Agent) -> Result<Challenge, String> {
    let resp = agent
        .get(&format!("{BASE}/api/public/challenge"))
        .set("Referer", REFERER)
        .call()
        .map_err(|e| format!("挑战请求失败：{e}"))?;
    let cookie = resp
        .header("set-cookie")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_string();
    let body = resp
        .into_string()
        .map_err(|e| format!("读取挑战失败：{e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("挑战解析失败：{e}"))?;
    let nonce = v
        .get("nonce")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let token = v
        .get("token")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    if nonce.is_empty() || token.is_empty() {
        return Err("挑战令牌为空".to_string());
    }
    Ok(Challenge {
        cookie,
        nonce,
        token,
    })
}

/// 带挑战重试的 API GET。
fn api_get(url: &str) -> Result<String, String> {
    let agent = crate::websrc::agent();
    let mut last = String::new();
    for _ in 0..3 {
        let ch = challenge(&agent)?;
        let ts = now_ms().to_string();
        match agent
            .get(url)
            .set("Referer", REFERER)
            .set("Cookie", &ch.cookie)
            .set("x-bf-nonce", &ch.nonce)
            .set("x-bf-token", &ch.token)
            .set("x-bf-ts", &ts)
            .call()
        {
            Ok(resp) => {
                return resp
                    .into_string()
                    .map_err(|e| format!("读取响应失败：{e}"));
            }
            Err(ureq::Error::Status(code, resp)) => {
                last = format!("HTTP {code}");
                if code == 429 {
                    std::thread::sleep(Duration::from_millis(800));
                    continue;
                }
                let body = resp.into_string().unwrap_or_default();
                let snippet: String = body.chars().take(120).collect();
                return Err(format!("接口错误 {code}：{snippet}"));
            }
            Err(e) => return Err(format!("请求失败：{e}")),
        }
    }
    Err(format!("被站点限流（{last}），请稍后再试"))
}

/// 解析 `/api/wallpapers` 的 JSON。
fn parse_page(body: &str, page: u32) -> Result<Page, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("解析失败：{e}"))?;
    let arr = v
        .get("items")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let cur = v.get("page").and_then(|x| x.as_u64()).unwrap_or(page as u64) as u32;
    let size = v
        .get("pageSize")
        .and_then(|x| x.as_u64())
        .unwrap_or(PAGESIZE as u64)
        .max(1) as u32;
    let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let has_more = v
        .get("hasMore")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let total_pages = ((total + size - 1) / size).max(1);
    let items = arr
        .iter()
        .map(|it| {
            let slug = it.get("slug").and_then(|x| x.as_str()).unwrap_or_default();
            let urlbase = it
                .get("urlbase")
                .and_then(|x| x.as_str())
                .unwrap_or_default();
            let title = it
                .get("title")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string();
            Item {
                id: fnv(slug),
                title,
                thumb: format!("{CDN}/th?id={urlbase}_1920x1080.jpg&w=800&h=450"),
                page: format!("{BASE}/wallpapers/{slug}"),
                src: SRC_BING,
                full_url: Some(format!("{CDN}/th?id={urlbase}_UHD.jpg")),
                video_url: None,
            }
        })
        .collect();
    Ok(Page {
        items,
        page: cur.max(1),
        total_pages,
        has_more,
        search_id: None,
    })
}

/// 浏览/搜索第 `page` 页；`keyword` 非空时按关键词搜索。
pub fn list(page: u32, keyword: &str) -> Result<Page, String> {
    let page = page.max(1);
    let kw = keyword.trim();
    let mut url = format!("{BASE}/api/wallpapers?page={page}&pageSize={PAGESIZE}");
    if !kw.is_empty() {
        url.push_str(&format!("&q={}", urlencode(kw)));
    }
    let body = api_get(&url)?;
    parse_page(&body, page)
}

/// 下载必应原图（UHD 4K）。
pub fn full(item: &Item, cache: &Path) -> Result<Full, String> {
    let url = item
        .full_url
        .clone()
        .ok_or_else(|| "缺少原图地址".to_string())?;
    std::fs::create_dir_all(cache).ok();
    let dest = cache.join(format!("bing_{}.{}", item.id, ext_of(&url)));
    let need = match std::fs::metadata(&dest) {
        Ok(m) => m.len() < 1024,
        Err(_) => true,
    };
    if need {
        download_to(&url, &dest, REFERER)?;
    }
    Ok(Full {
        id: item.id,
        title: item.title.clone(),
        url,
        path: dest,
        video: None,
        video_name: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_json() {
        let body = r#"{"items":[
            {"slug":"abc123","title":"大西洋森林，巴西","urlbase":"OHR.Atlantica_ZH-CN6593995386"},
            {"slug":"def456","title":"A field of poppies","urlbase":"OHR.PoppiesField_EN-GB3947926925"}
        ],"page":2,"pageSize":24,"total":4310,"hasMore":true}"#;
        let p = parse_page(body, 2).unwrap();
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.page, 2);
        assert!(p.has_more);
        assert_eq!(p.total_pages, (4310 + 23) / 24);
        assert_eq!(p.items[0].src, SRC_BING);
        assert_eq!(
            p.items[0].thumb,
            "https://cn.bing.com/th?id=OHR.Atlantica_ZH-CN6593995386_1920x1080.jpg&w=800&h=450"
        );
        assert_eq!(
            p.items[0].full_url.as_deref(),
            Some("https://cn.bing.com/th?id=OHR.Atlantica_ZH-CN6593995386_UHD.jpg")
        );
        assert_eq!(p.items[0].page, "https://bing.ioliu.cn/wallpapers/abc123");
        assert_eq!(p.items[0].title, "大西洋森林，巴西");
    }

    /// 真实联网冒烟测试（默认忽略）。运行：
    /// `cargo test bing_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bing_live_smoke() {
        let p1 = list(1, "").expect("home page");
        assert!(!p1.items.is_empty());
        assert!(p1.total_pages > 1);
        let p2 = list(2, "").expect("page 2");
        assert!(!p2.items.is_empty());
        assert_ne!(p1.items[0].id, p2.items[0].id, "pages differ");
        let s = list(1, "森林").expect("search");
        assert!(!s.items.is_empty());
        println!(
            "bing ok p1={} p2={} total={} search={}",
            p1.items.len(),
            p2.items.len(),
            p1.total_pages,
            s.items.len()
        );
        let dir = std::env::temp_dir().join("wp_bing_live");
        let full = full(&p1.items[0], &dir).expect("uhd download");
        let dim = crate::thumbs::image_dimensions(&full.path);
        let bytes = std::fs::metadata(&full.path).map(|m| m.len()).unwrap_or(0);
        println!("bing uhd {:?} bytes={}", dim, bytes);
        assert_eq!(dim, Some((3840, 2160)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
