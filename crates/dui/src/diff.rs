//! Capture-to-capture diff with changed source file attribution and PNG pixel-diff overlays.

use crate::{Reply, failure, sqlite_store::SqliteStore};
use dui_core::{
    diff::{DiffCellSnapshot, compute_rgba_diff, diff_captures},
    model::CacheKey,
    storage::CacheRepository,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufWriter,
    path::Path,
};

fn read_json_file(path: &Path) -> Result<Value, (u8, Value)> {
    let bytes = fs::read(path).map_err(|e| failure(2, &format!("{}: {e}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|e| failure(2, &format!("{}: {e}", path.display())))
}

fn optional_json_file(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn load_cell_snapshots(
    dir: &Path,
    store: Option<&SqliteStore>,
) -> Result<(Option<Value>, Vec<DiffCellSnapshot>), (u8, Value)> {
    let report = read_json_file(&dir.join("report.json"))?;
    let snapshot = optional_json_file(&dir.join("snapshot.json"));
    let report_hashes: BTreeMap<String, String> = report["sourceFileHashes"]
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    let cells = report["cells"]
        .as_array()
        .ok_or_else(|| failure(2, "report.json missing cells array"))?;

    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        let path = cell["path"]
            .as_str()
            .or_else(|| report["path"].as_str())
            .unwrap_or("/")
            .to_owned();
        let scenario = cell["scenario"].as_str().unwrap_or("default").to_owned();
        let state = cell["state"].as_str().unwrap_or("").to_owned();
        let width = cell["width"].as_u64().unwrap_or(0) as u32;
        let locale = cell["locale"].as_str().map(str::to_owned);
        let status = cell["status"].as_str().unwrap_or("unverified").to_owned();
        let reason = cell["reason"].as_str().map(str::to_owned);
        let image = cell["image"].as_str().map(str::to_owned);
        let cache_key = cell["cache"]["key"].as_str().map(str::to_owned);

        let mut source_files = BTreeMap::new();
        if let Some(list) = cell["sourceFiles"].as_array() {
            for file in list.iter().filter_map(Value::as_str) {
                if let Some(hash) = report_hashes.get(file) {
                    source_files.insert(file.to_owned(), hash.clone());
                }
            }
        }
        if source_files.is_empty()
            && let Some(key_str) = cache_key.as_deref()
            && let Some(db) = store
            && let Ok(key) = CacheKey::parse(key_str)
            && let Ok(Some(entry)) = db.get_cache(&key)
        {
            for (file, digest) in &entry.inputs().source_files {
                source_files.insert(file.clone(), digest.as_str().to_owned());
            }
        }
        if source_files.is_empty() && !report_hashes.is_empty() {
            source_files = report_hashes.clone();
        }

        out.push(DiffCellSnapshot {
            path,
            scenario,
            state,
            width,
            locale,
            status,
            reason,
            image,
            cache_key,
            source_files,
        });
    }
    Ok((snapshot, out))
}

fn decode_png_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let buf_size = reader
        .output_buffer_size()
        .ok_or_else(|| "png buffer size overflow".to_owned())?;
    let mut raw = vec![0u8; buf_size];
    let info = reader.next_frame(&mut raw).map_err(|e| e.to_string())?;
    let bytes = &raw[..info.buffer_size()];
    let pixel_count = (info.width as usize).saturating_mul(info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in bytes.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in bytes.chunks_exact(2) {
                out.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &g in bytes {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        png::ColorType::Indexed => {
            return Err("unexpected indexed png after EXPAND".to_owned());
        }
    };
    Ok((info.width, info.height, rgba))
}

fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    Ok(())
}

fn overlay_filename(before_img: &str, after_img: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(before_img.as_bytes());
    hasher.update(b":");
    hasher.update(after_img.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("{}-diff.png", &digest[..16])
}

pub fn compare(before: &Path, after: &Path, overlay_dir: Option<&Path>) -> Reply {
    let store = crate::archive::home_dir()
        .ok()
        .and_then(|home| SqliteStore::open(&home).ok().map(|(s, _)| s));

    let (before_snap, before_cells) = load_cell_snapshots(before, store.as_ref())?;
    let (after_snap, after_cells) = load_cell_snapshots(after, store.as_ref())?;

    let mut diff = diff_captures(&before_cells, &after_cells);

    for screen in &mut diff.screens {
        for cell in &mut screen.cells {
            if !cell.supports_pixel_diff() {
                cell.pixel_diff = None;
                continue;
            }
            let Some(before_img) = cell.before.as_ref().and_then(|s| s.image.as_deref()) else {
                continue;
            };
            let Some(after_img) = cell.after.as_ref().and_then(|s| s.image.as_deref()) else {
                continue;
            };
            let before_path = before.join(before_img);
            let after_path = after.join(after_img);
            let Ok((bw, bh, brgba)) = decode_png_rgba(&before_path) else {
                continue;
            };
            let Ok((aw, ah, argba)) = decode_png_rgba(&after_path) else {
                continue;
            };
            let (mut metrics, overlay_rgba) = compute_rgba_diff(bw, bh, &brgba, aw, ah, &argba);
            if let Some(dir) = overlay_dir {
                let name = overlay_filename(before_img, after_img);
                let out_path = dir.join(&name);
                if write_png_rgba(&out_path, metrics.width, metrics.height, &overlay_rgba).is_ok() {
                    metrics.overlay_image = Some(name);
                }
            }
            cell.pixel_diff = Some(metrics);
        }
    }

    Ok(json!({
        "before": {
            "path": before.display().to_string(),
            "snapshot": before_snap,
        },
        "after": {
            "path": after.display().to_string(),
            "snapshot": after_snap,
        },
        "summary": diff.summary,
        "screens": diff.screens,
    }))
}
