// Draw the versioned model in an inert document. Source tags, attributes and CSS never pass through.
import { Ajv2020 } from "ajv/dist/2020.js";
import schema from "../../../../../schemas/source-model.v1.schema.json";

type Expr = { kind: string; value?: unknown; slot?: string; path?: string[] };
type Value = { kind: string; value?: string | number; unit?: string; resolved?: Value };
type Declaration = { property: string; value: Value; when?: unknown; from: { text: string } };
type Node = {
  id: string;
  kind: string;
  tag?: string;
  module?: string;
  component?: string;
  props?: Record<string, Expr>;
  children?: Node[];
  parts?: Expr[];
  style?: Declaration[];
  reason?: string;
  unresolvedStyle?: { text: string }[];
};
type Component = {
  id: string;
  name: string;
  root: Node;
  slots?: { id: string; name: string; initial?: Expr }[];
};
export type Model = {
  screens?: { id: string; name: string; component: string }[];
  components?: Component[];
};
const validate = new Ajv2020({ strictRequired: false }).compile<Model>(schema);
export function readModel(text: string): Model {
  const model: unknown = JSON.parse(text);
  if (!validate(model) || !model.components?.length) throw Error("Invalid source model");
  return model;
}

const tags = new Set(
  "body dialog select option main section article header footer nav aside div span strong em h1 h2 h3 h4 h5 h6 p code pre a button label ul ol li dl dt dd small form output".split(
    " ",
  ),
);
const css: Record<string, string> = {
  direction: "flex-direction",
  wrap: "flex-wrap",
  grow: "flex-grow",
  background: "background-color",
  paddingStart: "padding-inline-start",
  paddingEnd: "padding-inline-end",
  marginStart: "margin-inline-start",
  marginEnd: "margin-inline-end",
};
function cssValue(v: Value): string | undefined {
  if (v.kind === "token") return v.resolved && cssValue(v.resolved);
  if (v.kind === "length")
    return String(v.value) + ({ percent: "%", dp: "px", pt: "px" }[v.unit ?? ""] ?? v.unit);
  if (v.kind === "number" || v.kind === "color") return String(v.value);
  if (v.kind === "keyword") {
    const keywords: Record<string, string> = {
      spaceBetween: "space-between",
      spaceAround: "space-around",
      spaceEvenly: "space-evenly",
      fill: "100%",
      fit: "fit-content",
    };
    return keywords[String(v.value)] ?? String(v.value);
  }
}
function resolveExpr(e: Expr, slots: Map<string, unknown>): unknown {
  if (e.kind === "literal") return e.value;
  if (e.kind !== "slot") return undefined;
  let value = slots.get(e.slot!);
  for (const key of e.path ?? []) {
    if (!value || typeof value !== "object" || !Object.hasOwn(value, key)) return undefined;
    value = (value as Record<string, unknown>)[key];
  }
  return value;
}

export function drawModel(model: Model, componentId: string) {
  const doc = document.implementation.createHTMLDocument("");
  const policy = doc.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content =
    "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'";
  doc.head.append(policy);
  const style = doc.createElement("style");
  style.textContent =
    "*{box-sizing:border-box}body{margin:0;font:16px system-ui,sans-serif}h1,h2,h3,h4,h5,h6,p,pre,ul,ol,dl,dd{margin:0}a{text-decoration:underline}button{font:inherit}";
  doc.head.append(style);
  const components = new Map(model.components?.map((c) => [c.id, c]));
  const unresolved = new Set<string>();
  let budget = 10000;
  const placeholder = (reason: string) => {
    unresolved.add(reason);
    const mark = doc.createElement("small");
    mark.textContent = "[" + reason + "]";
    return mark;
  };
  function component(id: string, props: Map<string, unknown>, stack: string[]): globalThis.Node {
    const c = components.get(id);
    if (!c || stack.includes(id) || stack.length >= 64) return placeholder("Unresolved component");
    const slots = new Map<string, unknown>();
    for (const s of c.slots ?? [])
      slots.set(
        s.id,
        props.has(s.name) ? props.get(s.name) : s.initial && resolveExpr(s.initial, slots),
      );
    return draw(c.root, slots, [...stack, id]);
  }
  function draw(n: Node, slots: Map<string, unknown>, stack: string[]): globalThis.Node {
    if (--budget < 0) throw Error("Model exceeds preview capacity");
    if (n.kind === "instance")
      return component(
        n.component!,
        new Map(Object.entries(n.props ?? {}).map(([k, e]) => [k, resolveExpr(e, slots)])),
        stack,
      );
    if (n.kind === "text") {
      const text = doc.createDocumentFragment();
      for (const expr of n.parts ?? []) {
        const value = resolveExpr(expr, slots);
        if (typeof value === "string" || typeof value === "number") text.append(String(value));
        else if (value === undefined || (typeof value === "object" && value !== null))
          text.append(placeholder(expr.kind));
      }
      return text;
    }
    if (n.kind !== "element" || n.module || !tags.has(n.tag!))
      return placeholder(n.reason ?? n.tag ?? n.kind);
    const el = n.tag === "body" ? doc.body : doc.createElement(n.tag!);
    el.dataset.modelNode = n.id;
    for (const d of n.style ?? []) {
      if (d.property === "gridColumnMinimum" && !d.when) {
        const min = cssValue(d.value);
        if (min === "single") {
          el.style.gridTemplateColumns = "minmax(0,1fr)";
          continue;
        }
        if (min && d.value.kind === "length") {
          el.style.gridTemplateColumns = `repeat(auto-fit,minmax(${min},1fr))`;
          continue;
        }
      }
      const key = css[d.property] ?? d.property.replace(/[A-Z]/g, (ch) => "-" + ch.toLowerCase());
      const val = cssValue(d.value);
      if (!val || d.when || !CSS.supports(key, val)) {
        unresolved.add(n.id + " · " + d.from.text);
        continue;
      }
      if (
        d.property === "direction" ||
        d.property === "alignItems" ||
        d.property === "justifyContent" ||
        d.property === "wrap"
      )
        if (!el.style.display) el.style.display = "flex";
      if (d.property === "borderWidth") el.style.borderStyle = "solid";
      el.style.setProperty(key, val);
    }
    for (const u of n.unresolvedStyle ?? []) unresolved.add(n.id + " · " + u.text);
    for (const child of n.children ?? []) el.append(draw(child, slots, stack));
    return el;
  }
  const root = component(componentId, new Map(), []);
  if (root !== doc.body) doc.body.append(root);
  return { html: "<!doctype html>" + doc.documentElement.outerHTML, unresolved: [...unresolved] };
}
