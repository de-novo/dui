//! Bounded build-stage font resource intake.
//!
//! The default renderer is offline. Remote bytes are resolved only while the source model is
//! extracted, content-hashed, and copied into the same run archive as first-class evidence.
use crate::syntax::{File, children};
use dui_core::source_model as m;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    net::ToSocketAddrs,
    path::{Path, PathBuf},
    time::Duration,
};
use tree_sitter::{Node as Ast, Parser};

const MAX_FONT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOTAL_FONT_BYTES: u64 = 12 * 1024 * 1024;
const MAX_CSS_BYTES: u64 = 1024 * 1024;
const MAX_REMOTE_SHEETS: usize = 16;
const MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;
const FONT_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Clone)]
pub(crate) struct DocumentResources {
    pub external_stylesheets: Vec<String>,
    pub stylesheet_links: BTreeMap<String, m::Location>,
    pub local_stylesheets: Vec<(String, m::Location)>,
    pub html_file: Option<m::SourceFile>,
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before = &rest[..at];
        if before
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
        {
            rest = &rest[at + name.len()..];
            continue;
        }
        rest = &rest[at + name.len()..];
        let rest_trim = rest.trim_start();
        let Some(raw) = rest_trim.strip_prefix('=') else {
            continue;
        };
        let raw = raw.trim_start();
        for mark in ['"', '\''] {
            if let Some(quoted) = raw.strip_prefix(mark)
                && let Some(end) = quoted.find(mark)
            {
                return Some(&quoted[..end]);
            }
        }
        return raw.split_ascii_whitespace().next();
    }
    None
}

fn line_column(src: &str, byte: usize) -> (u32, u32) {
    let before = &src[..byte.min(src.len())];
    let line = before.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
    let column = before.rsplit('\n').next().unwrap_or(before).chars().count() as u32 + 1;
    (line, column)
}

fn html_location(path: &str, src: &str, start: usize, end: usize) -> m::Location {
    let (line, column) = line_column(src, start);
    m::Location {
        file: path.into(),
        line,
        column,
        start: start as u32,
        end: end as u32,
    }
}

fn document_html(project: &Path, artifact: Option<&Path>) -> Option<(String, PathBuf)> {
    let artifact = artifact?;
    if artifact.starts_with("dist/") {
        let path = project.join("dist/index.html");
        if path.is_file() {
            return Some(("dist/index.html".into(), path));
        }
    }
    let path = project.join("index.html");
    path.is_file().then(|| ("index.html".into(), path))
}

/// Read stylesheet links declared by the project HTML without executing it. External URLs keep
/// exact byte provenance in the HTML file; relative links are font-tracing inputs only.
pub(crate) fn document_resources(project: &Path, artifact: Option<&Path>) -> DocumentResources {
    let mut out = DocumentResources {
        external_stylesheets: vec![],
        stylesheet_links: BTreeMap::new(),
        local_stylesheets: vec![],
        html_file: None,
    };
    let Some((html_path, full)) = document_html(project, artifact) else {
        return out;
    };
    let Ok(src) = fs::read_to_string(&full) else {
        return out;
    };
    out.html_file = Some(m::SourceFile {
        path: html_path.clone(),
        language: m::Language::Html,
        sha256: format!("{:x}", Sha256::digest(src.as_bytes())),
        imports: vec![],
    });
    let mut offset = 0usize;
    while let Some(relative) = src[offset..].find('<') {
        let start = offset + relative;
        let Some(close) = src[start..].find('>').map(|end| start + end) else {
            break;
        };
        let tag = &src[start + 1..close];
        let name = tag.split_ascii_whitespace().next().unwrap_or_default();
        if name.eq_ignore_ascii_case("link") {
            let rel = attr_value(tag, "rel").unwrap_or_default();
            let href = attr_value(tag, "href").unwrap_or_default();
            if rel
                .split_ascii_whitespace()
                .any(|v| v.eq_ignore_ascii_case("stylesheet"))
                && !href.is_empty()
            {
                let location = html_location(&html_path, &src, start, close + 1);
                if href.starts_with("https://")
                    || href.starts_with("http://")
                    || href.starts_with("//")
                {
                    let absolute = if let Some(rest) = href.strip_prefix("//") {
                        format!("https://{rest}")
                    } else {
                        href.to_owned()
                    };
                    if !out.external_stylesheets.iter().any(|v| v == &absolute) {
                        out.external_stylesheets.push(absolute.clone());
                        out.stylesheet_links.insert(absolute, location);
                    }
                } else if let Some(path) = normalize_local_path(&html_path, href) {
                    out.local_stylesheets.push((path, location));
                }
            }
        }
        offset = close + 1;
    }
    out
}

fn unquote_css(raw: &str) -> String {
    let raw = raw.trim();
    raw.strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| raw.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(raw)
        .trim()
        .to_owned()
}

fn parse_font_src(raw: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(at) = rest.find("url(") {
        let after = &rest[at + 4..];
        let Some(end) = after.find(')') else { break };
        let url = unquote_css(&after[..end]);
        let mut format = None;
        let tail = after[end + 1..].trim_start();
        if let Some(args) = tail.strip_prefix("format(")
            && let Some(close) = args.find(')')
        {
            format = Some(unquote_css(&args[..close]));
        }
        if !url.is_empty() {
            out.push((url, format));
        }
        rest = &after[end + 1..];
    }
    out
}

