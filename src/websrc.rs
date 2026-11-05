//! 网上壁纸库：统一的多站点客户端。
//!
//! 一个界面同时对接四个真实站点，每个站点都走各自站点真实的 HTTP 接口：
//! - **微软必应每日壁纸**（`bing.ioliu.cn`，JSON API + 反爬挑战令牌）
//! - **彼岸图网 4K**（`pic.netbian.com`，栏目/搜索 + 详情页原图直链）
//! - **MoeWalls**（`moewalls.com`，WordPress 列表 + 详情页下载令牌 -> MP4）
//! - **MotionBGs**（`motionbgs.com`，服务端渲染列表 + 直链下载 MP4）
//!
//! 站点顺序即 `Item::src`（见 `SRC_*` 常量），UI 依赖该顺序。

use crate::thumbs::{self, Thumb};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

pub const SRC_BING: usize = 0;
pub const SRC_NETBIAN: usize = 1;
pub const SRC_MOEWALLS: usize = 2;
pub const SRC_MOTIONBGS: usize = 3;

/// 一个分类（或站点栏目）导航项；`path` 为空表示「全部/最新」。
pub struct Category {
    pub name: &'static str,
    pub path: &'static str,
}

/// 一个壁纸站点。
pub struct Source {
    /// 稳定短标识（用于保存目录名）。
    pub key: &'static str,
    /// 中文/英文展示名。
    pub name: &'static str,
    /// 站点域名（展示用）。
    pub host: &'static str,
    /// 请求时携带的 `Referer`（部分站点有防盗链）。
    pub referer: &'static str,
    /// 站点特点小标签。
    pub tag: &'static str,
    /// 该站内容是否为动态壁纸（mp4）。
    pub video: bool,
    /// 分类列表。
    pub cats: &'static [Category],
}

/// 站点顺序固定，`Item::src` 即此数组下标；**必应排第一**。
pub const SOURCES: &[Source] = &[
    Source {
        key: "bing",
        name: "微软必应每日壁纸",
        host: "bing.ioliu.cn",
        referer: "https://bing.ioliu.cn/",
        tag: "4K 每日更新",
        video: false,
        cats: crate::bing::CATEGORIES,
    },
    Source {
        key: "netbian",
        name: "彼岸图网",
        host: "pic.netbian.com",
        referer: "https://pic.netbian.com/",
        tag: "4K / 8K 高清",
        video: false,
        cats: crate::netbian::CATEGORIES,
    },
    Source {
        key: "moewalls",
        name: "MoeWalls",
        host: "moewalls.com",
        referer: "https://moewalls.com/",
        tag: "动态壁纸 MP4",
        video: true,
        cats: crate::moewalls::CATEGORIES,
    },
    Source {
        key: "motionbgs",
        name: "MotionBGs",
        host: "motionbgs.com",
        referer: "https://motionbgs.com/",
        tag: "动态壁纸 MP4",
        video: true,
        cats: crate::motionbgs::CATEGORIES,
    },
];

/// 列表中的一张壁纸（轻量，用于网格展示）。
#[derive(Clone, Debug)]
pub struct Item {
    /// 站点内稳定的数字 id（用于预览/缓存键）。
    pub id: u32,
    pub title: String,
    /// 缩略图绝对地址（网格用）。
    pub thumb: String,
    /// 详情页绝对地址。
    pub page: String,
    /// 所属站点下标。
    pub src: usize,
    /// 已知的原图（封面）直链；未知时由 `full()` 现取。
    pub full_url: Option<String>,
    /// 已知的动态壁纸直链（mp4）。
    pub video_url: Option<String>,
}

/// 一页列表结果。
#[derive(Clone, Debug)]
pub struct Page {
    pub items: Vec<Item>,
    /// 当前页（1 基）。
    pub page: u32,
    /// 总页数；0 表示未知（部分站点只提供「下一页」）。
    pub total_pages: u32,
    /// 是否还有下一页。
    pub has_more: bool,
    /// 关键词搜索的结果 id（彼岸图网翻页用）；其它站点为 `None`。
    pub search_id: Option<u32>,
}

/// 一张已下载到本地的原图（+ 可选的动态壁纸直链）。
#[derive(Clone, Debug)]
pub struct Full {
    pub id: u32,
    pub title: String,
    /// 原图（封面）直链。
    pub url: String,
    /// 本地缓存文件路径。
    pub path: PathBuf,
    /// 动态壁纸（mp4）直链。
    pub video: Option<String>,
    /// 动态壁纸保存时的文件名。
    pub video_name: Option<String>,
}

/// 统一 UA。
pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// 浏览器式 `Accept`：部分站点（含百度 WAF 后的彼岸图网）会按请求头判定是否为脚本。
pub const ACCEPT_HTML: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8";
/// 浏览器式 `Accept-Language`。
pub const ACCEPT_LANG: &str = "zh-CN,zh;q=0.9,en;q=0.8";

