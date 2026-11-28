use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap, fontdb,
};
use serde_json::{Value, json};
use std::{error::Error, fs, io};
fn main() -> Result<(), Box<dyn Error>> {
    let v: Value = serde_json::from_reader(io::stdin())?;
    let text = v["text"].as_str().ok_or("text")?;
    let mut db = fontdb::Database::new();
    db.load_font_data(fs::read(v["font"].as_str().ok_or("font")?)?);
    let face = db.faces().next().ok_or("font face missing")?;
    let family = face.families[0].0.clone();
    let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
    let size = v["size"].as_f64().ok_or("size")? as f32;
    let attrs = Attrs::new()
        .family(Family::Name(&family))
        .weight(Weight(v["weight"].as_u64().ok_or("weight")? as u16));
    let mut buffer = Buffer::new(&mut fonts, Metrics::new(size, size * 1.5));
    let mut ret = Vec::new();
    for width in [660., 120., 180., 240.] {
        buffer.set_wrap(Wrap::Word);
        buffer.set_size(Some(width), None);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fonts, false);
        let mut lines = Vec::new();
        let mut baseln = 0.;
        for run in buffer.layout_runs() {
            let lo = run.glyphs.iter().map(|g| g.start).min().unwrap_or(0);
            let hi = run.glyphs.iter().map(|g| g.end).max().unwrap_or(0);
            lines.push(json!({"text":text[lo..hi].trim(),"width":run.line_w}));
            baseln = run.line_y;
        }
        let lines: Vec<_> = buffer
            .layout_runs()
            .map(|run| {
                let start = run.glyphs.iter().map(|g| g.start).min().unwrap_or(0);
                let end = run.glyphs.iter().map(|g| g.end).max().unwrap_or(start);
                json!({"text":text[start..end].trim(),"line_w":run.line_w})
            })
            .collect();
        if width == 660. {
            let mut pixels = vec![255u8; 660 * 160];
            let mut cache = SwashCache::new();
            let dy = 125 - baseln.round() as i32;
            buffer.draw(
                &mut fonts,
                &mut cache,
                Color::rgb(0, 0, 0),
                |x, y, w, h, color| {
                    for j in 0..h {
                        for i in 0..w {
                            let xx = x + i as i32 + 4;
                            let yy = y + j as i32 + dy;
                            if (0..660).contains(&xx) && (0..160).contains(&yy) {
                                let dst = &mut pixels[(yy * 660 + xx) as usize];
                                *dst = ((*dst as u16 * (255 - color.a()) as u16) / 255) as u8;
                            }
                        }
                    }
                },
            );
            fs::write(v["gray"].as_str().ok_or("gray")?, pixels)?;
        }
        ret.push(lines);
    }
    println!("{}", serde_json::to_string(&ret)?);
    Ok(())
}
