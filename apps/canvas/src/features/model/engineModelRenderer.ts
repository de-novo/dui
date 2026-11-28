import {
  AlignItems,
  Display,
  FlexDirection,
  FlexWrap,
  JustifyContent,
  Position,
  Style,
  TaffyTree,
  loadTaffy,
} from "taffy-layout";
import * as hbModule from "harfbuzzjs";
import roboto400Url from "typeface-roboto/files/roboto-latin-400.woff2?url";
import roboto700Url from "typeface-roboto/files/roboto-latin-700.woff2?url";
import type {
  Model,
  ModelNode,
  ModelRenderContext,
  ModelRenderResult,
  SourceModelRenderer,
} from "./modelDocument";

type Computed = {
  values: Record<string, unknown>;
  fontFamily: string;
  fontSize: number;
  fontWeight: number;
  fontStyle: string;
  lineHeight: number;
  letterSpacing: number;
  color: string;
  textAlign: string;
  display: string;
  unresolved: string[];
};

type Scene = {
  kind: "element" | "text";
  id: string;
  tag?: string;
  node?: ModelNode;
  attrs?: Record<string, unknown>;
  text?: string;
  computed: Computed;
  children: Scene[];
  lines?: string[];
  lineWidths?: number[];
  layout?: { x: number; y: number; width: number; height: number; padding: any; border: any };
  parent?: Scene;
};

const INHERITED = new Set([
  "color",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "fontStyle",
  "lineHeight",
  "letterSpacing",
  "textAlign",
]);
const BLOCK_TAGS = new Set([
  "address",
  "article",
  "aside",
  "blockquote",
  "body",
  "dd",
  "details",
  "dialog",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hr",
  "html",
  "li",
  "main",
  "nav",
  "ol",
  "p",
  "pre",
  "section",
  "summary",
  "table",
  "ul",
]);
const TEXT_FONT_CACHE = new Map<string, CanvasRenderingContext2D>();

function esc(value: unknown): string {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

type Scope = { slots: Map<string, unknown>; items: Map<string, unknown> };
function lookupPath(value: unknown, path: string[] | undefined) {
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
function resolveExpr(expr: any, scope: Scope) {
  if (!expr) return { known: false };
  if (expr.kind === "literal") return { known: true, value: expr.value };
  if (expr.kind === "slot") {
    if (!scope.slots.has(expr.slot)) return { known: false };
    return lookupPath(scope.slots.get(expr.slot), expr.path);
  }
  if (expr.kind === "item") {
    if (!scope.items.has(expr.list)) return { known: false };
    return lookupPath(scope.items.get(expr.list), expr.path);
  }
  return { known: false };
}
function truthyValue(value: unknown) {
  return Boolean(value);
}
function sameJson(a: unknown, b: unknown) {
  if (Object.is(a, b)) return true;
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}
function copyScope(scope: Scope): Scope {
  return { slots: new Map(scope.slots), items: new Map(scope.items) };
}
function conditionApplies(condition: any, context: ModelRenderContext, scope: Scope): boolean {
  if (!condition) return true;
  const width = context.width ?? 375;
  const initial = context.initialFontSize ?? 16;
  const lengthPx = (x: any) =>
    x
      ? ["px", "pt", "dp"].includes(x.unit)
        ? x.value
        : ["em", "rem"].includes(x.unit)
          ? x.value * initial
          : undefined
      : undefined;
  const min = lengthPx(condition.minWidth),
    max = lengthPx(condition.maxWidth);
  if (min !== undefined && width < min) return false;
  if (max !== undefined && width > max) return false;
  if (condition.colorScheme && (context.colorScheme ?? "light") !== condition.colorScheme)
    return false;
  if (condition.interaction) return false;
  if (condition.expr) {
    const value = resolveExpr(condition.expr, scope);
    if (!value.known || !truthyValue(value.value)) return false;
  }
  return true;
}

function primitive(value: any): unknown {
  if (value == null || typeof value !== "object") return value;
  if ((value.kind ?? value.type) === "number") return value.value;
  if ((value.kind ?? value.type) === "keyword") return value.value;
  if (
    (value.kind ?? value.type) === "unresolved" &&
    value.refinement &&
    typeof value.refinement === "object"
  )
    return primitive(value.refinement);
  return undefined;
}

function lengthValue(
  value: any,
  percentBase: number,
  fontSize: number,
): number | `${number}%` | undefined {
  const raw = primitive(value);
  if (typeof raw === "number") return raw;
  if (typeof value === "object" && (value?.kind ?? value?.type) === "length") {
    const n = Number(value.value);
    if (!Number.isFinite(n)) return undefined;
    switch (value.unit) {
      case "px":
        return n;
      case "rem":
        return n * 16;
      case "em":
        return n * fontSize;
      case "vw":
      case "svw":
      case "lvw":
      case "dvw":
        return (n * percentBase) / 100;
      case "vh":
      case "svh":
      case "lvh":
      case "dvh":
        return (n * percentBase) / 100;
      case "percent":
      case "%":
        return `${n}%` as `${number}%`;
      default:
        return undefined;
    }
  }
  if (typeof value === "object" && (value?.kind ?? value?.type) === "clamp") {
    const preferred = lengthValue(value.preferred, percentBase, fontSize);
    return typeof preferred === "number" ? preferred : undefined;
  }
  return undefined;
}

function numberValue(value: any): number | undefined {
  const raw = primitive(value);
  return typeof raw === "number" && Number.isFinite(raw) ? raw : undefined;
}

function stringValue(value: any): string | undefined {
  const raw = primitive(value);
  if (typeof raw === "string") return raw;
  if (typeof raw === "number") return String(raw);
  return undefined;
}

function colorValue(value: any): string | undefined {
  if (!value || typeof value !== "object") return typeof value === "string" ? value : undefined;
  if ((value.kind ?? value.type) === "unresolved" && value.refinement)
    return colorValue(value.refinement);
  if ((value.kind ?? value.type) !== "color") return undefined;
  return String(value.value);
}

function declarationMap(
  node: any,
  context: ModelRenderContext,
  scope: Scope,
): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const declaration of node.style ?? []) {
    if (!conditionApplies(declaration.when, context, scope)) continue;
    values[declaration.property] = declaration.value;
  }
  return values;
}