/// 全站共享的 HTTP agent。
///
/// 关键：复用**同一个**底层 `ureq::Agent`（克隆共享 Arc 状态），让站点下发的
/// Cookie 在同一会话内持久化、连接池复用。彼岸图网坐落在百度 WAF 之后，首包会
/// 下发 `abymg_id` 等令牌；旧实现每次请求都新建 agent，令牌无法回传，后续请求
/// 常被拦截，表现为「接口访问不通」。共享 agent 后即可稳定浏览。
pub fn agent() -> ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT
        .get_or_init(|| {
            ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(10))
                .timeout_read(Duration::from_secs(45))
                .user_agent(UA)
                .build()
        })
        .clone()
}

/// GET 文本（带 Referer 与浏览器式请求头），失败自动重试 3 次（线性退避）。
pub fn get_text(url: &str, referer: &str) -> Result<String, String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        match agent()
            .get(url)
            .set("Referer", referer)
            .set("Accept", ACCEPT_HTML)
            .set("Accept-Language", ACCEPT_LANG)
            .call()
        {
            Ok(resp) => {
                return resp
                    .into_string()
                    .map_err(|e| format!("读取响应失败：{e}"));
            }
            Err(e) => last = format!("请求失败：{e}"),
        }
        // 短暂退避后重试（0.3s / 0.6s），避免瞬时抖动直接判定失败。
        std::thread::sleep(Duration::from_millis(300 * (attempt + 1)));
    }
    Err(last)
}

/// 下载任意地址到 `dest`。
pub fn download_to(url: &str, dest: &Path, referer: &str) -> Result<(), String> {
    let resp = agent()
        .get(url)
        .set("Referer", referer)
        .set("Accept", "*/*")
        .set("Accept-Language", ACCEPT_LANG)
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
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(dest, &buf).map_err(|e| format!("写入缓存失败：{e}"))?;
    Ok(())
}

/// 提取 URL 的扩展名（小写，缺省 `jpg`）。
pub fn ext_of(url: &str) -> String {
    let last = url.rsplit('/').next().unwrap_or("jpg");
    let name = last.split(['?', '&', '#']).next().unwrap_or("jpg");
    let ext = name.rsplit('.').next().unwrap_or("jpg");
    let ext: String = ext.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if ext.is_empty() || !name.contains('.') {
        "jpg".to_string()
    } else {
        ext.to_lowercase()
    }
}

/// URL 百分号编码（UTF-8 字节），保留 RFC 3986 unreserved 字符。
pub fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

