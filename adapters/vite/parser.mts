// Unified AST parser for Vite 8 (Oxc) and Vite 7 (official TypeScript fallback).
import { createRequire } from "node:module";
import { extname, join } from "node:path";
import { pathToFileURL } from "node:url";

export interface AstNode {
  type: string;
  start?: number;
  end?: number;
  [key: string]: unknown;
}

export interface AstParser {
  parseSync(file: string, text: string): { program: AstNode; errors: unknown[] };
  Visitor: new (visitor: Record<string, (node: never) => void>) => {
    visit(program: unknown): void;
  };
  close?: () => void;
}

interface ViteModule {
  parseSync?: (file: string, text: string) => { program: AstNode; errors: unknown[] };
  Visitor?: AstParser["Visitor"];
}

interface TsNode {
  kind: number;
  pos: number;
  end: number;
  text?: string;
  getStart?: (sf?: TsSourceFile) => number;
  forEachChild: (cb: (child: TsNode) => void) => void;
  statements?: readonly TsNode[];
  moduleSpecifier?: TsNode;
  importClause?: {
    namedBindings?: {
      kind: number;
      elements?: readonly { propertyName?: TsNode; name: TsNode }[];
    };
  };
  declarationList?: {
    flags: number;
    declarations: readonly { name: TsNode; initializer?: TsNode }[];
  };
  expression?: TsNode;
  arguments?: readonly TsNode[];
  elements?: readonly TsNode[];
  properties?: readonly TsNode[];
  name?: TsNode;
  initializer?: TsNode;
  argumentExpression?: TsNode;
  head?: { text: string };
  templateSpans?: readonly { expression: TsNode; literal: { text: string } }[];
  openingElement?: TsNode;
  tagName?: TsNode;
  attributes?: { properties?: readonly TsNode[] };
  children?: readonly TsNode[];
}

interface TsSourceFile extends TsNode {
  fileName: string;
  statements: readonly TsNode[];
  parseDiagnostics?: readonly unknown[];
}

interface TsSyntaxEnums {
  SyntaxKind: Record<string, number>;
  NodeFlags: { Const: number; Let: number };
}

export function assertSupportedViteVersion(version: string): number {
  const major = Number(version.split(".")[0]);
  if (!/^\d+\.\d+\.\d+$/.test(version) || (major !== 7 && major !== 8)) {
    throw Error("Vite 7 or 8 required; found " + version);
  }
  return major;
}

function createVisitorClass(): AstParser["Visitor"] {
  return class TsAstVisitor {
    private handlers: Record<string, (node: AstNode) => void>;
    constructor(handlers: Record<string, (node: never) => void>) {
      this.handlers = handlers as Record<string, (node: AstNode) => void>;
    }
    visit(node: unknown): void {
      if (!node || typeof node !== "object") return;
      if (Array.isArray(node)) {
        for (const item of node) this.visit(item);
        return;
      }
      const astNode = node as AstNode;
      if (typeof astNode.type === "string") {
        this.handlers[astNode.type]?.(astNode);
      }
      for (const [key, val] of Object.entries(astNode)) {
        if (
          key === "type" ||
          key === "start" ||
          key === "end" ||
          key === "name" ||
          key === "value" ||
          key === "kind" ||
          key === "computed"
        ) {
          continue;
        }
        if (val && typeof val === "object") this.visit(val);
      }
    }
  };
}