function defaultDisplay(tag: string): string {
  if (
    tag === "span" ||
    tag === "a" ||
    tag === "strong" ||
    tag === "em" ||
    tag === "small" ||
    tag === "code"
  )
    return "inline";
  if (
    tag === "input" ||
    tag === "textarea" ||
    tag === "select" ||
    tag === "button" ||
    tag === "img" ||
    tag === "svg"
  )
    return "inline-block";
  return BLOCK_TAGS.has(tag) ? "block" : "inline-block";
}

function computeStyle(
  node: any,
  parent: Computed | undefined,
  context: ModelRenderContext,
  scope: Scope,
): Computed {
  const values = declarationMap(node, context, scope);
  const inheritedFrom = parent ?? rootComputed(context, undefined);
  const computed: Computed = {
    values,
    fontFamily: inheritedFrom.fontFamily,
    fontSize: inheritedFrom.fontSize,
    fontWeight: inheritedFrom.fontWeight,
    fontStyle: inheritedFrom.fontStyle,
    lineHeight: inheritedFrom.lineHeight,
    letterSpacing: inheritedFrom.letterSpacing,
    color: inheritedFrom.color,
    textAlign: inheritedFrom.textAlign,
    display: defaultDisplay(String(node.tag ?? "div")),
    unresolved: [...(parent?.unresolved ?? [])],
  };
  for (const [property, value] of Object.entries(values)) {
    if (
      value &&
      typeof value === "object" &&
      ((value as any).kind ?? (value as any).type) === "unresolved"
    ) {
      if (!(value as any).refinement) computed.unresolved.push(`${node.id}:${property}`);
    }
    switch (property) {
      case "display": {
        const display = stringValue(value);
        if (display) computed.display = display;
        break;
      }
      case "fontFamily": {
        const family = stringValue(value);
        if (family) computed.fontFamily = family;
        break;
      }
      case "fontSize": {
        const size = lengthValue(value, context.width ?? 375, inheritedFrom.fontSize);
        if (typeof size === "number") computed.fontSize = size;
        break;
      }
      case "fontWeight": {
        const weight = numberValue(value) ?? Number(stringValue(value));
        if (Number.isFinite(weight)) computed.fontWeight = weight;
        else if (stringValue(value) === "bold") computed.fontWeight = 700;
        else if (stringValue(value) === "normal") computed.fontWeight = 400;
        break;
      }
      case "fontStyle": {
        const style = stringValue(value);
        if (style) computed.fontStyle = style;
        break;
      }
      case "lineHeight": {
        const lineHeight =
          lengthValue(value, context.width ?? 375, computed.fontSize) ?? numberValue(value);
        if (typeof lineHeight === "number")
          computed.lineHeight = lineHeight > 4 ? lineHeight : lineHeight * computed.fontSize;
        break;
      }
      case "letterSpacing": {
        const spacing = lengthValue(value, context.width ?? 375, computed.fontSize);
        if (typeof spacing === "number") computed.letterSpacing = spacing;
        break;
      }
      case "color": {
        const color = colorValue(value) ?? stringValue(value);
        if (color) computed.color = color;
        break;
      }
      case "textAlign": {
        const align = stringValue(value);
        if (align) computed.textAlign = align;
        break;
      }
    }
  }
  if (node.tag === "strong" || node.tag === "b")
    computed.fontWeight = Math.max(computed.fontWeight, 700);
  if (node.tag === "em" || node.tag === "i") computed.fontStyle = "italic";
  if (/^h[1-6]$/.test(node.tag ?? "")) {
    computed.display = "block";
    computed.fontWeight = Math.max(computed.fontWeight, 700);
    computed.fontSize =
      { h1: 32, h2: 24, h3: 18.72, h4: 16, h5: 13.28, h6: 10.72 }[node.tag as "h1"] ??
      computed.fontSize;
  }
  for (const item of node.unresolvedStyle ?? [])
    computed.unresolved.push(`${node.id} · ${item.text}`);
  return computed;
}

function rootComputed(context: ModelRenderContext, documentStyle: any): Computed {
  const values: Record<string, unknown> = {};
  const root: Computed = {
    values,
    fontFamily: "system-ui",
    fontSize: context.initialFontSize ?? 16,
    fontWeight: 400,
    fontStyle: "normal",
    lineHeight: (context.initialFontSize ?? 16) * 1.2,
    letterSpacing: 0,
    color: context.colorScheme === "dark" ? "rgb(255 255 255 / 1)" : "rgb(0 0 0 / 1)",
    textAlign: "start",
    display: "block",
    unresolved: [],
  };
  for (const target of [documentStyle?.root, documentStyle?.body]) {
    for (const declaration of target?.style ?? []) {
      if (!conditionApplies(declaration.when, context, { slots: new Map(), items: new Map() }))
        continue;
      if (!INHERITED.has(declaration.property)) continue;
      const pseudo = { id: "document", tag: "body", style: [declaration] };
      const next = computeStyle(pseudo, root, context, { slots: new Map(), items: new Map() });
      Object.assign(root, {
        fontFamily: next.fontFamily,
        fontSize: next.fontSize,
        fontWeight: next.fontWeight,
        fontStyle: next.fontStyle,
        lineHeight: next.lineHeight,
        letterSpacing: next.letterSpacing,
        color: next.color,
        textAlign: next.textAlign,
      });
    }
  }
  return root;
}

