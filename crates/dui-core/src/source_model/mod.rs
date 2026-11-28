//! Versioned source model (schema version 1).
//!
//! The model is what source code declares about its UI: screens, components, data and
//! state slots, the branches and lists that depend on them, item schemas, navigation
//! edges, neutral style properties and design tokens. Language adapters write it from
//! syntax (and optionally an official compiler); the approximate renderer, hot reload,
//! slot toggles and later exports read only this model.
//!
//! The JSON shape is frozen in `schemas/source-model.v1.schema.json` and documented in
//! `docs/en/source-model.md`. Values that are only known at runtime stay explicit
//! placeholders (`opaque` expressions, `unknown` types, `placeholder` nodes, unresolved
//! styles); the model never invents them.

mod validate;

#[cfg(test)]
mod tests;

pub use validate::{ModelError, validate};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The only schema version this module reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceModel {
    pub schema_version: u32,
    pub extractor: Extractor,
    pub files: Vec<SourceFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screens: Vec<Screen>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    /// Effective document typography extracted from a bundled stylesheet, when proven.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_style: Option<DocumentStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_schemas: Vec<ItemSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<Token>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Extractor {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFile {
    /// Project-relative path with `/` separators.
    pub path: String,
    pub language: Language,
    /// Lowercase hex SHA-256 of the file bytes the model was built from.
    pub sha256: String,
    /// Project files this file imports, sorted. Packages are not listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    Typescript,
    Swift,
    Kotlin,
    Dart,
    Css,
    Html,
}

/// Where a fact came from. `inferred` marks a gap filled by AI or a heuristic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    Syntax,
    Compiler,
    Inferred,
}

/// A source range. `line` and `column` are 1-based (column counts Unicode scalar values);
/// `start` and `end` are 0-based UTF-8 byte offsets, `end` exclusive.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Screen {
    pub id: String,
    pub name: String,
    /// Route path when a router declares the screen; absent for root views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    pub component: String,
    pub origin: Origin,
    pub location: Location,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub location: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Slot>,
    pub root: Node,
}