fn percent_decode_path(raw: &str) -> Option<String> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            out.push((hex(bytes[index + 1])? << 4) | hex(bytes[index + 2])?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn normalize_local_path(css_path: &str, url: &str) -> Option<String> {
    let clean = url.split(['?', '#']).next()?;
    let clean = percent_decode_path(clean)?;
    if clean.is_empty() || clean.contains('\\') || clean.contains('\0') {
        return None;
    }
    let mut parts = Vec::new();
    if clean.starts_with('/') {
        let public_root = css_path
            .split('/')
            .next()
            .filter(|part| matches!(*part, "dist" | "build"));
        if let Some(root) = public_root {
            parts.push(root);
        } else if css_path == "index.html" {
            // A root-absolute HTML link is project-root relative.
        } else {
            return None;
        }
    } else if let Some(parent) = Path::new(css_path).parent() {
        for part in parent.iter() {
            parts.push(part.to_str()?);
        }
    }
    for part in clean.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part if part.bytes().all(|b| b.is_ascii_graphic() || b >= 0x80) => parts.push(part),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn font_magic_matches(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "woff2" => bytes.starts_with(b"wOF2"),
        "woff" => bytes.starts_with(b"wOFF"),
        "ttf" => bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true"),
        "otf" => bytes.starts_with(b"OTTO"),
        _ => false,
    }
}

fn font_diagnostic(
    diagnostics: &mut Vec<m::Diagnostic>,
    location: &m::Location,
    message: impl Into<String>,
) {
    diagnostics.push(m::Diagnostic {
        code: "unsupported-font-resource".into(),
        message: message.into(),
        location: Some(location.clone()),
    });
}

fn font_cache_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DUI_FONT_CACHE") {
        return (!path.is_empty()).then_some(PathBuf::from(path));
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(home).join(".dui/font-cache"));
    }
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".dui/font-cache"))
}

fn cache_key(url: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("font-fetch-v1\n{url}").as_bytes())
    )
}

fn cache_size(root: &Path) -> u64 {
    let blobs = root.join("blobs");
    let Ok(entries) = fs::read_dir(blobs) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum()
}

struct CachedResource {
    bytes: Vec<u8>,
    sha256: String,
    final_url: String,
}

fn cached_resource(root: &Path, url: &str) -> Option<CachedResource> {
    let key = cache_key(url);
    let meta_path = root.join("index").join(format!("{key}.json"));
    let meta = fs::read_to_string(meta_path).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&meta).ok()?;
    let sha256 = meta["sha256"].as_str()?.to_owned();
    let final_url = meta["finalUrl"].as_str().unwrap_or(url).to_owned();
    let bytes = fs::read(root.join("blobs").join(&sha256)).ok()?;
    if format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return None;
    }
    Some(CachedResource {
        bytes,
        sha256,
        final_url,
    })
}

fn store_cached_resource(root: &Path, url: &str, resource: &CachedResource) -> Result<(), String> {
    fs::create_dir_all(root.join("blobs"))
        .and_then(|_| fs::create_dir_all(root.join("index")))
        .map_err(|e| e.to_string())?;
    let len = resource.bytes.len() as u64;
    if cache_size(root) + len > MAX_CACHE_BYTES {
        return Err("bounded dui font cache is full".into());
    }
    let blob = root.join("blobs").join(&resource.sha256);
    if !blob.exists() {
        let tmp = root.join("blobs").join(format!(".{}-tmp", resource.sha256));
        fs::write(&tmp, &resource.bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &blob).map_err(|e| e.to_string())?;
    }
    let key = cache_key(url);
    let meta = serde_json::json!({
        "version": 1,
        "url": url,
        "finalUrl": resource.final_url,
        "sha256": resource.sha256,
        "bytes": resource.bytes.len(),
    });
    fs::write(
        root.join("index").join(format!("{key}.json")),
        serde_json::to_vec(&meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            let o = ip.octets();
            !(o[0] == 0
                || o[0] == 10
                || o[0] == 127
                || (o[0] == 169 && o[1] == 254)
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 192 && o[1] == 0)
                || (o[0] == 198 && (o[1] == 51 || o[1] == 18))
                || (o[0] == 203 && o[1] == 0 && o[2] == 113)
                || o[0] >= 224)
        }
        std::net::IpAddr::V6(ip) => {
            let s = ip.segments();
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] == 0x2001 && s[1] == 0x0db8))
        }
    }
}

/// Small RFC 3986 resolver for the HTTPS font URLs accepted by intake. It deliberately rejects
/// non-ASCII authorities rather than invoking a large IDNA dependency; such URLs remain evidence.
#[derive(Clone, Debug)]
struct Url {
    text: String,
}

