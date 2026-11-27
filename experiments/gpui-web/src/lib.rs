use gpui::{AppContext, Entity, IntoElement, ParentElement, Render, Styled};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use serde_json::Value;
use std::{borrow::Cow, cell::RefCell};
use wasm_bindgen::prelude::*;

thread_local! {
    static APP: RefCell<Option<gpui::ApplicationHandle>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = recordProbeValue)]
    fn record_value(text: &str);
}

struct Card {
    label: String,
    field: Entity<InputState>,
    _subscription: gpui::Subscription,
}

impl Render for Card {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .bg(gpui::rgb(0x237659))
            .child(self.label.clone())
            .child(Input::new(&self.field))
    }
}

#[wasm_bindgen]
pub fn run(model: &str, font: Vec<u8>) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let model: Value = serde_json::from_str(model).map_err(|e| e.to_string())?;
    let root = &model["components"][0]["root"];
    if model["schemaVersion"] != 1
        || root["tag"] != "div"
        || root["children"][0]["tag"] != "label"
        || root["children"][1]["tag"] != "input"
    {
        return Err("expected the label and input probe model".into());
    }
    let label = root["children"][0]["children"][0]["parts"][0]["value"]
        .as_str()
        .ok_or("missing literal label")?
        .to_owned();
    gpui_kit::platform::web_init();
    let platform = gpui_kit::web::WebPlatform::new_with_backend_and_font_fallback(
        false,
        gpui_kit::web::WebBackendPreference::Auto,
        gpui_kit::web::CanvasFontFallback::EmojiAndCjk,
    );
    let application = gpui::Application::with_platform(std::rc::Rc::new(platform));
    let handle = application.run_embedded(move |cx| {
        gpui_kit::init(cx);
        cx.text_system()
            .add_fonts(vec![Cow::Owned(font)])
            .expect("probe font");
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            cx.new(|cx| {
                let field = cx.new(|cx| InputState::new(window, cx));
                let subscription = cx.subscribe_in(
                    &field,
                    window,
                    |_: &mut Card, field, event: &InputEvent, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            record_value(&field.read(cx).value());
                        }
                    },
                );
                Card {
                    label,
                    field,
                    _subscription: subscription,
                }
            })
        })
        .expect("probe window");
        cx.activate(true);
    });
    APP.with(|app| app.replace(Some(handle)));
    Ok(())
}