class TextEngine {
  private constructor(
    private readonly hb: any,
    private readonly robotoRegular: any,
    private readonly robotoBold: any,
  ) {}

  static async create(): Promise<TextEngine> {
    const hb = hbModule as any;
    const load = async (url: string) => {
      const bytes = new Uint8Array(await (await fetch(url)).arrayBuffer());
      const font = new hb.Font(new hb.Face(new hb.Blob(bytes)));
      return font;
    };
    try {
      return new TextEngine(hb, await load(roboto400Url), await load(roboto700Url));
    } catch {
      return new TextEngine(hb, undefined, undefined);
    }
  }

  private canvasMeasure(text: string, computed: Computed): number {
    const key = `${computed.fontStyle} ${computed.fontWeight} ${computed.fontSize}px ${computed.fontFamily}`;
    let context = TEXT_FONT_CACHE.get(key);
    if (!context) {
      context = document.createElement("canvas").getContext("2d") ?? (undefined as any);
      if (!context) return text.length * computed.fontSize * 0.55;
      context.font = key;
      TEXT_FONT_CACHE.set(key, context);
    }
    return context.measureText(text).width + Math.max(0, text.length - 1) * computed.letterSpacing;
  }

  private hbMeasure(text: string, computed: Computed): number | undefined {
    if (!/roboto/i.test(computed.fontFamily) || !this.robotoRegular || !this.hb) return undefined;
    const font =
      computed.fontWeight >= 600 && this.robotoBold ? this.robotoBold : this.robotoRegular;
    const scale = Math.round(computed.fontSize * 64);
    font.setScale(scale, scale);
    const buffer = new this.hb.Buffer();
    try {
      buffer.addText(text);
      buffer.guessSegmentProperties();
      this.hb.shape(font, buffer);
      const positions = buffer.getGlyphPositions();
      const units = positions.reduce((sum: number, position: any) => sum + position.xAdvance, 0);
      return units / 64 + Math.max(0, text.length - 1) * computed.letterSpacing;
    } catch {
      return undefined;
    } finally {
      buffer.destroy?.();
    }
  }

  width(text: string, computed: Computed): number {
    return this.hbMeasure(text, computed) ?? this.canvasMeasure(text, computed);
  }

  wrap(text: string, computed: Computed, maxWidth: number): string[] {
    const paragraphs = text.replaceAll("\r\n", "\n").split("\n");
    const output: string[] = [];
    for (const paragraph of paragraphs) {
      if (!paragraph) {
        output.push("");
        continue;
      }
      const words = paragraph.split(/(\s+)/).filter((part) => part.length);
      let line = "";
      for (const word of words) {
        const candidate = line + word;
        if (line && !/^\s+$/.test(word) && this.width(candidate.trimEnd(), computed) > maxWidth) {
          output.push(line.trimEnd());
          line = word.trimStart();
          while (line && this.width(line, computed) > maxWidth && line.length > 1) {
            let cut = line.length - 1;
            while (cut > 1 && this.width(line.slice(0, cut), computed) > maxWidth) cut -= 1;
            output.push(line.slice(0, cut));
            line = line.slice(cut);
          }
        } else {
          line = candidate;
        }
      }
      output.push(line.trimEnd());
    }
    return output.length ? output : [""];
  }

  measure(
    text: string,
    computed: Computed,
    width?: number,
  ): { width: number; height: number; lines: string[] } {
    const lineHeight = computed.lineHeight || computed.fontSize * 1.2;
    if (width !== undefined && Number.isFinite(width)) {
      const lines = this.wrap(text, computed, Math.max(1, width));
      return {
        width: Math.min(width, Math.max(0, ...lines.map((line) => this.width(line, computed)))),
        height: Math.max(1, lines.length) * lineHeight,
        lines,
      };
    }
    const lines = text.replaceAll("\r\n", "\n").split("\n");
    return {
      width: Math.max(0, ...lines.map((line) => this.width(line, computed))),
      height: Math.max(1, lines.length) * lineHeight,
      lines,
    };
  }
}

