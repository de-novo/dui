import fs from "node:fs";
import { parseSync, visitorKeys } from "oxc-parser";
import { ResolverFactory } from "oxc-resolver";
export const walk = (n, fn) => {
  if (!n?.type) return;
  fn(n);
  for (const k of visitorKeys[n.type] ?? []) {
    const v = n[k];
    if (Array.isArray(v)) v.forEach((c) => walk(c, fn));
    else walk(v, fn);
  }
};
const member = (n) =>
  n?.type === "Identifier"
    ? n.name
    : n?.type === "MemberExpression"
      ? member(n.object) + "." + n.property.name
      : "";
const field = (o, k) => o?.properties?.find((p) => p.key?.name === k)?.value;
export function project(root) {
  root = fs.realpathSync(root);
  const cache = new Map(),
    refs = new Map(),
    resolver = new ResolverFactory({
      extensions: [".tsx", ".ts", ".jsx", ".js"],
      tsconfig: "auto",
    });
  function load(file) {
    if (cache.has(file)) return cache.get(file);
    const source = fs.readFileSync(file, "utf8"),
      p = parseSync(file, source);
    if (p.errors.length) throw Error("unparsed " + file);
    const links = [],
      used = new Set(),
      imports = [],
      exports = [];
    function resolve(s) {
      const f = resolver.resolveFileSync(file, s).path;
      return f && !f.includes("/node_modules/") && f.startsWith(root + "/") && /\.[jt]sx?$/.test(f)
        ? f
        : null;
    }
    for (const n of p.program.body) {
      if (n.type === "ImportDeclaration" && n.importKind !== "type")
        for (const spec of n.specifiers)
          if (spec.importKind !== "type")
            imports.push([spec.local.name, resolve(n.source.value), spec.imported?.name]);
      if (n.source && n.type.startsWith("Export")) exports.push(resolve(n.source.value));
    }
    walk(p.program, (n) => {
      if (n.type === "JSXOpeningElement") used.add(n.name.name);
      if (
        n.type === "JSXAttribute" &&
        ["href", "to"].includes(n.name.name) &&
        typeof n.value?.value === "string"
      ) {
        let s = n.value.value;
        if (s.startsWith("#") || s.startsWith("?")) s = s === "?" ? "/" : "/" + s;
        links.push([s, n.start]);
      }
      if (n.type === "CallExpression" && member(n.callee).endsWith(".getHref"))
        links.push([refs.get(member(n.callee).replace(/\.getHref$/, "")), n.start]);
      if (n.type === "CallExpression" && n.callee.name === "withSearchChanges") {
        const v = field(n.arguments[1], "view");
        if (v) links.push([v.value === null ? "/" : "/?view=" + v.value, n.start]);
      }
    });
    const module = { links, used, imports, exports, program: p.program, resolve };
    cache.set(file, module);
    return module;
  }
  const owners = [];
  if (fs.existsSync(root + "/src/app/router.tsx")) {
    const cfg = load(root + "/src/config/paths.ts").program;
    function index(o, key, parent) {
      for (const p of o.properties ?? []) {
        const k = key + "." + p.key.name,
          v = p.value,
          seg = field(v, "path")?.value;
        let current = parent;
        if (typeof seg === "string")
          current = seg.startsWith("/") ? seg : (parent + "/" + seg).replace(/\/$/, "");
        else if (field(field(v, "root"), "path")?.value)
          current = field(field(v, "root"), "path").value;
        if (typeof seg === "string") refs.set(k, current);
        if (v.type === "ObjectExpression") index(v, k, current);
      }
    }
    walk(cfg, (n) => {
      if (n.type === "VariableDeclarator" && n.id.name === "paths")
        index(n.init.expression, "paths", "");
    });
    const router = load(root + "/src/app/router.tsx");
    function route(o, layouts) {
      const prop = field(o, "path"),
        id = prop?.value ?? refs.get(member(prop).replace(/\.path$/, ""));
      let file;
      walk(field(o, "lazy"), (n) => {
        if (n.type === "ImportExpression") file = router.resolve(n.source.value);
      });
      if (file && id) owners.push({ id, file, layouts });
      const shell = [];
      walk(field(o, "element"), (n) => {
        if (n.type === "JSXOpeningElement") {
          const binding = router.imports.find((b) => b[0] === n.name.name);
          if (binding?.[1]) shell.push(binding[1]);
        }
      });
      for (const child of field(o, "children")?.elements ?? [])
        route(child, [...layouts, ...shell]);
    }
    walk(router.program, (n) => {
      if (n.type === "CallExpression" && n.callee.name === "createBrowserRouter")
        n.arguments[0].elements.forEach((o) => route(o, []));
    });
  } else if (fs.existsSync(root + "/js/app.tsx")) {
    const file = root + "/js/app.tsx";
    walk(load(file).program, (n) => {
      if (n.type === "Property" && typeof n.key.value === "string" && n.key.value.startsWith("/"))
        owners.push({ id: "/#" + n.key.value, file, layouts: [] });
    });
  } else if (fs.existsSync(root + "/app/routes")) {
    const dir = root + "/app/routes";
    const names = fs.readdirSync(dir).filter((f) => f.endsWith(".tsx"));
    for (const f of names) {
      const parts = f.slice(0, -4).split(".");
      const id =
        "/" +
        parts
          .filter((s) => !s.startsWith("_"))
          .map((s) => s.replace(/^\$/, ":"))
          .join("/");
      const parents = parts
        .slice(0, -1)
        .map((_, i) => parts.slice(0, i + 1).join(".") + ".tsx")
        .filter((s) => names.includes(s));
      owners.push({
        id,
        file: dir + "/" + f,
        layouts: [root + "/app/root.tsx", ...parents.map((s) => dir + "/" + s)],
      });
    }
  } else {
    for (const [id, rel] of [
      ["/", "src/preview.tsx"],
      ["/?view=diff", "src/preview.tsx"],
      ["/?view=model", "src/features/model/ModelView.tsx"],
    ])
      owners.push({ id, file: root + "/" + rel, layouts: [] });
  }
  return { load, owners };
}
