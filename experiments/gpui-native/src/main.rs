use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap, fontdb,
};
use gpui::{prelude::*, *};
use image::{Frame, Rgba, RgbaImage};
use serde_json::{Value, json};
use smallvec::smallvec;
use std::{fs, path::PathBuf, sync::Arc};
use taffy::prelude as t;
use taffy::prelude::TaffyMaxContent;

struct Label {
    text: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    size: f32,
    line: f32,
    spacing: f32,
}
struct Probe {
    model: PathBuf,
    revision: Vec<u8>,
    fonts: FontSystem,
    family: String,
    paint: SwashCache,
    ios: bool,
    width: u32,
    height: u32,
    native: bool,
    host_family: String,
    frame: Option<Arc<RenderImage>>,
    labels: Vec<Label>,
}
impl Probe {
    fn redraw(&mut self, v: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let stack = v["stack"].as_bool().ok_or("stack flag")?;
        let values = v["labels"].as_array().ok_or("labels")?;
        if values.len() != if stack { 4 } else { 1 } {
            return Err("unsupported scene".into());
        }
        let mut layout = t::TaffyTree::<()>::new();
        layout.disable_rounding();
        let mut nodes = Vec::new();
        let mut runs = Vec::new();
        for (index, raw) in values.iter().enumerate() {
            let text = raw.as_str().ok_or("literal text")?.to_owned();
            let size = if stack && index == 0 {
                20.
            } else if self.ios && !stack {
                17.
            } else {
                16.
            };
            let line: f32 = if self.ios {
                (size * 1.2_f32).ceil()
            } else if stack {
                (size * 1.18_f32).ceil()
            } else {
                24.
            };
            let spacing = if !stack && !self.ios { 0.5 } else { 0. };
            let cap = (stack && index == 3).then_some(180.);
            let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, line));
            buffer.set_size(cap, None);
            buffer.set_wrap(Wrap::Word);
            buffer.set_text(
                &text,
                &Attrs::new()
                    .family(Family::Name(&self.family))
                    .letter_spacing(spacing / size),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.fonts, false);
            let w =
                cap.unwrap_or_else(|| buffer.layout_runs().map(|r| r.line_w).fold(0., f32::max));
            let h = buffer.layout_runs().count() as f32 * line;
            nodes.push(layout.new_leaf(t::Style {
                size: t::Size {
                    width: t::length(w),
                    height: t::length(h),
                },
                ..Default::default()
            })?);
            runs.push((
                buffer,
                Label {
                    text,
                    x: 0.,
                    y: 0.,
                    w,
                    h,
                    size,
                    line,
                    spacing,
                },
            ));
        }
        let content = if stack {
            let row = layout.new_with_children(
                t::Style {
                    gap: t::Size {
                        width: t::length(12.),
                        height: t::zero(),
                    },
                    ..Default::default()
                },
                &nodes[1..3],
            )?;
            layout.new_with_children(
                t::Style {
                    size: t::Size {
                        width: t::length(204.),
                        height: t::auto(),
                    },
                    flex_direction: t::FlexDirection::Column,
                    align_items: t::AlignItems::START,
                    gap: t::Size {
                        height: t::length(8.),
                        width: t::zero(),
                    },
                    ..Default::default()
                },
                &[nodes[0], row, nodes[3]],
            )?
        } else {
            nodes[0]
        };
        let root = layout.new_with_children(
            t::Style {
                size: t::Size {
                    width: t::length(self.width as f32),
                    height: t::length(self.height as f32),
                },
                align_items: t::AlignItems::CENTER,
                justify_content: t::JustifyContent::CENTER,
                ..Default::default()
            },
            &[content],
        )?;
        layout.compute_layout(root, t::Size::MAX_CONTENT)?;
        let mut pixels = RgbaImage::from_pixel(self.width, self.height, Rgba([255; 4]));
        self.labels.clear();
        for (node, (mut buffer, mut label)) in nodes.into_iter().zip(runs) {
            let mut parent = Some(node);
            while let Some(n) = parent {
                let point = layout.layout(n)?.location;
                label.x += point.x;
                label.y += point.y;
                parent = layout.parent(n);
            }
            if !self.native {
                buffer.draw(
                    &mut self.fonts,
                    &mut self.paint,
                    Color::rgb(0, 0, 0),
                    |x, y, w, h, color| {
                        for j in 0..h {
                            for i in 0..w {
                                let xx = label.x.round() as i32 + x + i as i32;
                                let yy = label.y.round() as i32 + y + j as i32;
                                if xx >= 0
                                    && yy >= 0
                                    && let Some(pixel) =
                                        pixels.get_pixel_mut_checked(xx as u32, yy as u32)
                                {
                                    let value =
                                        ((pixel[0] as u16 * (255 - color.a()) as u16) / 255) as u8;
                                    *pixel = Rgba([value, value, value, 255]);
                                }
                            }
                        }
                    },
                );
            }
            self.labels.push(label);
        }
        if !self.native {
            self.frame = Some(Arc::new(RenderImage::new(smallvec![Frame::new(pixels)])));
        }
        Ok(())
    }
}
impl Render for Probe {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Ok(bytes) = fs::read(&self.model)
            && self.revision != bytes
        {
            let start = std::time::Instant::now();
            if let Ok(v) = serde_json::from_slice::<Value>(&bytes)
                && self.redraw(&v).is_ok()
            {
                self.revision = bytes;
                println!(
                    "{}",
                    json!({"revision":v["revision"],"drawUs":start.elapsed().as_micros(),"viewport":[f32::from(window.viewport_size().width),f32::from(window.viewport_size().height)],"scale":window.scale_factor()})
                );
            }
        }
        window.request_animation_frame();
        let mut el = div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(0xffffff));
        if self.native {
            for l in &self.labels {
                let mut label = div()
                    .absolute()
                    .left(px(l.x))
                    .top(px(l.y))
                    .w(px(l.w + 0.01))
                    .h(px(l.h))
                    .text_size(px(l.size))
                    .line_height(px(l.line))
                    .text_color(rgb(0))
                    .font_family(self.host_family.clone());
                let _ = l.spacing; // The default host text control uses GPUI's own tracking.
                label = label.child(l.text.clone());
                el = el.child(label);
            }
        } else if let Some(frame) = &self.frame {
            el = el.child(
                img(frame.clone())
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .w(px(self.width as f32))
                    .h(px(self.height as f32)),
            );
        }
        el
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("MODEL FONT ios|android WIDTH HEIGHT cosmic|host".into());
    }
    let bytes = fs::read(&args[1])?;
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes.clone());
    let family = db.faces().next().ok_or("font missing")?.families[0]
        .0
        .clone();
    let (width, height) = (args[3].parse::<u32>()?, args[4].parse::<u32>()?);
    let ios = args[2] == "ios";
    let native = args[5] == "host";
    let model = PathBuf::from(&args[0]);
    let host_family = if ios {
        ".SystemUIFont".to_string()
    } else {
        family.clone()
    };
    gpui_platform::application().run(move |cx| {
        if !ios {
            cx.text_system()
                .add_fonts(vec![std::borrow::Cow::Owned(bytes)])
                .expect("platform font load");
        }
        let bounds = Bounds::new(
            point(px(0.), px(0.)),
            size(px(width as f32), px(height as f32)),
        );
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Probe {
                    model,
                    revision: Vec::new(),
                    fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
                    family,
                    paint: SwashCache::new(),
                    ios,
                    width,
                    height,
                    native,
                    host_family,
                    frame: None,
                    labels: vec![],
                })
            },
        )
        .expect("native preview window");
        cx.activate(true);
    });
    Ok(())
}