/// A value the component reads that a reviewer may set: a prop, declared local state,
/// or data from a store, context, route, query or other external call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slot {
    pub id: String,
    pub name: String,
    pub kind: SlotKind,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// Initial value of local state, or the default of a prop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<Expr>,
    /// Source text of the read for store, context, route, query and external slots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub origin: Origin,
    pub location: Location,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlotKind {
    Prop,
    Local,
    Store,
    Context,
    Route,
    Query,
    External,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum TypeRef {
    Boolean,
    String,
    Number,
    /// A closed set of literal values.
    Enum {
        values: Vec<Value>,
    },
    Array {
        item: Box<TypeRef>,
    },
    /// An object described by an item schema.
    Object {
        schema: String,
    },
    /// UI content passed in (children, `ReactNode`, `@ViewBuilder`).
    Node,
    Function,
    Unknown {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Expr {
    Literal {
        value: Value,
    },
    /// A read of a slot of the enclosing component, optionally a field path into it.
    Slot {
        slot: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<String>,
    },
    /// A read of the current item of an enclosing list node: a field-to-UI binding.
    Item {
        list: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<String>,
    },
    /// Code the model does not evaluate. `reads` lists the slot and item reads in it.
    Opaque {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reads: Vec<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Node {
    Fragment(Fragment),
    Element(Element),
    Instance(Instance),
    Text(Text),
    Branch(Branch),
    List(List),
    Outlet(Outlet),
    Placeholder(Placeholder),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fragment {
    pub id: String,
    pub children: Vec<Node>,
    pub location: Location,
}

/// A platform element or a component the model does not describe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Element {
    pub id: String,
    pub tag: String,
    /// Package that exports the tag, for components from libraries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, Expr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style: Vec<StyleDeclaration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_style: Vec<UnresolvedStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    pub location: Location,
}

/// A use of a component in the model. Prop names match the target's `prop` slots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instance {
    pub id: String,
    pub component: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, Expr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    pub location: Location,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Text {
    pub id: String,
    pub parts: Vec<Expr>,
    pub location: Location,
}

/// UI that depends on a value: `if`, `&&`, ternaries, `switch`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Branch {
    pub id: String,
    pub on: Expr,
    pub arms: Vec<Arm>,
    pub location: Location,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Arm {
    pub when: Match,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Match {
    Equals(Value),
    Truthy(bool),
    Otherwise,
}

/// UI repeated for each item of a collection. Children are the per-item template.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct List {
    pub id: String,
    pub source: Expr,
    /// Name the source gives the item, for display.
    pub item: String,
    pub item_type: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Expr>,
    pub children: Vec<Node>,
    pub location: Location,
}

/// Renders UI content held by a `node`-typed slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Outlet {
    pub id: String,
    pub slot: String,
    pub location: Location,
}

/// Source the adapter could not map. Drawn as a labeled placeholder.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Placeholder {
    pub id: String,
    pub reason: String,
    pub location: Location,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentStyle {
    /// Declarations effective on the document root element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<DocumentElementStyle>,
    /// Declarations effective on the document body element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<DocumentElementStyle>,
    /// External stylesheets declared by the document HTML. These are facts needed for
    /// resources such as web fonts; the renderer must not execute the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_stylesheets: Vec<String>,
    /// Web font faces declared by stylesheets. Local and materialized remote sources are
    /// content-addressed archive assets; the renderer never fetches a remote font itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub font_faces: Vec<FontFace>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFace {
    pub id: String,
    pub family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode_range: Option<String>,
    /// Project-relative stylesheet path or remote stylesheet URL containing this face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylesheet: Option<String>,
    /// Lowercase hex SHA-256 of the stylesheet bytes used for a remote face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylesheet_sha256: Option<String>,
    pub sources: Vec<FontSource>,
    pub origin: Origin,
    pub location: Location,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum FontSource {
    /// A font file inside the measured project, copied into the archive under `asset`.
    Asset {
        /// Project-relative path with `/` separators.
        path: String,
        /// Content-addressed archive file `asset-<sha256>.<extension>`.
        asset: String,
        /// Lowercase hex SHA-256 of the font bytes.
        sha256: String,
        /// Font byte length. Extraction and archive writes bound this value.
        bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<String>,
        /// License fact read from the font bytes when present; omission means unknown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        license: Option<String>,
    },
    /// A remote font URL declared by the source. When the bounded build-stage intake could
    /// materialize public bytes, the content-addressed archive fields are present; otherwise
    /// this remains evidence only and archive rendering must not fetch it.
    External {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asset: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha256: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
        /// Final public HTTPS URL after redirects when bytes were materialized.
        #[serde(
            rename = "retrievedUrl",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        retrieved_url: Option<String>,
        /// License fact read from the font bytes when present; omission means unknown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        license: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentElementStyle {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style: Vec<StyleDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleDeclaration {
    pub property: StyleProperty,
    pub value: StyleValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub from: StyleSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleSource {
    pub syntax: StyleSyntax,
    pub text: String,
    pub location: Location,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleSyntax {
    /// Utility classes such as Tailwind.
    Class,
    /// Inline style objects or attributes.
    Inline,
    /// SwiftUI modifiers and Compose `Modifier` chains.
    Modifier,
    /// Rules from CSS files and CSS Modules.
    Stylesheet,
    /// Theme objects and design-token files.
    Theme,
}

/// Style the adapter saw but could not map onto neutral properties.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnresolvedStyle {
    pub syntax: StyleSyntax,
    pub text: String,
    pub reason: String,
    pub location: Location,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleProperty {
    // Layout
    Display,
    GridColumnMinimum,
    Order,
    OverflowX,
    Direction,
    AlignItems,
    JustifyContent,
    Wrap,
    Grow,
    Shrink,
    FlexBasis,
    RowGap,
    ColumnGap,
    // Spacing
    Gap,
    PaddingTop,
    PaddingBottom,
    PaddingStart,
    PaddingEnd,
    MarginTop,
    MarginBottom,
    MarginStart,
    MarginEnd,
    // Size
    Width,
    Height,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    // Color
    Color,
    Background,
    BorderColor,
    Opacity,
    // Typography
    FontFamily,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    LetterSpacing,
    TextAlign,
    TextDecorationLine,
    BorderCollapse,
    VerticalAlign,
    TextTransform,
    WhiteSpace,
    BackdropBlur,
    Position,
    Top,
    Right,
    Bottom,
    Left,
    ZIndex,
    DotRadius,
    DotColor,
    TileWidth,
    TileHeight,
    ShadowX,
    ShadowY,
    ShadowBlur,
    ShadowSpread,
    ShadowColor,
    /// Canonical serialized CSS shadow list. `none` or comma-separated shadow layers.
    BoxShadow,
    BorderTopLeftRadius,
    BorderTopRightRadius,
    BorderBottomRightRadius,
    BorderBottomLeftRadius,

    BorderStyle,
    BorderTopWidth,
    BorderTopColor,
    BorderTopStyle,
    BorderRightWidth,
    BorderRightColor,
    BorderRightStyle,
    BorderBottomWidth,
    BorderBottomColor,
    BorderBottomStyle,
    BorderLeftWidth,
    BorderLeftColor,
    BorderLeftStyle,

    // Radius and border
    BorderRadius,
    BorderWidth,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum StyleValue {
    Length {
        #[serde(serialize_with = "number")]
        value: f64,
        unit: LengthUnit,
    },
    /// A bounded CSS clamp() made of three literal lengths.
    Clamp {
        min: Length,
        preferred: Length,
        max: Length,
    },
    /// Lowercase sRGB hex, `#rrggbb` or `#rrggbbaa`.
    Color {
        value: String,
    },
    Number {
        #[serde(serialize_with = "number")]
        value: f64,
    },
    Keyword {
        value: String,
    },
    /// A design token, with its value when the adapter resolved it.
    Token {
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolved: Option<Box<StyleValue>>,
    },
    Unresolved {
        text: String,
        reason: String,
        /// Typed value for readers that understand the newer shape. The unresolved
        /// text remains the fallback for readers that do not.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refinement: Option<Box<StyleValue>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LengthUnit {
    Vw,
    Vh,
    Svw,
    Svh,
    Lvw,
    Lvh,
    Dvw,
    Dvh,
    Vmin,
    Vmax,
    Px,
    Rem,
    Em,
    Percent,
    Pt,
    Dp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Length {
    #[serde(serialize_with = "number")]
    pub value: f64,
    pub unit: LengthUnit,
}

/// When a declaration applies. All present fields must hold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<Interaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<ColorScheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<Expr>,
    /// When present, `expr` must resolve to exactly this JSON value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Interaction {
    Hover,
    Focus,
    Pressed,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemSchema {
    pub id: String,
    pub name: String,
    pub fields: Vec<Field>,
    pub origin: Origin,
    pub location: Location,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

/// A named design token. The name is kept as written so it can become a design variable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Token {
    pub id: String,
    pub name: String,
    pub group: TokenGroup,
    pub value: StyleValue,
    /// Values per mode, such as `dark`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modes: BTreeMap<String, StyleValue>,
    pub syntax: StyleSyntax,
    /// Where a project defines the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Package that defines the token, for framework defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenGroup {
    Color,
    Spacing,
    Typography,
    Radius,
    Size,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edge {
    pub id: String,
    /// The node that triggers the navigation.
    pub from: String,
    pub kind: EdgeKind,
    pub to: EdgeTarget,
    pub origin: Origin,
    pub location: Location,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    /// A declarative link (`<Link>`, `NavigationLink`).
    Link,
    /// An imperative navigation call.
    Navigate,
    /// A sheet, dialog or other presentation over the current screen.
    Present,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EdgeTarget {
    Screen {
        screen: String,
    },
    /// A route path that matches no screen in the model.
    Route {
        route: String,
    },
    Opaque {
        expr: Expr,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

/// Writes whole numbers without a fraction (`600`, not `600.0`) so the canonical text
/// matches what other JSON writers produce.
fn number<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    const EXACT: f64 = 9_007_199_254_740_992.0;
    if value.fract() == 0.0 && value.abs() < EXACT {
        // Exact: the value is whole and within the range where f64 holds every integer.
        #[allow(clippy::cast_possible_truncation)]
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

impl SourceModel {
    /// Parses and validates a model document.
    pub fn from_json(text: &str) -> Result<Self, Vec<ModelError>> {
        let model: Self = serde_json::from_str(text).map_err(|error| {
            vec![ModelError {
                pointer: String::new(),
                message: error.to_string(),
            }]
        })?;
        validate(&model)?;
        Ok(model)
    }

    /// Canonical JSON text: two-space indentation and a trailing newline.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|text| text + "\n")
    }

    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components
            .binary_search_by(|component| component.id.as_str().cmp(id))
            .ok()
            .and_then(|index| self.components.get(index))
    }

    /// Screen-to-screen relations: for each screen, the edges whose trigger node is drawn
    /// on it, following component instances. Pairs are sorted and unique.
    pub fn screen_edges(&self) -> Vec<ScreenEdge> {
        let mut out = BTreeSet::new();
        for screen in &self.screens {
            let mut nodes = BTreeSet::new();
            let mut seen = BTreeSet::new();
            self.collect_nodes(&screen.component, &mut seen, &mut nodes);
            for edge in &self.edges {
                if nodes.contains(edge.from.as_str()) {
                    out.insert(ScreenEdge {
                        from: screen.id.clone(),
                        edge: edge.id.clone(),
                    });
                }
            }
        }
        out.into_iter().collect()
    }

    fn collect_nodes<'a>(
        &'a self,
        component: &'a str,
        seen: &mut BTreeSet<&'a str>,
        nodes: &mut BTreeSet<&'a str>,
    ) {
        if !seen.insert(component) {
            return;
        }
        let Some(component) = self.component(component) else {
            return;
        };
        let mut stack = vec![&component.root];
        while let Some(node) = stack.pop() {
            nodes.insert(node.id());
            if let Node::Instance(instance) = node {
                self.collect_nodes(&instance.component, seen, nodes);
            }
            stack.extend(node.children());
        }
    }
}

/// An edge that starts on a screen. A shared component's edge starts on every screen
/// that draws it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenEdge {
    pub from: String,
    pub edge: String,
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Self::Fragment(node) => &node.id,
            Self::Element(node) => &node.id,
            Self::Instance(node) => &node.id,
            Self::Text(node) => &node.id,
            Self::Branch(node) => &node.id,
            Self::List(node) => &node.id,
            Self::Outlet(node) => &node.id,
            Self::Placeholder(node) => &node.id,
        }
    }

    pub fn location(&self) -> &Location {
        match self {
            Self::Fragment(node) => &node.location,
            Self::Element(node) => &node.location,
            Self::Instance(node) => &node.location,
            Self::Text(node) => &node.location,
            Self::Branch(node) => &node.location,
            Self::List(node) => &node.location,
            Self::Outlet(node) => &node.location,
            Self::Placeholder(node) => &node.location,
        }
    }

    /// Direct child nodes, including every branch arm.
    pub fn children(&self) -> Vec<&Node> {
        match self {
            Self::Fragment(node) => node.children.iter().collect(),
            Self::Element(node) => node.children.iter().collect(),
            Self::Instance(node) => node.children.iter().collect(),
            Self::List(node) => node.children.iter().collect(),
            Self::Branch(node) => node.arms.iter().flat_map(|arm| &arm.children).collect(),
            Self::Text(_) | Self::Outlet(_) | Self::Placeholder(_) => Vec::new(),
        }
    }
}