impl Url {
    fn parse(raw: &str) -> Result<Self, String> {
        let text = raw.split('#').next().unwrap_or(raw).to_owned();
        let Some(scheme_end) = text.find("://") else {
            return Err("font URL must be absolute or resolvable against a stylesheet URL".into());
        };
        let scheme = &text[..scheme_end];
        if scheme.is_empty()
            || !scheme.bytes().enumerate().all(|(i, b)| {
                b.is_ascii_alphabetic()
                    || (i > 0 && (b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')))
            })
        {
            return Err("font URL has an invalid scheme".into());
        }
        let authority_start = scheme_end + 3;
        let authority_end = text[authority_start..]
            .find(['/', '?'])
            .map(|i| authority_start + i)
            .unwrap_or(text.len());
        let authority = &text[authority_start..authority_end];
        if authority.is_empty() {
            return Err("font URL has no authority".into());
        }
        let hostport = authority.rsplit('@').next().unwrap_or(authority);
        if hostport.is_empty() || !hostport.is_ascii() {
            return Err("non-ASCII font hosts are not materialized".into());
        }
        let host = if let Some(rest) = hostport.strip_prefix('[') {
            let Some(end) = rest.find(']') else {
                return Err("font URL has an invalid IPv6 authority".into());
            };
            &rest[..end]
        } else {
            hostport.split(':').next().unwrap_or("")
        };
        if host.is_empty() {
            return Err("font URL has no host".into());
        }
        if let Some(port) = port_text(hostport)
            && (port.is_empty()
                || !port.bytes().all(|b| b.is_ascii_digit())
                || port.parse::<u16>().is_err())
        {
            return Err("font URL has an invalid port".into());
        }
        Ok(Self { text })
    }

    fn as_str(&self) -> &str {
        &self.text
    }

    fn scheme(&self) -> &str {
        &self.text[..self.text.find("://").unwrap_or(0)]
    }

    fn authority(&self) -> &str {
        let start = self.text.find("://").map(|i| i + 3).unwrap_or(0);
        let end = self.text[start..]
            .find(['/', '?'])
            .map(|i| start + i)
            .unwrap_or(self.text.len());
        &self.text[start..end]
    }

    fn userinfo(&self) -> Option<&str> {
        let authority = self.authority();
        authority.rsplit_once('@').map(|(userinfo, _)| userinfo)
    }

    fn username(&self) -> &str {
        self.userinfo()
            .map(|u| u.split(':').next().unwrap_or(""))
            .unwrap_or("")
    }

    fn password(&self) -> Option<&str> {
        self.userinfo()
            .and_then(|u| u.split_once(':'))
            .map(|(_, password)| password)
    }

    fn host_str(&self) -> Option<&str> {
        let hostport = self.authority().rsplit('@').next()?;
        if let Some(rest) = hostport.strip_prefix('[') {
            return rest.find(']').map(|end| &rest[..end]);
        }
        Some(hostport.split(':').next().unwrap_or(""))
    }

    fn port_or_known_default(&self) -> Option<u16> {
        port_text(self.authority().rsplit('@').next()?).and_then(|p| p.parse().ok())
    }

    fn origin(&self) -> String {
        let start = self.text.find("://").map(|i| i + 3).unwrap_or(0);
        let end = self.text[start..]
            .find(['/', '?'])
            .map(|i| start + i)
            .unwrap_or(self.text.len());
        self.text[..end].to_owned()
    }

    fn path_query(&self) -> &str {
        let start = self.text.find("://").map(|i| i + 3).unwrap_or(0);
        let Some(offset) = self.text[start..].find(['/', '?']) else {
            return "/";
        };
        &self.text[start + offset..]
    }

    fn path(&self) -> &str {
        self.path_query().split('?').next().unwrap_or("/")
    }

    fn join(&self, reference: &str) -> Result<Self, String> {
        let reference = reference.split('#').next().unwrap_or(reference);
        if reference
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic())
            && reference[..reference.len().min(32)]
                .find(':')
                .is_some_and(|colon| {
                    reference[..colon]
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
                })
        {
            return Self::parse(reference);
        }
        if let Some(rest) = reference.strip_prefix("//") {
            return Self::parse(&format!("{}://{rest}", self.scheme()));
        }
        if reference.starts_with('/') {
            return Self::parse(&format!(
                "{}{}",
                self.origin(),
                normalize_path_query(reference)
            ));
        }
        if reference.starts_with('?') {
            let path = self.path_query().split('?').next().unwrap_or("/");
            return Self::parse(&format!("{}{}{}", self.origin(), path, reference));
        }
        let path = self.path_query().split('?').next().unwrap_or("/");
        let directory = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
        let merged = if reference.starts_with('?') {
            format!("{path}{reference}")
        } else {
            format!("{directory}/{reference}")
        };
        Self::parse(&format!(
            "{}{}",
            self.origin(),
            normalize_path_query(&merged)
        ))
    }
}

fn port_text(hostport: &str) -> Option<&str> {
    if hostport.starts_with('[') {
        let end = hostport.find(']')?;
        return hostport[end + 1..].strip_prefix(':');
    }
    hostport.split_once(':').map(|(_, port)| port)
}

fn normalize_path_query(input: &str) -> String {
    let (path, query) = input.split_once('?').map_or((input, ""), |(p, q)| (p, q));
    let trailing = path.ends_with('/');
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    let mut normalized = format!("/{}", segments.join("/"));
    if trailing && normalized != "/" {
        normalized.push('/');
    }
    if !query.is_empty() {
        normalized.push('?');
        normalized.push_str(query);
    }
    normalized
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

fn public_https_url(raw: &str, base: Option<&Url>) -> Result<Url, String> {
    let url = if let Some(rest) = raw.strip_prefix("//") {
        let scheme = base.map(|u| u.scheme()).unwrap_or("https");
        Url::parse(&format!("{scheme}://{rest}")).map_err(|e| e.to_string())?
    } else if let Some(base) = base {
        base.join(raw).map_err(|e| e.to_string())?
    } else {
        Url::parse(raw).map_err(|e| e.to_string())?
    };
    if url.scheme() != "https" {
        return Err("only public HTTPS font resources are materialized".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("font URLs must not carry credentials".into());
    }
    let Some(host) = url.host_str() else {
        return Err("font URL has no host".into());
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return Err("local font hosts are not public".into());
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>()
        && !public_ip(ip)
    {
        return Err("private or reserved font IP addresses are not public".into());
    }
    Ok(url)
}

fn check_public_dns(url: &Url) -> Result<(), String> {
    let Some(host) = url.host_str() else {
        return Err("font URL has no host".into());
    };
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("public font host cannot be resolved: {e}"))?
        .collect();
    if addresses.is_empty() || addresses.iter().any(|addr| !public_ip(addr.ip())) {
        return Err("font host does not resolve only to public addresses".into());
    }
    Ok(())
}

fn fetch_public(url: &Url, max_bytes: u64) -> Result<CachedResource, String> {
    let url_text = url.to_string();
    if let Some(root) = font_cache_root()
        && let Some(hit) = cached_resource(&root, &url_text)
    {
        return Ok(hit);
    }
    check_public_dns(url)?;
    let config = ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(12)))
        .user_agent(FONT_UA)
        .accept_encoding("")
        .proxy(None)
        .build();
    let agent: ureq::Agent = config.into();
    let mut current = url.clone();
    for redirects in 0..=3 {
        let mut response = agent
            .get(current.as_str())
            .call()
            .map_err(|e| format!("{}: {e}", current.as_str()))?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            if redirects == 3 {
                return Err("font resource redirects exceed the bound".into());
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("font redirect has no location")?;
            current = public_https_url(location, Some(&current))?;
            check_public_dns(&current)?;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!("font resource returned HTTP {status}"));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| format!("bounded font download failed: {e}"))?;
        if bytes.is_empty() || bytes.len() as u64 > max_bytes {
            return Err("font resource exceeds its byte bound".into());
        }
        let resource = CachedResource {
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
            final_url: current.to_string(),
        };
        if let Some(root) = font_cache_root() {
            store_cached_resource(&root, &url_text, &resource)?;
        }
        return Ok(resource);
    }
    Err("font resource redirects exceed the bound".into())
}

