//! 网上壁纸库客户端 —— MotionBGs（`motionbgs.com`）。
//!
//! 服务端渲染 HTML + htmx：
//! - **分类浏览**：`GET /4k/`、`GET /mobile/`、`GET /gifs/`、`GET /tag:xxx/`，
//!   第 N 页为 `{栏目}/N/`。
//! - **关键词搜索**：`GET /search?q=关键词`（命中同名 tag 时 302 到 `/tag:xxx/`），
//!   翻页 `?page=N` 或 `{tag}/N/`。
//! - **缩略图**：`GET /i/c/364x205/media/{id}/{slug}.jpg`，去掉尺寸目录
//!   （`/1920x1080/`）即 1920x1080 封面。
//! - **动态壁纸**：`GET /dl/hd/{id}/`（1920x1080 mp4）或 `/dl/4k/{id}/`。
//!
//! 该站内容是**动态壁纸（mp4）**：设为桌面/锁屏使用其封面静图，下载可取 MP4。

use crate::websrc::{
    absolute_join, download_to, ext_of, get_text, urlencode, Category, Full, Item, Page,
    SRC_MOTIONBGS,
};
use std::path::Path;
use std::time::Duration;

pub const BASE: &str = "https://motionbgs.com";
pub const REFERER: &str = "https://motionbgs.com/";

pub const CATEGORIES: &[Category] = &[
    Category { name: "全部", path: "" },
    Category { name: "4K", path: "/4k/" },
    Category { name: "手机", path: "/mobile/" },
    Category { name: "GIF", path: "/gifs/" },
    Category { name: "动漫", path: "/tag:anime/" },
    Category { name: "游戏", path: "/tag:games/" },
    Category { name: "自然", path: "/tag:nature/" },
    Category { name: "车", path: "/tag:car/" },
    Category { name: "超级英雄", path: "/tag:superhero/" },
    Category { name: "太空", path: "/tag:space/" },
];

fn agent_noredirect() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(30))
        .user_agent(crate::websrc::UA)
        .redirects(0)
        .build()
}

fn absolute(u: &str) -> String {
    absolute_join(BASE, u)
}

