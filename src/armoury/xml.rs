//! 极简 XML 扫描器。
//!
//! Aura / Fan Xpert 的报文体量小（10–500 KB）且结构固定，引入完整 XML 依赖
//! 会给 `opt-level="z"` 的发行包增加体积；这里只做「取标签文本 / 取属性 /
//! 遍历同名节点」三件事，够用且零依赖。

/// 一个节点的起始位置与结束位置（不含结束标签）。
#[derive(Clone, Debug)]
pub struct Node {
    /// 起始标签之后的内容起点。
    pub body_start: usize,
    /// 结束标签的起点。
    pub body_end: usize,
    /// 起始标签内（`<tag ...>`）的文本，用于取属性。
    pub tag: String,
}

/// 找到所有 `<tag ...> ... </tag>` 节点（不嵌套同名标签时可靠）。
pub fn nodes(src: &str, tag: &str) -> Vec<Node> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = src[from..].find(&open) {
        let start = from + i;
        // 避免 `<tagx` 被当成 `<tag`
        let after = src[start + open.len()..].chars().next();
        if !matches!(
            after,
            Some('>') | Some(' ') | Some('\t') | Some('\r') | Some('\n') | Some('/')
        ) {
            from = start + open.len();
            continue;
        }
        let Some(gt_rel) = src[start..].find('>') else {
            break;
        };
        let body_start = start + gt_rel + 1;
        let Some(end_rel) = src[body_start..].find(&close) else {
            break;
        };
        let body_end = body_start + end_rel;
        out.push(Node {
            body_start,
            body_end,
            tag: src[start..body_start].to_string(),
        });
        from = body_end + close.len();
    }
    out
}

/// 第一个同名节点的文本内容（trim 后）。
pub fn text(src: &str, tag: &str) -> Option<String> {
    nodes(src, tag)
        .into_iter()
        .next()
        .map(|n| src[n.body_start..n.body_end].trim().to_string())
}

/// 所有同名节点的文本内容。
pub fn texts(src: &str, tag: &str) -> Vec<String> {
    nodes(src, tag)
        .into_iter()
        .map(|n| src[n.body_start..n.body_end].trim().to_string())
        .collect()
}

/// 取节点内某个子标签的文本。
pub fn child_text(src: &str, node: &Node, tag: &str) -> Option<String> {
    text(&src[node.body_start..node.body_end], tag)
}

/// 取起始标签里的属性值（`key="value"` 或 `key='value'`）。
pub fn attr(tag: &str, key: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pat = format!("{key}={quote}");
        if let Some(i) = tag.find(&pat) {
            let rest = &tag[i + pat.len()..];
            if let Some(j) = rest.find(quote) {
                return Some(rest[..j].to_string());
            }
        }
    }
    None
}

/// 转义后写入 XML 文本节点。
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 把某个标签的文本替换成新值（只替换第一个）。找不到时返回 `None`。
pub fn replace_text(src: &str, tag: &str, value: &str) -> Option<String> {
    let n = nodes(src, tag).into_iter().next()?;
    let mut out = String::with_capacity(src.len() + value.len());
    out.push_str(&src[..n.body_start]);
    out.push_str(value);
    out.push_str(&src[n.body_end..]);
    Some(out)
}

/// 把第一个 `<funcid>N</funcid>` 改成 1：Aura 下发要求（cli 结论，必须照做）。
pub fn force_funcid_one(src: &str) -> String {
    replace_text(src, "funcid", "1").unwrap_or_else(|| src.to_string())
}

/// 从 `key="N"` 形式的节点里取 key。
pub fn key_of(node: &Node) -> Option<u32> {
    attr(&node.tag, "key").and_then(|k| k.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_and_attrs() {
        let s =
            r#"<root><a key="1"><name>Static</name></a><a key="2"><name>Wave</name></a></root>"#;
        assert_eq!(text(s, "name").unwrap(), "Static");
        assert_eq!(texts(s, "name"), vec!["Static", "Wave"]);
        let ns = nodes(s, "a");
        assert_eq!(ns.len(), 2);
        assert_eq!(key_of(&ns[1]).unwrap(), 2);
        assert_eq!(child_text(s, &ns[1], "name").unwrap(), "Wave");
    }

    #[test]
    fn funcid_is_forced_to_one() {
        let s = "<root><funcid>20</funcid><x>1</x></root>";
        assert_eq!(
            force_funcid_one(s),
            "<root><funcid>1</funcid><x>1</x></root>"
        );
    }

    #[test]
    fn self_closing_and_prefix_tags_are_not_confused() {
        let s = r#"<root><ledlist><ledcount>4</ledcount></ledlist><ledger>9</ledger></root>"#;
        assert_eq!(nodes(s, "led").len(), 0);
        assert_eq!(text(s, "ledcount").unwrap(), "4");
    }
}
