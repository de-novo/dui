// Usage: node discover.mjs PROJECT ENTRY (entry is relative to project).
import { readFileSync, realpathSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { parseSync, Visitor } from "oxc-parser";
import { ResolverFactory } from "oxc-resolver";

const [projectArg, entryArg] = process.argv.slice(2);
if (!entryArg) throw Error("expected PROJECT ENTRY");
const root = realpathSync(projectArg);
const entry = realpathSync(path.resolve(root, entryArg));
const local = (file) => {
  const rel = path.relative(root, file);
  return (
    rel !== ".." &&
    !rel.startsWith(".." + path.sep) &&
    !path.isAbsolute(rel) &&
    !rel.split(path.sep).includes("node_modules")
  );
};
if (!local(entry)) throw Error("entry outside project");
const resolver = new ResolverFactory({
  tsconfig: "auto",
  extensions: [".tsx", ".ts", ".jsx", ".js", ".mts", ".mjs"],
});
const parents = new Map([[entry, null]]);
const diagnostics = [];
const routers = [];
const extract = fileURLToPath(new URL("../renderer-geometry/discover.py", import.meta.url));
for (const file of parents.keys()) {
  const parsed = parseSync(file, readFileSync(file, "utf8"));
  if (parsed.errors.length) {
    diagnostics.push({ file: path.relative(root, file), parseErrors: parsed.errors.length });
    continue;
  }
  const bindings = new Map();
  function follow(request) {
    const target = resolver.resolveFileSync(file, request).path;
    if (!target) {
      if (request.startsWith("."))
        diagnostics.push({ file: path.relative(root, file), unresolved: request });
      return;
    }
    if (!local(target) || !/\.[cm]?[jt]sx?$/.test(target) || /\.d\.[cm]?ts$/.test(target)) return;
    if (!parents.has(target)) parents.set(target, file);
    return target;
  }
  for (const item of parsed.module.staticImports) {
    const values = item.entries.filter((e) => !e.isType);
    if (item.entries.length && !values.length) continue;
    const target = follow(item.moduleRequest.value);
    for (const e of values)
      bindings.set(e.localName.value, {
        source: item.moduleRequest.value,
        imported: e.importName.name,
        target,
      });
  }
  for (const decl of parsed.module.staticExports) {
    for (const e of decl.entries) if (!e.isType && e.moduleRequest) follow(e.moduleRequest.value);
  }
  let candidate = false;
  let ambientRouter = false;
  let directorCall = false;
  new Visitor({
    ImportExpression(node) {
      if (typeof node.source.value === "string") follow(node.source.value);
    },
    VariableDeclaration(node) {
      if (node.declare && node.declarations.some((decl) => decl.id.name === "Router"))
        ambientRouter = true;
    },
    CallExpression(node) {
      const name = node.callee.name;
      const binding = bindings.get(name);
      if (
        name === "createBrowserRouter" &&
        binding?.imported === name &&
        binding.source === "react-router" &&
        node.arguments[0]?.type === "ArrayExpression"
      )
        candidate = true;
      if (name === "Router" && node.arguments[0]?.type === "ObjectExpression") directorCall = true;
    },
  }).visit(parsed.program);
  if (!candidate && !(ambientRouter && directorCall)) continue;
  const config = bindings.get("paths");
  const configFile = candidate && config?.imported === "paths" ? config.target : undefined;
  const args = [extract, "--experimental-router", file];
  if (configFile) args.push("--paths", configFile);
  const result = spawnSync(process.env.PYTHON || "python3", args, { encoding: "utf8" });
  if (result.error || result.status !== 0) throw Error(result.error?.message || result.stderr);
  const chain = [];
  for (let p = file; p; p = parents.get(p)) chain.unshift(path.relative(root, p));
  routers.push({
    chain,
    config: configFile && path.relative(root, configFile),
    routes: JSON.parse(result.stdout),
  });
}
console.log(JSON.stringify({ modules: parents.size, routers, diagnostics }));
