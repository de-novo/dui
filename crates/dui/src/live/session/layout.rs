use super::{Backend, Reply, Session, Value, invalid, io, json};
use crate::live::MAX_LAYOUT_SIZE;

impl Session {
    pub(super) fn layout(&mut self, request: Option<&Value>) -> Reply {
        if !matches!(self.backend, Backend::Web(_)) {
            return Err(invalid("resize-unavailable"));
        }
        self.tick()?;
        let mut args = String::new();
        if let Some(request) = request {
            // Bound the backing bitmap allocated by Chromium. Omitted axes keep their measured size.
            let axis = |key: &str| -> Result<Option<u64>, (u8, Value)> {
                request
                    .get(key)
                    .map(|v| {
                        v.as_u64()
                            .filter(|n| (1..=MAX_LAYOUT_SIZE).contains(n))
                            .ok_or_else(|| invalid("invalid-layout-size"))
                    })
                    .transpose()
            };
            let width = axis("width")?;
            let height = axis("height")?;
            if width.is_none() && height.is_none() {
                return Err(invalid("missing-layout-size"));
            }
            if let Some(container) = request.get("container") {
                let id = container
                    .as_u64()
                    .ok_or_else(|| invalid("invalid-container"))?;
                let js = |n: Option<u64>| {
                    n.map(|v| v.to_string())
                        .unwrap_or_else(|| "undefined".into())
                };
                args = format!("{id},{},{}", js(width), js(height));
            } else {
                let viewport = self.eval_web("({width:innerWidth,height:innerHeight})")?;
                let Backend::Web(web) = &mut self.backend else {
                    return Err(invalid("resize-unavailable"));
                };
                web.bench.browser.borrow_mut().call(Some(&web.page), "Emulation.setDeviceMetricsOverride", json!({
                    "width": width.map_or_else(|| viewport["width"].clone(), Value::from),
                    "height": height.map_or_else(|| viewport["height"].clone(), Value::from),
                    "deviceScaleFactor":1, "mobile":false
                })).map_err(io)?;
                if let Some(width) = width {
                    web.viewport_width = width;
                }
            }
        }
        let mut layout = self.eval_web(&format!("globalThis.__duiRuntime.liveLayout({args})"))?;
        if let Some(error) = layout["error"].as_str() {
            return Err(invalid(error));
        }
        layout["maxSize"] = json!(MAX_LAYOUT_SIZE);
        Ok(layout)
    }

    pub(super) fn fill(&mut self, request: &Value) -> Reply {
        if !matches!(self.backend, Backend::Web(_)) {
            return Err(invalid("resize-unavailable"));
        }
        self.tick()?;
        let id = request["field"]
            .as_u64()
            .ok_or_else(|| invalid("invalid-field"))?;
        let value = request["text"]
            .as_str()
            .ok_or_else(|| invalid("missing-text"))?;
        let field = self.eval_web(&format!(
            "globalThis.__duiRuntime.prepareLiveField({id},{})",
            json!(value)
        ))?;
        match field.as_str() {
            Some("selected") => return self.layout(None),
            Some("focused") => {}
            _ => return Err(invalid("field-or-option-unavailable")),
        }
        let Backend::Web(web) = &mut self.backend else {
            return Err(invalid("resize-unavailable"));
        };
        if value.is_empty() {
            for kind in ["rawKeyDown", "keyUp"] {
                web.bench.browser.borrow_mut().call(Some(&web.page), "Input.dispatchKeyEvent", json!({"type":kind,"key":"Backspace","code":"Backspace","windowsVirtualKeyCode":8})).map_err(io)?;
            }
        } else {
            web.bench
                .browser
                .borrow_mut()
                .call(Some(&web.page), "Input.insertText", json!({"text":value}))
                .map_err(io)?;
        }
        self.layout(None)
    }
}
