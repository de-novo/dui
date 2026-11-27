// Draw the versioned model in an inert document. Source tags, attributes and CSS never pass through.
import { Ajv2020 } from "ajv/dist/2020.js";
import schema from "../../../../../schemas/source-model.v1.schema.json";

type Json = string | number | boolean | null | Json[] | { [key: string]: Json };
export type Expr =
  | { kind: "literal"; value: Json }
  | { kind: "slot"; slot: string; path?: string[] }
  | { kind: "item"; list: string; path?: string[] }
  | { kind: "opaque"; text: string; reads?: Expr[] };
type TypeRef =
  | { kind: "boolean" | "string" | "number" | "node" | "function" }
  | { kind: "enum"; values: Json[] }
  | { kind: "array"; item: TypeRef }
  | { kind: "object"; schema: string }
  | { kind: "unknown"; text?: string };
type Value =
  | { kind: "length"; value: number; unit: "px" | "rem" | "em" | "percent" | "pt" | "dp" }
  | { kind: "number"; value: number }
  | { kind: "color"; value: string }
  | { kind: "keyword"; value: string }
  | { kind: "token"; token: string; resolved?: Value }
  | { kind: "unresolved"; text: string; reason: string };
type Condition = {
  minWidth?: { value: number; unit: string };
  maxWidth?: { value: number; unit: string };
  interaction?: "hover" | "focus" | "pressed" | "disabled";
  colorScheme?: "light" | "dark";
  expr?: Expr;
};
type Declaration = {
  property: string;
  value: Value;
  when?: Condition;
  from: { text: string };
};
type Arm = { when: "otherwise" | { equals: Json } | { truthy: boolean }; children?: Node[] };
export type ModelNode = {
  id: string;
  kind: "fragment" | "element" | "instance" | "text" | "branch" | "list" | "outlet" | "placeholder";
  tag?: string;
  module?: string;
  component?: string;
  slot?: string;
  reason?: string;
  props?: Record<string, Expr>;
  children?: Node[];
  parts?: Expr[];
  on?: Expr;
  arms?: Arm[];
  source?: Expr;
  item?: string;
  itemType?: TypeRef;
  key?: Expr;
  style?: Declaration[];
  unresolvedStyle?: { text: string; reason?: string }[];
};
type Node = ModelNode;
type Slot = {
  id: string;
  name: string;
  kind: string;
  type: TypeRef;
  optional?: boolean;
  initial?: Expr;
};
type Component = {
  id: string;
  name: string;
  root: Node;
  slots?: Slot[];
};
export type Model = {
  schemaVersion: number;
  screens?: { id: string; name: string; route?: string; component: string }[];
  components?: Component[];
};

const validate = new Ajv2020({ strictRequired: false }).compile<Model>(schema);
export function decodeModel(model: unknown): Model {
  if (!validate(model) || !model.components?.length) throw Error("Invalid source model");
  return model;
}
export function readModel(text: string): Model {
  return decodeModel(JSON.parse(text));
}

export type ModelRenderContext = {
  /** Logical viewport width used to evaluate width conditions. */
  width?: number;
  colorScheme?: "light" | "dark";
};
export type ModelRenderResult = {
  /** Full inert document, for the sandboxed model workbench iframe. */
  html: string;
  /** Style and body markup, for mounting in a shadow root inside a map card. */
  markup: string;
  /** Model facts that could not be drawn, with their source text where the model records it. */
  unresolved: string[];
};

/** The replaceable renderer boundary: a frozen versioned model in, inert drawing out. */
export interface SourceModelRenderer {
  render(model: Model, componentId: string, context?: ModelRenderContext): ModelRenderResult;
}

