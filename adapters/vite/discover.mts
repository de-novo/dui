// Discover navigable TanStack file routes without importing application modules.
import { existsSync, globSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import type { Rule } from "../typescript/contracts.mjs";

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
interface Node {
  type: string;
  start: number;
  name: string;
  value: string;
  computed: boolean;
  callee: Node;
  arguments: Node[];
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
}
const routes: { pattern: string; source: string }[] = [];
const candidates = new Map<string, string>();
const params = profile.routeParams ?? {};
const unresolved = new Set<string>();
const parseFailures: string[] = [];
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
for (const file of files) {
  if (/\.(test|spec|stories)\.[^.]+$/.test(file)) continue;
  const source = readFileSync(join(app, file), "utf8");
  if (!source.includes("${") && !routeRules.some((r) => source.includes(r.package))) continue;
  const ast = vite.parseSync(file, source);
  if (ast.errors.length) {
    parseFailures.push(relative(root, join(app, file)));
    continue;
  }
  const declarations = new Map<string, Node>();
  const factories = new Set<string>();
  for (const statement of ast.program.body as Node[]) {
    if (statement.type === "ImportDeclaration")
      for (const s of statement.specifiers)
        if (
          s.imported &&
          routeRules.some(
            (r) => r.package === statement.source.value && r.symbol === s.imported?.name,
          )
        )
          factories.add(s.local.name);
    const n = statement.declaration ?? statement;
    if (n.kind === "const")
      for (const d of n.declarations)
        if (d.init && d.id.type === "Identifier") declarations.set(d.id.name, d.init);
  }
  function literalTable(node: Node): string[] {
    const seen = new Set<Node>();
    while (node && !seen.has(node)) {
      seen.add(node);
      if (node.type === "Identifier") {
        const next = declarations.get(node.name);
        if (!next) break;
        node = next;
        continue;
      }
      if (
        ["TSAsExpression", "TSSatisfiesExpression", "ParenthesizedExpression"].includes(node.type)
      ) {
        node = node.expression;
        continue;
      }
      if (
        node.type === "ObjectExpression" &&
        node.properties.every(
          (p) =>
            p.type === "Property" &&
            !p.computed &&
            p.value.type === "Literal" &&
            typeof p.value.value === "string",
        )
      )
        return node.properties.map((p) => p.value.value);
      break;
    }
    return [];
  }
  new vite.Visitor({
    CallExpression(n: Node) {
      const factory = n.callee;
      if (factory.type !== "CallExpression" || !factories.has(factory.callee.name)) return;
      const pattern = factory.arguments[0]?.value;
      if (
        typeof pattern === "string" &&
        n.arguments[0]?.properties?.some((p) => p.key?.name === "component")
      )
        routes.push({ pattern, source: relative(root, join(app, file)) });
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
          candidates.set(prefix + tail, relative(root, join(app, file)));
    },
  }).visit(ast.program);
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
  const splat = parts.at(-1) === "$";
  if (splat) parts.pop();
  const segments = parts
    .map((part) => part.replace(/_$/, ""))
    .map((part) => (part.startsWith("$") ? segment(part.slice(1)) : part));
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
  stateBudget: 1,
  ...screen(entry),
  scenarios: paths.slice(1).map(screen),
  screenDiscovery: {
    sources: paths.length ? Object.fromEntries(screens) : { [entry]: { kind: "entry" } },
    unresolved: [...unresolved].sort(),
    parseFailures,
    mode: paths.length ? "router" : "links",
  },
};
if (!paths.length) {
  config.linkDefaults = {
    cookies: profile.cookies ?? {},
    readySelector: profile.readySelector ?? defaultReady,
    hasScreenAssertion: Boolean(profile.readySelector),
  };
  config.routeSelector = profile.routeSelector ?? "main a[href], nav a[href], header a[href]";
} else delete config.routeSelector;
if (config.mockPreload) config.mockPreload = resolve(root, config.mockPreload);
if (config.rulePacks) config.rulePacks = config.rulePacks.map((p: string) => resolve(root, p));
console.log(JSON.stringify(config));
