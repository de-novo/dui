use gpui::{prelude::*, *};
use std::time::Duration;
struct Probe {
    distance: f32,
    seconds: f32,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let distance = self.distance;
        div().size_full().bg(rgb(0xffffff)).child(
            div()
                .w(px(40.))
                .h(px(30.))
                .bg(rgb(0x135c52))
                .with_animation(
                    "track",
                    Animation::new(Duration::from_secs_f32(self.seconds))
                        .repeat()
                        .with_easing(gpui::linear),
                    move |el, t| el.ml(px(20. + distance * t)).mt(px(40.)),
                ),
        )
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let json: serde_json::Value = serde_json::from_reader(std::fs::File::open(
        std::env::args().nth(1).ok_or("expected motion.json")?,
    )?)?;
    let track = &json["tracks"][0];
    let distance = track["frames"][1]["x"].as_f64().ok_or("endpoint")? as f32;
    let seconds = track["durationMs"].as_f64().ok_or("duration")? as f32 / 1000.;
    gpui_platform::application().run(move |cx| {
        let bounds = Bounds::centered(None, size(px(320.), px(120.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("dui research GPUI".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Probe { distance, seconds }),
        )
        .expect("research window");
        cx.activate(true);
    });
    Ok(())
}
