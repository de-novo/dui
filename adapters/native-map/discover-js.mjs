// Discover source declarations without loading the application's modules.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const extensions = [".tsx", ".ts", ".jsx", ".js", ".mts", ".mjs", ".cts", ".cjs"];
const slash = (value) => value.split(sep).join("/");
const ignored = new Set([
  "node_modules",
  "dist",
  "build",
  "coverage",
  "android",
  "ios",
  "__tests__",
  "__mocks__",
  "test",
  "tests",
  "e2e",
]);

function* sourceFiles(dir, expo) {
  for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) =>
    compare(a.name, b.name),
  )) {
    if (entry.name.startsWith(".")) continue;
    const file = join(dir, entry.name);
    if (entry.isDirectory() && !(expo ? entry.name === "node_modules" : ignored.has(entry.name))) {
      yield* sourceFiles(file, expo);
    } else if (
      entry.isFile() &&
      extensions.includes(extname(file)) &&
      !/\.d\.[cm]?ts$/.test(file) &&
      !/\.(?:spec|test|stories)\.[^.]+$/.test(file)
    )
      yield file;
  }
}

function compare(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

// The TypeScript 7 API reads ASTs from a program snapshot. Give it only the
// collected sources and a private in-memory config so imports are never loaded.
async function syntax(root, files) {
  const req = createRequire(join(root, "package.json"));
  const pkg = req("typescript/package.json");
  if (pkg.name !== "typescript")
    throw Error("The project's official typescript package is required");
  if (pkg.version.startsWith("6.")) {
    const ts = req("typescript");
    return {
      K: ts.SyntaxKind,
      constFlag: ts.NodeFlags.Const,
      file(path) {
        const sf = ts.createSourceFile(
          path,
          readFileSync(path, "utf8"),
          ts.ScriptTarget.Latest,
          true,
        );
        return sf.parseDiagnostics.length ? undefined : sf;
      },
      close() {},
    };
  }
  if (!pkg.version.startsWith("7."))
    throw Error("Unsupported TypeScript syntax API " + pkg.version);
  const load = (name) => import(pathToFileURL(req.resolve("typescript/unstable/" + name)).href);
  const [{ API }, { SyntaxKind, NodeFlags }] = await Promise.all([load("sync"), load("ast")]);
  const virtual = "/__dui_native_inventory__";
  const config = virtual + "/tsconfig.json";
  const ids = new Map(files.map((file, i) => [file, virtual + "/s" + i + extname(file)]));
  const paths = new Map([...ids].map(([path, id]) => [id, path]));
  const configText = JSON.stringify({
    compilerOptions: { allowJs: true, jsx: "preserve", noLib: true, noResolve: true, types: [] },
    files: [...paths.keys()],
  });
  const api = new API({
    cwd: virtual,
    fs: {
      readFile: (p) =>
        p === config ? configText : paths.has(p) ? readFileSync(paths.get(p), "utf8") : undefined,
      fileExists: (p) => p === config || paths.has(p),
      directoryExists: (p) => p === virtual,
      realpath: (p) => p,
    },
  });
  try {
    const snapshot = api.updateSnapshot({ openProjects: [config] });
    const program = snapshot.getProject(config)?.program;
    if (!program) throw Error("TypeScript could not open the source inventory");
    return {
      K: SyntaxKind,
      constFlag: NodeFlags.Const,
      file(path) {
        const id = ids.get(path);
        const sf = program.getSourceFile(id);
        return sf && !program.getSyntacticDiagnostics(id).length ? sf : undefined;
      },
      close() {
        snapshot.dispose();
        api.close();
      },
    };
  } catch (error) {
    api.close();
    throw error;
  }
}

function lineOf(sf, node) {
  return sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1;
}

function discoverExpo(root, app, files, parser, report) {
  const variants = new Map();
  for (const file of files) {
    const stem = slash(relative(app, file)).slice(0, -extname(file).length);
    const platform = /\.(android|ios|native|web)$/.exec(stem);
    const key = platform ? stem.slice(0, platform.index) : stem;
    const leaf = key.split("/").at(-1);
    if (
      leaf === "_layout" ||
      leaf.endsWith("+api") ||
      (leaf.startsWith("+") && leaf !== "+not-found")
    )
      continue;
    if (!variants.has(key)) variants.set(key, new Map());
    variants.get(key).set(platform?.[1] ?? "base", file);
  }
  for (const [key, entries] of variants) {
    const base = entries.get("base");
    if (!base) {
      if ([...entries.keys()].some((v) => v !== "web")) {
        report.warn("Expo Router requires a non-platform route file for " + key);
      }
      continue;
    }
    const sf = parser.file(base);
    const decl = sf?.statements.find(
      (s) =>
        ([parser.K.FunctionDeclaration, parser.K.ClassDeclaration].includes(s.kind) &&
          s.modifiers?.some((m) => m.kind === parser.K.DefaultKeyword)) ||
        (s.kind === parser.K.ExportAssignment && !s.isExportEquals) ||
        (s.kind === parser.K.ExportDeclaration &&
          !s.isTypeOnly &&
          s.exportClause?.elements?.some((e) => !e.isTypeOnly && e.name.text === "default")),
    );
    if (!decl) {
      report.warn("No parseable default screen export in " + slash(relative(root, base)));
      continue;
    }
    const parts = key.split("/");
    if (parts.at(-1) === "index") parts.pop();
    let names = [""];
    for (const segment of parts) {
      const group = /^\(([^()]+,[^()]+)\)$/.exec(segment);
      const segments = group ? group[1].split(",").map((s) => "(" + s.trim() + ")") : [segment];
      names = names.flatMap((prefix) => segments.map((s) => prefix + "/" + s));
    }
    const path = "/" + parts.filter((p) => !/^\(.+\)$/.test(p)).join("/");
    for (const name of names) {
      report.screens.push({
        name: name || "/",
        path,
        source: slash(relative(root, base)),
        line: lineOf(sf, decl),
        ...(path.includes("[") ? { dynamic: true } : {}),
      });
    }
  }
}

function walk(node, visit) {
  visit(node);
  node.forEachChild((child) => {
    walk(child, visit);
  });
}

const navigatorExport = (module, name) =>
  module.startsWith("@react-navigation/") && /^create[A-Za-z]*Navigator$/.test(name);

function discoverNavigation(root, files, parser, report) {
  const K = parser.K;
  const fileSet = new Set(files);
  const modules = new Map();

  function unwrap(n) {
    while (
      n &&
      [
        K.ParenthesizedExpression,
        K.AsExpression,
        K.SatisfiesExpression,
        K.TypeAssertionExpression,
        K.NonNullExpression,
      ].includes(n.kind)
    )
      n = n.expression;
    return n;
  }
  function string(n) {
    n = unwrap(n);
    if (n && [K.StringLiteral, K.NoSubstitutionTemplateLiteral].includes(n.kind)) return n.text;
  }
  function member(obj, key) {
    return unwrap(obj)?.properties?.findLast((p) => p.name?.text === key)?.initializer;
  }
  function moduleFor(file) {
    if (modules.has(file)) return modules.get(file);
    const sf = parser.file(file);
    if (!sf) {
      report.warn("Cannot parse " + slash(relative(root, file)));
      return;
    }
    const mod = {
      sf,
      file,
      vars: new Map(),
      imports: new Map(),
      exports: new Map(),
      namespaces: new Map(),
      factories: new Set(),
    };
    modules.set(file, mod);
    for (const stmt of sf.statements) {
      const spec = stmt.moduleSpecifier?.text;
      if (stmt.kind === K.ImportDeclaration && !stmt.importClause?.isTypeOnly) {
        const named = stmt.importClause?.namedBindings;
        if (named?.kind === K.NamespaceImport) mod.namespaces.set(named.name.text, spec);
        for (const el of named?.elements ?? []) {
          if (!el.isTypeOnly)
            mod.imports.set(el.name.text, [spec, (el.propertyName ?? el.name).text]);
        }
      }
      if (stmt.kind === K.ExportDeclaration && spec && !stmt.isTypeOnly) {
        for (const el of stmt.exportClause?.elements ?? []) {
          if (!el.isTypeOnly)
            mod.exports.set(el.name.text, [spec, (el.propertyName ?? el.name).text]);
        }
      }
      if (!(stmt.declarationList?.flags & parser.constFlag)) continue;
      for (const d of stmt.declarationList.declarations) {
        if (d.name.kind === K.Identifier && d.initializer) mod.vars.set(d.name.text, d.initializer);
      }
    }
    // A custom navigator often delegates to the public createNavigatorFactory.
    // Only accept direct returns from the wrapper itself.
    function delegates(expr) {
      const outer = unwrap(expr);
      const inner = unwrap(outer?.expression);
      if (outer?.kind !== K.CallExpression || inner?.kind !== K.CallExpression) return false;
      const binding = mod.imports.get(inner.expression.text);
      return binding?.[0] === "@react-navigation/native" && binding[1] === "createNavigatorFactory";
    }
    for (const s of sf.statements) {
      if (
        s.kind === K.FunctionDeclaration &&
        s.name &&
        s.body?.statements.some((st) => st.kind === K.ReturnStatement && delegates(st.expression))
      ) {
        mod.factories.add(s.name.text);
      }
    }
    return mod;
  }

  function localFile(mod, spec) {
    if (!spec.startsWith(".")) return;
    const prefix = resolve(dirname(mod.file), spec);
    return [
      prefix,
      ...extensions.map((x) => prefix + x),
      ...extensions.map((x) => join(prefix, "index" + x)),
    ].find((p) => fileSet.has(p));
  }
  function isFactory(mod, expr) {
    if (mod.factories.has(expr?.text)) return true;
    const ns = mod.namespaces.get(expr?.expression?.text);
    if (ns && navigatorExport(ns, expr.name?.text)) return true;
    let binding = mod.imports.get(expr?.text);
    const seen = new Set();
    while (binding) {
      const [spec, name] = binding;
      if (navigatorExport(spec, name)) return true;
      const file = localFile(mod, spec);
      const key = file + "#" + name;
      if (!file || seen.has(key)) break;
      seen.add(key);
      mod = moduleFor(file);
      if (!mod) break;
      if (mod.factories.has(name)) return true;
      binding = mod.exports.get(name);
    }
    return false;
  }

  for (const file of files) {
    // A file re-exporting a factory can also be loaded lazily by moduleFor.
    if (!/Navigator|\.Screen\b|@react-navigation\/|\bscreens\s*:/.test(readFileSync(file, "utf8")))
      continue;
    const mod = moduleFor(file);
    if (!mod) continue;
    const { sf } = mod;
    const source = slash(relative(root, file));
    const platform = /\.(ios|android)\.[^.]+$/.exec(file)?.[1];
    const scope = platform ? ['Platform.OS === "' + platform + '"'] : [];
    function warnAt(n, message) {
      report.warn(source + ":" + lineOf(sf, n) + " " + message);
    }
    function literal(n, seen = new Set()) {
      n = unwrap(n);
      if (!n || seen.has(n)) return;
      if (n.kind === K.Identifier) return literal(mod.vars.get(n.text), new Set(seen).add(n));
      if (n.kind !== K.TemplateExpression) return string(n);
      let result = n.head.text;
      for (const span of n.templateSpans) {
        const value = literal(span.expression, seen);
        if (value === undefined) return;
        result += value + span.literal.text;
      }
      return result;
    }
    function add(n, name, linking, conditions) {
      const path = linking === undefined ? name : "/" + linking.replace(/^\/+/, "");
      report.screens.push({
        name,
        path,
        source,
        line: lineOf(sf, n),
        ...(conditions.length ? { condition: conditions.join(" && ") } : {}),
        ...(/(?:^|\/)(?::|\*)/.test(path) ? { dynamic: true } : {}),
      });
    }
    function entries(input, conditions, accept, seen = new Set()) {
      const n = unwrap(input);
      if (!n) return;
      if (n.kind === K.Identifier && mod.vars.has(n.text) && !seen.has(n)) {
        entries(mod.vars.get(n.text), conditions, accept, new Set(seen).add(n));
      } else if (n.kind === K.ConditionalExpression) {
        const test = n.condition.getText(sf);
        entries(n.whenTrue, [...conditions, test], accept, seen);
        entries(n.whenFalse, [...conditions, "!(" + test + ")"], accept, seen);
      } else if (
        n.kind === K.BinaryExpression &&
        n.operatorToken.kind === K.AmpersandAmpersandToken
      ) {
        entries(n.right, [...conditions, n.left.getText(sf)], accept, seen);
      } else if (n.kind === K.ObjectLiteralExpression) {
        for (const p of n.properties) {
          if (p.kind === K.SpreadAssignment) entries(p.expression, conditions, accept, seen);
          else if (p.name?.text !== undefined) accept(p, conditions);
          else warnAt(p, "Computed registration key could not be resolved");
        }
      } else warnAt(n, "Registration object could not be resolved statically");
    }
    function config(input, conditions = []) {
      entries(input, conditions, (p, branch) => {
        if (p.name.text === "screens") {
          entries(p.initializer ?? p.name, branch, (screen, active) => {
            const value = unwrap(screen.initializer);
            const body = value?.kind === K.CallExpression ? value.arguments[0] : value;
            const link = member(body, "linking");
            const href = literal(link) ?? literal(member(link, "path"));
            if (link && href === undefined)
              warnAt(link, "Linking path could not be resolved statically");
            const guard = member(body, "if");
            add(screen, screen.name.text, href, guard ? [...active, guard.getText(sf)] : active);
          });
        } else if (p.name.text === "groups") {
          entries(p.initializer ?? p.name, branch, (group, active) => {
            const guard = member(group.initializer, "if");
            config(group.initializer, guard ? [...active, guard.getText(sf)] : active);
          });
        }
      });
    }
    const owners = new Set();
    walk(sf, (node) => {
      if (node.kind !== K.CallExpression || !isFactory(mod, node.expression)) return;
      if (node.arguments.length) config(node.arguments[0], scope);
      const parent = node.parent;
      if (parent?.kind === K.VariableDeclaration) owners.add(parent.name.text);
    });
    function jsx(node, owner, conditions) {
      if (node.kind === K.IfStatement) {
        const test = node.expression.getText(sf);
        jsx(node.thenStatement, owner, [...conditions, test]);
        if (node.elseStatement) jsx(node.elseStatement, owner, [...conditions, "!(" + test + ")"]);
        return;
      }
      if (node.kind === K.ConditionalExpression) {
        const test = node.condition.getText(sf);
        jsx(node.whenTrue, owner, [...conditions, test]);
        jsx(node.whenFalse, owner, [...conditions, "!(" + test + ")"]);
        return;
      }
      if (
        node.kind === K.BinaryExpression &&
        node.operatorToken.kind === K.AmpersandAmpersandToken
      ) {
        jsx(node.right, owner, [...conditions, node.left.getText(sf)]);
        return;
      }
      const el = node.openingElement ?? (node.kind === K.JsxSelfClosingElement ? node : undefined);
      const tag = el?.tagName;
      if (tag?.name?.text === "Navigator")
        owner = owners.has(tag.expression.text) ? tag.expression.text : undefined;
      if (owner && tag?.expression?.text === owner && tag.name?.text === "Screen") {
        const attr = el.attributes.properties.find((p) => p.name?.text === "name")?.initializer;
        const name = literal(attr?.expression ?? attr);
        if (name !== undefined) add(el, name, undefined, conditions);
        else warnAt(el, "Dynamic screen name could not be resolved");
      }
      node.forEachChild((child) => {
        jsx(child, owner, conditions);
      });
    }
    if (owners.size) jsx(sf, undefined, scope);
  }
}

export async function discover(root) {
  root = resolve(root);
  const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const deps = {
    ...manifest.devDependencies,
    ...manifest.dependencies,
    ...manifest.peerDependencies,
  };
  const expo = Boolean(deps["expo-router"] || manifest.main?.startsWith("expo-router/"));
  const screens = [];
  const warnings = new Set();
  const report = { screens, warn: (text) => warnings.add(text) };
  const app = expo
    ? [join(root, "src", "app"), join(root, "app")].find(
        (p) => existsSync(p) && statSync(p).isDirectory(),
      )
    : root;
  if (!app) report.warn("No standard Expo Router app directory found");
  else {
    const files = [...sourceFiles(app, expo)].filter((p) => expo || !/\.web\.[^.]+$/.test(p));
    if (files.length) {
      const parser = await syntax(root, files);
      try {
        if (expo) discoverExpo(root, app, files, parser, report);
        else discoverNavigation(root, files, parser, report);
      } finally {
        parser.close();
      }
    }
  }
  if (!screens.length) report.warn("No statically declared native screens found");
  screens.sort(
    (a, b) =>
      compare(a.path, b.path) ||
      compare(a.name, b.name) ||
      compare(a.source, b.source) ||
      a.line - b.line,
  );
  return { screens, limitations: [...warnings].sort(compare) };
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try {
    console.log(JSON.stringify(await discover(process.argv[2])));
  } catch (error) {
    console.log(
      JSON.stringify({ screens: [], limitations: [String(error.message).split("\n")[0]] }),
    );
  }
}
