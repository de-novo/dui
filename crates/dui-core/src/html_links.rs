//! Links of a page read from its HTML without running it (docs/en/web-viewer.md#shared-screen-maps).
//!
//! A map that starts no browser reads `<a href>` from the markup the server sends. Links that only
//! scripts add are not seen; the map states that limitation instead of guessing them. The rules
//! follow the browser runtime's link collection: same origin, no downloads, no asset files.

/// A page as its HTML describes it: the title and the same-origin links in document order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub title: Option<String>,
    pub links: Vec<String>,
}

/// Reads `html` served at `page_path` (path and query, such as `/docs/?tab=a`). Links come back
/// as path, query and hash, each once, in document order. `origin` is the page's own origin, such
/// as `http://127.0.0.1:4173`, so absolute links to it count as same-origin.
pub fn read(html: &str, page_path: &str, origin: &str) -> Page {
    let mut page = Page::default();
    let mut base = page_path.to_owned();
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        if let Some(after) = rest.strip_prefix("!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            continue;
        }
        let name_end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        let name = rest[..name_end].to_ascii_lowercase();
        let Some(close) = tag_end(&rest[name_end..]) else {
            break;
        };
        let attributes = &rest[name_end..name_end + close];
        rest = &rest[name_end + close + 1..];
        match name.as_str() {
            // Their contents are not markup.
            "script" | "style" | "template" | "textarea" | "title" => {
                let closing = format!("</{name}");
                let end = find_ignore_case(rest, &closing).unwrap_or(rest.len());
                if name == "title" && page.title.is_none() {
                    let text = decode(rest[..end].trim());
                    if !text.is_empty() {
                        page.title = Some(text);
                    }
                }
                rest = &rest[end..];
            }
            "base" => {
                if let Some(href) = attribute(attributes, "href")
                    && let Some(resolved) = resolve(&href, page_path, origin)
                {
                    base = resolved;
                }
            }
            "a" => {
                if attribute(attributes, "download").is_some() {
                    continue;
                }
                if let Some(href) = attribute(attributes, "href")
                    && let Some(link) = resolve(&href, &base, origin)
                    && !is_asset(&link)
                    && !page.links.contains(&link)
                {
                    page.links.push(link);
                }
            }
            _ => {}
        }
    }
    page
}

/// The index of the `>` that ends a tag, skipping quoted attribute values.
fn tag_end(tag: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in tag.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle))
}

/// The decoded value of attribute `name`, or an empty string for a bare attribute.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let mut rest = attributes.trim_start_matches('/');
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
        if rest.is_empty() {
            return None;
        }
        let key_end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        let key = &rest[..key_end];
        rest = rest[key_end..].trim_start();
        let value = if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let (value, next) = match after.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let body = &after[1..];
                    let end = body.find(q).unwrap_or(body.len());
                    (&body[..end], body.get(end + 1..).unwrap_or(""))
                }
                _ => {
                    let end = after
                        .find(|c: char| c.is_ascii_whitespace())
                        .unwrap_or(after.len());
                    (&after[..end], &after[end..])
                }
            };
            rest = next;
            value
        } else {
            ""
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(decode(value));
        }
    }
}

