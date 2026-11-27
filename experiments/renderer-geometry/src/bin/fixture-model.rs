//! Extract the geometry fixture with the adapter from this checkout.
use std::{env, error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let root = env::args_os()
        .nth(1)
        .ok_or("expected a project directory")?;
    let model = dui_lang_typescript::extract_with_css(
        Path::new(&root),
        Path::new("App.tsx"),
        Some(Path::new("built.css")),
    )?;
    serde_json::to_writer(std::io::stdout().lock(), &model)?;
    Ok(())
}