fn normalize_text(raw: &str) -> Option<String> {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then(|| text.chars().take(512).collect())
}

fn sfnt_table<'a>(data: &'a [u8], wanted: &[u8; 4]) -> Option<&'a [u8]> {
    if data.len() < 12 || data.starts_with(b"wOF2") || data.starts_with(b"wOFF") {
        return None;
    }
    let count = u16::from_be_bytes([data[4], data[5]]) as usize;
    for index in 0..count {
        let at = 12 + index.checked_mul(16)?;
        let record = data.get(at..at + 16)?;
        if &record[..4] == wanted {
            let offset = u32::from_be_bytes(record[8..12].try_into().ok()?) as usize;
            let length = u32::from_be_bytes(record[12..16].try_into().ok()?) as usize;
            return data.get(offset..offset.checked_add(length)?);
        }
    }
    None
}

fn read_base128(data: &[u8], pos: &mut usize) -> Option<u32> {
    let mut value = 0u32;
    for index in 0..5 {
        let byte = *data.get(*pos)?;
        *pos += 1;
        if index == 0 && byte == 0x80 {
            return None;
        }
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

const WOFF2_TAGS: &[&[u8; 4]] = &[
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post", b"cvt ", b"fpgm",
    b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT", b"EBLC", b"gasp", b"hdmx", b"kern",
    b"LTSH", b"PCLT", b"VDMX", b"vhea", b"vmtx", b"BASE", b"GDEF", b"GPOS", b"GSUB", b"EBSC",
    b"JSTF", b"MATH", b"CBDT", b"CBLC", b"COLR", b"CPAL", b"SVG ", b"sbix", b"acnt", b"avar",
    b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc", b"feat", b"fmtx", b"fvar", b"gvar", b"hsty",
    b"just", b"lcar", b"mort", b"morx", b"opbd", b"prop", b"trak", b"Zapf", b"Silf", b"Glat",
    b"Gloc", b"Feat", b"Sill",
];

fn woff2_table(data: &[u8], wanted: &[u8; 4]) -> Option<Vec<u8>> {
    if data.len() < 48 || !data.starts_with(b"wOF2") {
        return None;
    }
    let count = u16::from_be_bytes([data[12], data[13]]) as usize;
    let compressed_len = u32::from_be_bytes(data[20..24].try_into().ok()?) as usize;
    let mut pos = 48;
    let mut tables: Vec<([u8; 4], usize, usize)> = Vec::new();
    let mut block_offset = 0usize;
    for _ in 0..count {
        let flag = *data.get(pos)?;
        pos += 1;
        let tag_index = flag & 0x3f;
        let transform = flag >> 6;
        let tag: [u8; 4] = if tag_index == 0x3f {
            let custom: &[u8; 4] = data.get(pos..pos + 4)?.try_into().ok()?;
            pos += 4;
            *custom
        } else {
            **WOFF2_TAGS.get(tag_index as usize)?
        };
        let original = read_base128(data, &mut pos)? as usize;
        // In WOFF2 the glyf/loca pair is transformed at version 0 and untransformed at
        // version 3; every other table is transformed when its version is non-zero. The
        // decompressed concatenation has no padding between table entries.
        let is_transformed = if matches!(&tag, b"glyf" | b"loca") {
            transform == 0
        } else {
            transform != 0
        };
        let transformed = if is_transformed {
            Some(read_base128(data, &mut pos)? as usize)
        } else {
            None
        };
        let stored = transformed.unwrap_or(original);
        tables.push((tag, block_offset, stored));
        block_offset = block_offset.checked_add(stored)?;
    }
    let compressed = data.get(pos..pos.checked_add(compressed_len)?)?;
    let mut decoder = brotli_decompressor::Decompressor::new(compressed, 4096);
    let mut block = Vec::new();
    decoder.read_to_end(&mut block).ok()?;
    let (_, offset, length) = tables.into_iter().find(|(tag, _, _)| *tag == *wanted)?;
    let end = offset.checked_add(length)?;
    Some(block.get(offset..end)?.to_vec())
}

fn name_table_license(table: &[u8]) -> Option<String> {
    if table.len() < 6 {
        return None;
    }
    let count = u16::from_be_bytes([table[2], table[3]]) as usize;
    let strings = u16::from_be_bytes([table[4], table[5]]) as usize;
    let mut license = None;
    let mut license_url = None;
    for index in 0..count {
        let at = 6 + index.checked_mul(12)?;
        let record = table.get(at..at + 12)?;
        let platform = u16::from_be_bytes(record[0..2].try_into().ok()?);
        let name_id = u16::from_be_bytes(record[6..8].try_into().ok()?);
        let length = u16::from_be_bytes(record[8..10].try_into().ok()?) as usize;
        let offset = u16::from_be_bytes(record[10..12].try_into().ok()?) as usize;
        let bytes = table
            .get(strings.checked_add(offset)?..strings.checked_add(offset)?.checked_add(length)?)?;
        let text = if matches!(platform, 0 | 3) {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|part| u16::from_be_bytes([part[0], part[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            bytes.iter().map(|b| *b as char).collect()
        };
        if name_id == 13 {
            license = normalize_text(&text).or(license);
        } else if name_id == 14 {
            license_url = normalize_text(&text).or(license_url);
        }
    }
    license.or(license_url)
}

fn font_license(bytes: &[u8], extension: &str) -> Option<String> {
    match extension {
        "ttf" | "otf" => sfnt_table(bytes, b"name").and_then(name_table_license),
        "woff2" => woff2_table(bytes, b"name").and_then(|table| name_table_license(&table)),
        _ => None,
    }
}

fn font_extension(url: &str, format: Option<&str>, bytes: &[u8]) -> Option<String> {
    let clean = url.split(['?', '#']).next().unwrap_or(url);
    let extension = clean
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    let detected = if bytes.starts_with(b"wOF2") {
        Some("woff2")
    } else if bytes.starts_with(b"wOFF") {
        Some("woff")
    } else if bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true") {
        Some("ttf")
    } else if bytes.starts_with(b"OTTO") {
        Some("otf")
    } else {
        None
    };
    let declared = extension.or_else(|| format.map(str::to_ascii_lowercase));
    match (declared, detected) {
        (Some(declared), Some(detected)) if declared == detected => Some(declared),
        (None, Some(detected)) => Some(detected.into()),
        (Some(declared), None) if matches!(declared.as_str(), "woff2" | "woff" | "ttf" | "otf") => {
            Some(declared)
        }
        _ => None,
    }
}

fn local_font_source(
    project: &Path,
    project_root: &Path,
    css_path: &str,
    url: &str,
    format: Option<String>,
    location: &m::Location,
    diagnostics: &mut Vec<m::Diagnostic>,
    cache: &mut BTreeMap<String, Option<m::FontSource>>,
    total_bytes: &mut u64,
) -> Option<m::FontSource> {
    let path = normalize_local_path(css_path, url)?;
    if let Some(cached) = cache.get(&path) {
        return cached.clone();
    }
    let mut invalid = |reason: &str, diagnostics: &mut Vec<m::Diagnostic>| {
        font_diagnostic(diagnostics, location, format!("{path}: {reason}"));
        cache.insert(path.clone(), None);
        None
    };
    let Some(extension) = path.rsplit_once('.').map(|(_, ext)| ext.to_owned()) else {
        return invalid("font path has no extension", diagnostics);
    };
    if !matches!(extension.as_str(), "woff2" | "woff" | "ttf" | "otf") {
        return invalid("font extension is not woff2, woff, ttf or otf", diagnostics);
    }
    let full = project.join(&path);
    let Ok(full) = full.canonicalize() else {
        return invalid("font file is unavailable", diagnostics);
    };
    if !full.starts_with(project_root) || !full.is_file() {
        return invalid(
            "font path escapes the project or is not a file",
            diagnostics,
        );
    }
    let Ok(bytes) = fs::read(&full) else {
        return invalid("font file could not be read", diagnostics);
    };
    let len = bytes.len() as u64;
    if len == 0 || len > MAX_FONT_BYTES || !font_magic_matches(&extension, &bytes) {
        return invalid(
            "font bytes are empty, oversized or do not match their extension",
            diagnostics,
        );
    }
    if *total_bytes + len > MAX_TOTAL_FONT_BYTES {
        return invalid(
            "project font assets exceed the bounded total size",
            diagnostics,
        );
    }
    *total_bytes += len;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let license = font_license(&bytes, &extension);
    let source = m::FontSource::Asset {
        path: path.clone(),
        asset: format!("asset-{sha256}.{extension}"),
        sha256,
        bytes: len,
        format,
        license,
    };
    cache.insert(path, Some(source.clone()));
    Some(source)
}

fn remote_font_source(
    raw_url: &str,
    base: Option<&Url>,
    format: Option<String>,
    location: &m::Location,
    diagnostics: &mut Vec<m::Diagnostic>,
    cache: &mut BTreeMap<String, Option<m::FontSource>>,
    total_bytes: &mut u64,
) -> m::FontSource {
    let url_for_evidence = if let Some(base) = base {
        base.join(raw_url)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| raw_url.to_owned())
    } else {
        raw_url.to_owned()
    };
    let evidence = |asset: Option<String>,
                    sha256: Option<String>,
                    bytes: Option<u64>,
                    retrieved_url: Option<String>,
                    license: Option<String>| {
        m::FontSource::External {
            url: url_for_evidence.clone(),
            format: format.clone(),
            asset,
            sha256,
            bytes,
            retrieved_url,
            license,
        }
    };
    if let Some(cached) = cache.get(&url_for_evidence) {
        return cached
            .clone()
            .unwrap_or_else(|| evidence(None, None, None, None, None));
    }
    let mut fail = |reason: String, diagnostics: &mut Vec<m::Diagnostic>| {
        font_diagnostic(
            diagnostics,
            location,
            format!("{url_for_evidence}: {reason}"),
        );
        cache.insert(url_for_evidence.clone(), None);
        evidence(None, None, None, None, None)
    };
    let url = match public_https_url(raw_url, base) {
        Ok(url) => url,
        Err(reason) => return fail(reason, diagnostics),
    };
    let resource = match fetch_public(&url, MAX_FONT_BYTES) {
        Ok(resource) => resource,
        Err(reason) => return fail(reason, diagnostics),
    };
    let len = resource.bytes.len() as u64;
    let Some(extension) = font_extension(url.path(), format.as_deref(), &resource.bytes) else {
        return fail(
            "font extension or bytes are not woff2, woff, ttf or otf".into(),
            diagnostics,
        );
    };
    if !font_magic_matches(&extension, &resource.bytes) {
        return fail(
            "font bytes do not match their extension".into(),
            diagnostics,
        );
    }
    if *total_bytes + len > MAX_TOTAL_FONT_BYTES {
        return fail(
            "project font assets exceed the bounded total size".into(),
            diagnostics,
        );
    }
    *total_bytes += len;
    let license = font_license(&resource.bytes, &extension);
    let source = evidence(
        Some(format!("asset-{}.{extension}", resource.sha256)),
        Some(resource.sha256),
        Some(len),
        Some(resource.final_url),
        license,
    );
    cache.insert(url_for_evidence.clone(), Some(source.clone()));
    source
}

struct CssSheet {
    id: String,
    src: String,
    tree: tree_sitter::Tree,
    fallback_location: Option<m::Location>,
    sha256: Option<String>,
    base_url: Option<Url>,
}

impl CssSheet {
    fn parse(
        id: String,
        src: String,
        fallback_location: Option<m::Location>,
        base_url: Option<Url>,
    ) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_css::LANGUAGE.into())
            .map_err(|e| e.to_string())?;
        let tree = parser
            .parse(src.as_bytes(), None)
            .ok_or("parse interrupted")?;
        let sha256 = base_url
            .as_ref()
            .map(|_| format!("{:x}", Sha256::digest(src.as_bytes())));
        Ok(Self {
            id,
            src,
            tree,
            fallback_location,
            sha256,
            base_url,
        })
    }

    fn text(&self, node: Ast<'_>) -> &str {
        &self.src[node.byte_range()]
    }

    fn location(&self, node: Ast<'_>) -> m::Location {
        if let Some(fallback) = &self.fallback_location {
            return fallback.clone();
        }
        let (line, column) = line_column(&self.src, node.start_byte());
        m::Location {
            file: self.id.clone(),
            line,
            column,
            start: node.start_byte() as u32,
            end: node.end_byte() as u32,
        }
    }
}

fn import_urls(sheet: &CssSheet) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![sheet.tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "import_statement" {
            let text = sheet.text(node);
            if let Some(rest) = text.trim_start().strip_prefix("@import") {
                let rest = rest.trim_start();
                let raw = if let Some(url) =
                    rest.strip_prefix("url(").and_then(|v| v.split(')').next())
                {
                    unquote_css(url)
                } else {
                    unquote_css(rest.split([';', ' ']).next().unwrap_or_default())
                };
                if !raw.is_empty() {
                    out.push(raw);
                }
            }
        }
        stack.extend(children(node));
    }
    out
}

