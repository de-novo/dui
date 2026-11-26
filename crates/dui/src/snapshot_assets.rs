//! Shared resources of HTML snapshots (docs/en/web-viewer.md#html-cards).
//!
//! A snapshot is self-contained when taken: images and fonts are `data:` URLs. Every screen of
//! an app usually embeds the same fonts and images, so a large resource is stored once per run
//! as `asset-<sha256>.<ext>` next to the snapshots, and the snapshot refers to it by that name.

use sha2::{Digest, Sha256};
use std::{fs, path::Path};

/// Resources at least this large move out of the snapshot.
const MIN_SHARED_BYTES: usize = 8 * 1024;

fn extension(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "font/woff2" | "application/font-woff2" => "woff2",
        "font/woff" | "application/font-woff" => "woff",
        "font/ttf" | "font/sfnt" | "application/x-font-ttf" => "ttf",
        "font/otf" | "application/x-font-opentype" => "otf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        _ => return None,
    })
}

/// The kind of a font or image from its first bytes, for servers that label every resource
/// `application/octet-stream`.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    Some(if starts(b"wOF2") {
        "woff2"
    } else if starts(b"wOFF") {
        "woff"
    } else if starts(&[0, 1, 0, 0]) || starts(b"true") {
        "ttf"
    } else if starts(b"OTTO") {
        "otf"
    } else if starts(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if starts(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        "gif"
    } else if starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        return None;
    })
}

/// Media type of a shared asset name, for serving it.
pub(crate) fn media_type(name: &str) -> Option<&'static str> {
    Some(match name.rsplit_once('.')?.1 {
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// `asset-<64 hex>.<known extension>`.
pub(crate) fn is_asset_name(name: &str) -> bool {
    name.strip_prefix("asset-")
        .and_then(|rest| rest.split_once('.'))
        .is_some_and(|(hash, _)| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        && media_type(name).is_some()
}

/// Move every `data:<mime>;base64,…` resource of at least `MIN_SHARED_BYTES` into
/// `out_dir/asset-<sha256>.<ext>` (written once) and refer to it by name. Returns the rewritten
/// HTML and the asset names it uses, sorted.
pub(crate) fn externalize(html: &str, out_dir: &Path) -> std::io::Result<(String, Vec<String>)> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut out = String::with_capacity(html.len().min(1 << 20));
    let mut names = std::collections::BTreeSet::new();
    let mut rest = html;
    while let Some(at) = rest.find("data:") {
        out.push_str(&rest[..at]);
        let candidate = &rest[at..];
        let parsed = candidate.find(";base64,").and_then(|marker| {
            let mime = &candidate[5..marker];
            let payload = &candidate[marker + 8..];
            let len = payload
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
                .count();
            (len * 3 / 4 >= MIN_SHARED_BYTES && !mime.contains(['"', '\'', ')', ' ', '<', '>']))
                .then(|| (mime, marker + 8 + len, &payload[..len]))
        });
        let shared = parsed.and_then(|(mime, end, data)| {
            let bytes = STANDARD.decode(data).ok()?;
            let ext = extension(mime).or_else(|| sniff(&bytes))?;
            Some((ext, end, bytes))
        });
        match shared {
            Some((ext, end, bytes)) => {
                let name = format!("asset-{:x}.{ext}", Sha256::digest(&bytes));
                let path = out_dir.join(&name);
                if !path.is_file() {
                    fs::write(&path, &bytes)?;
                }
                out.push_str(&name);
                names.insert(name);
                rest = &candidate[end..];
            }
            None => {
                out.push_str("data:");
                rest = &candidate[5..];
            }
        }
    }
    out.push_str(rest);
    Ok((out, names.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    #[test]
    fn large_resources_are_shared_and_small_ones_stay_inline() {
        let dir = tempfile::tempdir().expect("dir");
        let font = STANDARD.encode(vec![7u8; 20_000]);
        let icon = STANDARD.encode(vec![1u8; 100]);
        let html = format!(
            "<style>@font-face{{src:url(\"data:font/woff2;base64,{font}\")}}</style>\
             <img src=\"data:image/png;base64,{icon}\"><i style=\"background:url(data:font/woff2;base64,{font})\"></i>"
        );
        let (out, names) = externalize(&html, dir.path()).expect("split");
        assert_eq!(names.len(), 1, "the same font is stored once");
        let name = &names[0];
        assert!(is_asset_name(name) && name.ends_with(".woff2"));
        assert_eq!(out.matches(name.as_str()).count(), 2);
        assert!(out.contains(&format!("data:image/png;base64,{icon}")));
        assert_eq!(
            fs::read(dir.path().join(name)).expect("asset"),
            vec![7u8; 20_000]
        );
        assert!(out.len() < 1_000);
    }

    #[test]
    fn unlabeled_fonts_are_recognized_by_their_bytes() {
        let dir = tempfile::tempdir().expect("dir");
        let mut ttf = vec![0u8, 1, 0, 0];
        ttf.extend(vec![9u8; 20_000]);
        let unknown = STANDARD.encode(vec![5u8; 20_000]);
        let html = format!(
            "url(\"data:application/octet-stream;base64,{}\") url(\"data:application/octet-stream;base64,{unknown}\")",
            STANDARD.encode(&ttf)
        );
        let (out, names) = externalize(&html, dir.path()).expect("split");
        assert_eq!(names.len(), 1);
        assert!(names[0].ends_with(".ttf"), "{names:?}");
        assert!(out.contains(&unknown), "unrecognized bytes stay inline");
    }
}
