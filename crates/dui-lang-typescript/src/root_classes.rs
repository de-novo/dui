//! Deterministically recover default document-root class state for simple provider patterns.
use crate::syntax::File;
use std::collections::{BTreeMap, BTreeSet};

fn first_array_string(src: &str, array: &str) -> Option<String> {
    let declaration = src.find(&format!("const {array}"))?;
    let open = src[declaration..].find('[')? + declaration;
    let rest = &src[open + 1..];
    for quote in ['\'', '"'] {
        if let Some(start) = rest.find(quote) {
            let value = &rest[start + 1..];
            if let Some(end) = value.find(quote) {
                let value = &value[..end];
                if !value.is_empty()
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                {
                    return Some(value.to_owned());
                }
            }
        }
    }
    None
}

fn array_first(files: &BTreeMap<String, File>, file: &File, array: &str) -> Option<String> {
    if let Some((module, export)) = file.bindings.get(array) {
        let target = files.get(module)?;
        return first_array_string(&target.src, export);
    }
    first_array_string(&file.src, array)
}

fn template_parts(src: &str, add: usize) -> Option<(String, String, String)> {
    let window = src.get(add..std::cmp::min(add + 300, src.len()))?;
    let open = window.find('`')?;
    let body = &window[open + 1..];
    let close = body.find('`')?;
    let body = &body[..close];
    let start = body.find("${")?;
    let end = body[start + 2..].find('}')? + start + 2;
    let variable = body[start + 2..end].trim();
    if variable.is_empty()
        || !variable
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || body[end + 1..].contains("${")
    {
        return None;
    }
    Some((
        body[..start].to_owned(),
        variable.to_owned(),
        body[end + 1..].to_owned(),
    ))
}

/// Find classes unconditionally applied to `document.documentElement` from a statically
/// enumerable default. This deliberately recognizes only a narrow provider shape: a template
/// class with one state variable, and a same-file `<array>[0]` fallback whose first item is a
/// string literal. Anything more dynamic remains unknown rather than guessed.
pub(crate) fn default_document_classes(files: &BTreeMap<String, File>) -> Vec<String> {
    let mut out = BTreeSet::new();
    for file in files.values() {
        if !matches!(
            file.path.rsplit_once('.').map(|(_, ext)| ext),
            Some("ts" | "tsx")
        ) {
            continue;
        }
        let mut search = 0usize;
        while let Some(relative) = file.src[search..].find("classList.add") {
            let add = search + relative;
            search = add + 1;
            let before = &file.src[..add];
            if !before.ends_with("document.documentElement.") && !before.ends_with("root.") {
                continue;
            }
            let Some((prefix, variable, suffix)) = template_parts(&file.src, add) else {
                continue;
            };
            let state = file
                .src
                .find(&format!("const [{variable}"))
                .or_else(|| file.src.find(&format!("const [ {variable}")))
                .or_else(|| file.src.find(&format!("let [{variable}")))
                .or_else(|| file.src.find(&format!("let [ {variable}")));
            let Some(state) = state else {
                continue;
            };
            for (index, _) in file.src.match_indices("[0]") {
                if index < state {
                    continue;
                }
                let mut at = index;
                while at > 0 && file.src.as_bytes()[at - 1].is_ascii_whitespace() {
                    at -= 1;
                }
                let end = at;
                while at > 0 {
                    let b = file.src.as_bytes()[at - 1];
                    if b.is_ascii_alphanumeric() || b == b'_' {
                        at -= 1;
                    } else {
                        break;
                    }
                }
                if at == end {
                    continue;
                }
                let array = &file.src[at..end];
                if let Some(value) = array_first(files, file, array) {
                    let class = format!("{prefix}{value}{suffix}");
                    if !class.is_empty()
                        && class
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':'))
                    {
                        out.insert(class);
                    }
                    break;
                }
            }
        }
    }
    out.into_iter().collect()
}