#[derive(Clone)]
pub(crate) struct FontIntake {
    pub faces: Vec<m::FontFace>,
    pub extra_files: Vec<m::SourceFile>,
}

#[allow(clippy::too_many_arguments)]
fn intake_sheet(
    project: &Path,
    project_root: &Path,
    sheet: &CssSheet,
    depth: usize,
    diagnostics: &mut Vec<m::Diagnostic>,
    faces: &mut Vec<m::FontFace>,
    extra_files: &mut Vec<m::SourceFile>,
    seen_sheets: &mut BTreeSet<String>,
    font_cache: &mut BTreeMap<String, Option<m::FontSource>>,
    total_bytes: &mut u64,
) {
    if depth > 4 || seen_sheets.len() >= MAX_REMOTE_SHEETS {
        font_diagnostic(
            diagnostics,
            &sheet.location(sheet.tree.root_node()),
            format!("{}: stylesheet import bound exceeded", sheet.id),
        );
        return;
    }
    let mut stack = vec![sheet.tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "at_rule"
            && children(node)
                .first()
                .is_some_and(|part| sheet.text(*part) == "@font-face")
        {
            let parts = children(node);
            let location = sheet.location(node);
            let mut family = None;
            let mut src = None;
            let mut descriptors: BTreeMap<String, String> = BTreeMap::new();
            for declaration in parts
                .iter()
                .flat_map(|part| children(*part))
                .filter(|part| part.kind() == "declaration")
            {
                let text = sheet.text(declaration);
                let Some((name, value)) = text.split_once(':') else {
                    continue;
                };
                let value = value.trim().trim_end_matches(';').trim();
                match name.trim() {
                    "font-family" => family = Some(unquote_css(value)),
                    "src" => src = Some(value.to_owned()),
                    "font-style" | "font-weight" | "font-stretch" | "font-display" => {
                        descriptors.insert(
                            name.trim().replace("font-", ""),
                            value.split_whitespace().collect::<Vec<_>>().join(" "),
                        );
                    }
                    "unicode-range" => {
                        descriptors.insert(
                            "unicode-range".into(),
                            value.split_whitespace().collect::<Vec<_>>().join(" "),
                        );
                    }
                    _ => {}
                }
            }
            let (Some(family), Some(src)) = (family.filter(|v| !v.is_empty()), src) else {
                continue;
            };
            let mut sources = Vec::new();
            for (url, format) in parse_font_src(&src) {
                if url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("//")
                    || sheet.base_url.is_some() && Url::parse(&url).is_ok()
                {
                    sources.push(remote_font_source(
                        &url,
                        sheet.base_url.as_ref(),
                        format,
                        &location,
                        diagnostics,
                        font_cache,
                        total_bytes,
                    ));
                } else if url.starts_with("data:") {
                    font_diagnostic(
                        diagnostics,
                        &location,
                        format!("{family}: embedded data font URLs are not extracted"),
                    );
                } else if sheet.base_url.is_some() {
                    sources.push(remote_font_source(
                        &url,
                        sheet.base_url.as_ref(),
                        format,
                        &location,
                        diagnostics,
                        font_cache,
                        total_bytes,
                    ));
                } else if let Some(source) = local_font_source(
                    project,
                    project_root,
                    &sheet.id,
                    &url,
                    format,
                    &location,
                    diagnostics,
                    font_cache,
                    total_bytes,
                ) {
                    if !sources.contains(&source) {
                        sources.push(source);
                    }
                }
            }
            if sources.is_empty() {
                continue;
            }
            faces.push(m::FontFace {
                id: format!("{}#font-face-{}", sheet.id, node.start_byte()),
                family,
                style: descriptors.get("style").cloned(),
                weight: descriptors.get("weight").cloned(),
                stretch: descriptors.get("stretch").cloned(),
                display: descriptors.get("display").cloned(),
                unicode_range: descriptors.get("unicode-range").cloned(),
                stylesheet: Some(sheet.id.clone()),
                stylesheet_sha256: sheet.sha256.clone(),
                sources,
                origin: m::Origin::Syntax,
                location,
            });
            continue;
        }
        stack.extend(children(node));
    }

    for import in import_urls(sheet) {
        if import.starts_with("https://")
            || import.starts_with("http://")
            || import.starts_with("//")
            || sheet.base_url.is_some()
        {
            let url = match public_https_url(&import, sheet.base_url.as_ref()) {
                Ok(url) => url,
                Err(reason) => {
                    font_diagnostic(
                        diagnostics,
                        &sheet.location(sheet.tree.root_node()),
                        format!("{import}: {reason}"),
                    );
                    continue;
                }
            };
            let key = url.to_string();
            if !seen_sheets.insert(key.clone()) {
                continue;
            }
            match fetch_public(&url, MAX_CSS_BYTES) {
                Ok(resource) => match String::from_utf8(resource.bytes) {
                    Ok(src) => match CssSheet::parse(
                        key.clone(),
                        src,
                        Some(sheet.location(sheet.tree.root_node())),
                        Some(url),
                    ) {
                        Ok(imported) => intake_sheet(
                            project,
                            project_root,
                            &imported,
                            depth + 1,
                            diagnostics,
                            faces,
                            extra_files,
                            seen_sheets,
                            font_cache,
                            total_bytes,
                        ),
                        Err(reason) => font_diagnostic(
                            diagnostics,
                            &sheet.location(sheet.tree.root_node()),
                            format!("{key}: {reason}"),
                        ),
                    },
                    Err(_) => font_diagnostic(
                        diagnostics,
                        &sheet.location(sheet.tree.root_node()),
                        format!("{key}: stylesheet is not UTF-8"),
                    ),
                },
                Err(reason) => font_diagnostic(
                    diagnostics,
                    &sheet.location(sheet.tree.root_node()),
                    format!("{key}: {reason}"),
                ),
            }
        } else if sheet.base_url.is_none()
            && let Some(path) = normalize_local_path(&sheet.id, &import)
        {
            if !seen_sheets.insert(path.clone()) {
                continue;
            }
            let full = project.join(&path);
            let Ok(full) = full.canonicalize() else {
                font_diagnostic(
                    diagnostics,
                    &sheet.location(sheet.tree.root_node()),
                    format!("{path}: imported stylesheet is unavailable"),
                );
                continue;
            };
            if !full.starts_with(project_root) || !full.is_file() {
                font_diagnostic(
                    diagnostics,
                    &sheet.location(sheet.tree.root_node()),
                    format!("{path}: imported stylesheet escapes the project"),
                );
                continue;
            }
            match fs::read_to_string(&full) {
                Ok(src) if src.len() as u64 <= MAX_CSS_BYTES => {
                    extra_files.push(m::SourceFile {
                        path: path.clone(),
                        language: m::Language::Css,
                        sha256: format!("{:x}", Sha256::digest(src.as_bytes())),
                        imports: vec![],
                    });
                    match CssSheet::parse(path.clone(), src, None, None) {
                        Ok(imported) => intake_sheet(
                            project,
                            project_root,
                            &imported,
                            depth + 1,
                            diagnostics,
                            faces,
                            extra_files,
                            seen_sheets,
                            font_cache,
                            total_bytes,
                        ),
                        Err(reason) => font_diagnostic(
                            diagnostics,
                            &sheet.location(sheet.tree.root_node()),
                            format!("{path}: {reason}"),
                        ),
                    }
                }
                Ok(_) => font_diagnostic(
                    diagnostics,
                    &sheet.location(sheet.tree.root_node()),
                    format!("{path}: stylesheet exceeds the byte bound"),
                ),
                Err(e) => font_diagnostic(
                    diagnostics,
                    &sheet.location(sheet.tree.root_node()),
                    format!("{path}: {e}"),
                ),
            }
        }
    }
}

