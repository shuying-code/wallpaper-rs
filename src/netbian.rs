//! 网上壁纸库客户端 —— 彼岸图网 4K 站（`pic.netbian.com`）。
//!
//! 全部对应站点真实的 HTTP 接口：
//! - **分类浏览**：`GET /4k*/`、`GET /4k*/index_N.html`（分页）。
//! - **关键词搜索**：`POST /e/search/index.php`（表单字段 `keyboard`），
//!   站点 302 跳转到 `/search/{id}-0.html`，翻页用 `/search/{id}-N.html`。
//! - **原图下载**：详情页 `/tupian/{id}.html` 里 `#img` 的 `src` 即原始大图
//!   直链（`/uploads/allimg/...jpg`），直接 GET 即可下载。
//!
//! 站点为 UTF-8 编码，因此关键词无需再做 GBK 转码。

use crate::websrc::{
    absolute_join, attr_after, download_to, ext_of, urlencode, Category, Full, Item, Page,
    SRC_NETBIAN,
};
use std::path::Path;

/// 站点根地址（4K 站）。
pub const BASE: &str = "https://pic.netbian.com";
/// 下载/请求时携带的 Referer，避免被防盗链拦截。
pub const REFERER: &str = "https://pic.netbian.com/";

/// 4K 站分类导航（`path` 为空表示首页「全部/最新」）。
pub const CATEGORIES: &[Category] = &[
    Category { name: "全部", path: "" },
    Category { name: "4K动漫", path: "/4kdongman/" },
    Category { name: "4K游戏", path: "/4kyouxi/" },
    Category { name: "4K美女", path: "/4kmeinv/" },
    Category { name: "4K风景", path: "/4kfengjing/" },
    Category { name: "4K剧照", path: "/4kjuzhao/" },
    Category { name: "4K汽车", path: "/4kqiche/" },
    Category { name: "4K动物", path: "/4kdongwu/" },
    Category { name: "4K宗教", path: "/4kzongjiao/" },
    Category { name: "4K背景", path: "/4kbeijing/" },
    Category { name: "4K手机", path: "/shoujibizhi/" },
    Category { name: "车机", path: "/cheji/" },
    Category { name: "5K带鱼屏", path: "/5120x2160/" },
    Category { name: "4K极简", path: "/s/4Kjijian/" },
];

/// 使用全站共享 agent（带 Cookie 罐）：彼岸图网在百度 WAF 之后，必须把首包下发的
/// `abymg_id` 等 Cookie 回传，否则后续请求会被拦截（「接口访问不通」）。
fn agent() -> ureq::Agent {
    crate::websrc::agent()
}

/// 带浏览器式请求头 + 退避重试的 GET。
fn get(url: &str) -> Result<String, String> {
    crate::websrc::get_text(url, REFERER)
}

/// 把站点里可能出现的相对地址补全为绝对地址。
fn absolute(u: &str) -> String {
    absolute_join(BASE, u)
}

/// 构造分类分页地址：第 1 页是栏目根，第 N(≥2) 页是 `index_N.html`。
fn category_url(path: &str, page: u32) -> String {
    let base = if path.is_empty() {
        format!("{BASE}/")
    } else {
        format!("{BASE}{path}")
    };
    if page <= 1 {
        base
    } else {
        format!("{base}index_{page}.html")
    }
}