/// Character references that appear in URLs and titles.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `href` resolved against `base` (a path with query), as path, query and hash when it stays on
/// `origin`; `None` for other origins, other schemes and fragment-only links.
fn resolve(href: &str, base: &str, origin: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    let reference = if lower.starts_with("http://") || lower.starts_with("https://") {
        let rest = href.get(origin.len()..)?;
        if !href[..origin.len()].eq_ignore_ascii_case(origin)
            || !(rest.is_empty() || rest.starts_with(['/', '?', '#']))
        {
            return None;
        }
        if rest.is_empty() { "/" } else { rest }
    } else if href.starts_with("//")
        || lower
            .split_once(':')
            .is_some_and(|(scheme, _)| !scheme.contains(['/', '?', '#']))
    {
        return None;
    } else {
        href
    };
    let (base_path, base_query) = base.split_once('?').unwrap_or((base, ""));
    let base_path = base_path.split('#').next().unwrap_or("/");
    let (path, tail) = match reference.find(['?', '#']) {
        Some(at) => (&reference[..at], &reference[at..]),
        None => (reference, ""),
    };
    let joined = if path.starts_with('/') {
        path.to_owned()
    } else if path.is_empty() {
        base_path.to_owned()
    } else {
        let dir = &base_path[..base_path.rfind('/').map_or(0, |i| i + 1)];
        format!("{dir}{path}")
    };
    let mut segments: Vec<&str> = Vec::new();
    let parts: Vec<&str> = joined.split('/').collect();
    for (i, segment) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        match *segment {
            "." => {
                if last {
                    segments.push("");
                }
            }
            ".." => {
                if segments.len() > 1 {
                    segments.pop();
                }
                if last {
                    segments.push("");
                }
            }
            s => segments.push(s),
        }
    }
    let mut resolved = segments.join("/");
    if !resolved.starts_with('/') {
        resolved.insert(0, '/');
    }
    if resolved.starts_with("//") {
        return None;
    }
    if path.is_empty() && !tail.starts_with('?') && !base_query.is_empty() {
        resolved.push('?');
        resolved.push_str(base_query);
    }
    resolved.push_str(tail);
    Some(resolved)
}

fn is_asset(link: &str) -> bool {
    const ASSETS: &[&str] = &[
        "json", "xml", "rss", "atom", "pdf", "zip", "svg", "png", "jpg", "jpeg", "webp", "gif",
        "csv", "txt", "ico", "woff", "woff2", "avif", "mp3", "mp4", "css", "js", "mjs",
    ];
    let path = link.split(['?', '#']).next().unwrap_or(link);
    path.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.ends_with('/')
            && !ext.contains('/')
            && ASSETS.contains(&ext.to_ascii_lowercase().as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "http://127.0.0.1:4173";

    #[test]
    fn reads_same_origin_links_in_document_order_once() {
        let html = r##"<!doctype html><html><head><title>Orders &amp; stock</title></head><body>
<!-- <a href="/commented">no</a> -->
<nav><a href="/">Home</a> <a class=x href=cli.html>CLI</a> <a href='../up/'>Up</a></nav>
<a href="/cli.html">again</a> <a href="https://example.com/x">out</a> <a href="mailto:a@b.c">m</a>
<a href="#top">top</a> <a href="?tab=b">tab</a> <a href="//cdn.example.com/a">cdn</a>
<a href="http://127.0.0.1:4173/abs?x=1&amp;y=2#h">abs</a> <a href="/report.pdf">pdf</a>
<a href="/file" download>dl</a> <a>no href</a>
<script>document.write('<a href="/from-script">s</a>')</script>
<a href="javascript:void(0)">js</a> <a href="./same/./x/../y">dots</a>
</body></html>"##;
        let page = read(html, "/docs/index.html?tab=a", ORIGIN);
        assert_eq!(page.title.as_deref(), Some("Orders & stock"));
        assert_eq!(
            page.links,
            [
                "/",
                "/docs/cli.html",
                "/up/",
                "/cli.html",
                "/docs/index.html?tab=b",
                "/abs?x=1&y=2#h",
                "/docs/same/y",
            ]
        );
    }

    #[test]
    fn a_base_element_changes_where_relative_links_point() {
        let html = r#"<base href="/app/"><a href="settings">S</a><a href="/root">R</a>"#;
        assert_eq!(read(html, "/", ORIGIN).links, ["/app/settings", "/root"]);
    }

    #[test]
    fn a_page_without_markup_links_has_none() {
        let spa = r#"<div id="root"></div><script type="module" src="/main.js"></script>"#;
        assert_eq!(read(spa, "/", ORIGIN), Page::default());
    }

    #[test]
    fn other_ports_and_hosts_are_other_origins() {
        let html =
            r#"<a href="http://127.0.0.1:9999/x">p</a><a href="http://127.0.0.1:41730/y">q</a>"#;
        assert!(read(html, "/", ORIGIN).links.is_empty());
    }
}