function isNodeContent(value: unknown) {
  return typeof value === "object" && value !== null && (value as any).nodeContent === true;
}
function materializeNode(
  node: any,
  model: Model,
  context: ModelRenderContext,
  parentComputed: Computed,
  scope: Scope,
  parent?: Scene,
  stack: string[] = [],
): Scene[] {
  if (!node) return [];
  if (node.kind === "fragment")
    return materialize(node.children ?? [], model, context, parentComputed, scope, parent, stack);
  if (node.kind === "text") {
    let text = "";
    for (const expr of node.parts ?? []) {
      const value = resolveExpr(expr, scope);
      if (value.known && (typeof value.value === "string" || typeof value.value === "number"))
        text += String(value.value);
    }
    return text
      ? [
          {
            kind: "text",
            id: node.id,
            text,
            computed: { ...parentComputed, values: {} },
            children: [],
            parent,
          },
        ]
      : [];
  }
  if (node.kind === "branch") {
    const value = node.on ? resolveExpr(node.on, scope) : { known: false };
    if (!value.known) return [];
    const arm = (node.arms ?? []).find((arm: any) =>
      arm.when === "otherwise"
        ? true
        : "equals" in arm.when
          ? sameJson(value.value, arm.when.equals)
          : truthyValue(value.value) === arm.when.truthy,
    );
    return materialize(arm?.children ?? [], model, context, parentComputed, scope, parent, stack);
  }
  if (node.kind === "list") {
    const value = node.source ? resolveExpr(node.source, scope) : { known: false };
    if (!value.known || !Array.isArray(value.value)) return [];
    const out: Scene[] = [];
    for (const item of value.value.slice(0, 200)) {
      const nested = copyScope(scope);
      nested.items.set(node.id, item);
      out.push(
        ...materialize(node.children ?? [], model, context, parentComputed, nested, parent, stack),
      );
    }
    return out;
  }
  if (node.kind === "outlet") {
    const value = scope.slots.get(node.slot);
    if (!isNodeContent(value)) return [];
    return materialize(
      (value as any).nodes,
      model,
      context,
      parentComputed,
      (value as any).scope,
      parent,
      stack,
    );
  }
  if (node.kind === "instance") {
    const target = model.components?.find((c: any) => c.id === node.component);
    if (!target || stack.includes(node.component) || stack.length >= 64) return [];
    const props = new Map<string, unknown>();
    for (const [key, expr] of Object.entries(node.props ?? {})) {
      const resolved = resolveExpr(expr, scope);
      if (resolved.known) props.set(key, resolved.value);
    }
    if (node.children?.length) {
      const content = { nodeContent: true, nodes: node.children, scope: copyScope(scope) };
      for (const slot of target.slots ?? [])
        if (slot.type?.kind === "node" && slot.name === "children" && !props.has(slot.name))
          props.set(slot.name, content);
    }
    const nested: Scope = { slots: new Map(), items: new Map() };
    for (const slot of target.slots ?? []) {
      const initial = props.has(slot.name)
        ? { known: true, value: props.get(slot.name) }
        : slot.initial
          ? resolveExpr(slot.initial, nested)
          : { known: false };
      if (initial.known) nested.slots.set(slot.id, initial.value);
    }
    return materializeNode(target.root, model, context, parentComputed, nested, parent, [
      ...stack,
      node.component,
    ]);
  }
  if (node.kind !== "element") return [];
  const computed = computeStyle(node, parentComputed, context, scope);
  const attrs: Record<string, unknown> = {};
  for (const [name, expr] of Object.entries(node.props ?? {})) {
    const resolved = resolveExpr(expr, scope);
    if (resolved.known && resolved.value !== undefined && resolved.value !== null)
      attrs[name] = resolved.value;
  }
  if (attrs.value === undefined && attrs.defaultValue !== undefined)
    attrs.value = attrs.defaultValue;
  if (attrs.checked === undefined && attrs.defaultChecked !== undefined)
    attrs.checked = attrs.defaultChecked;
  const scene: Scene = {
    kind: "element",
    id: node.id,
    tag: node.tag,
    node,
    attrs,
    computed,
    children: [],
    parent,
  };
  scene.children = materialize(node.children ?? [], model, context, computed, scope, scene, stack);
  return [scene];
}
function materialize(
  nodes: any[],
  model: Model,
  context: ModelRenderContext,
  parentComputed: Computed,
  scope: Scope,
  parent?: Scene,
  stack: string[] = [],
): Scene[] {
  return nodes.flatMap((node) =>
    materializeNode(node, model, context, parentComputed, scope, parent, stack),
  );
}

function keywordToAlign(value: unknown): AlignItems | undefined {
  switch (stringValue(value)) {
    case "start":
      return AlignItems.Start;
    case "end":
      return AlignItems.End;
    case "flex-start":
      return AlignItems.FlexStart;
    case "flex-end":
      return AlignItems.FlexEnd;
    case "center":
      return AlignItems.Center;
    case "baseline":
      return AlignItems.Baseline;
    case "stretch":
      return AlignItems.Stretch;
    default:
      return undefined;
  }
}

function keywordToJustify(value: unknown): JustifyContent | undefined {
  switch (stringValue(value)) {
    case "start":
      return JustifyContent.Start;
    case "end":
      return JustifyContent.End;
    case "flex-start":
      return JustifyContent.FlexStart;
    case "flex-end":
      return JustifyContent.FlexEnd;
    case "center":
      return JustifyContent.Center;
    case "space-between":
      return JustifyContent.SpaceBetween;
    case "space-around":
      return JustifyContent.SpaceAround;
    case "space-evenly":
      return JustifyContent.SpaceEvenly;
    default:
      return undefined;
  }
}

function hasLayoutDeclaration(values: Record<string, unknown>, names: string[]) {
  return names.some((name) => values[name] !== undefined);
}

function logicalRect(
  scene: Scene,
  context: ModelRenderContext,
  prefix: "padding" | "margin",
  style: Style,
) {
  const object: any = { left: 0, right: 0, top: 0, bottom: 0 };
  for (const [suffix, key] of [
    ["Start", "left"],
    ["End", "right"],
    ["Top", "top"],
    ["Bottom", "bottom"],
  ] as const) {
    const v = lengthValue(
      scene.computed.values[`${prefix}${suffix}`],
      context.width ?? 375,
      scene.computed.fontSize,
    );
    object[key] = typeof v === "number" || typeof v === "string" ? v : 0;
  }
  (style as any)[prefix] = object;
  return object;
}

function borderRect(scene: Scene, context: ModelRenderContext, style: Style) {
  const object: any = { left: 0, right: 0, top: 0, bottom: 0 };
  const all = lengthValue(
    scene.computed.values.borderWidth,
    context.width ?? 375,
    scene.computed.fontSize,
  );
  for (const [suffix, key] of [
    ["Left", "left"],
    ["Right", "right"],
    ["Top", "top"],
    ["Bottom", "bottom"],
  ] as const) {
    const v =
      lengthValue(
        scene.computed.values[`border${suffix}Width`],
        context.width ?? 375,
        scene.computed.fontSize,
      ) ?? all;
    object[key] = typeof v === "number" ? v : 0;
  }
  style.border = object;
  return object;
}

