import type { Syntax, Type, Shape, Manifest, Rule, Fact } from "./contracts.mjs";
import type { NodeHandle, UnionType, LiteralType } from "typescript/unstable/sync";
import type { SourceFile } from "typescript/unstable/ast";
type Declaration = Syntax | NodeHandle;
type Diagnostic = { code: number };

// The installed project compiler owns resolution, alias chasing and all type queries.
import { createRequire } from "node:module";
import { existsSync, readFileSync } from "node:fs";
import { resolve, relative, join, sep, dirname } from "node:path";
import { pathToFileURL } from "node:url";

const [root, specPath, rulesPath] = process.argv.slice(2);
const spec: Manifest = JSON.parse(readFileSync(specPath, "utf8"));
const rules: Rule[] = [
  rulesPath,
  ...(spec.rulePacks ?? []).map((p) => resolve(dirname(specPath), p)),
].flatMap((path) => {
  const pack = JSON.parse(readFileSync(path, "utf8"));
  if (
    pack.schemaVersion !== 1 ||
    !Array.isArray(pack.rules) ||
    Object.keys(pack).some((k) => !["schemaVersion", "rules"].includes(k))
  )
    throw Error("unsupported rule pack: " + path);
  return pack.rules;
});
const kinds = new Set([
  "slot",
  "store",
  "machine",
  "form",
  "route",
  "query",
  "schema",
  "search",
  "control",
]);
const entries = new Set();
for (const rule of rules) {
  const id = JSON.stringify([rule.package, rule.symbol]);
  if (
    typeof rule.package !== "string" ||
    !rule.package ||
    typeof rule.symbol !== "string" ||
    !rule.symbol ||
    !kinds.has(rule.kind) ||
    entries.has(id) ||
    Object.keys(rule).some((k) => !["package", "symbol", "kind"].includes(k))
  )
    throw Error("invalid rule: " + id);
  entries.add(id);
}
const require = createRequire(join(root, "package.json"));
const appRequire = createRequire(join(resolve(root, spec.appRoot ?? "."), "package.json"));
const versions: Record<string, string> = {};
function unsupported(reason: string): never {
  process.stderr.write(JSON.stringify({ status: "unsupported", reason }));
  process.exit(4);
}
function readInstalledPackageVersion(name: string): string | null {
  try {
    return appRequire(name + "/package.json").version ?? null;
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    if (code === "MODULE_NOT_FOUND") return null;
    if (code !== "ERR_PACKAGE_PATH_NOT_EXPORTED") throw error;
    let entry: string;
    try {
      entry = appRequire.resolve(name);
    } catch (resolveErr) {
      if ((resolveErr as NodeJS.ErrnoException).code === "MODULE_NOT_FOUND") return null;
      throw resolveErr;
    }
    let dir = dirname(entry);
    for (let depth = 0; depth < 8; depth++) {
      const candidate = join(dir, "package.json");
      if (existsSync(candidate)) {
        const pkg = JSON.parse(readFileSync(candidate, "utf8")) as {
          name?: string;
          version?: string;
        };
        if (pkg.name === name && typeof pkg.version === "string") return pkg.version;
      }
      const parent = dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
    return null;
  }
}
for (const [name, major] of Object.entries({
  react: 19,
  "react-dom": 19,
  vite: 8,
  "@tanstack/react-router": 1,
  "@tanstack/react-start": 1,
  "@tanstack/react-query": 5,
  zod: 4,
  zustand: 5,
  "@reduxjs/toolkit": 2,
  jotai: 3,
  xstate: 5,
  "react-hook-form": 7,
  valibot: 1,
  "react-router": 8,
  next: 16,
  "radix-ui": 1,
  "@radix-ui/react-dialog": 1,
  "@radix-ui/react-popover": 1,
  "@radix-ui/react-tabs": 1,
  "@radix-ui/react-accordion": 1,
  "@radix-ui/react-select": 1,
  "@radix-ui/react-dropdown-menu": 1,
  "@radix-ui/react-switch": 1,
  "@radix-ui/react-checkbox": 1,
  ink: 5,
  "expo-router": 5,
})) {
  const actual = readInstalledPackageVersion(name);
  if (actual === null) continue;
  if (!/^\d+\.\d+\.\d+$/.test(actual) || Number(actual.split(".")[0]) !== major)
    unsupported(name + " requires stable " + major + ".x; found " + actual);
  versions[name] = actual;
}
async function loadOfficial(part: string) {
  for (const prefix of ["typescript/", "typescript/unstable/"]) {
    let path;
    try {
      path = require.resolve(prefix + part);
    } catch (error) {
      if (
        ["ERR_PACKAGE_PATH_NOT_EXPORTED", "MODULE_NOT_FOUND"].includes(
          (error as NodeJS.ErrnoException).code ?? "",
        )
      )
        continue;
      throw error;
    }
    return import(pathToFileURL(path).href);
  }
  throw Error("installed TypeScript does not expose the required official " + part + " API");
}
// Only the compiler API boundary differs. Both versions use the same fact extractor.
async function openCompiler() {
  if (require("typescript/package.json").version.startsWith("6.")) {
    const ts = require("typescript");
    return {
      version: ts.version,
      enums: ts,
      ast: ts,
      project(config: string) {
        const parsed = ts.getParsedCommandLineOfConfigFile(
          config,
          {},
          {
            ...ts.sys,
            onUnRecoverableConfigFileDiagnostic(diagnostic: { messageText: unknown }) {
              throw Error(ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"));
            },
          },
        );
        if (!parsed) throw Error("compiler did not open " + config);
        const program = ts.createProgram({
          rootNames: parsed.fileNames,
          options: parsed.options,
          projectReferences: parsed.projectReferences,
        });
        return {
          program,
          checker: program.getTypeChecker(),
          rootFiles: parsed.fileNames as string[],
          configDiagnostics: parsed.errors as Diagnostic[],
          diagnostics: (sf: SourceFile) => [
            ...program.getSyntacticDiagnostics(sf),
            ...program.getSemanticDiagnostics(sf),
          ],
        };
      },
      declaration: (node?: Declaration) => node as Syntax | undefined,
      declarationPath: (node?: Declaration) =>
        (node as Syntax | undefined)?.getSourceFile().fileName,
      unionMembers: (type: Type) => (type as Type & { types: Type[] }).types,
    };
  }
  const enums = await loadOfficial("sync");
  const ast = await loadOfficial("ast");
  const api = new enums.API({ cwd: root });
  return {
    version: require("typescript/package.json").version,
    enums,
    ast,
    project(config: string) {
      const project = api.updateSnapshot({ openProjects: [config] }).getProject(config);
      if (!project) throw Error("compiler did not open " + config);
      const { program, checker, rootFiles } = project;
      return {
        program,
        checker,
        rootFiles: rootFiles as string[],
        configDiagnostics: [],
        diagnostics: (sf: SourceFile) => [
          ...program.getSyntacticDiagnostics(sf.fileName),
          ...program.getSemanticDiagnostics(sf.fileName),
        ],
      };
    },
    declaration: (handle?: Declaration) =>
      (handle as NodeHandle | undefined)?.resolve() as Syntax | undefined,
    declarationPath: (handle?: Declaration) => (handle as NodeHandle | undefined)?.path,
    unionMembers: (type: Type) => (type as UnionType).getTypes(),
    close: () => api.close(),
  };
}
const compiler = await openCompiler();
const { SymbolFlags, TypeFlags, SignatureKind } = compiler.enums;
const { SyntaxKind: K, NodeFlags } = compiler.ast;
try {
  const sourceFiles = new Set();
  const moduleGraph = new Map<string, Set<string>>();
  const allFacts: Fact[] = [],
    allDiagnostics = [];
  for (const scope of [spec, ...(spec.additionalScopes ?? [])]) {
    const config = resolve(root, scope.tsconfig ?? "tsconfig.json");
    const project = compiler.project(config);
    const { program, checker } = project;
    const localFiles = project.rootFiles
      .filter((f) => !f.includes("/node_modules/") && !f.endsWith(".d.ts"))
      .map((f) => relative(root, f));
    const dirs = scope.sourceDirs ?? [];
    const files = [
      ...new Set([
        ...(scope.sources ?? (dirs.length ? [] : localFiles)),
        ...localFiles.filter((file) => dirs.some((dir) => file.startsWith(dir + "/"))),
      ]),
    ].sort();
    const facts: Fact[] = [];
    const diagnostics = project.configDiagnostics.map((d) => ({
      file: relative(root, config),
      code: d.code,
    }));
    function renderedText(node: Syntax | undefined) {
      if (
        !node ||
        node.kind !== K.JsxElement ||
        node.children.length !== 1 ||
        node.children[0].kind !== K.JsxText
      )
        return null;
      return node.children[0].getText().trim() || null;
    }
    function binding(node: Syntax) {
      const decl = compiler.declaration(checker.getSymbolAtLocation(node)?.valueDeclaration);
      return decl && decl.getSourceFile().fileName === node.getSourceFile().fileName
        ? decl.getStart()
        : null;
    }
    function predicate(expr: Syntax) {
      if (expr.kind === K.Identifier) return { binding: binding(expr), operator: "truthy" };
      if (
        expr.kind === K.BinaryExpression &&
        expr.left.kind === K.Identifier &&
        expr.operatorToken.kind === K.EqualsEqualsEqualsToken &&
        [K.StringLiteral, K.NumericLiteral, K.TrueKeyword, K.FalseKeyword].includes(expr.right.kind)
      ) {
        const literal = checker.getTypeAtLocation(expr.right);
        const rep = shape(literal);
        if (rep.kind === "literal")
          return {
            binding: binding(expr.left),
            operator: "equals",
            value: rep.value,
          };
      }
      return null;
    }
    const shape = (t: Type | undefined, depth = 0): Shape => {
      if (!t || depth > 3) return { kind: "unknown" };
      if (t.flags & TypeFlags.Union)
        return {
          kind: "union",
          members: compiler.unionMembers(t).map((v) => shape(v, depth + 1)),
        };
      if (t.flags & (TypeFlags.StringLiteral | TypeFlags.NumberLiteral))
        return { kind: "literal", value: (t as LiteralType).value };
      if (t.flags & TypeFlags.BooleanLiteral)
        return { kind: "literal", value: checker.typeToString(t) === "true" };
      if (t.flags & TypeFlags.Null) return { kind: "literal", value: null };
      if (t.flags & TypeFlags.Boolean) return { kind: "boolean" };
      if (t.flags & TypeFlags.String) return { kind: "string" };
      if (t.flags & TypeFlags.Number) return { kind: "number" };
      if (checker.isArrayType(t))
        return {
          kind: "array",
          item: shape(checker.getTypeArguments(t)[0], depth + 1),
        };
      return { kind: "unknown" };
    };
    function symbol(node: Syntax) {
      let sym = checker.getSymbolAtLocation(node);
      if (sym && sym.flags & SymbolFlags.Alias) sym = checker.getAliasedSymbol(sym);
      return sym;
    }
    // Bind the JSON decoder to the fetch that produced its Response.
    function unwrap(node: Syntax | undefined): Syntax | undefined {
      while (node && [K.ParenthesizedExpression, K.AwaitExpression].includes(node.kind))
        node = node.expression;
      return node;
    }
    function responseContract(call: Syntax) {
      const access = call.expression;
      if (access.name?.text !== "json" || call.arguments.length) return;
      let request = unwrap(access.expression);
      if (request && request.kind === K.Identifier) {
        const decl = compiler.declaration(symbol(request)?.valueDeclaration);
        if (!decl || decl.kind !== K.VariableDeclaration || !(decl.parent.flags & NodeFlags.Const))
          return;
        request = unwrap(decl.initializer);
      }
      if (!request || request.kind !== K.CallExpression || request.arguments.length !== 1) return;
      const fetchSymbol = symbol(request.expression.name ?? request.expression);
      if (
        fetchSymbol?.name !== "fetch" ||
        !fetchSymbol.declarations?.some((d: Declaration) =>
          compiler.declarationPath(d)?.endsWith("/lib.dom.d.ts"),
        )
      )
        return;
      const url = request.arguments[0];
      if (url.kind !== K.StringLiteral || !/^\/(?!\/)/.test(url.text) || /[\\#]/.test(url.text))
        return;
      let result = call;
      while ([K.AwaitExpression, K.ParenthesizedExpression].includes(result.parent.kind))
        result = result.parent;
      let annotation;
      if (
        [K.AsExpression, K.TypeAssertionExpression, K.VariableDeclaration].includes(
          result.parent.kind,
        )
      )
        annotation = result.parent.type;
      let type = annotation && checker.getTypeFromTypeNode(annotation);
      if (!annotation && result.parent.kind === K.ReturnStatement) {
        let fn = result.parent.parent;
        while (
          fn &&
          ![
            K.FunctionDeclaration,
            K.ArrowFunction,
            K.MethodDeclaration,
            K.FunctionExpression,
          ].includes(fn.kind)
        )
          fn = fn.parent;
        const ret = fn?.type && checker.getTypeFromTypeNode(fn.type);
        if (ret?.getSymbol()?.name === "Promise") type = checker.getTypeArguments(ret)[0];
      }
      let budget = 180;
      function describe(t: Type | undefined, depth: number): Shape {
        if (!t || --budget < 0 || depth > 9) return { kind: "unknown" };
        if (t.flags & TypeFlags.Union)
          return {
            kind: "union",
            members: compiler.unionMembers(t).map((x) => describe(x, depth + 1)),
          };
        if (checker.isArrayType(t))
          return {
            kind: "array",
            item: describe(checker.getTypeArguments(t)[0], depth + 1),
          };
        if (t.flags & TypeFlags.Object) {
          const props = checker.getPropertiesOfType(t);
          if (
            checker.isTupleType(t) ||
            checker.getIndexInfosOfType(t).length ||
            !props.length ||
            props.length > 48 ||
            checker.getSignaturesOfType(t, SignatureKind.Call).length ||
            checker.getSignaturesOfType(t, SignatureKind.Construct).length
          )
            return { kind: "unknown" };
          const fields = [];
          for (const p of props) {
            if (p.flags & SymbolFlags.Optional) continue;
            const decl = compiler.declaration(p.declarations?.[0]);
            if (
              !decl ||
              ![K.PropertySignature, K.PropertyDeclaration, K.PropertyAssignment].includes(
                decl.kind,
              )
            )
              return { kind: "unknown" };
            fields.push({
              name: p.name,
              shape: describe(checker.getTypeOfSymbolAtLocation(p, call), depth + 1),
            });
          }
          fields.sort((a, b) => (a.name < b.name ? -1 : Number(a.name > b.name)));
          return { kind: "object", fields };
        }
        return shape(t);
      }
      return { path: url.text, shape: describe(type, 0) };
    }
    for (const filename of files) {
      const normFile = filename.split(sep).join("/");
      sourceFiles.add(normFile);
      let deps = moduleGraph.get(normFile);
      if (!deps) {
        deps = new Set();
        moduleGraph.set(normFile, deps);
      }
      const recordDep = (declPath?: string) => {
        if (!declPath || declPath.includes("/node_modules/")) return;
        const target = relative(root, declPath).split(sep).join("/");
        if (target && target !== normFile && target !== ".." && !target.startsWith("../")) {
          deps.add(target);
        }
      };
      const sf = program.getSourceFile(resolve(root, filename));
      if (!sf) throw Error("not in compiler project: " + filename);
      for (const d of project.diagnostics(sf)) diagnostics.push({ file: filename, code: d.code });
      function walk(n: Syntax, owner: string) {
        const modSpec = n.moduleSpecifier;
        if (
          (n.kind === K.ImportDeclaration || n.kind === K.ExportDeclaration) &&
          modSpec &&
          modSpec.kind === K.StringLiteral
        ) {
          const modSym = symbol(modSpec);
          recordDep(compiler.declarationPath(modSym?.declarations?.[0]));
        }
        if ((n.kind === K.FunctionDeclaration || n.kind === K.MethodDeclaration) && n.name)
          owner = n.name.getText();
        if (n.kind === K.ArrowFunction && n.parent.kind === K.VariableDeclaration)
          owner = n.parent.name.getText();
        const start = n.getStart(sf);
        const base = {
          file: filename,
          owner,
          start,
          end: n.end,
          line: sf.getLineAndCharacterOfPosition(start).line + 1,
        };
        const jsxGuard =
          n.kind === K.BinaryExpression &&
          n.parent.kind === K.JsxExpression &&
          [K.AmpersandAmpersandToken, K.BarBarToken].includes(n.operatorToken.kind);
        if (n.kind === K.IfStatement || n.kind === K.ConditionalExpression || jsxGuard) {
          const expr = jsxGuard ? n.left : (n.expression ?? n.condition);
          const span = (node: Syntax) => [node.getStart(sf), node.end];
          const showRight = jsxGuard && n.operatorToken.kind === K.AmpersandAmpersandToken;
          facts.push({
            ...base,
            kind: "branch",
            label: expr.getText(),
            predicate: predicate(expr),
            textWhenTrue: renderedText(showRight ? n.right : n.whenTrue),
            textWhenFalse: renderedText(jsxGuard && !showRight ? n.right : n.whenFalse),
            condition: span(expr),
            arms: jsxGuard
              ? { kind: "logical", right: span(n.right), and: showRight }
              : n.kind === K.IfStatement
                ? {
                    kind: "if",
                    yes: span(n.thenStatement),
                    no: n.elseStatement ? span(n.elseStatement) : null,
                  }
                : {
                    kind: "conditional",
                    yes: span(n.whenTrue),
                    no: span(n.whenFalse),
                  },
          });
        }
        if (n.kind === K.JsxOpeningElement || n.kind === K.JsxSelfClosingElement) {
          const sym = symbol(n.tagName);
          const decl = sym?.valueDeclaration;
          const declPath = compiler.declarationPath(decl);
          recordDep(declPath);
          if (declPath && !declPath.includes("/node_modules/"))
            facts.push({
              ...base,
              kind: "renders",
              label: sym.name,
              target: relative(root, declPath).split(sep).join("/"),
            });
        }
        if (n.kind === K.CallExpression) {
          const contract = responseContract(n);
          if (contract)
            facts.push({
              ...base,
              kind: "httpResponse",
              label: contract.path,
              ...contract,
            });
          const fn = n.expression;
          // Framework-agnostic standard Web URLSearchParams (.get("key") / .has("key")).
          if (
            fn.kind === K.PropertyAccessExpression &&
            (fn.name?.text === "get" || fn.name?.text === "has") &&
            n.arguments[0]?.kind === K.StringLiteral &&
            n.arguments[0].text.length > 0
          ) {
            const recvType = checker.getTypeAtLocation(fn.expression);
            const recvName = recvType?.getSymbol()?.name;
            if (recvName === "URLSearchParams") {
              const isPresence = fn.name.text === "has";
              // Recognize inline literal comparisons like `params.get("view") === "diff"`
              // without attributing the outer boolean comparison result to the query key.
              const parentBin =
                !isPresence &&
                n.parent?.kind === K.BinaryExpression &&
                (n.parent.operatorToken.kind === K.EqualsEqualsEqualsToken ||
                  n.parent.operatorToken.kind === K.ExclamationEqualsEqualsToken)
                  ? n.parent
                  : undefined;
              const comparedLit = parentBin
                ? parentBin.left === n
                  ? parentBin.right
                  : parentBin.left
                : undefined;
              if (
                comparedLit &&
                (comparedLit.kind === K.StringLiteral || comparedLit.kind === K.NumericLiteral)
              ) {
                const litType = checker.getTypeAtLocation(comparedLit);
                const litShape = shape(litType);
                if (litShape.kind === "literal") {
                  facts.push({
                    ...base,
                    kind: "searchParam",
                    label: n.arguments[0].text,
                    presence: false,
                    optional: true,
                    binding: null,
                    type: litType ? checker.typeToString(litType) : "unknown",
                    shape: litShape,
                  });
                }
              } else {
                let holder: Syntax | undefined = n.parent;
                while (
                  holder &&
                  ([K.AsExpression, K.ParenthesizedExpression, K.NonNullExpression].includes(
                    holder.kind,
                  ) ||
                    (holder.kind === K.BinaryExpression &&
                      [K.QuestionQuestionToken, K.BarBarToken].includes(holder.operatorToken.kind)))
                ) {
                  holder = holder.parent;
                }
                const varDecl = holder?.kind === K.VariableDeclaration ? holder : undefined;
                const targetNode = varDecl?.name ?? n;
                const t = checker.getTypeAtLocation(targetNode);
                facts.push({
                  ...base,
                  kind: "searchParam",
                  label: n.arguments[0].text,
                  presence: isPresence,
                  optional: true,
                  binding: varDecl?.name ? binding(varDecl.name) : null,
                  type: t ? checker.typeToString(t) : "unknown",
                  shape: isPresence ? { kind: "boolean" } : shape(t),
                });
              }
            }
          }
          const sym = symbol(fn.name ?? fn);
          const path = compiler.declarationPath(sym?.declarations?.[0]) ?? "";
          const rule = rules.find(
            (r) =>
              sym?.name === r.symbol &&
              (path.includes("/node_modules/" + r.package + "/") ||
                path.includes("/node_modules/@types/" + r.package + "/")),
          );
          if (
            rule?.kind === "slot" &&
            n.parent.kind === K.VariableDeclaration &&
            n.parent.name.elements?.[0]?.name
          ) {
            const name = n.parent.name.elements[0].name;
            const t = checker.getTypeAtLocation(name);
            const stmt = n.parent.parent.parent;
            facts.push({
              ...base,
              kind: "slot",
              statementStart: stmt.getStart(sf),
              ownerBlock:
                name.kind === K.Identifier &&
                stmt.parent.kind === K.Block &&
                [
                  K.FunctionDeclaration,
                  K.FunctionExpression,
                  K.ArrowFunction,
                  K.MethodDeclaration,
                ].includes(stmt.parent.parent.kind)
                  ? stmt.parent.getStart(sf)
                  : null,
              binding: binding(name),
              setter:
                n.parent.name.elements[1]?.name?.kind === K.Identifier
                  ? n.parent.name.elements[1].name.getText()
                  : null,
              label: name.getText(),
              type: t ? checker.typeToString(t) : "unknown",
              shape: shape(t),
              init:
                n.arguments.length === 1 ? [n.arguments[0].getStart(sf), n.arguments[0].end] : null,
              after:
                stmt.kind === K.VariableStatement && stmt.parent.kind === K.Block ? stmt.end : null,
            });
          } else if (rule?.kind === "query") {
            const t = checker.getTypeAtLocation(n);
            const prop = t && checker.getPropertyOfType(t, "data");
            facts.push({
              ...base,
              kind: rule.kind,
              label: sym.name,
              shape: prop ? shape(checker.getTypeOfSymbolAtLocation(prop, n)) : { kind: "unknown" },
            });
          } else if (rule?.kind === "store" || rule?.kind === "machine" || rule?.kind === "form") {
            const t = checker.getTypeAtLocation(n);
            const props = t ? checker.getPropertiesOfType(t) : [];
            const fields = [];
            if (props.length > 0 && props.length <= 32) {
              for (const prop of props) {
                const propType = checker.getTypeOfSymbolAtLocation(prop, n);
                if (checker.getSignaturesOfType(propType, SignatureKind.Call).length) continue;
                fields.push({
                  name: prop.name,
                  type: propType ? checker.typeToString(propType) : "unknown",
                  shape: shape(propType),
                });
              }
            }
            facts.push({
              ...base,
              kind: rule.kind,
              package: rule.package,
              label:
                n.parent.kind === K.VariableDeclaration && n.parent.name.kind === K.Identifier
                  ? n.parent.name.getText()
                  : sym.name,
              type: t ? checker.typeToString(t) : "unknown",
              shape: shape(t),
              fields,
            });
          } else if (rule?.kind === "search") {
            const t = checker.getTypeAtLocation(n);
            const props = t ? checker.getPropertiesOfType(t) : [];
            if (props.length > 0 && props.length <= 32) {
              for (const prop of props) {
                const propType = checker.getTypeOfSymbolAtLocation(prop, n);
                if (checker.getSignaturesOfType(propType, SignatureKind.Call).length) continue;
                facts.push({
                  ...base,
                  kind: "searchParam",
                  label: prop.name,
                  presence: false,
                  optional: Boolean(prop.flags & SymbolFlags.Optional),
                  binding: null,
                  type: propType ? checker.typeToString(propType) : "unknown",
                  shape: shape(propType),
                });
              }
            }
          } else if (rule)
            facts.push({
              ...base,
              kind: rule.kind,
              label:
                rule.kind === "route" && n.arguments[0]?.kind === K.StringLiteral
                  ? n.arguments[0].text
                  : sym.name,
            });
        } else if (n.kind === K.JsxOpeningElement || n.kind === K.JsxSelfClosingElement) {
          const tagNode = n.tagName;
          const tagSym = tagNode ? symbol(tagNode.name ?? tagNode) : null;
          const tagPath = compiler.declarationPath(tagSym?.declarations?.[0]) ?? "";
          const controlRule = rules.find(
            (r) =>
              r.kind === "control" &&
              tagSym?.name === r.symbol &&
              (tagPath.includes("/node_modules/" + r.package + "/") ||
                tagPath.includes("/node_modules/@types/" + r.package + "/")),
          );
          if (controlRule) {
            const stateProps = [
              "open",
              "defaultOpen",
              "visible",
              "value",
              "defaultValue",
              "checked",
              "defaultChecked",
            ];
            const fields: { name: string; type: string; shape: Shape }[] = [];
            const attrNodes = n.attributes?.properties ?? [];
            const seenProps = new Set<string>();
            for (const attr of attrNodes) {
              if (attr.kind !== K.JsxAttribute || !attr.name) continue;
              const propName = attr.name.getText();
              if (!stateProps.includes(propName) || seenProps.has(propName)) continue;
              seenProps.add(propName);
              const init = attr.initializer;
              const expr = init?.kind === K.JsxExpression ? init.expression : init;
              const t = expr ? checker.getTypeAtLocation(expr) : null;
              fields.push({
                name: propName,
                type: t ? checker.typeToString(t) : "boolean",
                shape: t ? shape(t) : { kind: "boolean" },
              });
            }
            if (fields.length === 0) {
              // Default to boolean open/checked state for uncontrolled Radix primitives when props omitted
              const defaultProp =
                controlRule.package.includes("tabs") ||
                controlRule.package.includes("select") ||
                controlRule.package.includes("accordion")
                  ? "value"
                  : controlRule.package.includes("switch") ||
                      controlRule.package.includes("checkbox")
                    ? "checked"
                    : "open";
              fields.push({
                name: defaultProp,
                type: defaultProp === "value" ? "string" : "boolean",
                shape: defaultProp === "value" ? { kind: "string" } : { kind: "boolean" },
              });
            }
            facts.push({
              ...base,
              kind: "control",
              package: controlRule.package,
              label: tagNode ? tagNode.getText() : controlRule.symbol,
              fields,
            });
          }
        }
        n.forEachChild((child) => {
          walk(child as Syntax, owner);
        });
      }
      walk(sf, "(module)");
    }
    allFacts.push(...facts);
    allDiagnostics.push(...diagnostics);
  }
  const unique = new Map(
    allFacts.map((f) => [
      JSON.stringify([f.file, f.start, f.end, f.kind, f.kind === "searchParam" ? f.label : ""]),
      f,
    ]),
  );
  console.log(
    JSON.stringify({
      sourceFiles: [...sourceFiles].sort(),
      moduleGraph: Object.fromEntries(
        [...moduleGraph.entries()]
          .sort(([a], [b]) => (a < b ? -1 : Number(a > b)))
          .map(([k, v]) => [k, [...v].sort()]),
      ),
      versions,
      compiler: compiler.version,
      diagnostics: allDiagnostics,
      facts: [...unique.values()].sort(
        (a, b) =>
          (a.file < b.file ? -1 : a.file > b.file ? 1 : 0) ||
          a.start - b.start ||
          b.end - a.end ||
          (a.kind < b.kind ? -1 : a.kind > b.kind ? 1 : 0),
      ),
    }),
  );
} finally {
  compiler.close?.();
}
