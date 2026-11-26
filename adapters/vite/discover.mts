// Discover navigable TanStack file routes without importing application modules.
import { existsSync, globSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import type { Rule } from "../typescript/contracts.mjs";
import { createAstParser } from "./parser.mjs";

const [root, builtin] = process.argv.slice(2);
const configPath = join(root, "dui.json");
const profile = existsSync(configPath) ? JSON.parse(readFileSync(configPath, "utf8")) : {};
const app = resolve(root, profile.appRoot ?? ".");
const packs = [
  JSON.parse(builtin),
  ...(profile.rulePacks ?? []).map((p: string) =>
    JSON.parse(readFileSync(resolve(root, p), "utf8")),
  ),
];
const routeRules: Rule[] = packs
  .flatMap((pack) => pack.rules)
  .filter((r: Rule) => r.kind === "route");
const rel = relative(root, app);
if (
  rel === ".." ||
  rel.startsWith(".." + sep) ||
  isAbsolute(rel) ||
  !existsSync(join(app, "package.json"))
)
  throw Error("appRoot must point to a package.json inside the project");
const require = createRequire(join(app, "package.json"));
const vite = await import(pathToFileURL(require.resolve("vite")).href);
const parser = await createAstParser({ vite, app, root });
interface Node {
  type: string;
  start: number;
  name: string;
  value: unknown;
  computed: boolean;
  callee: Node;
  arguments: Node[];
  elements?: (Node | null)[];
  expression: Node;
  object: Node;
  property: Node;
  properties: { type: string; key: Node; value: Node; computed: boolean }[];
  specifiers: { imported?: Node; local: Node }[];
  source: Node;
  kind: string;
  declarations: { id: Node; init?: Node }[];
  declaration?: Node;
  expressions: Node[];
  quasis: { value: { cooked: string } }[];
  openingElement?: {
    name?: Node;
    attributes?: { type: string; name?: Node; value?: Node | null }[];
  };
  children?: Node[];
}
const routes: { pattern: string; source: string }[] = [];
const candidates = new Map<string, string>();
const params = profile.routeParams ?? {};
const unresolved = new Set<string>();
// Route objects that group children without rendering a screen of their own, and routes whose
// element is a static `<Navigate to>`: neither is captured as a screen.
const layouts = new Map<string, string>();
const redirects = new Map<string, { to: string; source: string }>();
const parseFailures: string[] = [];
const REACT_ROUTER_CREATORS = new Set([
  "createBrowserRouter",
  "createHashRouter",
  "createMemoryRouter",
]);
function joinRoutePath(parent: string, child: string): string {
  const cleanChild = child.trim();
  if (!cleanChild) return parent || "/";
  if (cleanChild.startsWith("/")) return cleanChild;
  const base = parent && parent !== "/" ? parent.replace(/\/+$/, "") : "";
  return `${base}/${cleanChild.replace(/^\/+/, "")}`;
}
function isLocalPath(path: string): boolean {
  return (
    path.startsWith("/") &&
    !/\/\/|[?#\\]|(?:^|\/)\.{1,2}(?:\/|$)/.test(path) &&
    new URL(path, "http://localhost").pathname === path
  );
}
function segment(key: string): string | undefined {
  const value = params[key];
  if (typeof value !== "string" || !value || [".", ".."].includes(value)) return undefined;
  return encodeURIComponent(value);
}
const excluded = new Set([
  "node_modules",
  ".git",
  ".dui",
  "dist",
  "build",
  "target",
  "coverage",
  "__tests__",
  "__fixtures__",
]);
const files = globSync("**/*.{ts,tsx,mts}", {
  cwd: app,
  exclude: (p) => p.split(/[\\/]/).some((s) => excluded.has(s) || s.startsWith(".")),
}).sort();

// Extract resolved URL string-literal keys from official compiler/router-generated type blocks
// (e.g. `export interface FileRoutesByTo { '/sign-in': ... }` or React Router `pages: { ... }`)
// without reimplementing any router-specific filename or route-group conventions.
function extractInterfacePathKeys(source: string, anchorPattern: RegExp): string[] {
  const match = anchorPattern.exec(source);
  if (!match) return [];
  const openIdx = source.indexOf("{", match.index + match[0].length - 1);
  if (openIdx < 0) return [];
  let depth = 0;
  let bodyEnd = -1;
  for (let i = openIdx; i < source.length; i += 1) {
    const ch = source[i];
    if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) {
        bodyEnd = i;
        break;
      }
    }
  }
  if (bodyEnd < 0) return [];
  const body = source.slice(openIdx + 1, bodyEnd);
  const found: string[] = [];
  let level = 0;
  for (const line of body.split(/\r?\n/)) {
    if (level === 0) {
      const m = line.match(/^\s*(['"])(\/[^'"]*)\1\s*:/);
      if (m) found.push(m[2]);
    }
    for (const ch of line) {
      if (ch === "{") level += 1;
      else if (ch === "}") level = Math.max(0, level - 1);
    }
  }
  return found;
}

let hasOfficialRouteRegistry = false;
const officialRegistryCandidates = [
  ...files.filter((f) => /(?:^|[\\/])routeTree\.gen\.tsx?$/.test(f)),
  ...globSync(".react-router/types/+register.ts", { cwd: app }),
];
for (const regFile of officialRegistryCandidates) {
  const absReg = join(app, regFile);
  if (!existsSync(absReg)) continue;
  const src = readFileSync(absReg, "utf8");
  const relSource = relative(root, absReg);
  const keys = regFile.endsWith("+register.ts")
    ? extractInterfacePathKeys(src, /\bpages\s*:\s*\{/)
    : extractInterfacePathKeys(src, /\binterface\s+FileRoutesByTo\s*\{/);
  if (keys.length > 0) {
    hasOfficialRouteRegistry = true;
    for (const key of keys) {
      routes.push({ pattern: key, source: relSource });
    }
  }
}

try {
  for (const file of files) {
    if (/\.(test|spec|stories)\.[^.]+$/.test(file)) continue;
    const source = readFileSync(join(app, file), "utf8");
    if (!source.includes("${") && !routeRules.some((r) => source.includes(r.package))) continue;
    const ast = parser.parseSync(file, source);
    if (ast.errors.length) {
      parseFailures.push(relative(root, join(app, file)));
      continue;
    }
    const declarations = new Map<string, Node>();
    const factories = new Set<string>();
    const routerCreators = new Set<string>();
    const routeComponents = new Set<string>();
    const navigateComponents = new Set<string>();
    const relSource = relative(root, join(app, file));
    for (const statement of ast.program.body as Node[]) {
      if (statement.type === "ImportDeclaration") {
        const pkg = typeof statement.source.value === "string" ? statement.source.value : "";
        for (const s of statement.specifiers) {
          const imported = s.imported?.name;
          if (!imported) continue;
          if (imported === "Navigate" && /^react-router(-dom)?$/.test(pkg))
            navigateComponents.add(s.local.name);
          if (routeRules.some((r) => r.package === pkg && r.symbol === imported)) {
            if (REACT_ROUTER_CREATORS.has(imported)) routerCreators.add(s.local.name);
            else if (imported === "Route") routeComponents.add(s.local.name);
            else if (!hasOfficialRouteRegistry) factories.add(s.local.name);
          }
        }
      }
      const n = statement.declaration ?? statement;
      if (n.kind === "const")
        for (const d of n.declarations)
          if (d.init && d.id.type === "Identifier") declarations.set(d.id.name, d.init);
    }
    function unwrapExpr(node: Node | undefined): Node | undefined {
      const seen = new Set<Node>();
      let cur = node;
      while (cur && !seen.has(cur)) {
        seen.add(cur);
        if (cur.type === "Identifier") {
          const next = declarations.get(cur.name);
          if (!next) break;
          cur = next;
          continue;
        }
        if (
          [
            "TSAsExpression",
            "TSSatisfiesExpression",
            "ParenthesizedExpression",
            "JSXExpressionContainer",
          ].includes(cur.type)
        ) {
          cur = cur.expression;
          continue;
        }
        break;
      }
      return cur;
    }
    function literalString(node: Node | undefined): string | undefined {
      const resolved = unwrapExpr(node);
      return resolved?.type === "Literal" && typeof resolved.value === "string"
        ? resolved.value
        : undefined;
    }
    function literalTable(node: Node): string[] {
      const resolved = unwrapExpr(node);
      if (
        resolved?.type === "ObjectExpression" &&
        resolved.properties.every(
          (p) =>
            p.type === "Property" &&
            !p.computed &&
            p.value.type === "Literal" &&
            typeof p.value.value === "string",
        )
      )
        return resolved.properties.map((p) => p.value.value as string);
      return [];
    }
    /** A static `<Navigate to="…">` element: its target resolved against the route's path.
     * `null` for a Navigate whose target is not a literal; `undefined` for any other element. */
    function navigateTarget(node: Node | undefined, routePath: string): string | null | undefined {
      const el = unwrapExpr(node);
      const name = el?.type === "JSXElement" ? el.openingElement?.name?.name : undefined;
      if (!name || !navigateComponents.has(name)) return undefined;
      const toAttr = el?.openingElement?.attributes?.find(
        (a) => a.type === "JSXAttribute" && a.name?.name === "to",
      );
      const to = literalString(toAttr?.value ?? undefined);
      if (to === undefined || /[{}:*]/.test(to)) return null;
      const base = "http://route" + (routePath.endsWith("/") ? routePath : routePath + "/");
      return new URL(to, base).pathname.replace(/(.)\/$/, "$1");
    }
    function collectRouteObjects(node: Node | undefined, parentPath: string): void {
      const arr = unwrapExpr(node);
      if (arr?.type !== "ArrayExpression" || !arr.elements) return;
      for (const el of arr.elements) {
        const obj = unwrapExpr(el ?? undefined);
        if (obj?.type !== "ObjectExpression") continue;
        let routePath: string | undefined;
        let isIndex = false;
        let childrenNode: Node | undefined;
        let elementNode: Node | undefined;
        let renders = false;
        for (const prop of obj.properties) {
          if (prop.type !== "Property" || prop.computed) continue;
          const key = prop.key?.name;
          if (key === "path") routePath = literalString(prop.value);
          else if (key === "index") {
            const val = unwrapExpr(prop.value);
            isIndex = val?.type === "Literal" && val.value === true;
          } else if (key === "children") {
            childrenNode = prop.value;
          } else if (key === "element") {
            elementNode = prop.value;
            renders = true;
          } else if (key === "Component" || key === "lazy") {
            renders = true;
          }
        }
        const fullPath = isIndex
          ? parentPath || "/"
          : routePath !== undefined
            ? joinRoutePath(parentPath, routePath)
            : parentPath;
        const resolvedChildren = unwrapExpr(childrenNode);
        const hasChildren =
          resolvedChildren?.type === "ArrayExpression" &&
          (resolvedChildren.elements?.length ?? 0) > 0;
        if (hasChildren) collectRouteObjects(resolvedChildren, fullPath);
        else if (childrenNode && !(resolvedChildren?.type === "ArrayExpression"))
          // Children defined elsewhere: their routes cannot be read here.
          unresolved.add((fullPath || "") + "/*");
        const target = navigateTarget(elementNode, fullPath || "/");
        if (typeof target === "string") {
          redirects.set(fullPath || "/", { to: target, source: relSource });
          continue;
        }
        // A route with children renders them; without an element or index it is only a group.
        if (childrenNode && !renders && !isIndex) layouts.set(fullPath || "/", relSource);
        // A rendering parent whose children cannot be read still shows its own screen.
        if (isIndex || (routePath !== undefined && (!childrenNode || (!hasChildren && renders)))) {
          routes.push({ pattern: fullPath || "/", source: relSource });
        }
      }
    }
    const visitedJsxRoutes = new WeakSet<Node>();
    function collectJsxRoute(el: Node, parentPath: string): void {
      if (visitedJsxRoutes.has(el)) return;
      visitedJsxRoutes.add(el);
      const attrs = el.openingElement?.attributes ?? [];
      let routePath: string | undefined;
      let isIndex = false;
      let elementNode: Node | undefined;
      for (const attr of attrs) {
        if (attr.type !== "JSXAttribute") continue;
        const name = attr.name?.name;
        if (name === "element") elementNode = attr.value ?? undefined;
        if (name === "path") routePath = literalString(attr.value ?? undefined);
        else if (name === "index") {
          if (attr.value == null) isIndex = true;
          else {
            const val = unwrapExpr(attr.value);
            isIndex = val?.type === "Literal" && val.value === true;
          }
        }
      }
      const fullPath = isIndex
        ? parentPath || "/"
        : routePath !== undefined
          ? joinRoutePath(parentPath, routePath)
          : parentPath;
      const childRoutes = (el.children ?? []).filter(
        (c) =>
          c.type === "JSXElement" &&
          c.openingElement?.name?.name !== undefined &&
          routeComponents.has(c.openingElement.name.name),
      );
      for (const child of childRoutes) collectJsxRoute(child, fullPath);
      const target = navigateTarget(elementNode, fullPath || "/");
      if (typeof target === "string") {
        redirects.set(fullPath || "/", { to: target, source: relSource });
        return;
      }
      if (childRoutes.length > 0 && !elementNode && !isIndex)
        layouts.set(fullPath || "/", relSource);
      if (isIndex || (routePath !== undefined && childRoutes.length === 0)) {
        routes.push({ pattern: fullPath || "/", source: relSource });
      }
    }
    new parser.Visitor({
      CallExpression(n: Node) {
        if (n.callee.type === "Identifier" && routerCreators.has(n.callee.name)) {
          collectRouteObjects(n.arguments[0], "");
          return;
        }
        const factory = n.callee;
        if (factory.type !== "CallExpression" || !factories.has(factory.callee.name)) return;
        const pattern = factory.arguments[0]?.value;
        if (
          typeof pattern === "string" &&
          n.arguments[0]?.properties?.some((p) => p.key?.name === "component")
        )
          routes.push({ pattern, source: relSource });
      },
      JSXElement(n: Node) {
        const tag = n.openingElement?.name?.name;
        if (tag && routeComponents.has(tag)) collectJsxRoute(n, "");
      },
      TemplateLiteral(n: Node) {
        if (!n.quasis[0].value.cooked.startsWith("/") || !n.expressions.length) return;
        const last = n.expressions.at(-1)!;
        if (
          last.type !== "MemberExpression" ||
          !last.computed ||
          n.quasis.at(-1)?.value.cooked !== ""
        )
          return;
        let prefix = n.quasis[0].value.cooked;
        for (const [index, expr] of n.expressions.slice(0, -1).entries()) {
          const key =
            expr.type === "Identifier"
              ? expr.name
              : expr.callee?.name === "encodeURIComponent"
                ? expr.arguments[0]?.name
                : undefined;
          const value = key && segment(key);
          if (!value) return;
          prefix += value + n.quasis[index + 1].value.cooked;
        }
        for (const tail of literalTable(last.object))
          if (/^[\w/-]*$/.test(tail) && !tail.startsWith("/"))
            candidates.set(prefix + tail, relSource);
      },
    }).visit(ast.program);
  }
} finally {
  parser.close?.();
}
const screens = new Map<string, string>();
for (const { pattern, source } of routes) {
  if (!isLocalPath(pattern) || /[{}]/.test(pattern)) {
    unresolved.add(pattern);
    continue;
  }
  const parts = pattern
    .split("/")
    .filter(Boolean)
    .filter((part) => !part.startsWith("_"));
  const splat = parts.at(-1) === "$" || parts.at(-1) === "*";
  if (splat) parts.pop();
  const segments = parts
    .map((part) => part.replace(/_$/, ""))
    .map((part) => {
      if (part.startsWith("$")) return segment(part.slice(1));
      if (part.startsWith(":")) return segment(part.slice(1).replace(/\?$/, ""));
      return part;
    });
  if (segments.includes(undefined)) {
    unresolved.add(pattern);
    continue;
  }
  const prefix = "/" + segments.join("/");
  if (splat) {
    for (const [url, file] of candidates)
      if (isLocalPath(url) && url.startsWith(prefix.replace(/\/$/, "") + "/")) {
        screens.set(url.replace(/\/$/, "") || "/", file);
      }
    // A finite table supplies examples, but an open splat may accept other values.
    unresolved.add(pattern);
  } else screens.set(prefix, source);
}
const paths = [...screens.keys()].sort();
const entry = paths[0] ?? profile.path ?? "/";
if (typeof entry !== "string" || !isLocalPath(entry.split(/[?#]/)[0]))
  throw Error("entry path must be a same-origin absolute path");
const defaultReady = "main *, [role=main] *, h1, nav a[href], form input, canvas, article";
function screen(path: string) {
  const override = profile.routeOptions?.[path];
  return {
    path,
    cookies: override?.cookies ?? profile.cookies ?? {},
    readySelector: override?.readySelector ?? profile.readySelector ?? defaultReady,
    hasScreenAssertion: Boolean(override?.readySelector || profile.readySelector),
  };
}
const discoveredPaths = new Set([entry, ...paths]);
const extraScenarios: Record<string, unknown>[] = [];
const discoverySources: Record<string, unknown> = paths.length
  ? Object.fromEntries(screens)
  : { [entry]: { kind: "entry" } };
if (Array.isArray(profile.scenarios)) {
  for (const extra of profile.scenarios) {
    if (!extra || typeof extra !== "object" || typeof extra.path !== "string") continue;
    const p = extra.path;
    if (!isLocalPath(p.split(/[?#]/)[0]) || discoveredPaths.has(p)) continue;
    discoveredPaths.add(p);
    extraScenarios.push({
      ...screen(p),
      ...extra,
      hasScreenAssertion: Boolean(
        extra.readySelector || profile.routeOptions?.[p]?.readySelector || profile.readySelector,
      ),
    });
    discoverySources[p] = "dui.json";
  }
}
// Dev-server behaviour that a production build served by dui's static host does not have:
// `configureServer` middleware and `server.proxy`. Reported so a blank capture can name it.
async function devServerFeatures() {
  try {
    const loaded = await vite.loadConfigFromFile(
      { command: "serve", mode: "development", isSsrBuild: false, isPreview: false },
      undefined,
      app,
      "silent",
    );
    const flatten = async (items: unknown[]): Promise<Record<string, unknown>[]> =>
      (
        await Promise.all(
          items.map(async (item) => {
            const value = await item;
            if (Array.isArray(value)) return flatten(value);
            return value && typeof value === "object" ? [value as Record<string, unknown>] : [];
          }),
        )
      ).flat();
    const plugins = await flatten(loaded?.config.plugins ?? []);
    const configureServer = plugins
      .filter((p) => p.configureServer)
      .map((p) => String(p.name ?? "unnamed plugin"))
      .sort();
    const proxy = Object.keys(loaded?.config.server?.proxy ?? {}).sort();
    return configureServer.length || proxy.length ? { configureServer, proxy } : undefined;
  } catch {
    return undefined;
  }
}
const devServer = await devServerFeatures();
const config = {
  ...profile,
  appRoot: rel || ".",
  widths: profile.widths ?? [375, 1280],
  tsconfig:
    profile.tsconfig ??
    relative(
      root,
      join(app, existsSync(join(app, "tsconfig.app.json")) ? "tsconfig.app.json" : "tsconfig.json"),
    ),
  sources: [],
  sourceDirs: [],
  additionalScopes: [],
  ...screen(entry),
  scenarios: [...paths.slice(1).map(screen), ...extraScenarios],
  screenDiscovery: {
    sources: discoverySources,
    unresolved: [...unresolved].sort(),
    parseFailures,
    mode: paths.length ? "router" : "links",
    // Not captured as screens, so they are not in the coverage denominator.
    ...(layouts.size
      ? {
          layouts: Object.fromEntries(
            [...layouts].filter(([path]) => !discoveredPaths.has(path)).sort(),
          ),
        }
      : {}),
    ...(redirects.size ? { redirects: Object.fromEntries([...redirects].sort()) } : {}),
    ...(devServer ? { devServer } : {}),
  },
};
if (profile.routeSelector !== false) {
  config.linkDefaults = {
    cookies: profile.cookies ?? {},
    readySelector: profile.readySelector ?? defaultReady,
    hasScreenAssertion: Boolean(profile.readySelector),
  };
  config.routeSelector =
    typeof profile.routeSelector === "string"
      ? profile.routeSelector
      : "main a[href], nav a[href], aside a[href], header a[href]";
} else delete config.routeSelector;
if (profile.discoverOverlays !== false) {
  config.discoverOverlays = true;
}
if (config.mockPreload) config.mockPreload = resolve(root, config.mockPreload);
if (config.rulePacks) config.rulePacks = config.rulePacks.map((p: string) => resolve(root, p));
console.log(JSON.stringify(config));