/// 读取 `key=值`（值不带引号）到空白或 `>` 前。
fn attr_unquoted(s: &str, key: &str) -> Option<String> {
    let i = s.find(key)?;
    let rest = &s[i + key.len()..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '>')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// 探测关键词是否命中同名 tag；命中返回 `Some(tag 路径)`。
fn search_tag_base(keyword: &str) -> Option<String> {
    let url = format!("{BASE}/search?q={}", urlencode(keyword));
    match agent_noredirect().get(&url).set("Referer", REFERER).call() {
        Err(ureq::Error::Status(_, resp)) => {
            let loc = resp.header("location").unwrap_or("").to_string();
            if loc.is_empty() {
                None
            } else if loc.starts_with('/') {
                Some(format!("{BASE}{loc}"))
            } else {
                Some(loc)
            }
        }
        _ => None,
    }
}

fn with_page(base: &str, page: u32) -> String {
    if page <= 1 {
        base.to_string()
    } else if base.ends_with('/') {
        format!("{base}{page}/")
    } else {
        format!("{base}/{page}/")
    }
}

/// 构造列表/搜索分页地址。
fn list_url(path: &str, page: u32, keyword: &str) -> String {
    let kw = keyword.trim();
    if !kw.is_empty() {
        if let Some(tag) = search_tag_base(kw) {
            with_page(&tag, page)
        } else if page <= 1 {
            format!("{BASE}/search?q={}", urlencode(kw))
        } else {
            format!("{BASE}/search?q={}&page={}", urlencode(kw), page)
        }
    } else {
        let base = if path.is_empty() {
            format!("{BASE}/")
        } else {
            format!("{BASE}{path}")
        };
        with_page(&base, page)
    }
}

/// 解析卡片列表。
fn parse_items(html: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("<a title=\"") {
        let after = &rest[i + "<a title=\"".len()..];
        let Some(q) = after.find('"') else { break };
        let title = after[..q].to_string();
        let Some(endrel) = after.find("</a>") else { break };
        let seg = &after[..endrel];
        let href = attr_unquoted(seg, "href=");
        let thumb = attr_unquoted(seg, "src=");
        if title.ends_with("wallpaper") {
            if let (Some(href), Some(thumb)) = (href, thumb) {
                if thumb.starts_with("/i/c/") {
                    let id = thumb
                        .find("/media/")
                        .map(|p| {
                            let d: String = thumb[p + "/media/".len()..]
                                .chars()
                                .take_while(|c| c.is_ascii_digit())
                                .collect();
                            d.parse::<u32>().unwrap_or(0)
                        })
                        .unwrap_or(0);
                    let full_url = if thumb.contains("/364x205/") {
                        Some(absolute(&thumb.replace("/364x205/", "/1920x1080/")))
                    } else {
                        None
                    };
                    out.push(Item {
                        id: if id == 0 { crate::websrc::fnv(&href) } else { id },
                        title,
                        thumb: absolute(&thumb),
                        page: absolute(&href),
                        src: SRC_MOTIONBGS,
                        full_url,
                        video_url: Some(format!("{BASE}/dl/hd/{id}/")),
                    });
                }
            }
        }
        rest = &after[q..];
    }
    out
}

/// 浏览/搜索第 `page` 页。
pub fn list(path: &str, page: u32, keyword: &str) -> Result<Page, String> {
    let page = page.max(1);
    let html = get_text(&list_url(path, page, keyword), REFERER)?;
    let has_more = html.contains("rel=next");
    Ok(Page {
        items: parse_items(&html),
        page,
        // 站点只提供「下一页」，无法可靠得知总页数。
        total_pages: 0,
        has_more,
        search_id: None,
    })
}

/// 取 1920x1080 封面并下载，同时带回 HD MP4 直链。
pub fn full(item: &Item, cache: &Path) -> Result<Full, String> {
    let url = item
        .full_url
        .clone()
        .ok_or_else(|| "缺少封面地址".to_string())?;
    std::fs::create_dir_all(cache).ok();
    let dest = cache.join(format!("mbgs_{}.{}", item.id, ext_of(&url)));
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
        video_name: item.video_url.as_ref().map(|_| format!("{}.mp4", item.id)),
        video: item.video_url.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cards() {
        let html = r#"<section class=tmb><a title="A Galaxy Within live wallpaper" href=/a-galaxy-within> <figure><picture><source media="(-webkit-min-device-pixel-ratio: 1.5)" srcset=/i/c/546x308/media/10270/a-galaxy-within.3840x2160.jpg.webp type=image/webp><source media="(-webkit-min-device-pixel-ratio: 1.5)" srcset=/i/c/546x308/media/10270/a-galaxy-within.3840x2160.jpg type=image/jpeg><source srcset=/i/c/364x205/media/10270/a-galaxy-within.3840x2160.jpg.webp type=image/webp><source srcset=/i/c/364x205/media/10270/a-galaxy-within.3840x2160.jpg type=image/jpeg><img alt="a galaxy within live wallpaper" height=205 src=/i/c/364x205/media/10270/a-galaxy-within.3840x2160.jpg width=364></picture></figure> <span class=ttl>A Galaxy Within</span> <span class=frm> 4K </span> </a></section>"#;
        let items = parse_items(html);
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id, 10270);
        assert_eq!(it.title, "A Galaxy Within live wallpaper");
        assert_eq!(it.thumb, "https://motionbgs.com/i/c/364x205/media/10270/a-galaxy-within.3840x2160.jpg");
        assert_eq!(it.full_url.as_deref(), Some("https://motionbgs.com/i/c/1920x1080/media/10270/a-galaxy-within.3840x2160.jpg"));
        assert_eq!(it.video_url.as_deref(), Some("https://motionbgs.com/dl/hd/10270/"));
        assert_eq!(it.src, SRC_MOTIONBGS);
    }

    #[test]
    fn list_urls_for_categories() {
        assert_eq!(list_url("/tag:anime/", 2, ""), "https://motionbgs.com/tag:anime/2/");
        assert_eq!(list_url("", 3, ""), "https://motionbgs.com/3/");
        assert_eq!(list_url("", 1, ""), "https://motionbgs.com/");
    }

    /// 真实联网冒烟测试（默认忽略）。运行：
    /// `cargo test motionbgs_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn motionbgs_live_smoke() {
        let p = list("/tag:anime/", 1, "").expect("tag page");
        assert!(!p.items.is_empty());
        let s = list("", 1, "galaxy").expect("search");
        assert!(!s.items.is_empty());
        let first = p.items[0].clone();
        let dir = std::env::temp_dir().join("wp_mbgs_live");
        let f = full(&first, &dir).expect("poster");
        let dim = crate::thumbs::image_dimensions(&f.path);
        println!("motionbgs ok id={} title={:?} dim={:?} video={:?}", f.id, f.title, dim, f.video);
        assert!(dim.map(|(w, _)| w >= 1280).unwrap_or(false));
        assert!(f.video.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
