//! Source ranges, declarations and local module imports. No project code is executed.
use dui_core::source_model::{Diagnostic, Location};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Node as Ast, Parser, Tree};

pub struct File {
    pub path: String,
    pub src: String,
    pub tree: Tree,
    pub imports: BTreeSet<String>,
    // Local alias -> (module or relative file, exported name).
    pub bindings: BTreeMap<String, (String, String)>,
    // Literal dynamic-import request -> relative file.
    pub dynamic_imports: BTreeMap<String, String>,
}
pub fn children(n: Ast<'_>) -> Vec<Ast<'_>> {
    n.named_children(&mut n.walk())
        .filter(|n| n.kind() != "comment")
        .collect()
}
pub fn first(n: Ast<'_>) -> Option<Ast<'_>> {
    children(n).into_iter().next()
}
pub fn field<'a>(n: Ast<'a>, key: &str) -> Option<Ast<'a>> {
    n.child_by_field_name(key)
}
pub fn unwrap_expr(mut n: Ast<'_>) -> Ast<'_> {
    while matches!(
        n.kind(),
        "parenthesized_expression" | "jsx_expression" | "type_annotation" | "literal_type"
    ) {
        let Some(child) = first(n) else { break };
        n = child;
    }
    n
}
impl File {
    pub fn text(&self, n: Ast<'_>) -> &str {
        &self.src[n.byte_range()]
    }
    pub fn loc(&self, n: Ast<'_>) -> Location {
        let p = n.start_position();
        Location {
            file: self.path.clone(),
            line: p.row as u32 + 1,
            column: self.src[n.start_byte() - p.column..n.start_byte()]
                .chars()
                .count() as u32
                + 1,
            start: n.start_byte() as u32,
            end: n.end_byte() as u32,
        }
    }
    pub fn name(&self, n: Ast<'_>, key: &str) -> Option<&str> {
        field(n, key).map(|x| self.text(x))
    }
    pub fn declarations(&self) -> Vec<Ast<'_>> {
        children(self.tree.root_node())
            .into_iter()
            .filter_map(|n| {
                if n.kind() == "export_statement" {
                    field(n, "declaration")
                } else {
                    Some(n)
                }
            })
            .flat_map(|n| {
                if matches!(n.kind(), "lexical_declaration" | "ambient_declaration") {
                    children(n)
                } else {
                    vec![n]
                }
            })
            .collect()
    }
    pub fn functions(&self) -> Vec<(&str, Ast<'_>)> {
        self.declarations()
            .into_iter()
            .filter_map(|n| {
                let name = self.name(n, "name")?;
                if !name.starts_with(char::is_uppercase) {
                    return None;
                }
                let f = match n.kind() {
                    "function_declaration" => n,
                    "variable_declarator" => {
                        field(n, "value").filter(|v| v.kind() == "arrow_function")?
                    }
                    _ => return None,
                };
                Some((name, f))
            })
            .collect()
    }
    pub fn exported(&self, name: &str) -> Option<&str> {
        children(self.tree.root_node())
            .into_iter()
            .filter(|n| n.kind() == "export_statement")
            .find_map(|n| {
                let default = n.children(&mut n.walk()).any(|c| c.kind() == "default");
                if let Some(decl) = field(n, "declaration") {
                    if default != (name == "default") {
                        return None;
                    }
                    let decls =
                        if matches!(decl.kind(), "ambient_declaration" | "lexical_declaration") {
                            children(decl)
                        } else {
                            vec![decl]
                        };
                    return decls
                        .into_iter()
                        .filter_map(|d| self.name(d, "name"))
                        .find(|s| default || *s == name);
                }
                if default {
                    if name != "default" {
                        return None;
                    }
                    return field(n, "value")
                        .or_else(|| children(n).into_iter().find(|c| c.kind() == "identifier"))
                        .filter(|v| v.kind() == "identifier")
                        .map(|v| self.text(v));
                }
                if field(n, "source").is_some() {
                    return None;
                }
                children(n)
                    .into_iter()
                    .find(|c| c.kind() == "export_clause")
                    .and_then(|clause| {
                        children(clause).into_iter().find_map(|spec| {
                            if spec.kind() != "export_specifier" {
                                return None;
                            }
                            let local_node = field(spec, "name")?;
                            let exported = field(spec, "alias")
                                .map(|alias| export_text(self.text(alias)))
                                .unwrap_or_else(|| export_text(self.text(local_node)));
                            (exported == name).then(|| export_text(self.text(local_node)))
                        })
                    })
            })
    }
}
fn export_text(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(text)
}
fn relative(root: &Path, path: &Path) -> Option<String> {
    let full = path.canonicalize().ok()?;
    if !full.is_file() {
        return None;
    }
    Some(
        full.strip_prefix(root)
            .ok()?
            .to_str()?
            .replace(std::path::MAIN_SEPARATOR, "/"),
    )
}
struct Alias {
    pattern: String,
    replacements: Vec<PathBuf>,
}

