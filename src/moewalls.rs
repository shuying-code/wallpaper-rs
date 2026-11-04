//! 网上壁纸库客户端 —— MoeWalls（`moewalls.com`）。
//!
//! WordPress（bimber 主题）站点，列表为服务端渲染 HTML：
//! - **分类浏览**：`GET /`、`GET /page/N/`、`GET /category/{slug}/`、
//!   `GET /category/{slug}/page/N/`。
//! - **关键词搜索**：`GET /?s=关键词`、`GET /page/N/?s=关键词`。
//! - **详情/下载**：详情页里 `<a id="moe-download" data-url="...">` 的令牌，
//!   真实文件地址 `GET https://go.moewalls.com/download.php?video=<令牌>`（mp4）。
//!   封面图：`.../xxx-thumb-364x205.jpg` 去掉尺寸后缀即 1920 宽的 `xxx-thumb.jpg`。
//!
//! 该站内容是**动态壁纸（mp4）**：设为桌面/锁屏使用其封面静图，下载可取 MP4。

use crate::websrc::{
    absolute_join, attr_after, download_to, ext_of, get_text, max_number_after, urlencode,
    Category, Full, Item, Page, SRC_MOEWALLS,
};
use std::path::Path;

pub const BASE: &str = "https://moewalls.com";
pub const REFERER: &str = "https://moewalls.com/";
/// 真实文件下载走独立子域。
pub const DL_BASE: &str = "https://go.moewalls.com";

pub const CATEGORIES: &[Category] = &[
    Category { name: "全部", path: "" },
    Category { name: "抽象", path: "/category/abstract/" },
    Category { name: "动物", path: "/category/animal/" },
    Category { name: "动漫", path: "/category/anime/" },
    Category { name: "奇幻", path: "/category/fantasy/" },
    Category { name: "游戏", path: "/category/games/" },
    Category { name: "风景", path: "/category/landscape/" },
    Category { name: "生活", path: "/category/lifestyle/" },
    Category { name: "电影", path: "/category/movies/" },
    Category { name: "其他", path: "/category/others/" },
    Category { name: "像素", path: "/category/pixel-art/" },
    Category { name: "科幻", path: "/category/sci-fi/" },
    Category { name: "车辆", path: "/category/vehicle/" },
];

fn absolute(u: &str) -> String {
    absolute_join(BASE, u)
}

/// 构造列表/搜索分页地址。
fn list_url(path: &str, page: u32, keyword: &str) -> String {
    let page = page.max(1);
    let kw = keyword.trim();
    if !kw.is_empty() {
        if page <= 1 {
            format!("{BASE}/?s={}", urlencode(kw))
        } else {
            format!("{BASE}/page/{page}/?s={}", urlencode(kw))
        }
    } else if page <= 1 {
        if path.is_empty() {
            format!("{BASE}/")
        } else {
            format!("{BASE}{path}")
        }
    } else if path.is_empty() {
        format!("{BASE}/page/{page}/")
    } else {
        format!("{BASE}{path}page/{page}/")
    }
}