const tags = new Set(
  "main section article header footer nav aside div span strong em h1 h2 h3 h4 h5 h6 p code pre a button label input textarea select option progress meter ul ol li dl dt dd small form output dialog table caption thead tbody tfoot tr th td br hr".split(
    " ",
  ),
);
const directCss: Record<string, string> = {
  display: "display",
  order: "order",
  overflowX: "overflow-x",
  direction: "flex-direction",
  alignItems: "align-items",
  justifyContent: "justify-content",
  wrap: "flex-wrap",
  grow: "flex-grow",
  shrink: "flex-shrink",
  flexBasis: "flex-basis",
  rowGap: "row-gap",
  columnGap: "column-gap",
  gap: "gap",
  paddingTop: "padding-top",
  paddingBottom: "padding-bottom",
  paddingStart: "padding-inline-start",
  paddingEnd: "padding-inline-end",
  marginTop: "margin-top",
  marginBottom: "margin-bottom",
  marginStart: "margin-inline-start",
  marginEnd: "margin-inline-end",
  width: "width",
  height: "height",
  minWidth: "min-width",
  maxWidth: "max-width",
  minHeight: "min-height",
  maxHeight: "max-height",
  color: "color",
  background: "background-color",
  borderColor: "border-color",
  opacity: "opacity",
  fontFamily: "font-family",
  fontSize: "font-size",
  fontWeight: "font-weight",
  fontStyle: "font-style",
  lineHeight: "line-height",
  letterSpacing: "letter-spacing",
  textAlign: "text-align",
  textDecorationLine: "text-decoration-line",
  borderCollapse: "border-collapse",
  verticalAlign: "vertical-align",
  textTransform: "text-transform",
  whiteSpace: "white-space",
  position: "position",
  top: "top",
  right: "right",
  bottom: "bottom",
  left: "left",
  zIndex: "z-index",
  borderTopLeftRadius: "border-top-left-radius",
  borderTopRightRadius: "border-top-right-radius",
  borderBottomRightRadius: "border-bottom-right-radius",
  borderBottomLeftRadius: "border-bottom-left-radius",
  borderStyle: "border-style",
  borderTopWidth: "border-top-width",
  borderTopColor: "border-top-color",
  borderTopStyle: "border-top-style",
  borderRightWidth: "border-right-width",
  borderRightColor: "border-right-color",
  borderRightStyle: "border-right-style",
  borderBottomWidth: "border-bottom-width",
  borderBottomColor: "border-bottom-color",
  borderBottomStyle: "border-bottom-style",
  borderLeftWidth: "border-left-width",
  borderLeftColor: "border-left-color",
  borderLeftStyle: "border-left-style",
  borderRadius: "border-radius",
  borderWidth: "border-width",
};
const compositeProperties = new Set([
  "dotRadius",
  "dotColor",
  "tileWidth",
  "tileHeight",
  "shadowX",
  "shadowY",
  "shadowBlur",
  "shadowSpread",
  "shadowColor",
]);

const baseCss = `
*{box-sizing:border-box}:host{display:block;color:#111;background:#fff;font:16px system-ui,sans-serif}body{margin:0;color:#111;background:#fff;font:16px system-ui,sans-serif}h1,h2,h3,h4,h5,h6,p,pre,ul,ol,dl,dd{margin:0}a{text-decoration:underline}button,input,textarea,select{font:inherit}.dui-placeholder{display:inline-block;max-width:100%;padding:1px 4px;border:1px dashed #999;border-radius:3px;color:#555;background:#f4f4f4;font:11px/1.4 ui-monospace,monospace;overflow:hidden;text-overflow:ellipsis;vertical-align:baseline}.dui-overlay{display:grid}.dui-overlay>*{grid-area:1/1}[data-model-module]{outline:1px dashed color-mix(in srgb,currentColor 24%,transparent);outline-offset:-1px}
`;

function cssValue(v: Value): string | undefined {
  if (v.kind === "token") return v.resolved && cssValue(v.resolved);
  if (v.kind === "length") {
    const units = { px: "px", rem: "rem", em: "em", percent: "%", pt: "px", dp: "px" } as const;
    return String(v.value) + units[v.unit];
  }
  if (v.kind === "number" || v.kind === "color") return String(v.value);
  if (v.kind === "keyword") {
    const keywords: Record<string, string> = {
      spaceBetween: "space-between",
      spaceAround: "space-around",
      spaceEvenly: "space-evenly",
      fill: "100%",
      fit: "fit-content",
    };
    return keywords[v.value] ?? v.value;
  }
}

type Resolved = { known: boolean; value?: unknown };
type Scope = { slots: Map<string, unknown>; items: Map<string, unknown> };
type NodeContent = { nodeContent: true; nodes: Node[]; scope: Scope };
const isNodeContent = (value: unknown): value is NodeContent =>
  typeof value === "object" && value !== null && (value as NodeContent).nodeContent === true;

