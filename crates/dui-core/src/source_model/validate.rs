//! Rules the JSON Schema cannot express: references resolve, identifiers are unique and
//! sorted, item reads stay inside their list, style values fit their property, and token
//! aliases do not loop.

use super::{
    Component, Condition, Edge, EdgeTarget, Expr, FontSource, Location, Match, Node,
    SCHEMA_VERSION, Slot, SourceModel, StyleDeclaration, StyleProperty, StyleValue, TypeRef,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelError {
    /// JSON Pointer to the offending value.
    pub pointer: String,
    pub message: String,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pointer.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.pointer, self.message)
        }
    }
}

pub fn validate(model: &SourceModel) -> Result<(), Vec<ModelError>> {
    let mut check = Check::new(model);
    check.run();
    if check.errors.is_empty() {
        Ok(())
    } else {
        Err(check.errors)
    }
}

/// Where a node sits: its component and the lists around it, innermost last.
#[derive(Clone)]
struct Scope<'a> {
    component: &'a Component,
    slots: BTreeMap<&'a str, &'a Slot>,
    lists: Vec<&'a str>,
}

struct Check<'a> {
    model: &'a SourceModel,
    errors: Vec<ModelError>,
    files: BTreeSet<&'a str>,
    screens: BTreeSet<&'a str>,
    components: BTreeMap<&'a str, &'a Component>,
    schemas: BTreeSet<&'a str>,
    tokens: BTreeMap<&'a str, &'a StyleValue>,
    slot_ids: BTreeSet<&'a str>,
    nodes: BTreeMap<&'a str, Scope<'a>>,
}

impl<'a> Check<'a> {
    fn new(model: &'a SourceModel) -> Self {
        Self {
            model,
            errors: Vec::new(),
            files: model.files.iter().map(|file| file.path.as_str()).collect(),
            screens: model
                .screens
                .iter()
                .map(|screen| screen.id.as_str())
                .collect(),
            components: model
                .components
                .iter()
                .map(|component| (component.id.as_str(), component))
                .collect(),
            schemas: model
                .item_schemas
                .iter()
                .map(|schema| schema.id.as_str())
                .collect(),
            tokens: model
                .tokens
                .iter()
                .map(|token| (token.id.as_str(), &token.value))
                .collect(),
            slot_ids: BTreeSet::new(),
            nodes: BTreeMap::new(),
        }
    }

