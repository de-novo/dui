//! Source ranges, declarations and local module imports. No project code is executed.
use dui_core::source_model::{Diagnostic, Location};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Node as Ast, Parser, Tree};

#[derive(Clone)]
pub struct File {
    pub path: String,
    pub src: String,
    pub tree: Tree,
    pub imports: BTreeSet<String>,
    // Local alias -> (module or relative file, exported name).
    pub bindings: BTreeMap<String, (String, String)>,
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
                if default != (name == "default") {
                    return None;
                }
                let decl = field(n, "declaration")?;
                let decls = if matches!(decl.kind(), "ambient_declaration" | "lexical_declaration")
                {
                    children(decl)
                } else {
                    vec![decl]
                };
                decls
                    .into_iter()
                    .filter_map(|d| self.name(d, "name"))
                    .find(|s| default || *s == name)
            })
    }
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
fn resolve(root: &Path, from: &str, request: &str) -> Option<String> {
    let path = root.join(Path::new(from).parent()?).join(request);
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
        .into_iter()
        .filter(|p| {
            matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("tsx" | "ts" | "css")
            )
        })
        .find_map(|p| relative(root, &p))
}

/// Read the import statements of an already parsed file against the current filesystem. This
/// refreshes added and removed imports without re-parsing the file's source.
fn scan_imports(
    root: &Path,
    file: &File,
) -> (
    BTreeSet<String>,
    BTreeMap<String, (String, String)>,
    Vec<Diagnostic>,
) {
    let mut imports = BTreeSet::new();
    let mut bindings = BTreeMap::new();
    let mut diagnostics = Vec::new();
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
        if request.starts_with('.') {
            if let Some(resolved) = resolve(root, &file.path, request) {
                module = resolved;
                if module != file.path {
                    imports.insert(module.clone());
                }
            } else {
                diagnostics.push(Diagnostic {
                    code: "unresolved-import".into(),
                    message: request.into(),
                    location: Some(file.loc(source)),
                });
            }
        }
        let mut imported = Vec::new();
        for clause in children(stmt)
            .into_iter()
            .filter(|n| n.kind() == "import_clause")
            .flat_map(children)
        {
            match clause.kind() {
                "identifier" => {
                    imported.push((file.text(clause).to_owned(), "default".to_owned()))
                }
                "named_imports" => {
                    for spec in children(clause) {
                        if let Some(name) = file.name(spec, "name") {
                            imported.push((
                                file.name(spec, "alias").unwrap_or(name).to_owned(),
                                name.to_owned(),
                            ))
                        }
                    }
                }
                _ => {}
            }
        }
        for (alias, name) in imported {
            bindings.insert(alias, (module.clone(), name));
        }
    }
    (imports, bindings, diagnostics)
}

fn parse_file(path: &str, src: String, parser: &mut Parser) -> Result<File, String> {
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
    let tree = parser.parse(src.as_bytes(), None).ok_or("parse interrupted")?;
    Ok(File {
        path: path.into(),
        src,
        tree,
        imports: BTreeSet::new(),
        bindings: BTreeMap::new(),
    })
}

pub struct Sources {
    pub entry: String,
    pub artifact: Option<String>,
    pub files: BTreeMap<String, File>,
}

#[derive(Default)]
pub struct ParseCache {
    files: BTreeMap<String, (String, File)>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LoadStats {
    pub parsed: Vec<String>,
    pub reused: Vec<String>,
}

pub fn load_cached(
    root: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
    cache: &mut ParseCache,
) -> Result<(Sources, LoadStats), String> {
    load_inner(root, entry, built_css, diagnostics, Some(cache))
}

fn load_inner(
    root: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
    mut cache: Option<&mut ParseCache>,
) -> Result<(Sources, LoadStats), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
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
    let mut stats = LoadStats::default();
    let mut parser = Parser::new();
    while let Some(path) = pending.pop() {
        if files.contains_key(&path) {
            continue;
        }
        let src = fs::read_to_string(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
        if src.len() >= u32::MAX as usize {
            return Err(format!("{path}: source exceeds model location range"));
        }
        let sha256 = format!("{:x}", Sha256::digest(src.as_bytes()));
        let cached = cache
            .as_ref()
            .and_then(|cache| cache.files.get(&path))
            .filter(|(cached_sha, _)| cached_sha == &sha256)
            .map(|(_, file)| file.clone());
        let mut file = if let Some(file) = cached {
            stats.reused.push(path.clone());
            file
        } else {
            stats.parsed.push(path.clone());
            let parsed = parse_file(&path, src, &mut parser)?;
            if let Some(cache) = cache.as_mut() {
                cache.files.insert(
                    path.clone(),
                    (sha256.clone(), parsed.clone()),
                );
            }
            parsed
        };
        if file.tree.root_node().has_error() {
            diagnostics.push(Diagnostic {
                code: "parse-error".into(),
                message: "Source contains syntax the parser could not read".into(),
                location: Some(file.loc(file.tree.root_node())),
            });
        }
        let (imports, bindings, mut unresolved) = scan_imports(&root, &file);
        file.imports = imports;
        file.bindings = bindings;
        diagnostics.append(&mut unresolved);
        for module in &file.imports {
            if !files.contains_key(module) {
                pending.push(module.clone());
            }
        }
        files.insert(path, file);
    }
    Ok((
        Sources {
            entry,
            artifact: built_css,
            files,
        },
        stats,
    ))
}