function lookupPath(value: unknown, path: string[] | undefined): Resolved {
  let current = value;
  for (const key of path ?? []) {
    if (
      (typeof current !== "object" && typeof current !== "function") ||
      current === null ||
      !Object.hasOwn(current, key)
    )
      return { known: false };
    current = (current as Record<string, unknown>)[key];
  }
  return { known: true, value: current };
}
function resolveExpr(e: Expr, scope: Scope): Resolved {
  if (e.kind === "literal") return { known: true, value: e.value };
  if (e.kind === "slot") {
    if (!scope.slots.has(e.slot)) return { known: false };
    return lookupPath(scope.slots.get(e.slot), e.path);
  }
  if (e.kind === "item") {
    if (!scope.items.has(e.list)) return { known: false };
    return lookupPath(scope.items.get(e.list), e.path);
  }
  return { known: false };
}
function exprLabel(e: Expr): string {
  if (e.kind === "literal") return JSON.stringify(e.value);
  if (e.kind === "opaque") return e.text.length > 42 ? e.text.slice(0, 41) + "…" : e.text;
  const base = e.kind === "slot" ? e.slot : `${e.list} item`;
  const name = base.split("#").at(-1)?.split(".").at(-1) ?? base;
  return name + (e.path?.length ? "." + e.path.join(".") : "");
}
function sameJson(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}
function truthy(value: unknown): boolean {
  return Boolean(value);
}
function copyScope(scope: Scope): Scope {
  return { slots: new Map(scope.slots), items: new Map(scope.items) };
}
function lengthPx(length: { value: number; unit: string } | undefined): number | undefined {
  if (!length) return undefined;
  if (length.unit === "px" || length.unit === "pt" || length.unit === "dp") return length.value;
  if (length.unit === "rem" || length.unit === "em") return length.value * 16;
  return undefined;
}