function gridTemplate(
  value: unknown,
  context: ModelRenderContext,
  fontSize: number,
): any[] | undefined {
  const text = stringValue(value);
  if (!text) return undefined;
  const tracks: any[] = [];
  for (const part of text.trim().split(/\s+/)) {
    if (part.endsWith("fr"))
      tracks.push({ min: "auto", max: `${Number(part.slice(0, -2)) || 1}fr` });
    else if (part === "auto") tracks.push({ min: "auto", max: "auto" });
    else {
      const unit = part.endsWith("%") ? "percent" : part.endsWith("px") ? "px" : "px";
      const n = lengthValue(
        { kind: "length", value: Number.parseFloat(part), unit },
        context.width ?? 375,
        fontSize,
      );
      if (n !== undefined) tracks.push({ min: n, max: n });
    }
  }
  return tracks.length ? tracks : undefined;
}

function controlTextWidth(text: string): number {
  const canvas = document.createElement("canvas").getContext("2d");
  if (!canvas) return text.length * 7.5;
  canvas.font = "normal 400 13.333px Arial";
  return canvas.measureText(text).width;
}

function taffyStyle(scene: Scene, context: ModelRenderContext): Style {
  const style = new Style();
  const computed = scene.computed;
  const values = computed.values;
  const inlineLike = computed.display === "inline" || computed.display === "inline-block";
  const flexLike =
    computed.display === "flex" ||
    computed.display === "inline-flex" ||
    hasLayoutDeclaration(values, ["direction", "alignItems", "justifyContent", "wrap"]);
  const inlineContainer =
    scene.children.length > 0 &&
    scene.children.every(
      (child) =>
        child.kind === "text" ||
        child.computed.display === "inline" ||
        child.computed.display === "inline-block",
    );
  if (computed.display === "none") style.display = Display.None;
  else if (computed.display === "grid") style.display = Display.Grid;
  else if (flexLike || inlineLike || inlineContainer) style.display = Display.Flex;
  else style.display = Display.Block;
  if (inlineLike || inlineContainer) {
    style.flexWrap = FlexWrap.Wrap;
    style.alignItems = AlignItems.Baseline;
  }
  const direction = stringValue(values.direction);
  if (direction === "column") style.flexDirection = FlexDirection.Column;
  else if (direction === "row-reverse") style.flexDirection = FlexDirection.RowReverse;
  else if (direction === "column-reverse") style.flexDirection = FlexDirection.ColumnReverse;
  else style.flexDirection = FlexDirection.Row;
  const wrap = stringValue(values.wrap);
  if (wrap)
    style.flexWrap =
      wrap === "wrap"
        ? FlexWrap.Wrap
        : wrap === "wrap-reverse"
          ? FlexWrap.WrapReverse
          : FlexWrap.NoWrap;
  const align = keywordToAlign(values.alignItems);
  if (align !== undefined) style.alignItems = align;
  const justify = keywordToJustify(values.justifyContent);
  if (justify !== undefined) style.justifyContent = justify;
  const grow = numberValue(values.grow);
  if (grow !== undefined) style.flexGrow = grow;
  const shrink = numberValue(values.shrink);
  if (shrink !== undefined) style.flexShrink = shrink;
  const basis = lengthValue(values.flexBasis, context.width ?? 375, computed.fontSize);
  if (basis !== undefined) style.flexBasis = basis;
  for (const prop of [
    "width",
    "height",
    "minWidth",
    "minHeight",
    "maxWidth",
    "maxHeight",
  ] as const) {
    const v = lengthValue(values[prop], context.width ?? 375, computed.fontSize);
    if (v !== undefined) (style as any)[prop] = v;
  }
  if (scene.tag === "input" && values.width === undefined)
    style.width = scene.attrs?.type === "checkbox" || scene.attrs?.type === "radio" ? 13 : 153;
  if (scene.tag === "input" && values.height === undefined)
    style.height = scene.attrs?.type === "checkbox" || scene.attrs?.type === "radio" ? 13 : 21;
  if (scene.tag === "button" && values.width === undefined)
    style.width = controlTextWidth(scene.text ?? "") + 16;
  if (scene.tag === "button" && values.height === undefined) style.height = 21;
  if (scene.tag === "select" && values.width === undefined) {
    const selected = scene.attrs?.defaultValue ?? scene.attrs?.value;
    const selectedChild =
      scene.children.find(
        (child) =>
          selected !== undefined && String(child.attrs?.value ?? child.text) === String(selected),
      ) ?? scene.children[0];
    style.width = Math.max(27, controlTextWidth(selectedChild?.text ?? "") + 18);
  }
  if (scene.tag === "select" && values.height === undefined) style.height = 19;
  if (!values.width && scene.kind === "element" && !inlineLike && style.display === Display.Block)
    style.width = "100%";
  logicalRect(scene, context, "padding", style);
  logicalRect(scene, context, "margin", style);
  if (scene.tag === "input" && (scene.attrs?.type === "checkbox" || scene.attrs?.type === "radio"))
    style.margin = { left: 4, right: 3, top: 3, bottom: 3 };
  if (scene.tag === "select") style.margin = { left: 0, right: 0, top: 1, bottom: 1 };
  borderRect(scene, context, style);
  const gap = lengthValue(values.gap, context.width ?? 375, computed.fontSize);
  if (gap !== undefined) style.gap = { width: gap, height: gap };
  const rowGap = lengthValue(values.rowGap, context.width ?? 375, computed.fontSize);
  const columnGap = lengthValue(values.columnGap, context.width ?? 375, computed.fontSize);
  if (rowGap !== undefined || columnGap !== undefined) {
    style.gap = {
      width: columnGap ?? (typeof gap === "number" ? gap : 0),
      height: rowGap ?? (typeof gap === "number" ? gap : 0),
    };
  }
  const position = stringValue(values.position);
  if (position === "absolute") style.position = Position.Absolute;
  if (position === "relative") style.position = Position.Relative;
  if (position === "fixed") style.position = Position.Fixed;
  const inset: any = {};
  for (const key of ["left", "right", "top", "bottom"] as const) {
    const v = lengthValue(values[key], context.width ?? 375, computed.fontSize);
    inset[key] = v ?? "auto";
  }
  style.inset = inset;
  const columns = gridTemplate(values.gridTemplateColumns, context, computed.fontSize);
  if (columns) style.gridTemplateColumns = columns;
  const minimum = lengthValue(values.gridColumnMinimum, context.width ?? 375, computed.fontSize);
  if (typeof minimum === "number") {
    const count = Math.max(1, Math.floor((context.width ?? 375) / minimum));
    style.gridTemplateColumns = Array.from({ length: count }, () => ({ min: "auto", max: "1fr" }));
  } else if (stringValue(values.gridColumnMinimum) === "single") {
    style.gridTemplateColumns = [{ min: "auto", max: "1fr" }];
  }
  const rows = gridTemplate(values.gridTemplateRows, context, computed.fontSize);
  if (rows) style.gridTemplateRows = rows;
  return style;
}