fn path_aliases(root: &Path) -> Vec<Alias> {
    let Ok(text) = fs::read_to_string(root.join("tsconfig.json"))
        .or_else(|_| fs::read_to_string(root.join("jsconfig.json")))
    else {
        return Vec::new();
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(options) = config.get("compilerOptions") else {
        return Vec::new();
    };
    let base = root.join(
        options
            .get("baseUrl")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("."),
    );
    let Some(paths) = options.get("paths").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    paths
        .iter()
        .filter_map(|(pattern, targets)| {
            let targets: Vec<PathBuf> = targets
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(|target| base.join(target))
                .collect();
            (!targets.is_empty()).then(|| Alias {
                pattern: pattern.clone(),
                replacements: targets,
            })
        })
        .collect()
}

fn alias_match<'a>(pattern: &str, request: &'a str) -> Option<&'a str> {
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        if request.len() >= prefix.len() + suffix.len()
            && request.starts_with(prefix)
            && request.ends_with(suffix)
        {
            return Some(&request[prefix.len()..request.len() - suffix.len()]);
        }
        None
    } else {
        (pattern == request).then_some("")
    }
}

fn uses_alias(request: &str, aliases: &[Alias]) -> bool {
    aliases
        .iter()
        .any(|alias| alias_match(&alias.pattern, request).is_some())
}

fn module_candidates(path: PathBuf) -> Vec<PathBuf> {
    let mut paths = vec![path.clone()];
    if path.extension().is_none() {
        for ext in ["ts", "tsx"] {
            paths.push(PathBuf::from(format!("{}.{ext}", path.display())));
        }
        for ext in ["ts", "tsx"] {
            paths.push(path.join(format!("index.{ext}")));
        }
    }
    paths
}

fn resolve(root: &Path, from: &str, request: &str, aliases: &[Alias]) -> Option<String> {
    let mut bases = Vec::new();
    if request.starts_with('.') {
        bases.push(root.join(Path::new(from).parent()?).join(request));
    } else {
        for alias in aliases {
            let Some(matched) = alias_match(&alias.pattern, request) else {
                continue;
            };
            bases.extend(alias.replacements.iter().map(|target| {
                let text = target.to_string_lossy();
                if text.contains('*') {
                    PathBuf::from(text.replacen('*', matched, 1))
                } else {
                    target.clone()
                }
            }));
        }
    }
    bases
        .into_iter()
        .flat_map(module_candidates)
        .filter(|p| {
            matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("tsx" | "ts" | "css")
            )
        })
        .find_map(|p| relative(root, &p))
}
pub struct Sources {
    pub entry: String,
    pub artifact: Option<String>,
    pub files: BTreeMap<String, File>,
}