function renderDom(model: Model, componentId: string, context: ModelRenderContext = {}) {
  const width = context.width && context.width > 0 ? context.width : 375;
  const colorScheme = context.colorScheme ?? "light";
  const doc = document.implementation.createHTMLDocument("");
  const policy = doc.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content =
    "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'";
  doc.head.append(policy);
  const style = doc.createElement("style");
  style.textContent = baseCss;
  doc.head.append(style);
  const components = new Map(model.components?.map((c) => [c.id, c]));
  const unresolved = new Set<string>();
  let budget = 10000;
  const note = (node: Node | undefined, text: string) => {
    unresolved.add((node ? node.id + " · " : "") + text);
  };
  const placeholder = (reason: string) => {
    unresolved.add(reason);
    const mark = doc.createElement("small");
    mark.className = "dui-placeholder";
    mark.textContent = "[" + reason + "]";
    return mark;
  };

  function conditionStatus(when: Condition | undefined, scope: Scope): "yes" | "no" | "unknown" {
    if (!when) return "yes";
    let unknown = false;
    const min = lengthPx(when.minWidth);
    const max = lengthPx(when.maxWidth);
    if (when.minWidth && min === undefined) unknown = true;
    if (when.maxWidth && max === undefined) unknown = true;
    if (min !== undefined && width < min) return "no";
    if (max !== undefined && width > max) return "no";
    if (when.interaction) return "no";
    if (when.colorScheme && when.colorScheme !== colorScheme) return "no";
    if (when.expr) {
      const value = resolveExpr(when.expr, scope);
      if (!value.known) unknown = true;
      else if (!truthy(value.value)) return "no";
    }
    return unknown ? "unknown" : "yes";
  }

  function applyStyles(el: HTMLElement, n: Node, scope: Scope) {
    const declarations = n.style ?? [];
    const declared = new Set(declarations.map((d) => d.property));
    const composite = new Map<string, string>();
    for (const d of declarations) {
      const applies = conditionStatus(d.when, scope);
      if (applies === "no") continue;
      if (applies === "unknown") {
        note(n, d.from.text);
        continue;
      }
      if (compositeProperties.has(d.property)) {
        const val = cssValue(d.value);
        if (val) composite.set(d.property, val);
        else note(n, d.from.text);
        continue;
      }
      if (d.property === "gridColumnMinimum") {
        if (d.value.kind === "keyword" && d.value.value === "single") {
          el.style.gridTemplateColumns = "minmax(0,1fr)";
          continue;
        }
        const min = cssValue(d.value);
        if (min && d.value.kind === "length") {
          el.style.gridTemplateColumns = `repeat(auto-fit,minmax(${min},1fr))`;
          continue;
        }
        note(n, d.from.text);
        continue;
      }
      if (d.property === "backdropBlur") {
        const amount = cssValue(d.value);
        if (amount && d.value.kind === "length") el.style.backdropFilter = `blur(${amount})`;
        else note(n, d.from.text);
        continue;
      }
      const key = directCss[d.property];
      const val = cssValue(d.value);
      if (!key || !val || !CSS.supports(key, val)) {
        note(n, d.from.text);
        continue;
      }
      let value = val;
      if (
        (d.property === "alignItems" || d.property === "justifyContent") &&
        (val === "start" || val === "end")
      )
        value = val === "start" ? "flex-start" : "flex-end";
      if (
        d.property === "direction" ||
        d.property === "alignItems" ||
        d.property === "justifyContent" ||
        d.property === "wrap"
      ) {
        if (d.property === "direction" && value === "overlay") {
          el.classList.add("dui-overlay");
          continue;
        }
        if (!el.style.display) el.style.display = "flex";
      }
      if (
        (d.property === "alignItems" || d.property === "justifyContent") &&
        (val === "flex-start" || val === "flex-end") &&
        !el.style.display
      )
        el.style.display = "flex";
      el.style.setProperty(key, value);
    }
    if (declared.has("borderWidth") && ![...declared].some((p) => p.endsWith("Style")))
      el.style.borderStyle = "solid";
    for (const [widthProperty, styleProperty, cssProperty] of [
      ["borderTopWidth", "borderTopStyle", "border-top-style"],
      ["borderRightWidth", "borderRightStyle", "border-right-style"],
      ["borderBottomWidth", "borderBottomStyle", "border-bottom-style"],
      ["borderLeftWidth", "borderLeftStyle", "border-left-style"],
    ] as const)
      if (declared.has(widthProperty) && !declared.has(styleProperty))
        el.style.setProperty(cssProperty, "solid");

    const get = (k: string) => composite.get(k);
    if (get("dotColor") && get("dotRadius"))
      el.style.backgroundImage = `radial-gradient(${get("dotColor")} ${get("dotRadius")},transparent ${get("dotRadius")})`;
    if (get("tileWidth") && get("tileHeight"))
      el.style.backgroundSize = `${get("tileWidth")} ${get("tileHeight")}`;
    const shadow = ["shadowX", "shadowY", "shadowBlur", "shadowSpread", "shadowColor"].map(get);
    if (shadow.every(Boolean)) el.style.boxShadow = shadow.join(" ");
    for (const u of n.unresolvedStyle ?? []) note(n, u.text);
  }

  function applyProps(el: HTMLElement, n: Node, scope: Scope) {
    const visualAttributes = new Set([
      "type",
      "checked",
      "disabled",
      "placeholder",
      "value",
      "selected",
      "multiple",
      "rows",
      "cols",
      "min",
      "max",
      "step",
      "alt",
      "title",
      "colspan",
      "rowspan",
      "for",
    ]);
    for (const [name, expr] of Object.entries(n.props ?? {})) {
      if (!visualAttributes.has(name)) continue;
      const value = resolveExpr(expr, scope);
      if (!value.known || value.value === undefined || value.value === null) continue;
      const attr = name === "for" ? "for" : name;
      if (typeof value.value === "boolean") {
        if (value.value) el.setAttribute(attr, "");
      } else if (typeof value.value === "string" || typeof value.value === "number") {
        el.setAttribute(attr, String(value.value));
      }
    }
    if (el instanceof HTMLInputElement || el instanceof HTMLButtonElement) el.tabIndex = -1;
  }

  function drawChildren(
    children: Node[] | undefined,
    scope: Scope,
    stack: string[],
    target: ParentNode,
  ) {
    for (const child of children ?? []) target.append(draw(child, scope, stack));
  }

  function component(id: string, props: Map<string, unknown>, stack: string[]): globalThis.Node {
    const c = components.get(id);
    if (!c || stack.includes(id) || stack.length >= 64) return placeholder("Unresolved component");
    const slots = new Map<string, unknown>();
    const scope: Scope = { slots, items: new Map() };
    for (const s of c.slots ?? []) {
      const initial = props.has(s.name)
        ? { known: true, value: props.get(s.name) }
        : s.initial
          ? resolveExpr(s.initial, scope)
          : { known: false };
      if (initial.known) slots.set(s.id, initial.value);
    }
    return draw(c.root, scope, [...stack, id]);
  }

  function draw(n: Node, scope: Scope, stack: string[]): globalThis.Node {
    if (--budget < 0) throw Error("Model exceeds preview capacity");
    if (n.kind === "fragment") {
      const group = doc.createDocumentFragment();
      drawChildren(n.children, scope, stack, group);
      return group;
    }
    if (n.kind === "instance") {
      const target = components.get(n.component!);
      const props = new Map(
        Object.entries(n.props ?? {}).map(([k, e]) => {
          const value = resolveExpr(e, scope);
          return [k, value.known ? value.value : undefined];
        }),
      );
      if (target && n.children?.length) {
        const content: NodeContent = {
          nodeContent: true,
          nodes: n.children,
          scope: copyScope(scope),
        };
        for (const slot of target.slots ?? [])
          if (slot.type.kind === "node" && slot.name === "children" && !props.has(slot.name))
            props.set(slot.name, content);
      }
      return component(n.component!, props, stack);
    }
    if (n.kind === "text") {
      const text = doc.createDocumentFragment();
      for (const expr of n.parts ?? []) {
        const value = resolveExpr(expr, scope);
        if (value.known && (typeof value.value === "string" || typeof value.value === "number"))
          text.append(String(value.value));
        else if (
          !value.known ||
          (typeof value.value === "object" && value.value !== null && value.value !== undefined)
        )
          text.append(placeholder(exprLabel(expr)));
      }
      return text;
    }
    if (n.kind === "branch") {
      const value = n.on ? resolveExpr(n.on, scope) : { known: false };
      if (!value.known) return placeholder(exprLabel(n.on ?? { kind: "opaque", text: "branch" }));
      const arm = (n.arms ?? []).find((arm) => {
        if (arm.when === "otherwise") return true;
        if ("equals" in arm.when) return sameJson(value.value, arm.when.equals);
        return truthy(value.value) === arm.when.truthy;
      });
      const group = doc.createDocumentFragment();
      drawChildren(arm?.children, scope, stack, group);
      return group;
    }
    if (n.kind === "list") {
      const value = n.source ? resolveExpr(n.source, scope) : { known: false };
      if (!value.known || !Array.isArray(value.value))
        return placeholder(exprLabel(n.source ?? { kind: "opaque", text: "list" }));
      const group = doc.createDocumentFragment();
      for (const item of value.value.slice(0, 200)) {
        const nested = copyScope(scope);
        nested.items.set(n.id, item);
        drawChildren(n.children, nested, stack, group);
      }
      return group;
    }
    if (n.kind === "outlet") {
      const value = scope.slots.get(n.slot!);
      if (!isNodeContent(value)) return doc.createDocumentFragment();
      const group = doc.createDocumentFragment();
      drawChildren(value.nodes, value.scope, stack, group);
      return group;
    }
    if (n.kind !== "element") return placeholder(n.reason ?? n.kind);
    if (n.module) {
      const el = doc.createElement("span");
      el.dataset.modelNode = n.id;
      el.dataset.modelModule = `${n.module} ${n.tag ?? ""}`.trim();
      applyStyles(el, n, scope);
      if (n.children?.length) drawChildren(n.children, scope, stack, el);
      else {
        note(n, `${n.module} ${n.tag ?? "component"}`);
        el.append(placeholder(n.tag ?? n.module));
      }
      return el;
    }
    if (!tags.has(n.tag!)) return placeholder(n.reason ?? n.tag ?? n.kind);
    const tag = n.tag === "body" ? "div" : n.tag!;
    const el = doc.createElement(tag);
    el.dataset.modelNode = n.id;
    applyStyles(el, n, scope);
    applyProps(el, n, scope);
    drawChildren(n.children, scope, stack, el);
    return el;
  }

  const root = component(componentId, new Map(), []);
  doc.body.append(root);
  const markup = `<style>${baseCss}</style>` + doc.body.innerHTML;
  return {
    html: "<!doctype html>" + doc.documentElement.outerHTML,
    markup,
    unresolved: [...unresolved],
  };
}

/** The v0.3.0 approximate renderer: TypeScript/DOM, model-only, and replaceable. */
export function createDomModelRenderer(): SourceModelRenderer {
  return { render: renderDom };
}

const defaultRenderer = createDomModelRenderer();
export function drawModel(model: Model, componentId: string, context?: ModelRenderContext) {
  return defaultRenderer.render(model, componentId, context);
}