function prepareScene(scene: Scene) {
  for (const child of scene.children) prepareScene(child);
  if (
    scene.kind === "element" &&
    scene.children.length > 0 &&
    scene.children.every((child) => child.kind === "text")
  ) {
    scene.text = scene.children.map((child) => child.text ?? "").join("");
    scene.children = [];
  }
}

function nativeControl(scene: Scene): boolean {
  return (
    scene.kind === "element" &&
    ["input", "textarea", "select", "button", "img", "svg"].includes(scene.tag ?? "")
  );
}

function buildTaffy(scene: Scene, tree: TaffyTree, context: ModelRenderContext): bigint {
  const style = taffyStyle(scene, context);
  const isLeafText = scene.kind === "text";
  let id: bigint;
  if (isLeafText || scene.text !== undefined || nativeControl(scene)) {
    id = tree.newLeafWithContext(style, scene);
  } else {
    const childIds = scene.children
      .filter((child) => child.computed.display !== "none")
      .map((child) => buildTaffy(child, tree, context));
    id = childIds.length ? tree.newWithChildren(style, childIds) : tree.newLeaf(style);
  }
  (scene as any).taffyId = id;
  return id;
}

function intrinsicSize(scene: Scene, engine: TextEngine): { width: number; height: number } {
  if (scene.tag === "button") {
    const measured = engine.measure(scene.text ?? "", {
      ...scene.computed,
      fontSize: 13.333,
      lineHeight: 16,
      fontFamily: "Arial",
    });
    return { width: measured.width + 16, height: 21 };
  }
  if (scene.tag === "select") {
    const optionText = scene.children.map((child) => child.text ?? "").join(" ");
    const measured = engine.measure(optionText || " ", {
      ...scene.computed,
      fontSize: 13.333,
      lineHeight: 16,
      fontFamily: "Arial",
    });
    return { width: Math.max(27, measured.width + 18), height: 19 };
  }
  if ((scene.kind === "text" || scene.text !== undefined) && !nativeControl(scene)) {
    const measured = engine.measure(scene.text ?? "", scene.computed);
    return { width: measured.width, height: measured.height };
  }
  switch (scene.tag) {
    case "button": {
      const measured = engine.measure(scene.text ?? "", {
        ...scene.computed,
        fontSize: 13.333,
        lineHeight: 16,
      });
      return { width: measured.width + 24, height: 21 };
    }
    case "input":
      return scene.attrs?.type === "checkbox" || scene.attrs?.type === "radio"
        ? { width: 13, height: 13 }
        : { width: 153, height: 21 };
    case "textarea":
      return { width: 153, height: 42 };
    case "select": {
      const optionText = scene.children.map((child) => child.text ?? "").join(" ");
      const measured = engine.measure(optionText || " ", {
        ...scene.computed,
        fontSize: 13.333,
        lineHeight: 16,
      });
      return { width: Math.max(27, measured.width + 18), height: 19 };
    }
    case "img":
    case "svg": {
      const width =
        numberValue(scene.computed.values.width) ?? numberValue(scene.attrs?.width) ?? 300;
      const height =
        numberValue(scene.computed.values.height) ?? numberValue(scene.attrs?.height) ?? 150;
      return { width, height };
    }
    default:
      return { width: 0, height: 0 };
  }
}

function collectLayout(scene: Scene, tree: TaffyTree, ox: number, oy: number, engine: TextEngine) {
  const id = (scene as any).taffyId as bigint | undefined;
  if (id === undefined) return;
  const layout = tree.getLayout(id);
  const x = ox + layout.x;
  const y = oy + layout.y;
  scene.layout = {
    x,
    y,
    width: layout.width,
    height: layout.height,
    padding: {
      left: layout.paddingLeft ?? layout.padding?.left ?? 0,
      right: layout.paddingRight ?? layout.padding?.right ?? 0,
      top: layout.paddingTop ?? layout.padding?.top ?? 0,
      bottom: layout.paddingBottom ?? layout.padding?.bottom ?? 0,
    },
    border: {
      left: layout.borderLeft ?? 0,
      right: layout.borderRight ?? 0,
      top: layout.borderTop ?? 0,
      bottom: layout.borderBottom ?? 0,
    },
  };
  if (scene.text !== undefined) {
    const contentWidth = Math.max(
      1,
      layout.width -
        scene.layout.padding.left -
        scene.layout.padding.right -
        scene.layout.border.left -
        scene.layout.border.right,
    );
    scene.lines = engine.wrap(scene.text, scene.computed, contentWidth);
    scene.lineWidths = scene.lines.map((line) => engine.width(line, scene.computed));
  }
  for (const child of scene.children) collectLayout(child, tree, x, y, engine);
  layout.free?.();
}