/// 从 `/tupian/{id}.html` 里取数字 id。
fn extract_id(chunk: &str) -> Option<u32> {
    let i = chunk.find("/tupian/")?;
    let rest = &chunk[i + "/tupian/".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// 解析列表页里的所有壁纸卡片。
fn parse_items(html: &str) -> Vec<Item> {
    // 优先只取列表容器（`class="slist"`，手机壁纸页是 `class="alist"`）到第一个
    // `</ul>` 之间的条目，避开友情链接等无关内容。
    let start = html
        .find("class=\"slist\"")
        .or_else(|| html.find("class=\"alist\""));
    let region = match start {
        Some(i) => {
            let r = &html[i..];
            match r.find("</ul>") {
                Some(j) => &r[..j],
                None => r,
            }
        }
        None => html,
    };
    let mut out = Vec::new();
    // 以 `<li` 切分，兼容 `<li>` 与 `<li data-w="…" data-h="…">` 两种写法。
    for raw in region.split("<li") {
        let Some(gt) = raw.find('>') else { continue };
        let body = &raw[gt + 1..];
        let chunk = match body.find("</li>") {
            Some(j) => &body[..j],
            None => body,
        };
        let Some(id) = extract_id(chunk) else { continue };
        let Some(src) = attr_after(chunk, "src=\"") else { continue };
        let title = attr_after(chunk, "alt=\"").unwrap_or_default();
        out.push(Item {
            id,
            title,
            thumb: absolute(&src),
            page: format!("{BASE}/tupian/{id}.html"),
            src: SRC_NETBIAN,
            full_url: None,
            video_url: None,
        });
    }
    out
}

/// 取字符串末尾的十进制数字（无则 0）。
fn trailing_num(s: &str) -> u32 {
    crate::websrc::trailing_num(s)
}

/// 在字符串里反复匹配 `marker` 后面的数字并取最大值。
///
/// - 分类：`marker = "index_"`，数字即页码（1 基）。
/// - 搜索：`marker = "/search/"`，需先跳过结果 id 与 `-`，数字为 0 基页号。
fn max_number_after(s: &str, marker: &str, skip_id_dash: bool, plus_one: bool) -> u32 {
    let mut maxn = 0u32;
    let mut idx = 0usize;
    let bytes = s.as_bytes();
    while let Some(rel) = s[idx..].find(marker) {
        let mut p = idx + rel + marker.len();
        if skip_id_dash {
            while p < bytes.len() && bytes[p].is_ascii_digit() {
                p += 1;
            }
            if p < bytes.len() && bytes[p] == b'-' {
                p += 1;
            } else {
                idx = p.max(idx + 1);
                if idx >= s.len() {
                    break;
                }
                continue;
            }
        }
        let digits: String = s[p..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<u32>() {
            maxn = maxn.max(if plus_one { n + 1 } else { n });
        }
        idx = p + digits.len().max(1);
        if idx >= s.len() {
            break;
        }
    }
    maxn
}

/// 从分页区（`class="page"`）推断总页数：优先「尾页」链接，缺失时回退到
/// 分页区里出现的最大页码。
fn parse_total_pages(html: &str, search: bool) -> u32 {
    let region = match html.find("class=\"page\"") {
        Some(i) => {
            let r = &html[i..];
            match r.find("</div>") {
                Some(j) => &r[..j],
                None => r,
            }
        }
        None => return 1,
    };
    // 1) 尾页链接（href 位于「尾页」文字之前）。
    if let Some(pos) = region.find("尾页") {
        if let Some(h) = region[..pos].rfind("href=\"") {
            let rest = &region[h + "href=\"".len()..];
            let end = rest.find('"').unwrap_or(rest.len());
            let n = trailing_num(rest[..end].trim_end_matches(".html"));
            if n > 0 {
                return if search { n + 1 } else { n };
            }
        }
    }
    // 2) 回退：分页区里最大的页码。
    let n = if search {
        max_number_after(region, "/search/", true, true)
    } else {
        max_number_after(region, "index_", false, false)
    };
    n.max(1)
}

/// 从任意 `/search/{id}-N.html` 链接里取结果 id。
fn parse_search_id(html: &str) -> Option<u32> {
    let region = match html.find("class=\"page\"") {
        Some(i) => &html[i..],
        None => html,
    };
    let i = region.find("/search/")?;
    let rest = &region[i + "/search/".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// 解析详情页 `#img` 的原图直链（返回 `(绝对地址, 标题)`）。
fn parse_detail_image(html: &str) -> Option<(String, String)> {
    let i = html.find("class=\"photo-pic\"")?;
    let r = &html[i..];
    let region = match r.find("</div>") {
        Some(j) => &r[..j],
        None => r,
    };
    let src = attr_after(region, "src=\"")?;
    let alt = attr_after(region, "alt=\"").unwrap_or_default();
    Some((absolute(&src), alt))
}

fn html_title(html: &str) -> Option<String> {
    let i = html.find("<title>")?;
    let rest = &html[i + "<title>".len()..];
    let end = rest.find("</title>")?;
    Some(rest[..end].trim().to_string())
}

/// 浏览某个分类的第 `page` 页（1 基）。
fn fetch_category(path: &str, page: u32) -> Result<Page, String> {
    let html = get(&category_url(path, page.max(1)))?;
    Ok(Page {
        items: parse_items(&html),
        page,
        total_pages: parse_total_pages(&html, false).max(1),
        has_more: page < parse_total_pages(&html, false),
        search_id: None,
    })
}

/// 关键词搜索：`id` 已知且 `page > 1` 时直接翻页，否则重新发起一次
/// `POST /e/search/index.php`（第 1 页）并顺带取回结果 `id`。
fn fetch_search(keyword: &str, page: u32, id: Option<u32>) -> Result<Page, String> {
    if let (Some(id), true) = (id, page > 1) {
        let url = format!("{BASE}/search/{id}-{}.html", page - 1);
        let html = get(&url)?;
        let total = parse_total_pages(&html, true).max(1);
        return Ok(Page {
            items: parse_items(&html),
            page,
            total_pages: total,
            has_more: page < total,
            search_id: Some(id),
        });
    }

    // 第 1 页：POST 表单，站点 302 跳到 /search/{id}-0.html。
    // 现在跟随跳转（沿用同一 Cookie 罐），再用最终 URL 解析搜索结果 id。
    let body = format!(
        "keyboard={}&tempid=1&tbname=photo&show=title",
        urlencode(keyword)
    );
    let resp = agent()
        .post(&format!("{BASE}/e/search/index.php"))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Referer", REFERER)
        .set("Accept", crate::websrc::ACCEPT_HTML)
        .set("Accept-Language", crate::websrc::ACCEPT_LANG)
        .send_string(&body)
        .map_err(|e| format!("搜索请求失败：{e}"))?;
    let final_url = resp.get_url().to_string();
    let html = resp
        .into_string()
        .map_err(|e| format!("读取响应失败：{e}"))?;
    let search_id = parse_search_id(&final_url).or_else(|| parse_search_id(&html));

    let total = parse_total_pages(&html, true).max(1);
    Ok(Page {
        items: parse_items(&html),
        page: 1,
        total_pages: total,
        has_more: 1 < total,
        search_id,
    })
}

/// 统一入口：`keyword` 非空走搜索，否则按分类浏览。
pub fn list(
    path: &str,
    page: u32,
    keyword: &str,
    search_id: Option<u32>,
) -> Result<Page, String> {
    let kw = keyword.trim();
    if kw.is_empty() {
        fetch_category(path, page)
    } else {
        fetch_search(kw, page, search_id)
    }
}

/// 取详情页原图地址并下载到 `cache_dir`，返回原图元信息。
pub fn full(item: &Item, cache_dir: &Path) -> Result<Full, String> {
    let html = get(&item.page)?;
    let (url, alt) = parse_detail_image(&html).ok_or_else(|| "未找到原图地址".to_string())?;
    std::fs::create_dir_all(cache_dir).ok();
    let dest = cache_dir.join(format!("netbian_{}.{}", item.id, ext_of(&url)));
    let need = match std::fs::metadata(&dest) {
        Ok(m) => m.len() < 1024,
        Err(_) => true,
    };
    if need {
        download_to(&url, &dest, REFERER)?;
    }
    let title = if alt.trim().is_empty() {
        html_title(&html).unwrap_or_else(|| item.title.clone())
    } else {
        alt
    };
    Ok(Full {
        id: item.id,
        title,
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
    fn parses_list_items_from_slist() {
        let html = r#"<div class="slist"><ul class="clearfix">
        <li><a href="/tupian/44679.html" target="_blank"><img src="/uploads/allimg/261104/122549-179376634911c8.jpg" alt="天空云朵 山脉 极简4K壁纸" /><b>天空云朵</b></a></li>
        <li><a href="/tupian/44394.html" target="_blank"><img src="/uploads/allimg/261018/080747-17922820678d67.jpg" alt="内藏山秋景4k壁纸" /><b>内藏山</b></a></li>
        </ul></div>"#;
        let items = parse_items(html);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 44679);
        assert_eq!(items[0].title, "天空云朵 山脉 极简4K壁纸");
        assert!(items[0].thumb.starts_with("https://pic.netbian.com/uploads/"));
        assert_eq!(items[0].page, "https://pic.netbian.com/tupian/44679.html");
        assert_eq!(items[0].src, SRC_NETBIAN);
    }

    #[test]
    fn parses_category_total_pages() {
        let html = r#"<div class="page"><b>1</b><a href="/4kfengjing/index_2.html">2</a>
        <span class="slh">…</span><a href="/4kfengjing/index_326.html">326</a>
        <a href="/4kfengjing/index_2.html">下一页</a></div>"#;
        assert_eq!(parse_total_pages(html, false), 326);
    }

    #[test]
    fn parses_search_total_pages_is_zero_based() {
        let html = r#"<div class="page"><b>1</b><a href="/search/220-1.html">2</a>
        <a href="/search/220-1.html">下一页</a><a href="/search/220-9.html">尾页</a></div>"#;
        assert_eq!(parse_total_pages(html, true), 10);
        assert_eq!(parse_search_id(html), Some(220));
    }

    #[test]
    fn parses_detail_image() {
        let html = r#"<div class="photo-pic"><a href="" id="img"><img src="/uploads/allimg/261104/122549-17937663495501.jpg" data-pic="/uploads/allimg/261104/122549-179376634911c8.jpg" alt="天空云朵 极简4K壁纸" title="x" /></a></div>"#;
        let (url, alt) = parse_detail_image(html).unwrap();
        assert_eq!(
            url,
            "https://pic.netbian.com/uploads/allimg/261104/122549-17937663495501.jpg"
        );
        assert_eq!(alt, "天空云朵 极简4K壁纸");
    }

    /// 真实联网冒烟测试（默认忽略）。运行：
    /// `cargo test netbian_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn netbian_live_smoke() {
        let cat = list("/4kfengjing/", 1, "", None).expect("category fetch");
        assert!(!cat.items.is_empty(), "category should return items");
        let first = cat.items[0].clone();

        let s1 = list("", 1, "风景", None).expect("search page 1");
        assert!(!s1.items.is_empty(), "search should return items");

        let dir = std::env::temp_dir().join("wp_netbian_live");
        let full = full(&first, &dir).expect("detail + download");
        let bytes = std::fs::metadata(&full.path).expect("file exists").len();
        assert!(bytes > 10_000, "downloaded image should be non-trivial");
        println!("netbian ok first=#{} {:?} bytes={}", full.id, full.title, bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 统计各分类的总页数（默认忽略）。运行：
    /// `cargo test netbian_live_stats -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn netbian_live_stats() {
        for c in CATEGORIES {
            match fetch_category(c.path, 1) {
                Ok(p) => println!(
                    "{:>10}  path={:<14} pages={:<5} first_page_items={}",
                    c.name,
                    c.path,
                    p.total_pages,
                    p.items.len()
                ),
                Err(e) => println!("{:>10}  path={:<14} ERR={e}", c.name, c.path),
            }
        }
    }
}