/// 由字符串生成稳定的 u32 键（FNV-1a）。
pub fn fnv(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// 把可能为相对地址的 `u` 补全为绝对地址（相对某个站点 `base`）。
pub fn absolute_join(base: &str, u: &str) -> String {
    if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else if let Some(rest) = u.strip_prefix("//") {
        format!("https://{rest}")
    } else if u.starts_with('/') {
        format!("{base}{u}")
    } else {
        format!("{base}/{u}")
    }
}

/// 取字符串末尾的十进制数字（无则 0）。
pub fn trailing_num(s: &str) -> u32 {
    let digits: String = s
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse().unwrap_or(0)
}

/// 取 `key="..."` 形式属性里引号内的值（`key` 需包含左引号，如 `src="`）。
pub fn attr_after(s: &str, key: &str) -> Option<String> {
    let i = s.find(key)?;
    let rest = &s[i + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// 扫描 `marker` 之后紧跟的数字并取最大值（用于推断总页数）。
pub fn max_number_after(s: &str, marker: &str) -> u32 {
    let mut maxn = 0u32;
    let mut idx = 0usize;
    while let Some(rel) = s[idx..].find(marker) {
        let p = idx + rel + marker.len();
        let digits: String = s[p..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<u32>() {
            maxn = maxn.max(n);
        }
        idx = p + digits.len().max(1);
        if idx >= s.len() {
            break;
        }
    }
    maxn
}

/// 浏览/搜索统一入口：`src` 是站点下标，`cat` 是分类下标。
pub fn fetch(
    src: usize,
    cat: usize,
    page: u32,
    keyword: &str,
    search_id: Option<u32>,
) -> Result<Page, String> {
    let s = &SOURCES[src.min(SOURCES.len() - 1)];
    let cat = cat.min(s.cats.len().saturating_sub(1));
    match s.key {
        "bing" => crate::bing::list(page, keyword),
        "netbian" => crate::netbian::list(&s.cats[cat].path, page, keyword, search_id),
        "moewalls" => crate::moewalls::list(&s.cats[cat].path, page, keyword),
        "motionbgs" => crate::motionbgs::list(&s.cats[cat].path, page, keyword),
        _ => Err("未知站点".to_string()),
    }
}

/// 由站点 key 反查下标；未知 key 回落为 0（必应）。
pub fn source_index_by_key(key: &str) -> usize {
    SOURCES.iter().position(|s| s.key == key).unwrap_or(0)
}

/// 解析并下载某张壁纸的原图（封面），必要时附带动态壁纸直链。
pub fn full(item: &Item, cache: &Path) -> Result<Full, String> {
    match SOURCES[item.src.min(SOURCES.len() - 1)].key {
        "bing" => crate::bing::full(item, cache),
        "netbian" => crate::netbian::full(item, cache),
        "moewalls" => crate::moewalls::full(item, cache),
        "motionbgs" => crate::motionbgs::full(item, cache),
        _ => Err("未知站点".to_string()),
    }
}

/// 由 URL 生成稳定的缓存文件名（FNV-1a 哈希 + 扩展名）。
pub fn cache_name(url: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("t{h:016x}.{}", ext_of(url))
}

/// 缩略图后台下载 + 解码器：把远端 URL 拉取到本地缓存并解码为可上传的纹理数据。
///
/// 与 `crate::thumbs::Thumbnailer` 同一套「请求 / 轮询 / generation」模型，
/// 输入是 URL（带各自的 Referer），并带一个小型磁盘缓存避免重复下载。
pub struct ThumbWorker {
    tx: Sender<(String, String, u64)>,
    rx: Receiver<(String, u64, Option<Thumb>)>,
    requested: HashSet<String>,
    generation: u64,
    cache_dir: PathBuf,
}

impl ThumbWorker {
    pub fn new(cache_dir: PathBuf) -> Self {
        let (tx, rx_req) = channel::<(String, String, u64)>();
        let (tx_res, rx) = channel::<(String, u64, Option<Thumb>)>();
        // 4 个并发线程共享同一请求队列（仅在取任务时短暂加锁），
        // 让整屏缩略图并行下载，明显快于单线程串行。
        const WORKERS: usize = 4;
        let shared = Arc::new(Mutex::new(rx_req));
        for _ in 0..WORKERS {
            let shared = Arc::clone(&shared);
            let tx_res = tx_res.clone();
            let dir = cache_dir.clone();
            thread::spawn(move || loop {
                let job = {
                    let guard = match shared.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    match guard.recv() {
                        Ok(j) => j,
                        Err(_) => break,
                    }
                };
                let (url, referer, gen) = job;
                std::fs::create_dir_all(&dir).ok();
                let file = dir.join(cache_name(&url));
                let need = match std::fs::metadata(&file) {
                    Ok(m) => m.len() < 1024,
                    Err(_) => true,
                };
                if need && download_to(&url, &file, &referer).is_err() {
                    let _ = tx_res.send((url, gen, None));
                    continue;
                }
                let t = thumbs::load_image(&file, thumbs::THUMB_MAX);
                if tx_res.send((url, gen, t)).is_err() {
                    break;
                }
            });
        }
        Self {
            tx,
            rx,
            requested: HashSet::new(),
            generation: 0,
            cache_dir,
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn request(&mut self, url: &str, referer: &str) {
        if self.requested.insert(url.to_string()) {
            let _ = self
                .tx
                .send((url.to_string(), referer.to_string(), self.generation));
        }
    }

    pub fn is_requested(&self, url: &str) -> bool {
        self.requested.contains(url)
    }

    pub fn forget(&mut self, url: &str) {
        self.requested.remove(url);
    }

    /// 切换列表时调用：丢弃之前排队的缩略图结果。
    pub fn reset(&mut self) {
        self.requested.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn poll(&mut self) -> Vec<(String, Option<Thumb>)> {
        let mut v = Vec::new();
        while let Ok((url, gen, t)) = self.rx.try_recv() {
            if gen == self.generation {
                v.push((url, t));
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_ordered_with_bing_first() {
        assert_eq!(SOURCES[SRC_BING].key, "bing");
        assert_eq!(SOURCES[SRC_NETBIAN].key, "netbian");
        assert_eq!(SOURCES[SRC_MOEWALLS].key, "moewalls");
        assert_eq!(SOURCES[SRC_MOTIONBGS].key, "motionbgs");
    }

    #[test]
    fn urlencode_encodes_cjk_as_utf8() {
        assert_eq!(urlencode("风景"), "%E9%A3%8E%E6%99%AF");
        assert_eq!(urlencode("a b"), "a%20b");
    }

    #[test]
    fn ext_of_handles_query_strings() {
        assert_eq!(ext_of("https://a/b/c.jpg"), "jpg");
        assert_eq!(ext_of("https://a/b/c.3840x2160.jpg?x=1&y=2"), "jpg");
        assert_eq!(ext_of("https://a/b/c.mp4"), "mp4");
        assert_eq!(ext_of("https://a/b/c"), "jpg");
    }

    #[test]
    fn max_number_after_picks_largest() {
        let html = r#"<a href="/page/2/">2</a><a href="/page/313/">313</a><a href="/page/4/">4</a>"#;
        assert_eq!(max_number_after(html, "/page/"), 313);
    }
}