/// Extract font facts from built/source sheets and the public font stylesheets those facts link to.
pub(crate) fn font_faces(
    project: &Path,
    files: &BTreeMap<String, std::sync::Arc<File>>,
    artifact: Option<&str>,
    resources: &DocumentResources,
    diagnostics: &mut Vec<m::Diagnostic>,
) -> FontIntake {
    let Ok(project_root) = project.canonicalize() else {
        return FontIntake {
            faces: vec![],
            extra_files: vec![],
        };
    };
    let mut initial: Vec<String> = if let Some(path) = artifact {
        vec![path.to_owned()]
    } else {
        files
            .keys()
            .filter(|path| path.ends_with(".css") && !path.ends_with(".module.css"))
            .cloned()
            .collect()
    };
    for (path, _) in &resources.local_stylesheets {
        if !initial.iter().any(|existing| existing == path) {
            initial.push(path.clone());
        }
    }
    initial.sort();
    initial.dedup();
    let mut faces = Vec::new();
    let mut extra_files = Vec::new();
    let mut font_cache = BTreeMap::new();
    let mut total_bytes = 0u64;
    let mut seen_sheets = BTreeSet::new();
    for path in initial {
        if !seen_sheets.insert(path.clone()) {
            continue;
        }
        let (src, tree_owned) = if let Some(file) = files.get(&path) {
            (file.src.clone(), Some(file.tree.clone()))
        } else {
            match fs::read_to_string(project.join(&path)) {
                Ok(src) if src.len() as u64 <= MAX_CSS_BYTES => {
                    extra_files.push(m::SourceFile {
                        path: path.clone(),
                        language: m::Language::Css,
                        sha256: format!("{:x}", Sha256::digest(src.as_bytes())),
                        imports: vec![],
                    });
                    (src, None)
                }
                _ => continue,
            }
        };
        let sheet = if let Some(tree) = tree_owned {
            CssSheet {
                id: path.clone(),
                src,
                tree,
                fallback_location: None,
                sha256: None,
                base_url: None,
            }
        } else {
            match CssSheet::parse(path.clone(), src, None, None) {
                Ok(sheet) => sheet,
                Err(reason) => {
                    font_diagnostic(
                        diagnostics,
                        &m::Location {
                            file: path.clone(),
                            line: 1,
                            column: 1,
                            start: 0,
                            end: 0,
                        },
                        format!("{path}: {reason}"),
                    );
                    continue;
                }
            }
        };
        intake_sheet(
            project,
            &project_root,
            &sheet,
            0,
            diagnostics,
            &mut faces,
            &mut extra_files,
            &mut seen_sheets,
            &mut font_cache,
            &mut total_bytes,
        );
    }
    for url in &resources.external_stylesheets {
        let Ok(parsed) = public_https_url(url, None) else {
            font_diagnostic(
                diagnostics,
                &resources.stylesheet_links[url],
                format!("{url}: only public HTTPS font stylesheets are materialized"),
            );
            continue;
        };
        if !seen_sheets.insert(parsed.to_string()) {
            continue;
        }
        match fetch_public(&parsed, MAX_CSS_BYTES) {
            Ok(resource) => match String::from_utf8(resource.bytes) {
                Ok(src) => match CssSheet::parse(
                    parsed.to_string(),
                    src,
                    Some(resources.stylesheet_links[url].clone()),
                    Some(parsed),
                ) {
                    Ok(sheet) => intake_sheet(
                        project,
                        &project_root,
                        &sheet,
                        0,
                        diagnostics,
                        &mut faces,
                        &mut extra_files,
                        &mut seen_sheets,
                        &mut font_cache,
                        &mut total_bytes,
                    ),
                    Err(reason) => font_diagnostic(
                        diagnostics,
                        &resources.stylesheet_links[url],
                        format!("{url}: {reason}"),
                    ),
                },
                Err(_) => font_diagnostic(
                    diagnostics,
                    &resources.stylesheet_links[url],
                    format!("{url}: stylesheet is not UTF-8"),
                ),
            },
            Err(reason) => font_diagnostic(
                diagnostics,
                &resources.stylesheet_links[url],
                format!("{url}: {reason}"),
            ),
        }
    }
    faces.sort_by(|a, b| a.id.cmp(&b.id));
    faces.dedup_by(|a, b| a.id == b.id);
    extra_files.sort_by(|a, b| a.path.cmp(&b.path));
    extra_files.dedup_by(|a, b| a.path == b.path);
    FontIntake { faces, extra_files }
}

/// Read one materialized cached font by digest for archive publication. The caller still
/// re-validates the bytes against the source-model metadata before writing them.
pub fn cached_font_bytes(sha256: &str) -> Result<Option<Vec<u8>>, String> {
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("invalid font sha256".into());
    }
    let Some(root) = font_cache_root() else {
        return Ok(None);
    };
    match fs::read(root.join("blobs").join(sha256)) {
        Ok(bytes) if format!("{:x}", Sha256::digest(&bytes)) == sha256 => Ok(Some(bytes)),
        Ok(_) => Err("cached font bytes do not match their content address".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