pub fn load(
    root: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Sources, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let aliases = path_aliases(&root);
    let entry = relative(&root, &root.join(entry))
        .filter(|p| p.ends_with(".tsx"))
        .ok_or("expected a TSX entry file inside PROJECT")?;
    let built_css = built_css
        .map(|path| {
            relative(&root, &root.join(path))
                .filter(|p| p.ends_with(".css"))
                .ok_or("expected a built .css file inside PROJECT")
        })
        .transpose()?;
    let mut pending = vec![entry.clone()];
    pending.extend(built_css.iter().cloned());
    let mut files = BTreeMap::new();
    let mut parser = Parser::new();
    while let Some(path) = pending.pop() {
        if files.contains_key(&path) {
            continue;
        }
        let src = fs::read_to_string(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
        if src.len() >= u32::MAX as usize {
            return Err(format!("{path}: source exceeds model location range"));
        }
        let lang = if path.ends_with(".css") {
            tree_sitter_css::LANGUAGE
        } else if path.ends_with(".tsx") {
            tree_sitter_typescript::LANGUAGE_TSX
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        };
        parser
            .set_language(&lang.into())
            .map_err(|e| e.to_string())?;
        let tree = parser
            .parse(src.as_bytes(), None)
            .ok_or("parse interrupted")?;
        let mut file = File {
            path: path.clone(),
            src,
            tree,
            imports: BTreeSet::new(),
            bindings: BTreeMap::new(),
            dynamic_imports: BTreeMap::new(),
        };
        if file.tree.root_node().has_error() {
            diagnostics.push(Diagnostic {
                code: "parse-error".into(),
                message: "Source contains syntax the parser could not read".into(),
                location: Some(file.loc(file.tree.root_node())),
            });
        }
        for stmt in children(file.tree.root_node())
            .into_iter()
            .filter(|n| n.kind() == "import_statement")
        {
            let Some(source) = field(stmt, "source") else {
                continue;
            };
            let raw = file.text(source);
            let Some(request) = raw.get(1..raw.len().saturating_sub(1)) else {
                continue;
            };
            let mut module = request.to_owned();
            if request.starts_with('.') || uses_alias(request, &aliases) {
                if let Some(resolved) = resolve(&root, &path, request, &aliases) {
                    module = resolved;
                    if module != path && file.imports.insert(module.clone()) {
                        pending.push(module.clone());
                    }
                } else {
                    diagnostics.push(Diagnostic {
                        code: "unresolved-import".into(),
                        message: request.into(),
                        location: Some(file.loc(source)),
                    });
                }
            }
            let mut bindings = Vec::new();
            for clause in children(stmt)
                .into_iter()
                .filter(|n| n.kind() == "import_clause")
                .flat_map(children)
            {
                match clause.kind() {
                    "identifier" => {
                        bindings.push((file.text(clause).to_owned(), "default".to_owned()))
                    }
                    "named_imports" => {
                        for spec in children(clause) {
                            if let Some(name) = file.name(spec, "name") {
                                bindings.push((
                                    file.name(spec, "alias").unwrap_or(name).to_owned(),
                                    name.to_owned(),
                                ))
                            }
                        }
                    }
                    _ => {}
                }
            }
            for (alias, name) in bindings {
                file.bindings.insert(alias, (module.clone(), name));
            }
        }
        let mut pending_nodes = vec![file.tree.root_node()];
        while let Some(node) = pending_nodes.pop() {
            if node.kind() == "call_expression"
                && field(node, "function").is_some_and(|f| f.kind() == "import")
                && let Some(argument) = field(node, "arguments").and_then(first)
                && argument.kind() == "string"
            {
                let raw = file.text(argument);
                if let Some(request) = raw.get(1..raw.len().saturating_sub(1)) {
                    if let Some(resolved) = resolve(&root, &path, request, &aliases) {
                        file.dynamic_imports
                            .insert(request.to_owned(), resolved.clone());
                        if resolved != path && file.imports.insert(resolved.clone()) {
                            pending.push(resolved);
                        }
                    } else if request.starts_with('.') || uses_alias(request, &aliases) {
                        diagnostics.push(Diagnostic {
                            code: "unresolved-import".into(),
                            message: request.into(),
                            location: Some(file.loc(argument)),
                        });
                    }
                }
            }
            pending_nodes.extend(children(node));
        }
        files.insert(path, file);
    }
    Ok(Sources {
        entry,
        artifact: built_css,
        files,
    })
}