function paintStyle(scene: Scene): string {
  const v = scene.computed.values;
  const declarations: string[] = [];
  const background = colorValue(v.background) ?? stringValue(v.background);
  if (background) declarations.push(`background:${background}`);
  const fg = colorValue(v.color) ?? stringValue(v.color);
  declarations.push(`color:${fg ?? scene.computed.color}`);
  declarations.push(
    `font:${scene.computed.fontStyle} ${scene.computed.fontWeight} ${scene.computed.fontSize}px/${scene.computed.lineHeight}px ${scene.computed.fontFamily}`,
  );
  if (scene.computed.letterSpacing)
    declarations.push(`letter-spacing:${scene.computed.letterSpacing}px`);
  if (scene.computed.textAlign !== "start")
    declarations.push(`text-align:${scene.computed.textAlign}`);
  const paddingTop = lengthValue(v.paddingTop, 0, scene.computed.fontSize);
  const paddingBottom = lengthValue(v.paddingBottom, 0, scene.computed.fontSize);
  const paddingLeft = lengthValue(v.paddingStart, 0, scene.computed.fontSize);
  const paddingRight = lengthValue(v.paddingEnd, 0, scene.computed.fontSize);
  if (typeof paddingTop === "number") declarations.push(`padding-top:${paddingTop}px`);
  if (typeof paddingBottom === "number") declarations.push(`padding-bottom:${paddingBottom}px`);
  if (typeof paddingLeft === "number") declarations.push(`padding-left:${paddingLeft}px`);
  if (typeof paddingRight === "number") declarations.push(`padding-right:${paddingRight}px`);
  const radius = lengthValue(v.borderRadius, 0, scene.computed.fontSize);
  if (typeof radius === "number") declarations.push(`border-radius:${radius}px`);
  for (const [prop, css] of [
    ["borderTopLeftRadius", "border-top-left-radius"],
    ["borderTopRightRadius", "border-top-right-radius"],
    ["borderBottomRightRadius", "border-bottom-right-radius"],
    ["borderBottomLeftRadius", "border-bottom-left-radius"],
  ] as const) {
    const value = lengthValue(v[prop], 0, scene.computed.fontSize);
    if (typeof value === "number") declarations.push(`${css}:${value}px`);
  }
  const borderColor = colorValue(v.borderColor) ?? stringValue(v.borderColor);
  const borderWidth = lengthValue(v.borderWidth, 0, scene.computed.fontSize);
  if (borderColor || typeof borderWidth === "number")
    declarations.push(
      `border:${typeof borderWidth === "number" ? borderWidth : 0}px solid ${borderColor ?? "currentColor"}`,
    );
  for (const side of ["Top", "Right", "Bottom", "Left"] as const) {
    const width = lengthValue(v[`border${side}Width`], 0, scene.computed.fontSize);
    const color = colorValue(v[`border${side}Color`]) ?? stringValue(v[`border${side}Color`]);
    const line =
      stringValue(v[`border${side}Style`]) ?? (width !== undefined ? "solid" : undefined);
    if (width !== undefined || color || line)
      declarations.push(
        `border-${side.toLowerCase()}:${width ?? 0}px ${line ?? "solid"} ${color ?? borderColor ?? "currentColor"}`,
      );
  }
  const opacity = numberValue(v.opacity);
  if (opacity !== undefined) declarations.push(`opacity:${opacity}`);
  const overflow = stringValue(v.overflowX);
  if (overflow)
    declarations.push(
      `overflow-x:${overflow};overflow-y:${overflow === "hidden" ? "hidden" : "auto"}`,
    );
  const zIndex = numberValue(v.zIndex);
  if (zIndex !== undefined) declarations.push(`z-index:${zIndex}`);
  return declarations.join(";");
}

function paintAttributes(scene: Scene): string {
  const output: string[] = [];
  const visual = new Set([
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
    "role",
    "aria-label",
    "width",
    "height",
    "src",
    "viewBox",
  ]);
  for (const [name, value] of Object.entries(scene.attrs ?? {})) {
    if (!visual.has(name) || value === undefined || value === null || value === false) continue;
    output.push(value === true ? ` ${name}` : ` ${name}="${esc(value)}"`);
  }
  if (scene.tag === "option" && scene.parent?.tag === "select") {
    const selected = scene.parent.attrs?.defaultValue ?? scene.parent.attrs?.value;
    const value = scene.attrs?.value ?? scene.text;
    if (
      selected !== undefined &&
      String(selected) === String(value) &&
      !output.includes(" selected")
    )
      output.push(" selected");
  }
  return output.join("");
}

function paintInlineScene(scene: Scene): string {
  if (scene.kind === "text") return esc(scene.text ?? "");
  const tag = /^(span|strong|em|small|code|a|label)$/.test(scene.tag ?? "") ? scene.tag! : "span";
  const attrs = [`data-model-node="${esc(scene.id)}"`, `data-node-kind="${scene.kind}"`];
  const content =
    scene.text !== undefined ? esc(scene.text) : scene.children.map(paintInlineScene).join("");
  const inlineStyle =
    scene.computed.values.lineHeight === undefined
      ? paintStyle(scene).replace(`/${scene.computed.lineHeight}px `, " ")
      : paintStyle(scene);
  return `<${tag} ${attrs.join(" ")}${paintAttributes(scene)} style="${inlineStyle}">${content}</${tag}>`;
}

