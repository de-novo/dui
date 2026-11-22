use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextAppRoute {
    pub path: String,
    pub file: String,
    pub is_client_component: bool,
    pub is_dynamic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_note: Option<String>,
}

/// Convert a project-relative Next.js App Router file path into its URL route path.
/// Supports `app/**/page.{tsx,ts,jsx,js}` and `src/app/**/page.{tsx,ts,jsx,js}`,
/// strips `(group)` route groups, ignores `_private` folders and parallel `@slots`,
/// and converts `[param]` / `[...slug]` segments to `:param` / `*slug`.
pub fn parse_app_router_path(rel_file: &str) -> Option<(String, bool)> {
    let normalized = rel_file.replace('\\', "/");
    let stripped = normalized
        .strip_prefix("src/app/")
        .or_else(|| normalized.strip_prefix("app/"))?;
    let parts: Vec<&str> = stripped.split('/').collect();
    let file_name = *parts.last()?;
    if !matches!(file_name, "page.tsx" | "page.ts" | "page.jsx" | "page.js") {
        return None;
    }
    let dir_segments = &parts[..parts.len() - 1];
    let mut url_segments = Vec::new();
    let mut is_dynamic = false;
    for &seg in dir_segments {
        if seg.is_empty() || seg.starts_with('_') || seg.starts_with('@') {
            return None;
        }
        if seg.starts_with('(') && seg.ends_with(')') {
            // Route group — transparent in URL path
            continue;
        }
        if let Some(inner) = seg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            is_dynamic = true;
            let clean = inner.trim_start_matches('[').trim_end_matches(']');
            if let Some(rest) = clean.strip_prefix("...") {
                url_segments.push(format!("*{rest}"));
            } else {
                url_segments.push(format!(":{clean}"));
            }
        } else {
            url_segments.push(seg.to_owned());
        }
    }
    let path = if url_segments.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", url_segments.join("/"))
    };
    Some((path, is_dynamic))
}

/// Detect whether a Next.js source file begins with a `"use client"` / `'use client'` directive.
pub fn is_use_client_source(source: &str) -> bool {
    let mut in_block_comment = false;
    for raw_line in source.lines() {
        let mut line = raw_line.trim();
        if in_block_comment {
            if let Some(end) = line.find("*/") {
                in_block_comment = false;
                line = line[end + 2..].trim();
            } else {
                continue;
            }
        }
        while line.starts_with("/*") {
            if let Some(end) = line.find("*/") {
                line = line[end + 2..].trim();
            } else {
                in_block_comment = true;
                line = "";
                break;
            }
        }
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let cleaned = line.trim_end_matches(';').trim();
        return cleaned == "\"use client\"" || cleaned == "'use client'";
    }
    false
}

pub fn build_next_route(rel_file: &str, source: &str) -> Option<NextAppRoute> {
    let (path, is_dynamic) = parse_app_router_path(rel_file)?;
    let is_client_component = is_use_client_source(source);
    let boundary_note = (!is_client_component).then(|| {
        "server-component: direct client-hook state injection unavailable; rendered via Next.js server".to_owned()
    });
    Some(NextAppRoute {
        path,
        file: rel_file.replace('\\', "/"),
        is_client_component,
        is_dynamic,
        boundary_note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_app_router_pages_route_groups_and_dynamic_segments() {
        assert_eq!(
            parse_app_router_path("app/page.tsx"),
            Some(("/".into(), false))
        );
        assert_eq!(
            parse_app_router_path("src/app/(marketing)/pricing/page.tsx"),
            Some(("/pricing".into(), false))
        );
        assert_eq!(
            parse_app_router_path("app/orders/[id]/page.tsx"),
            Some(("/orders/:id".into(), true))
        );
        assert_eq!(
            parse_app_router_path("app/docs/[...slug]/page.tsx"),
            Some(("/docs/*slug".into(), true))
        );
        assert_eq!(parse_app_router_path("app/layout.tsx"), None);
        assert_eq!(parse_app_router_path("app/_components/page.tsx"), None);
        assert_eq!(parse_app_router_path("app/@modal/login/page.tsx"), None);
    }

    #[test]
    fn detects_use_client_directive_and_reports_server_component_boundary() {
        let client_route = build_next_route(
            "app/pricing/page.tsx",
            "// banner\n\"use client\";\nexport default function Page() {}",
        )
        .expect("client route");
        assert!(client_route.is_client_component);
        assert_eq!(client_route.boundary_note, None);

        let server_route = build_next_route(
            "app/page.tsx",
            "export default async function Home() { return <main />; }",
        )
        .expect("server route");
        assert!(!server_route.is_client_component);
        assert!(
            server_route
                .boundary_note
                .as_deref()
                .is_some_and(|n| n.contains("server-component"))
        );
    }
}