function convertSourceFile(sf: TsSourceFile, enums: TsSyntaxEnums): AstNode {
  const K = enums.SyntaxKind;
  const NodeFlags = enums.NodeFlags;
  const startOf = (node: TsNode): number =>
    typeof node.getStart === "function" ? node.getStart(sf) : node.pos;

  function convert(node: TsNode): AstNode {
    const start = startOf(node);
    const end = node.end;

    if (node.kind === K.ImportDeclaration) {
      const specifiers: AstNode[] = [];
      const named = node.importClause?.namedBindings;
      if (named && named.kind === K.NamedImports && named.elements) {
        for (const el of named.elements) {
          specifiers.push({
            type: "ImportSpecifier",
            imported: convert(el.propertyName ?? el.name),
            local: convert(el.name),
          });
        }
      }
      return {
        type: "ImportDeclaration",
        start,
        end,
        specifiers,
        source: node.moduleSpecifier
          ? convert(node.moduleSpecifier)
          : { type: "Literal", value: "" },
      };
    }

    if (node.kind === K.VariableStatement && node.declarationList) {
      const flags = node.declarationList.flags;
      const kind =
        (flags & NodeFlags.Const) !== 0 ? "const" : (flags & NodeFlags.Let) !== 0 ? "let" : "var";
      return {
        type: "VariableDeclaration",
        start,
        end,
        kind,
        declarations: node.declarationList.declarations.map((d) => ({
          type: "VariableDeclarator",
          id: convert(d.name),
          init: d.initializer ? convert(d.initializer) : undefined,
        })),
      };
    }

    if (node.kind === K.CallExpression && node.expression) {
      return {
        type: "CallExpression",
        start,
        end,
        callee: convert(node.expression),
        arguments: (node.arguments ?? []).map(convert),
      };
    }

    if (node.kind === K.Identifier) {
      return { type: "Identifier", start, end, name: node.text ?? "" };
    }

    if (node.kind === K.StringLiteral || node.kind === K.NoSubstitutionTemplateLiteral) {
      return { type: "Literal", start, end, value: node.text ?? "" };
    }

    if (node.kind === K.NumericLiteral) {
      return { type: "Literal", start, end, value: Number(node.text ?? 0) };
    }

    if (node.kind === K.TrueKeyword || node.kind === K.FalseKeyword) {
      return { type: "Literal", start, end, value: node.kind === K.TrueKeyword };
    }

    if (node.kind === K.ArrayLiteralExpression) {
      return {
        type: "ArrayExpression",
        start,
        end,
        elements: (node.elements ?? []).map(convert),
      };
    }

    if (node.kind === K.JsxElement || node.kind === K.JsxSelfClosingElement) {
      const opening = node.kind === K.JsxElement ? node.openingElement : node;
      const tag = opening?.tagName;
      const jsxName: AstNode =
        tag && tag.kind === K.Identifier
          ? { type: "JSXIdentifier", start: startOf(tag), end: tag.end, name: tag.text ?? "" }
          : tag
            ? convert(tag)
            : { type: "JSXIdentifier", name: "" };
      const attributes: AstNode[] = (opening?.attributes?.properties ?? []).map((attr) => {
        if (attr.kind === K.JsxAttribute && attr.name) {
          let value: AstNode | null = null;
          if (attr.initializer) {
            if (attr.initializer.kind === K.JsxExpression) {
              value = {
                type: "JSXExpressionContainer",
                start: startOf(attr.initializer),
                end: attr.initializer.end,
                expression: attr.initializer.expression
                  ? convert(attr.initializer.expression)
                  : undefined,
              };
            } else {
              value = convert(attr.initializer);
            }
          }
          return {
            type: "JSXAttribute",
            start: startOf(attr),
            end: attr.end,
            name: {
              type: "JSXIdentifier",
              start: startOf(attr.name),
              end: attr.name.end,
              name: attr.name.text ?? "",
            },
            value,
          };
        }
        return convert(attr);
      });
      return {
        type: "JSXElement",
        start,
        end,
        openingElement: {
          type: "JSXOpeningElement",
          start: opening ? startOf(opening) : start,
          end: opening ? opening.end : end,
          name: jsxName,
          attributes,
        },
        children: node.kind === K.JsxElement ? (node.children ?? []).map(convert) : [],
      };
    }

    if (node.kind === K.ObjectLiteralExpression) {
      return {
        type: "ObjectExpression",
        start,
        end,
        properties: (node.properties ?? []).map((p) => {
          if (p.kind === K.PropertyAssignment && p.name && p.initializer) {
            const computed = p.name.kind === K.ComputedPropertyName;
            const keyNode = computed && p.name.expression ? p.name.expression : p.name;
            return {
              type: "Property",
              start: startOf(p),
              end: p.end,
              computed,
              key: convert(keyNode),
              value: convert(p.initializer),
            };
          }
          if (p.kind === K.ShorthandPropertyAssignment && p.name) {
            return {
              type: "Property",
              start: startOf(p),
              end: p.end,
              computed: false,
              key: convert(p.name),
              value: convert(p.name),
            };
          }
          return convert(p);
        }),
      };
    }

    if (node.kind === K.ElementAccessExpression && node.expression && node.argumentExpression) {
      return {
        type: "MemberExpression",
        start,
        end,
        computed: true,
        object: convert(node.expression),
        property: convert(node.argumentExpression),
      };
    }

    if (node.kind === K.PropertyAccessExpression && node.expression && node.name) {
      return {
        type: "MemberExpression",
        start,
        end,
        computed: false,
        object: convert(node.expression),
        property: convert(node.name),
      };
    }

    if (node.kind === K.TemplateExpression && node.head && node.templateSpans) {
      return {
        type: "TemplateLiteral",
        start,
        end,
        quasis: [
          { value: { cooked: node.head.text } },
          ...node.templateSpans.map((span) => ({ value: { cooked: span.literal.text } })),
        ],
        expressions: node.templateSpans.map((span) => convert(span.expression)),
      };
    }

    if (node.kind === K.AsExpression && node.expression) {
      return { type: "TSAsExpression", start, end, expression: convert(node.expression) };
    }

    if (node.kind === K.SatisfiesExpression && node.expression) {
      return { type: "TSSatisfiesExpression", start, end, expression: convert(node.expression) };
    }

    if (node.kind === K.ParenthesizedExpression && node.expression) {
      return { type: "ParenthesizedExpression", start, end, expression: convert(node.expression) };
    }

    const children: AstNode[] = [];
    node.forEachChild((child) => {
      children.push(convert(child));
    });
    return { type: "Node", start, end, children };
  }

  return {
    type: "Program",
    start: 0,
    end: sf.end,
    body: sf.statements.map(convert),
  };
}