function paintScene(
  scene: Scene,
  parentX: number,
  parentY: number,
  parentBorderX = 0,
  parentBorderY = 0,
): string {
  if (!scene.layout || scene.computed.display === "none") return "";
  const { x, y, width, height, border } = scene.layout;
  const left = x - parentX - parentBorderX;
  const top = y - parentY - parentBorderY;
  const base = `position:absolute;left:${left}px;top:${top}px;width:${width}px;height:${height}px;box-sizing:border-box;margin:0`;
  const tag =
    /^(div|section|main|header|footer|article|aside|nav|p|span|strong|em|small|code|button|input|textarea|select|option|h1|h2|h3|h4|h5|h6|ul|ol|li|label|form)$/.test(
      scene.tag ?? "",
    )
      ? scene.tag!
      : scene.kind === "text"
        ? "span"
        : "div";
  const style = `${base};${paintStyle(scene)}`;
  const attrs = [`data-model-node="${esc(scene.id)}"`, `data-node-kind="${scene.kind}"`];
  if (scene.kind === "text" || scene.text !== undefined) {
    return `<${tag} ${attrs.join(" ")}${paintAttributes(scene)} style="${style}">${esc(scene.text ?? "")}</${tag}>`;
  }
  const voidTag = tag === "input";
  const controlAttrs =
    tag === "input" || tag === "textarea" || tag === "select"
      ? ' readonly tabindex="-1"'
      : tag === "button"
        ? ' type="button" tabindex="-1"'
        : "";
  const inlineFlow =
    scene.children.length > 0 &&
    scene.children.every((child) => child.kind === "text" || child.computed.display === "inline");
  const children =
    tag === "select"
      ? scene.children
          .map((child) => `<option${paintAttributes(child)}>${esc(child.text ?? "")}</option>`)
          .join("")
      : inlineFlow
        ? scene.children.map(paintInlineScene).join("")
        : scene.children.map((child) => paintScene(child, x, y, border.left, border.top)).join("");
  if (voidTag)
    return `<${tag} ${attrs.join(" ")}${controlAttrs}${paintAttributes(scene)} style="${style}" />`;
  return `<${tag} ${attrs.join(" ")}${controlAttrs}${paintAttributes(scene)} style="${style}">${children}</${tag}>`;
}

function renderEngine(
  model: Model,
  componentId: string,
  inputContext: ModelRenderContext,
  engine: TextEngine,
): ModelRenderResult {
  const context: ModelRenderContext = {
    width: 375,
    initialFontSize: 16,
    colorScheme: "light",
    ...inputContext,
  };
  const component = model.components?.find((entry: any) => entry.id === componentId);
  if (!component) return { html: "", markup: "", unresolved: ["Unresolved component"] };
  const initialScope: Scope = { slots: new Map(), items: new Map() };
  for (const slot of component.slots ?? []) {
    if (!slot.initial) continue;
    const value = resolveExpr(slot.initial, initialScope);
    if (value.known) initialScope.slots.set(slot.id, value.value);
  }
  const rootBase = rootComputed(context, model.documentStyle);
  const children = materializeNode(component.root, model, context, rootBase, initialScope);
  const root: Scene = {
    kind: "element",
    id: "__engine_root",
    tag: "div",
    computed: { ...rootBase, display: "block" },
    children,
  };
  for (const child of children) child.parent = root;
  const tree = new TaffyTree();
  try {
    prepareScene(root);
    const rootId = buildTaffy(root, tree, context);
    tree.computeLayoutWithMeasure(
      rootId,
      { width: context.width!, height: "max-content" },
      (known, available, _node, scene, style) => {
        style.free?.();
        const intrinsic = intrinsicSize(scene as Scene, engine);
        const padding = (scene as Scene).computed.values;
        const left = lengthValue(
          padding.paddingLeft,
          context.width!,
          (scene as Scene).computed.fontSize,
        );
        const right = lengthValue(
          padding.paddingRight,
          context.width!,
          (scene as Scene).computed.fontSize,
        );
        const horizontal =
          (typeof left === "number" ? left : 0) + (typeof right === "number" ? right : 0);
        const maxWidth =
          typeof available.width === "number"
            ? Math.max(1, available.width - horizontal)
            : undefined;
        if ((scene as Scene).text !== undefined) {
          const measured = engine.measure(
            (scene as Scene).text ?? "",
            (scene as Scene).computed,
            known.width ?? maxWidth,
          );
          return { width: known.width ?? measured.width, height: known.height ?? measured.height };
        }
        return { width: known.width ?? intrinsic.width, height: known.height ?? intrinsic.height };
      },
    );
    collectLayout(root, tree, 0, 0, engine);
    const box = root.layout ?? { width: context.width!, height: 0 };
    const body = root.children.map((child) => paintScene(child, 0, 0)).join("");
    const unresolved = new Set<string>();
    const walk = (scene: Scene) => {
      for (const item of scene.computed.unresolved) unresolved.add(item);
      scene.children.forEach(walk);
    };
    walk(root);
    const markup = `<style>*{box-sizing:border-box}:host{display:block;color:#111;background:#fff}body{margin:0}</style><div class="dui-engine-frame" style="position:relative;display:block;width:${context.width}px;min-height:${Math.max(0, box.height)}px;overflow:visible">${body}</div>`;
    return {
      html: `<!doctype html><html><head><meta charset="utf-8"></head><body>${markup}</body></html>`,
      markup,
      unresolved: [...unresolved],
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const markup = `<div class="dui-engine-error" style="padding:8px;color:#b00020">dui layout engine could not render this component: ${esc(message)}</div>`;
    return { html: markup, markup, unresolved: [`engine-error:${message}`] };
  } finally {
    tree.free?.();
  }
}

export async function createEngineModelRenderer(): Promise<SourceModelRenderer> {
  await loadTaffy();
  const textEngine = await TextEngine.create();
  return {
    render(model, componentId, context) {
      return renderEngine(model, componentId, context ?? {}, textEngine);
    },
  };
}