/// 从 `post-{id}` 类名里取数字 id。
fn post_id(chunk: &str) -> Option<u32> {
    let i = chunk.find("post-")?;
    let rest = &chunk[i + "post-".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// 解析列表里的壁纸卡片。
fn parse_items(html: &str) -> Vec<Item> {
    let mut out = Vec::new();
    for raw in html.split("<article class=\"entry-tpl-grid") {
        let chunk = match raw.find("</article>") {
            Some(j) => &raw[..j],
            None => continue,
        };
        let Some(href) = attr_after(chunk, "g1-frame\" href=\"") else {
            continue;
        };
        let Some(thumb) = attr_after(chunk, "src=\"") else {
            continue;
        };
        let title = attr_after(chunk, "title=\"").unwrap_or_default();
        let id = post_id(chunk).unwrap_or_else(|| crate::websrc::fnv(&href));
        // 364x205 缩略图去掉尺寸后缀即 1920 宽封面。
        let full_url = if thumb.contains("-364x205") {
            Some(thumb.replace("-364x205", ""))
        } else {
            None
        };
        out.push(Item {
            id,
            title,
            thumb: absolute(&thumb),
            page: absolute(&href),
            src: SRC_MOEWALLS,
            full_url,
            video_url: None,
        });
    }
    out
}

/// 从分页区推断总页数。
fn parse_total_pages(html: &str) -> u32 {
    let region = match html.find("class=\"pagination") {
        Some(i) => &html[i..],
        None => html,
    };
    max_number_after(region, "/page/").max(1)
}

/// 浏览/搜索第 `page` 页。
pub fn list(path: &str, page: u32, keyword: &str) -> Result<Page, String> {
    let page = page.max(1);
    let html = get_text(&list_url(path, page, keyword), REFERER)?;
    let total = parse_total_pages(&html).max(1);
    let items = parse_items(&html);
    Ok(Page {
        items,
        page,
        total_pages: total,
        has_more: page < total,
        search_id: None,
    })
}

/// 从详情页取下载令牌。
fn download_token(html: &str) -> Option<String> {
    let i = html.find("moe-download")?;
    let region = &html[i..];
    attr_after(region, "data-url=\"")
}

/// 从详情页取封面。
fn detail_poster(html: &str) -> Option<String> {
    attr_after(html, "poster=\"").map(|p| absolute(&p))
}

fn slug_of(page: &str) -> String {
    page.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("moewalls")
        .to_string()
}

/// 取详情页封面并下载，同时带回 MP4 直链。
pub fn full(item: &Item, cache: &Path) -> Result<Full, String> {
    let html = get_text(&item.page, REFERER)?;
    let poster = item
        .full_url
        .clone()
        .or_else(|| detail_poster(&html))
        .ok_or_else(|| "未找到封面地址".to_string())?;
    let video = download_token(&html).map(|t| format!("{DL_BASE}/download.php?video={t}"));
    std::fs::create_dir_all(cache).ok();
    let dest = cache.join(format!("moe_{}.{}", item.id, ext_of(&poster)));
    let need = match std::fs::metadata(&dest) {
        Ok(m) => m.len() < 1024,
        Err(_) => true,
    };
    if need {
        download_to(&poster, &dest, REFERER)?;
    }
    Ok(Full {
        id: item.id,
        title: item.title.clone(),
        url: poster,
        path: dest,
        video_name: video.as_ref().map(|_| format!("{}.mp4", slug_of(&item.page))),
        video,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_grid_items() {
        let html = r#"<li class="g1-collection-item"><article class="entry-tpl-grid entry-tpl-grid-m g1-card post-21641 post type-post category-anime resolutions-3840x2160">
        <div class="entry-featured-media "><a title="Elaina Blade Wandering Witch Live Wallpaper" class="g1-frame" href="https://moewalls.com/anime/elaina-blade-wandering-witch-live-wallpaper/"><div class="g1-frame-inner"><img width="364" height="205" src="https://moewalls.com/wp-content/uploads/2026/11/elaina-blade-wandering-witch-thumb-364x205.jpg" class="attachment-bimber-grid-standard wp-post-image" alt="" /></div></a></div>
        </article></li>"#;
        let items = parse_items(html);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 21641);
        assert_eq!(items[0].title, "Elaina Blade Wandering Witch Live Wallpaper");
        assert_eq!(
            items[0].thumb,
            "https://moewalls.com/wp-content/uploads/2026/11/elaina-blade-wandering-witch-thumb-364x205.jpg"
        );
        assert_eq!(
            items[0].full_url.as_deref(),
            Some("https://moewalls.com/wp-content/uploads/2026/11/elaina-blade-wandering-witch-thumb.jpg")
        );
        assert_eq!(items[0].src, SRC_MOEWALLS);
    }

    #[test]
    fn parses_download_token_and_pages() {
        let html = r##"<div class="pagination"><a class="page-numbers" href="https://moewalls.com/page/2/">2</a><a class="page-numbers" href="https://moewalls.com/page/313/">313</a></div>
        <a id="moe-download" data-id ="21641" data-url="ABC%2Bdef" rel="nofollow" class="g1-button" href="#"> Download Wallpaper</a>"##;
        assert_eq!(parse_total_pages(html), 313);
        assert_eq!(download_token(html).as_deref(), Some("ABC%2Bdef"));
    }

    #[test]
    fn builds_list_urls() {
        assert_eq!(list_url("/category/anime/", 2, ""), "https://moewalls.com/category/anime/page/2/");
        assert_eq!(list_url("", 3, ""), "https://moewalls.com/page/3/");
        assert_eq!(list_url("", 1, "elaina"), "https://moewalls.com/?s=elaina");
    }

    /// 真实联网冒烟测试（默认忽略）。运行：
    /// `cargo test moewalls_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn moewalls_live_smoke() {
        let p = list("/category/anime/", 1, "").expect("category");
        assert!(!p.items.is_empty());
        assert!(p.total_pages > 1);
        let first = p.items[0].clone();
        let dir = std::env::temp_dir().join("wp_moewalls_live");
        let f = full(&first, &dir).expect("detail");
        println!(
            "moewalls ok id={} title={:?} poster={} video={:?}",
            f.id, f.title, f.url, f.video
        );
        let bytes = std::fs::metadata(&f.path).map(|m| m.len()).unwrap_or(0);
        assert!(bytes > 10_000);
        assert!(f.video.is_some(), "should expose an mp4 link");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