function resolveTypeScriptRequire(app: string, root: string): NodeRequire {
  for (const base of [app, root]) {
    try {
      const req = createRequire(join(base, "package.json"));
      req.resolve("typescript/package.json");
      return req;
    } catch {
      // Try next candidate directory.
    }
  }
  throw Error("Vite 7 AST fallback requires an installed official TypeScript 6.x or 7.x compiler");
}

function normalizedExt(file: string): string {
  const ext = extname(file).toLowerCase();
  return [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"].includes(ext)
    ? ext
    : ".tsx";
}

async function createTypeScriptAstParser(app: string, root: string): Promise<AstParser> {
  const tsRequire = resolveTypeScriptRequire(app, root);
  const tsVersion = (tsRequire("typescript/package.json") as { version: string }).version;
  const Visitor = createVisitorClass();

  if (tsVersion.startsWith("6.")) {
    const ts = tsRequire("typescript") as TsSyntaxEnums & {
      ScriptTarget: { Latest: number };
      ScriptKind: Record<string, number>;
      createSourceFile: (
        fileName: string,
        sourceText: string,
        languageVersion: number,
        setParentNodes: boolean,
        scriptKind: number,
      ) => TsSourceFile;
    };
    const kindFor = (file: string): number => {
      const ext = normalizedExt(file);
      if (ext === ".tsx") return ts.ScriptKind.TSX;
      if (ext === ".jsx") return ts.ScriptKind.JSX;
      if (ext === ".js" || ext === ".mjs" || ext === ".cjs") return ts.ScriptKind.JS;
      return ts.ScriptKind.TS;
    };
    return {
      parseSync(file: string, text: string) {
        const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, kindFor(file));
        return {
          program: convertSourceFile(sf, ts),
          errors: [...(sf.parseDiagnostics ?? [])],
        };
      },
      Visitor,
      close() {},
    };
  }

  async function loadOfficial(part: string): Promise<Record<string, unknown>> {
    for (const prefix of ["typescript/", "typescript/unstable/"]) {
      let resolved: string;
      try {
        resolved = tsRequire.resolve(prefix + part);
      } catch (error) {
        if (
          ["ERR_PACKAGE_PATH_NOT_EXPORTED", "MODULE_NOT_FOUND"].includes(
            (error as NodeJS.ErrnoException).code ?? "",
          )
        ) {
          continue;
        }
        throw error;
      }
      return (await import(pathToFileURL(resolved).href)) as Record<string, unknown>;
    }
    throw Error("installed TypeScript does not expose the required official " + part + " API");
  }

  const syncMod = (await loadOfficial("sync")) as {
    API: new (opts: {
      cwd: string;
      fs: {
        readFile: (p: string) => string | undefined;
        fileExists: (p: string) => boolean;
        directoryExists: (p: string) => boolean;
        realpath: (p: string) => string;
      };
    }) => {
      updateSnapshot: (params: { openProjects: string[]; closeProjects?: string[] }) => {
        getProject: (cfg: string) =>
          | {
              program: {
                getSourceFile: (fp: string) => TsSourceFile | undefined;
                getSyntacticDiagnostics: (fp: string) => readonly unknown[];
              };
            }
          | undefined;
        dispose: () => void;
      };
      close: () => void;
    };
  };
  const astMod = (await loadOfficial("ast")) as unknown as TsSyntaxEnums;

  const virtualDir = "/__dui_virtual__";
  const files = new Map<string, string>();
  let seq = 0;
  let prevCfg: string | undefined;
  let api: InstanceType<typeof syncMod.API> | undefined;
  let closed = false;

  function ensureApi(): InstanceType<typeof syncMod.API> {
    if (!api) {
      api = new syncMod.API({
        cwd: virtualDir,
        fs: {
          readFile: (p) => files.get(p),
          fileExists: (p) => files.has(p),
          directoryExists: (p) => p === virtualDir,
          realpath: (p) => p,
        },
      });
    }
    return api;
  }

  return {
    parseSync(file: string, text: string) {
      files.clear();
      const id = ++seq;
      const cfg = `${virtualDir}/tsconfig.${id}.json`;
      const fp = `${virtualDir}/f${id}${normalizedExt(file)}`;
      files.set(fp, text);
      files.set(
        cfg,
        JSON.stringify({
          compilerOptions: { allowJs: true, jsx: "preserve", noLib: true, types: [] },
          files: [fp],
        }),
      );
      const snap = ensureApi().updateSnapshot({
        openProjects: [cfg],
        ...(prevCfg ? { closeProjects: [prevCfg] } : {}),
      });
      prevCfg = cfg;
      try {
        const proj = snap.getProject(cfg);
        const sf = proj?.program.getSourceFile(fp);
        if (!proj || !sf) {
          return { program: { type: "Program", body: [] }, errors: ["failed to parse " + file] };
        }
        const errors = [...proj.program.getSyntacticDiagnostics(sf.fileName)];
        return {
          program: convertSourceFile(sf, astMod),
          errors,
        };
      } finally {
        snap.dispose();
      }
    },
    Visitor,
    close() {
      if (closed || !api) return;
      closed = true;
      api.close();
    },
  };
}

export async function createAstParser({
  vite,
  app,
  root,
}: {
  vite: ViteModule;
  app: string;
  root: string;
}): Promise<AstParser> {
  if (typeof vite.parseSync === "function" && typeof vite.Visitor === "function") {
    return {
      parseSync: vite.parseSync,
      Visitor: vite.Visitor,
      close() {},
    };
  }
  return createTypeScriptAstParser(app, root);
}