    fn error(&mut self, pointer: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ModelError {
            pointer: pointer.into(),
            message: message.into(),
        });
    }

    fn non_empty(&mut self, pointer: impl Into<String>, value: &str) {
        if value.trim().is_empty() {
            self.error(pointer, "must not be empty");
        }
    }

    fn sorted<T>(&mut self, pointer: &str, items: &[T], key: impl Fn(&T) -> &str) {
        for (index, pair) in items.windows(2).enumerate() {
            if let [left, right] = pair
                && key(left) >= key(right)
            {
                let message = if key(left) == key(right) {
                    format!("duplicate identifier {:?}", key(right))
                } else {
                    format!("{:?} must sort after {:?}", key(left), key(right))
                };
                self.error(format!("{pointer}/{}", index + 1), message);
            }
        }
    }

    fn run(&mut self) {
        let model = self.model;
        if model.schema_version != SCHEMA_VERSION {
            self.error(
                "/schemaVersion",
                format!("unsupported schema version {}", model.schema_version),
            );
            return;
        }
        self.non_empty("/extractor/name", &model.extractor.name);
        self.non_empty("/extractor/version", &model.extractor.version);

        self.sorted("/files", &model.files, |file| &file.path);
        self.sorted("/screens", &model.screens, |screen| &screen.id);
        self.sorted("/components", &model.components, |component| &component.id);
        self.sorted("/itemSchemas", &model.item_schemas, |schema| &schema.id);
        self.sorted("/tokens", &model.tokens, |token| &token.id);
        self.sorted("/edges", &model.edges, |edge| &edge.id);

        for (index, file) in model.files.iter().enumerate() {
            let pointer = format!("/files/{index}");
            if !is_relative_path(&file.path) {
                self.error(format!("{pointer}/path"), "must be a project-relative path");
            }
            if !is_sha256(&file.sha256) {
                self.error(format!("{pointer}/sha256"), "must be lowercase hex SHA-256");
            }
            self.sorted(&format!("{pointer}/imports"), &file.imports, |path| path);
            for (at, import) in file.imports.iter().enumerate() {
                if import == &file.path || !self.files.contains(import.as_str()) {
                    self.error(
                        format!("{pointer}/imports/{at}"),
                        format!("{import:?} is not another file of the model"),
                    );
                }
            }
        }

        for (index, schema) in model.item_schemas.iter().enumerate() {
            let pointer = format!("/itemSchemas/{index}");
            self.non_empty(format!("{pointer}/id"), &schema.id);
            self.non_empty(format!("{pointer}/name"), &schema.name);
            self.location(&format!("{pointer}/location"), &schema.location);
            let mut names = BTreeSet::new();
            for (at, field) in schema.fields.iter().enumerate() {
                let field_pointer = format!("{pointer}/fields/{at}");
                self.non_empty(format!("{field_pointer}/name"), &field.name);
                if !names.insert(field.name.as_str()) {
                    self.error(
                        format!("{field_pointer}/name"),
                        format!("duplicate field {:?}", field.name),
                    );
                }
                self.type_ref(&format!("{field_pointer}/type"), &field.ty);
            }
        }

        for (index, token) in model.tokens.iter().enumerate() {
            let pointer = format!("/tokens/{index}");
            self.non_empty(format!("{pointer}/id"), &token.id);
            self.non_empty(format!("{pointer}/name"), &token.name);
            match (&token.location, &token.package) {
                (Some(location), _) => self.location(&format!("{pointer}/location"), location),
                (None, Some(package)) => self.non_empty(format!("{pointer}/package"), package),
                (None, None) => self.error(pointer.clone(), "needs a location or a package"),
            }
            self.token_value(&format!("{pointer}/value"), &token.value);
            for (mode, value) in &token.modes {
                self.non_empty(format!("{pointer}/modes"), mode);
                self.token_value(&format!("{pointer}/modes/{mode}"), value);
            }
            if self.alias_loops(&token.id) {
                self.error(format!("{pointer}/value"), "token aliases form a cycle");
            }
        }

        for (index, component) in model.components.iter().enumerate() {
            self.component(&format!("/components/{index}"), component);
        }

        if let Some(document) = &model.document_style {
            if document.root.is_none()
                && document.body.is_none()
                && document.external_stylesheets.is_empty()
                && document.font_faces.is_empty()
            {
                self.error(
                    "/documentStyle",
                    "must describe the root, body or document resources",
                );
            }
            for (name, part) in [("root", &document.root), ("body", &document.body)] {
                if let Some(part) = part {
                    for (index, declaration) in part.style.iter().enumerate() {
                        self.document_declaration(
                            &format!("/documentStyle/{name}/style/{index}"),
                            declaration,
                        );
                    }
                }
            }
            for (index, url) in document.external_stylesheets.iter().enumerate() {
                if !(url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("//"))
                {
                    self.error(
                        format!("/documentStyle/externalStylesheets/{index}"),
                        "must be an external HTTP URL",
                    );
                }
            }
            self.sorted("/documentStyle/fontFaces", &document.font_faces, |face| {
                &face.id
            });
            for (index, face) in document.font_faces.iter().enumerate() {
                let pointer = format!("/documentStyle/fontFaces/{index}");
                self.non_empty(format!("{pointer}/id"), &face.id);
                self.non_empty(format!("{pointer}/family"), &face.family);
                self.location(&format!("{pointer}/location"), &face.location);
                if let Some(unicode_range) = &face.unicode_range {
                    self.non_empty(format!("{pointer}/unicodeRange"), unicode_range);
                }
                if let Some(stylesheet) = &face.stylesheet {
                    self.non_empty(format!("{pointer}/stylesheet"), stylesheet);
                }
                if let Some(sha256) = &face.stylesheet_sha256
                    && !is_sha256(sha256)
                {
                    self.error(
                        format!("{pointer}/stylesheetSha256"),
                        "must be lowercase hex SHA-256",
                    );
                }
                if face.stylesheet.as_ref().is_some_and(|sheet| {
                    sheet.starts_with("https://")
                        || sheet.starts_with("http://")
                        || sheet.starts_with("//")
                }) && face.stylesheet_sha256.is_none()
                {
                    self.error(
                        format!("{pointer}/stylesheetSha256"),
                        "remote stylesheet provenance must include its content hash",
                    );
                }
                if face.sources.is_empty() {
                    self.error(format!("{pointer}/sources"), "must not be empty");
                }
                for (at, source) in face.sources.iter().enumerate() {
                    let source_pointer = format!("{pointer}/sources/{at}");
                    match source {
                        FontSource::Asset {
                            path,
                            asset,
                            sha256,
                            bytes,
                            format,
                            license,
                        } => {
                            if !is_relative_path(path) {
                                self.error(
                                    format!("{source_pointer}/path"),
                                    "must be a project-relative path",
                                );
                            }
                            if !is_sha256(sha256) {
                                self.error(
                                    format!("{source_pointer}/sha256"),
                                    "must be lowercase hex SHA-256",
                                );
                            }
                            let extension = path.rsplit_once('.').map(|(_, ext)| ext);
                            let expected = extension.map(|ext| format!("asset-{sha256}.{ext}"));
                            if !matches!(extension, Some("woff2" | "woff" | "ttf" | "otf"))
                                || expected.as_deref() != Some(asset.as_str())
                            {
                                self.error(
                                    format!("{source_pointer}/asset"),
                                    "must be asset-<sha256>.<woff2|woff|ttf|otf> for its source path",
                                );
                            }
                            if *bytes == 0 || *bytes > 4 * 1024 * 1024 {
                                self.error(
                                    format!("{source_pointer}/bytes"),
                                    "must be within the bounded local font size",
                                );
                            }
                            if let Some(format) = format {
                                self.non_empty(format!("{source_pointer}/format"), format);
                            }
                            if let Some(license) = license {
                                self.non_empty(format!("{source_pointer}/license"), license);
                            }
                        }
                        FontSource::External {
                            url,
                            format,
                            asset,
                            sha256,
                            bytes,
                            retrieved_url,
                            license,
                        } => {
                            if !(url.starts_with("https://")
                                || url.starts_with("http://")
                                || url.starts_with("//"))
                            {
                                self.error(
                                    format!("{source_pointer}/url"),
                                    "must be an external HTTP URL",
                                );
                            }
                            if let Some(format) = format {
                                self.non_empty(format!("{source_pointer}/format"), format);
                            }
                            if let Some(license) = license {
                                self.non_empty(format!("{source_pointer}/license"), license);
                            }
                            let materialized = asset.is_some()
                                || sha256.is_some()
                                || bytes.is_some()
                                || retrieved_url.is_some();
                            if materialized {
                                let (Some(asset), Some(sha256), Some(bytes), Some(retrieved_url)) =
                                    (asset, sha256, bytes, retrieved_url)
                                else {
                                    self.error(
                                        source_pointer.clone(),
                                        "materialized remote font fields must be complete",
                                    );
                                    continue;
                                };
                                if !is_sha256(sha256) {
                                    self.error(
                                        format!("{source_pointer}/sha256"),
                                        "must be lowercase hex SHA-256",
                                    );
                                }
                                let extension = asset.rsplit_once('.').map(|(_, ext)| ext);
                                if !matches!(extension, Some("woff2" | "woff" | "ttf" | "otf"))
                                    || *asset
                                        != format!("asset-{sha256}.{}", extension.unwrap_or(""))
                                {
                                    self.error(
                                        format!("{source_pointer}/asset"),
                                        "must be asset-<sha256>.<woff2|woff|ttf|otf>",
                                    );
                                }
                                if *bytes == 0 || *bytes > 4 * 1024 * 1024 {
                                    self.error(
                                        format!("{source_pointer}/bytes"),
                                        "must be within the bounded font size",
                                    );
                                }
                                if !retrieved_url.starts_with("https://") {
                                    self.error(
                                        format!("{source_pointer}/retrievedUrl"),
                                        "must be the final public HTTPS URL",
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        for (index, screen) in model.screens.iter().enumerate() {
            let pointer = format!("/screens/{index}");
            self.non_empty(format!("{pointer}/id"), &screen.id);
            self.non_empty(format!("{pointer}/name"), &screen.name);
            if let Some(route) = &screen.route
                && !route.starts_with('/')
            {
                self.error(format!("{pointer}/route"), "must start with /");
            }
            if !self.components.contains_key(screen.component.as_str()) {
                self.error(
                    format!("{pointer}/component"),
                    format!("unknown component {:?}", screen.component),
                );
            }
            self.location(&format!("{pointer}/location"), &screen.location);
        }

        for (index, edge) in model.edges.iter().enumerate() {
            self.edge(&format!("/edges/{index}"), edge);
        }

        for (index, diagnostic) in model.diagnostics.iter().enumerate() {
            let pointer = format!("/diagnostics/{index}");
            self.non_empty(format!("{pointer}/code"), &diagnostic.code);
            self.non_empty(format!("{pointer}/message"), &diagnostic.message);
            if let Some(location) = &diagnostic.location {
                self.location(&format!("{pointer}/location"), location);
            }
        }
    }

    fn location(&mut self, pointer: &str, location: &Location) {
        if !self.files.contains(location.file.as_str()) {
            self.error(
                format!("{pointer}/file"),
                format!("{:?} is not a file of the model", location.file),
            );
        }
        if location.line == 0 || location.column == 0 {
            self.error(pointer.to_owned(), "line and column are 1-based");
        }
        if location.start > location.end {
            self.error(pointer.to_owned(), "start must not be after end");
        }
    }

    fn type_ref(&mut self, pointer: &str, ty: &TypeRef) {
        match ty {
            TypeRef::Enum { values } => {
                if values.is_empty() {
                    self.error(format!("{pointer}/values"), "must list at least one value");
                }
                for (index, value) in values.iter().enumerate() {
                    if values.iter().take(index).any(|earlier| earlier == value) {
                        self.error(format!("{pointer}/values/{index}"), "duplicate value");
                    }
                }
            }
            TypeRef::Array { item } => self.type_ref(&format!("{pointer}/item"), item),
            TypeRef::Object { schema } => {
                if !self.schemas.contains(schema.as_str()) {
                    self.error(
                        format!("{pointer}/schema"),
                        format!("unknown item schema {schema:?}"),
                    );
                }
            }
            TypeRef::Boolean
            | TypeRef::String
            | TypeRef::Number
            | TypeRef::Node
            | TypeRef::Function
            | TypeRef::Unknown { .. } => {}
        }
    }

    fn alias_loops(&self, start: &str) -> bool {
        let mut current = start;
        for _ in 0..=self.tokens.len() {
            match self.tokens.get(current) {
                Some(StyleValue::Token { token, .. }) => current = token,
                Some(_) | None => return false,
            }
        }
        true
    }

    fn token_value(&mut self, pointer: &str, value: &StyleValue) {
        self.style_value(pointer, value);
        if let StyleValue::Token { token, .. } = value
            && !self.tokens.contains_key(token.as_str())
        {
            self.error(
                format!("{pointer}/token"),
                format!("unknown token {token:?}"),
            );
        }
    }

    fn style_value(&mut self, pointer: &str, value: &StyleValue) {
        match value {
            StyleValue::Length { value, .. } | StyleValue::Number { value } => {
                if !value.is_finite() {
                    self.error(format!("{pointer}/value"), "must be a finite number");
                }
            }
            StyleValue::Clamp {
                min,
                preferred,
                max,
            } => {
                for (name, length) in [("min", min), ("preferred", preferred), ("max", max)] {
                    if !length.value.is_finite() {
                        self.error(format!("{pointer}/{name}/value"), "must be a finite number");
                    }
                }
            }
            StyleValue::Color { value } => {
                if !is_hex_color(value) {
                    self.error(format!("{pointer}/value"), "must be #rrggbb or #rrggbbaa");
                }
            }
            StyleValue::Keyword { value } => self.non_empty(format!("{pointer}/value"), value),
            StyleValue::Token { resolved, .. } => {
                if let Some(resolved) = resolved {
                    if let StyleValue::Token { .. } = resolved.as_ref() {
                        self.error(format!("{pointer}/resolved"), "must be a final value");
                    }
                    self.style_value(&format!("{pointer}/resolved"), resolved);
                }
            }
            StyleValue::Unresolved {
                text,
                reason,
                refinement,
            } => {
                self.non_empty(format!("{pointer}/text"), text);
                self.non_empty(format!("{pointer}/reason"), reason);
                if let Some(refinement) = refinement {
                    if matches!(
                        refinement.as_ref(),
                        StyleValue::Token { .. } | StyleValue::Unresolved { .. }
                    ) {
                        self.error(
                            format!("{pointer}/refinement"),
                            "must be a final typed value",
                        );
                    } else {
                        self.style_value(&format!("{pointer}/refinement"), refinement);
                    }
                }
            }
        }
    }

    fn component(&mut self, pointer: &str, component: &'a Component) {
        self.non_empty(format!("{pointer}/id"), &component.id);
        self.non_empty(format!("{pointer}/name"), &component.name);
        self.location(&format!("{pointer}/location"), &component.location);
        let mut scope = Scope {
            component,
            slots: BTreeMap::new(),
            lists: Vec::new(),
        };
        let mut names = BTreeSet::new();
        for (index, slot) in component.slots.iter().enumerate() {
            let slot_pointer = format!("{pointer}/slots/{index}");
            self.non_empty(format!("{slot_pointer}/id"), &slot.id);
            self.non_empty(format!("{slot_pointer}/name"), &slot.name);
            if !self.slot_ids.insert(slot.id.as_str()) {
                self.error(
                    format!("{slot_pointer}/id"),
                    format!("duplicate identifier {:?}", slot.id),
                );
            }
            if !names.insert(slot.name.as_str()) {
                self.error(
                    format!("{slot_pointer}/name"),
                    format!("duplicate slot name {:?}", slot.name),
                );
            }
            self.type_ref(&format!("{slot_pointer}/type"), &slot.ty);
            self.location(&format!("{slot_pointer}/location"), &slot.location);
            scope.slots.insert(slot.id.as_str(), slot);
        }
        for (index, slot) in component.slots.iter().enumerate() {
            if let Some(initial) = &slot.initial {
                self.expr(&format!("{pointer}/slots/{index}/initial"), initial, &scope);
            }
        }
        self.node(&format!("{pointer}/root"), &component.root, &scope);
    }

    fn node(&mut self, pointer: &str, node: &'a Node, scope: &Scope<'a>) {
        let id = node.id();
        self.non_empty(format!("{pointer}/id"), id);
        if self.nodes.insert(id, scope.clone()).is_some() {
            self.error(
                format!("{pointer}/id"),
                format!("duplicate identifier {id:?}"),
            );
        }
        self.location(&format!("{pointer}/location"), node.location());
        match node {
            Node::Element(element) => {
                self.non_empty(format!("{pointer}/tag"), &element.tag);
                for (name, value) in &element.props {
                    self.expr(&format!("{pointer}/props/{name}"), value, scope);
                }
                for (index, declaration) in element.style.iter().enumerate() {
                    self.declaration(&format!("{pointer}/style/{index}"), declaration, scope);
                }
                for (index, style) in element.unresolved_style.iter().enumerate() {
                    let style_pointer = format!("{pointer}/unresolvedStyle/{index}");
                    self.non_empty(format!("{style_pointer}/text"), &style.text);
                    self.non_empty(format!("{style_pointer}/reason"), &style.reason);
                    self.location(&format!("{style_pointer}/location"), &style.location);
                }
                self.children(&format!("{pointer}/children"), &element.children, scope);
            }
            Node::Instance(instance) => {
                match self.components.get(instance.component.as_str()) {
                    Some(target) => {
                        let props: BTreeSet<&str> = target
                            .slots
                            .iter()
                            .filter(|slot| slot.kind == super::SlotKind::Prop)
                            .map(|slot| slot.name.as_str())
                            .collect();
                        let accepts_rest = target.slots.iter().any(|slot| {
                            slot.kind == super::SlotKind::Prop && slot.name.starts_with("...")
                        });
                        for name in instance.props.keys() {
                            if !props.contains(name.as_str()) && !accepts_rest {
                                self.error(
                                    format!("{pointer}/props/{name}"),
                                    format!("{:?} has no prop {name:?}", instance.component),
                                );
                            }
                        }
                    }
                    None => self.error(
                        format!("{pointer}/component"),
                        format!("unknown component {:?}", instance.component),
                    ),
                }
                for (name, value) in &instance.props {
                    self.expr(&format!("{pointer}/props/{name}"), value, scope);
                }
                self.children(&format!("{pointer}/children"), &instance.children, scope);
            }
            Node::Fragment(f) => self.children(&format!("{pointer}/children"), &f.children, scope),
            Node::Text(text) => {
                if text.parts.is_empty() {
                    self.error(format!("{pointer}/parts"), "must have at least one part");
                }
                for (index, part) in text.parts.iter().enumerate() {
                    self.expr(&format!("{pointer}/parts/{index}"), part, scope);
                }
            }
            Node::Branch(branch) => {
                self.expr(&format!("{pointer}/on"), &branch.on, scope);
                if branch.arms.is_empty() {
                    self.error(format!("{pointer}/arms"), "must have at least one arm");
                }
                let last = branch.arms.len().saturating_sub(1);
                for (index, arm) in branch.arms.iter().enumerate() {
                    let arm_pointer = format!("{pointer}/arms/{index}");
                    if arm.when == Match::Otherwise && index != last {
                        self.error(format!("{arm_pointer}/when"), "otherwise must be last");
                    }
                    self.children(&format!("{arm_pointer}/children"), &arm.children, scope);
                }
            }
            Node::List(list) => {
                self.expr(&format!("{pointer}/source"), &list.source, scope);
                self.non_empty(format!("{pointer}/item"), &list.item);
                self.type_ref(&format!("{pointer}/itemType"), &list.item_type);
                let mut inner = scope.clone();
                inner.lists.push(list.id.as_str());
                if let Some(key) = &list.key {
                    self.expr(&format!("{pointer}/key"), key, &inner);
                }
                if list.children.is_empty() {
                    self.error(format!("{pointer}/children"), "must have an item template");
                }
                self.children(&format!("{pointer}/children"), &list.children, &inner);
            }
            Node::Outlet(outlet) => match scope.slots.get(outlet.slot.as_str()) {
                Some(slot) if slot.ty == TypeRef::Node => {}
                Some(_) => self.error(format!("{pointer}/slot"), "slot type must be node"),
                None => self.error(
                    format!("{pointer}/slot"),
                    format!(
                        "{:?} is not a slot of {:?}",
                        outlet.slot, scope.component.id
                    ),
                ),
            },
            Node::Placeholder(placeholder) => {
                self.non_empty(format!("{pointer}/reason"), &placeholder.reason);
            }
        }
    }

    fn children(&mut self, pointer: &str, children: &'a [Node], scope: &Scope<'a>) {
        for (index, child) in children.iter().enumerate() {
            self.node(&format!("{pointer}/{index}"), child, scope);
        }
    }

    fn expr(&mut self, pointer: &str, expr: &Expr, scope: &Scope<'a>) {
        match expr {
            Expr::Literal { .. } => {}
            Expr::Slot { slot, path } => {
                if !scope.slots.contains_key(slot.as_str()) {
                    self.error(
                        format!("{pointer}/slot"),
                        format!("{slot:?} is not a slot of {:?}", scope.component.id),
                    );
                }
                self.field_path(pointer, path);
            }
            Expr::Item { list, path } => {
                if !scope.lists.contains(&list.as_str()) {
                    self.error(
                        format!("{pointer}/list"),
                        format!("{list:?} is not a list around this value"),
                    );
                }
                self.field_path(pointer, path);
            }
            Expr::Opaque { text, reads } => {
                self.non_empty(format!("{pointer}/text"), text);
                for (index, read) in reads.iter().enumerate() {
                    let read_pointer = format!("{pointer}/reads/{index}");
                    match read {
                        Expr::Slot { .. } | Expr::Item { .. } => {
                            self.expr(&read_pointer, read, scope);
                        }
                        Expr::Literal { .. } | Expr::Opaque { .. } => {
                            self.error(read_pointer, "must be a slot or item read");
                        }
                    }
                }
            }
        }
    }

    fn field_path(&mut self, pointer: &str, path: &[String]) {
        for (index, segment) in path.iter().enumerate() {
            self.non_empty(format!("{pointer}/path/{index}"), segment);
        }
    }

    fn declaration(&mut self, pointer: &str, declaration: &StyleDeclaration, scope: &Scope<'a>) {
        let value_pointer = format!("{pointer}/value");
        self.style_value(&value_pointer, &declaration.value);
        let checked = match &declaration.value {
            StyleValue::Token { token, resolved } => {
                if !self.tokens.contains_key(token.as_str()) {
                    self.error(
                        format!("{value_pointer}/token"),
                        format!("unknown token {token:?}"),
                    );
                }
                resolved
                    .as_deref()
                    .map(|value| (format!("{value_pointer}/resolved"), value))
            }
            other @ (StyleValue::Length { .. }
            | StyleValue::Clamp { .. }
            | StyleValue::Color { .. }
            | StyleValue::Number { .. }
            | StyleValue::Keyword { .. }
            | StyleValue::Unresolved { .. }) => Some((value_pointer.clone(), other)),
        };
        if let Some((pointer, value)) = checked
            && !accepts(declaration.property, value)
        {
            self.error(
                pointer,
                format!("not a value for {:?}", declaration.property),
            );
        }
        if let StyleValue::Unresolved {
            refinement: Some(refinement),
            ..
        } = &declaration.value
            && !accepts(declaration.property, refinement)
        {
            self.error(
                format!("{value_pointer}/refinement"),
                format!("not a value for {:?}", declaration.property),
            );
        }
        if let Some(condition) = &declaration.when {
            self.condition(&format!("{pointer}/when"), condition, scope);
        }
        self.non_empty(format!("{pointer}/from/text"), &declaration.from.text);
        self.location(
            &format!("{pointer}/from/location"),
            &declaration.from.location,
        );
    }

    fn document_declaration(&mut self, pointer: &str, declaration: &StyleDeclaration) {
        let value_pointer = format!("{pointer}/value");
        self.style_value(&value_pointer, &declaration.value);
        let checked = match &declaration.value {
            StyleValue::Token { token, resolved } => {
                if !self.tokens.contains_key(token.as_str()) {
                    self.error(
                        format!("{value_pointer}/token"),
                        format!("unknown token {token:?}"),
                    );
                }
                resolved
                    .as_deref()
                    .map(|value| (format!("{value_pointer}/resolved"), value))
            }
            other @ (StyleValue::Length { .. }
            | StyleValue::Clamp { .. }
            | StyleValue::Color { .. }
            | StyleValue::Number { .. }
            | StyleValue::Keyword { .. }
            | StyleValue::Unresolved { .. }) => Some((value_pointer.clone(), other)),
        };
        if let Some((pointer, value)) = checked
            && !accepts(declaration.property, value)
        {
            self.error(
                pointer,
                format!("not a value for {:?}", declaration.property),
            );
        }
        if let StyleValue::Unresolved {
            refinement: Some(refinement),
            ..
        } = &declaration.value
            && !accepts(declaration.property, refinement)
        {
            self.error(
                format!("{value_pointer}/refinement"),
                format!("not a value for {:?}", declaration.property),
            );
        }
        if let Some(condition) = &declaration.when {
            if condition.expr.is_some() {
                self.error(
                    format!("{pointer}/when/expr"),
                    "document style conditions cannot read component expressions",
                );
            }
            for (name, length) in [
                ("minWidth", &condition.min_width),
                ("maxWidth", &condition.max_width),
            ] {
                if let Some(length) = length
                    && !length.value.is_finite()
                {
                    self.error(
                        format!("{pointer}/when/{name}/value"),
                        "must be a finite number",
                    );
                }
            }
        }
        self.non_empty(format!("{pointer}/from/text"), &declaration.from.text);
        self.location(
            &format!("{pointer}/from/location"),
            &declaration.from.location,
        );
    }

    fn condition(&mut self, pointer: &str, condition: &Condition, scope: &Scope<'a>) {
        if condition.min_width.is_none()
            && condition.max_width.is_none()
            && condition.interaction.is_none()
            && condition.color_scheme.is_none()
            && condition.expr.is_none()
        {
            self.error(pointer.to_owned(), "must set at least one condition");
        }
        for (name, length) in [
            ("minWidth", &condition.min_width),
            ("maxWidth", &condition.max_width),
        ] {
            if let Some(length) = length
                && !length.value.is_finite()
            {
                self.error(format!("{pointer}/{name}/value"), "must be a finite number");
            }
        }
        if condition.equals.is_some() && condition.expr.is_none() {
            self.error(
                format!("{pointer}/equals"),
                "requires expr so the compared value is explicit",
            );
        }
        if let Some(expr) = &condition.expr {
            self.expr(&format!("{pointer}/expr"), expr, scope);
        }
    }

    fn edge(&mut self, pointer: &str, edge: &Edge) {
        self.non_empty(format!("{pointer}/id"), &edge.id);
        self.location(&format!("{pointer}/location"), &edge.location);
        let scope = self.nodes.get(edge.from.as_str()).cloned();
        if scope.is_none() {
            self.error(
                format!("{pointer}/from"),
                format!("unknown node {:?}", edge.from),
            );
        }
        match &edge.to {
            EdgeTarget::Screen { screen } => {
                if !self.screens.contains(screen.as_str()) {
                    self.error(
                        format!("{pointer}/to/screen"),
                        format!("unknown screen {screen:?}"),
                    );
                }
            }
            EdgeTarget::Route { route } => {
                if !route.starts_with('/') {
                    self.error(format!("{pointer}/to/route"), "must start with /");
                }
            }
            EdgeTarget::Opaque { expr } => {
                if let Some(scope) = scope {
                    self.expr(&format!("{pointer}/to/expr"), expr, &scope);
                }
            }
        }
    }
}

/// Which values a property takes. Unresolved values are accepted everywhere.
fn accepts(property: StyleProperty, value: &StyleValue) -> bool {
    use StyleProperty as P;
    const SIZE: &[&str] = &["auto", "fill", "fit"];
    let (length, number, color, keywords): (bool, bool, bool, Option<&[&str]>) = match property {
        P::Display => (
            false,
            false,
            false,
            Some(&[
                "grid",
                "flex",
                "inline-flex",
                "block",
                "inline",
                "inline-block",
                "none",
            ]),
        ),
        P::OverflowX => (
            false,
            false,
            false,
            Some(&["auto", "scroll", "hidden", "clip", "visible"]),
        ),
        P::GridColumnMinimum => (true, false, false, Some(&["single"])),
        P::Position => (
            false,
            false,
            false,
            Some(&["static", "relative", "absolute", "fixed", "sticky"]),
        ),
        P::Top | P::Right | P::Bottom | P::Left => (true, false, false, Some(&["auto"])),
        P::ZIndex => (false, true, false, Some(&["auto"])),
        P::DotColor | P::ShadowColor => (false, false, true, Some(&[])),
        P::BoxShadow => (false, false, false, None),
        P::DotRadius
        | P::TileWidth
        | P::TileHeight
        | P::ShadowX
        | P::ShadowY
        | P::ShadowBlur
        | P::ShadowSpread
        | P::BorderTopLeftRadius
        | P::BorderTopRightRadius
        | P::BorderBottomRightRadius
        | P::BorderBottomLeftRadius => (true, false, false, Some(&[])),
        P::Direction => (false, false, false, Some(&["row", "column", "overlay"])),
        P::AlignItems => (
            false,
            false,
            false,
            Some(&["start", "center", "end", "stretch", "baseline"]),
        ),
        P::JustifyContent => (
            false,
            false,
            false,
            Some(&[
                "start",
                "center",
                "end",
                "spaceBetween",
                "spaceAround",
                "spaceEvenly",
            ]),
        ),
        P::Wrap => (false, false, false, Some(&["wrap", "nowrap"])),
        P::Shrink | P::Order | P::Grow | P::Opacity | P::FontWeight => {
            (false, true, false, Some(&[]))
        }
        P::FlexBasis => (
            true,
            false,
            false,
            Some(&[
                "auto",
                "min-content",
                "max-content",
                "content",
                "fit-content",
            ]),
        ),
        P::RowGap
        | P::ColumnGap
        | P::Gap
        | P::PaddingTop
        | P::PaddingBottom
        | P::PaddingStart
        | P::PaddingEnd
        | P::FontSize
        | P::LetterSpacing
        | P::BorderRadius
        | P::BorderWidth => (true, false, false, Some(&[])),
        P::MarginTop | P::MarginBottom | P::MarginStart | P::MarginEnd => {
            (true, false, false, Some(&["auto"]))
        }
        P::Width | P::Height | P::MinWidth | P::MaxWidth | P::MinHeight | P::MaxHeight => {
            (true, false, false, Some(SIZE))
        }
        P::Color | P::Background | P::BorderColor => (false, false, true, Some(&[])),
        P::FontFamily => (false, false, false, None),
        P::FontStyle => (false, false, false, Some(&["normal", "italic"])),
        P::LineHeight => (true, true, false, Some(&[])),
        P::TextDecorationLine => (
            false,
            false,
            false,
            Some(&["none", "underline", "overline", "line-through"]),
        ),
        P::BorderCollapse => (false, false, false, Some(&["collapse", "separate"])),
        P::VerticalAlign => (
            false,
            false,
            false,
            Some(&[
                "baseline",
                "top",
                "middle",
                "bottom",
                "text-top",
                "text-bottom",
                "sub",
                "super",
            ]),
        ),
        P::TextTransform => (
            false,
            false,
            false,
            Some(&["none", "uppercase", "lowercase", "capitalize"]),
        ),
        P::WhiteSpace => (
            false,
            false,
            false,
            Some(&[
                "normal",
                "pre",
                "nowrap",
                "pre-wrap",
                "pre-line",
                "break-spaces",
            ]),
        ),
        P::BorderStyle
        | P::BorderTopStyle
        | P::BorderRightStyle
        | P::BorderBottomStyle
        | P::BorderLeftStyle => (
            false,
            false,
            false,
            Some(&[
                "none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge",
                "inset", "outset",
            ]),
        ),
        P::BorderTopWidth
        | P::BorderRightWidth
        | P::BorderBottomWidth
        | P::BorderLeftWidth
        | P::BackdropBlur => (true, false, false, Some(&[])),
        P::BorderTopColor | P::BorderRightColor | P::BorderBottomColor | P::BorderLeftColor => {
            (false, false, true, Some(&[]))
        }
        P::TextAlign => (
            false,
            false,
            false,
            Some(&["start", "center", "end", "justify", "left", "right"]),
        ),
    };
    match value {
        StyleValue::Length { .. } | StyleValue::Clamp { .. } => length,
        StyleValue::Number { value } => {
            number && (!matches!(property, P::Order | P::ZIndex) || value.fract() == 0.0)
        }
        StyleValue::Color { .. } => color,
        StyleValue::Keyword { value } => {
            value == "inherit" || keywords.is_none_or(|allowed| allowed.contains(&value.as_str()))
        }
        StyleValue::Unresolved { .. } => true,
        StyleValue::Token { .. } => false,
    }
}

fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_hex_color(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(|hex| {
        (hex.len() == 6 || hex.len() == 8)
            && hex
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}
