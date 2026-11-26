//! Small copies of captures that map cards draw while they are small on screen
//! (docs/en/web-viewer.md#shared-screen-maps). The PNG stays the evidence; a thumbnail is
//! derived from it, so it is named after the PNG and made again whenever it is missing.

/// Thumbnails are this many pixels wide; captures no wider are not copied.
pub const WIDTH: u32 = 200;

/// The thumbnail's file name for capture `image`.
pub fn name(image: &str) -> String {
    format!("thumb-{image}")
}

/// `png` scaled down to [`WIDTH`] pixels wide, each pixel the average of the area it covers,
/// or `None` when the capture is no wider than that.
pub fn make(png: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let (width, height, rgba) = crate::diff::decode_png_rgba_bytes(png)?;
    if width <= WIDTH || height == 0 {
        return Ok(None);
    }
    let (w, h) = (WIDTH as usize, height as usize);
    let (sw, sh) = (width as usize, height as usize);
    let th = ((h * w).div_ceil(sw)).max(1);
    // Source columns and rows each output pixel averages: [start, end).
    let span = |i: usize, out: usize, src: usize| {
        let start = i * src / out;
        let end = ((i + 1) * src).div_ceil(out).clamp(start + 1, src);
        (start, end)
    };
    let columns: Vec<(usize, usize)> = (0..w).map(|x| span(x, w, sw)).collect();
    let mut out = Vec::with_capacity(w * th * 4);
    let mut sums = vec![[0u32; 4]; w];
    for y in 0..th {
        let (top, bottom) = span(y, th, sh);
        sums.iter_mut().for_each(|s| *s = [0; 4]);
        for row in rgba.chunks_exact(sw * 4).take(bottom).skip(top) {
            for (sum, &(left, right)) in sums.iter_mut().zip(&columns) {
                for pixel in row[left * 4..right * 4].chunks_exact(4) {
                    for (s, &v) in sum.iter_mut().zip(pixel) {
                        *s += u32::from(v);
                    }
                }
            }
        }
        for (sum, &(left, right)) in sums.iter().zip(&columns) {
            let count = u32::try_from((right - left) * (bottom - top))
                .unwrap_or(u32::MAX)
                .max(1);
            out.extend(
                sum.iter()
                    .map(|s| u8::try_from((s + count / 2) / count).unwrap_or(255)),
            );
        }
    }
    let opaque = out.chunks_exact(4).all(|p| p[3] == 255);
    let (color, data) = if opaque {
        let rgb: Vec<u8> = out
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        (png::ColorType::Rgb, rgb)
    } else {
        (png::ColorType::Rgba, out)
    };
    let mut bytes = Vec::new();
    {
        let height = u32::try_from(th).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(&mut bytes, WIDTH, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&data).map_err(|e| e.to_string())?;
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut data = Vec::new();
        for y in 0..height {
            for x in 0..width {
                data.extend(pixel(x, y));
            }
        }
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&data).expect("data");
        drop(writer);
        bytes
    }

    #[test]
    fn a_narrow_capture_has_no_thumbnail() {
        let small = png(WIDTH, 10, |_, _| [1, 2, 3, 255]);
        assert_eq!(make(&small).expect("decodes"), None);
    }

    #[test]
    fn a_wide_capture_shrinks_to_the_thumbnail_width_keeping_its_shape() {
        // Black on the left half, white on the right.
        let wide = png(1280, 2000, |x, _| {
            if x < 640 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        let thumb = make(&wide).expect("decodes").expect("wide enough");
        let (w, h, rgba) = crate::diff::decode_png_rgba_bytes(&thumb).expect("thumbnail decodes");
        assert_eq!((w, h), (WIDTH, 313));
        let row = &rgba[..(WIDTH as usize) * 4];
        assert_eq!(&row[..4], &[0, 0, 0, 255]);
        assert_eq!(&row[row.len() - 4..], &[255, 255, 255, 255]);
        assert_eq!(name("ab12-1280.png"), "thumb-ab12-1280.png");
    }
}
