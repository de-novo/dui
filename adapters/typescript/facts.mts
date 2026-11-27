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

const [root, specPath, ...rulePaths] = process.argv.slice(2);
const spec: Manifest = JSON.parse(readFileSync(specPath, "utf8"));
const rules: Rule[] = [
  ...rulePaths,
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
  "primitive",
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
// Hard version gates apply ONLY to the host/runtime layers whose APIs dui invokes directly.
// Newer stable majors are always accepted (`major >= minMajor`).
const CORE_MIN_MAJORS: Record<string, number> = {
  react: 18,
  "react-dom": 18,
  vite: 7,
  ...(spec.host === "next" ? { next: 16 } : {}),
};
for (const [name, minMajor] of Object.entries(CORE_MIN_MAJORS)) {
  const actual = readInstalledPackageVersion(name);
  if (actual === null) continue;
  if (!/^\d+\.\d+\.\d+$/.test(actual) || Number(actual.split(".")[0]) < minMajor) {
    unsupported(`${name} requires stable ${minMajor}.x+; found ${actual}`);
  }
  versions[name] = actual;
}
// Ecosystem / rule-pack libraries are analyzed through the official TypeScript compiler without
// coupling dui to a fixed library major. Record their versions and emit non-fatal notes only.
const TESTED_ECOSYSTEM_BASELINES: Record<string, number> = {
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
  "react-router": 7,
  "react-router-dom": 7,
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
};
const versionNotes: { package: string; version: string; note: string }[] = [];
const rulePackages = new Set([
  ...Object.keys(TESTED_ECOSYSTEM_BASELINES),
  ...rules.map((r) => r.package),
]);
for (const name of [...rulePackages].sort()) {
  if (name in CORE_MIN_MAJORS) continue;
  const actual = readInstalledPackageVersion(name);
  if (actual === null) continue;
  versions[name] = actual;
  const baseline = TESTED_ECOSYSTEM_BASELINES[name];
  if (baseline !== undefined) {
    if (!/^\d+\.\d+\.\d+$/.test(actual)) {
      versionNotes.push({
        package: name,
        version: actual,
        note: `non-stable version; tested baseline is ${baseline}.x+`,
      });
    } else if (Number(actual.split(".")[0]) < baseline) {
      versionNotes.push({
        package: name,
        version: actual,
        note: `below tested baseline ${baseline}.x+; continuing via official compiler types`,
      });
    }
  }
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
      let sym = checker.getSymbolAtLocation(node);
      if (node.parent?.kind === K.ShorthandPropertyAssignment && node.parent.name === node) {
        const shorthand = (
          checker as { getShorthandAssignmentValueSymbol?: (node: Syntax) => typeof sym }
        ).getShorthandAssignmentValueSymbol;
        if (typeof shorthand === "function") sym = shorthand.call(checker, node.parent) ?? sym;
      }
      const decl = compiler.declaration(sym?.valueDeclaration);
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
    function unwrapAll(node: Syntax | undefined): Syntax | undefined {
      while (
        node &&
        [
          K.ParenthesizedExpression,
          K.AsExpression,
          K.SatisfiesExpression,
          K.NonNullExpression,
        ].includes(node.kind)
      )
        node = node.expression;
      return node;
    }
    function objectProperty(obj: Syntax | undefined, name: string) {
      return obj?.properties?.find(
        (prop) =>
          [K.PropertyAssignment, K.ShorthandPropertyAssignment, K.MethodDeclaration].includes(
            prop.kind,
          ) && propertyName(prop.name) === name,
      );
    }
    function propertyName(node: Syntax | undefined) {
      if (!node) return undefined;
      return node.kind === K.StringLiteral || node.kind === K.NumericLiteral
        ? node.text
        : (node.text ?? node.getText());
    }
    function functionValue(prop: Syntax | undefined) {
      if (!prop) return undefined;
      if ([K.ArrowFunction, K.FunctionExpression, K.MethodDeclaration].includes(prop.kind))
        return prop;
      if (prop.kind !== K.PropertyAssignment) return undefined;
      const value = unwrapAll(prop.initializer);
      return value && [K.ArrowFunction, K.FunctionExpression].includes(value.kind)
        ? value
        : undefined;
    }
    function returnedObject(fn: Syntax | undefined) {
      const body = fn?.body;
      if (!body) return undefined;
      const direct = unwrapAll(body);
      if (direct?.kind === K.ObjectLiteralExpression) return direct;
      if (body.kind !== K.Block) return undefined;
      const returns = (body.statements ?? []).filter((stmt) => stmt.kind === K.ReturnStatement);
      if (returns.length !== 1) return undefined;
      const value = unwrapAll(returns[0].expression);
      return value?.kind === K.ObjectLiteralExpression ? value : undefined;
    }
    function signatureReturnType(signature: unknown) {
      if (!signature) return undefined;
      const direct = (signature as { getReturnType?: () => Type }).getReturnType;
      if (typeof direct === "function") return direct.call(signature);
      const throughChecker = (
        checker as { getReturnTypeOfSignature?: (signature: unknown) => Type | undefined }
      ).getReturnTypeOfSignature;
      return typeof throughChecker === "function"
        ? throughChecker.call(checker, signature)
        : undefined;
    }
    function objectShape(t: Type | undefined, at: Syntax, depth = 0): Shape {
      if (!t || depth > 4) return { kind: "unknown" };
      if (t.flags & TypeFlags.Object) {
        const props = checker.getPropertiesOfType(t);
        if (
          checker.isTupleType(t) ||
          checker.getIndexInfosOfType(t).length ||
          !props.length ||
          props.length > 64 ||
          checker.getSignaturesOfType(t, SignatureKind.Call).length ||
          checker.getSignaturesOfType(t, SignatureKind.Construct).length
        )
          return { kind: "unknown" };
        const fields: { name: string; shape: Shape }[] = [];
        for (const prop of props) {
          const propType = checker.getTypeOfSymbolAtLocation(prop, at);
          if (checker.getSignaturesOfType(propType, SignatureKind.Call).length) continue;
          fields.push({
            name: prop.name,
            shape:
              propType.flags & TypeFlags.Object && !checker.isArrayType(propType)
                ? objectShape(propType, at, depth + 1)
                : shape(propType, depth + 1),
          });
        }
        fields.sort((a, b) => (a.name < b.name ? -1 : Number(a.name > b.name)));
        return fields.length ? { kind: "object", fields } : { kind: "unknown" };
      }
      return shape(t, depth);
    }
    function firstOptionKey(node: Syntax | undefined, parameter: string): string | undefined {
      if (!node) return undefined;
      if (
        node.kind === K.PropertyAccessExpression &&
        node.expression?.kind === K.Identifier &&
        node.expression.text === parameter
      )
        return node.name?.text;
      let found: string | undefined;
      node.forEachChild((child) => {
        found ??= firstOptionKey(child as Syntax, parameter);
      });
      return found;
    }
    function stateControlMap(stateFn: Syntax | undefined) {
      const parameter = stateFn?.parameters?.[0]?.name;
      const object = returnedObject(stateFn);
      if (!parameter || parameter.kind !== K.Identifier || !object) return {};
      const controls: Record<string, string> = {};
      for (const prop of object.properties ?? []) {
        if (prop.kind !== K.PropertyAssignment) continue;
        const stateKey = propertyName(prop.name);
        const optionKey = stateKey && firstOptionKey(prop.initializer, parameter.text);
        if (stateKey && optionKey) controls[stateKey] = optionKey;
      }
      return controls;
    }
    function literalAt(node: Syntax | undefined) {
      if (!node) return undefined;
      if (node.kind === K.TrueKeyword) return true;
      if (node.kind === K.FalseKeyword) return false;
      if (node.kind === K.NullKeyword) return null;
      const value = shape(checker.getTypeAtLocation(node));
      return value.kind === "literal" ? value.value : undefined;
    }
    function stateKeyAccess(node: Syntax | undefined, parameter: string) {
      const expr = unwrapAll(node);
      if (!expr || expr.kind !== K.PropertyAccessExpression) return undefined;
      const receiver = expr.expression;
      if (receiver?.kind === K.Identifier && receiver.text === parameter)
        return expr.name?.text as string | undefined;
      return undefined;
    }
    function presenceBody(fn: Syntax) {
      const body = fn.body;
      if (!body) return undefined;
      if (body.kind !== K.Block) return unwrapAll(body);
      const returns = (body.statements ?? []).filter((stmt) => stmt.kind === K.ReturnStatement);
      return returns.length === 1 ? unwrapAll(returns[0].expression) : undefined;
    }
    function analyzePresence(fn: Syntax, stateKeys: Set<string>) {
      const parameter = fn.parameters?.[0]?.name;
      if (!parameter || parameter.kind !== K.Identifier) {
        return {
          status: "unresolved",
          reason: "presence predicate does not expose a state parameter",
        };
      }
      const expr = presenceBody(fn);
      if (!expr) {
        return {
          status: "unresolved",
          reason: "presence predicate body is not a single expression",
        };
      }
      let key = stateKeyAccess(expr, parameter.text);
      let operator = "truthy";
      let value: unknown;
      if (!key && expr.kind === K.PrefixUnaryExpression && expr.operator === K.ExclamationToken) {
        key = stateKeyAccess(expr.operand, parameter.text);
        operator = "falsy";
      }
      if (!key && expr.kind === K.BinaryExpression) {
        const op = expr.operatorToken.kind;
        if ([K.EqualsEqualsEqualsToken, K.ExclamationEqualsEqualsToken].includes(op)) {
          for (const flip of [false, true]) {
            const access = flip ? expr.right : expr.left;
            const literalNode = flip ? expr.left : expr.right;
            key = stateKeyAccess(access, parameter.text);
            if (!key) continue;
            value = literalAt(literalNode);
            if (value === undefined) key = undefined;
            else operator = op === K.EqualsEqualsEqualsToken ? "eq" : "neq";
            if (key) break;
          }
        }
      }
      if (!key) {
        return {
          status: "unresolved",
          reason:
            "presence predicate is not a state key, its negation, or a strict literal equality",
        };
      }
      if (stateKeys.size && !stateKeys.has(key)) {
        return {
          status: "unresolved",
          reason: `presence predicate reads ${key}, which is absent from the inferred state shape`,
        };
      }
      return {
        status: "resolved",
        predicate:
          operator === "eq" || operator === "neq" ? { operator, key, value } : { operator, key },
      };
    }
    function primitiveFact(call: Syntax, sf: SourceFile, filename: string, packageName: string) {
      const start = call.getStart(sf);
      const base = {
        file: filename,
        owner: "(module)",
        start,
        end: call.end,
        line: sf.getLineAndCharacterOfPosition(start).line + 1,
      };
      const limitations: Record<string, unknown>[] = [];
      const definition = unwrapAll(call.arguments[0]);
      const nameProp = definition && objectProperty(definition, "name");
      const nameInitial =
        nameProp && nameProp.kind === K.PropertyAssignment
          ? unwrapAll(nameProp.initializer)
          : undefined;
      const primitiveName =
        nameInitial && nameInitial.kind === K.StringLiteral && nameInitial.text.length > 0
          ? nameInitial.text
          : undefined;
      if (definition?.kind !== K.ObjectLiteralExpression) {
        limitations.push({ reason: "definePrimitive argument is not an object literal" });
      }
      if (!primitiveName) {
        limitations.push({ reason: "primitive name is not a string literal" });
      }
      const stateFn = functionValue(definition && objectProperty(definition, "state"));
      let stateShape: Shape = { kind: "unknown" };
      let controls: Record<string, string> = {};
      if (!stateFn) {
        limitations.push({ reason: "state is not a function literal" });
      } else {
        const fnType = checker.getTypeAtLocation(stateFn);
        const signature = fnType && checker.getSignaturesOfType(fnType, SignatureKind.Call)[0];
        stateShape = objectShape(signatureReturnType(signature), stateFn);
        if (stateShape.kind !== "object") {
          limitations.push({ reason: "state return type is not an object shape" });
        }
        controls = stateControlMap(stateFn);
        if (!Object.keys(controls).length) {
          limitations.push({
            reason: "state return does not map state keys directly from options",
          });
        }
      }
      const stateKeys = new Set(
        stateShape.kind === "object" ? stateShape.fields.map((field) => field.name) : [],
      );
      const slots: Record<string, unknown>[] = [];
      const slotsFn = functionValue(definition && objectProperty(definition, "slots"));
      const slotsObject = returnedObject(slotsFn);
      if (!slotsObject) {
        limitations.push({ reason: "slots does not return one object literal" });
      } else {
        for (const slotProp of slotsObject.properties ?? []) {
          if (slotProp.kind !== K.PropertyAssignment) {
            limitations.push({ reason: "a slots entry is not a direct property assignment" });
            continue;
          }
          const slotName = propertyName(slotProp.name);
          const slotCall = unwrapAll(slotProp.initializer);
          const slotDef = unwrapAll(slotCall?.arguments?.[0]);
          const slotStart = slotProp.getStart(sf);
          if (
            !slotName ||
            slotCall?.kind !== K.CallExpression ||
            slotDef?.kind !== K.ObjectLiteralExpression
          ) {
            limitations.push({
              slot: slotName,
              reason: "slot is not a direct slot({ ... }) call",
              line: sf.getLineAndCharacterOfPosition(slotStart).line + 1,
            });
            continue;
          }
          const presentProp = objectProperty(slotDef, "present");
          const presentFn = functionValue(presentProp);
          const entry: Record<string, unknown> = {
            name: slotName,
            start: slotStart,
            end: slotProp.end,
            line: sf.getLineAndCharacterOfPosition(slotStart).line + 1,
          };
          if (presentProp && !presentFn) {
            entry.present = { status: "unresolved", reason: "present is not a function literal" };
            limitations.push({ slot: slotName, reason: "present is not a function literal" });
          } else if (presentFn) {
            entry.present = analyzePresence(presentFn, stateKeys);
            if ((entry.present as { status?: string }).status !== "resolved") {
              limitations.push({
                slot: slotName,
                reason: (entry.present as { reason?: string }).reason,
                line: entry.line,
              });
            }
          }
          slots.push(entry);
        }
      }
      return {
        ...base,
        kind: "primitive",
        package: packageName,
        label: primitiveName ?? "definePrimitive",
        primitive: primitiveName,
        state: { shape: stateShape, controls },
        slots,
        limitations,
      } as Fact;
    }
    function primitiveReference(call: Syntax) {
      const adapterCallee = symbol(call.expression?.name ?? call.expression);
      const adapterPath = compiler.declarationPath(adapterCallee?.declarations?.[0]) ?? "";
      if (
        adapterCallee?.name !== "usePrimitive" ||
        !adapterPath.includes("/node_modules/@ui-sdk/react/")
      )
        return undefined;
      const expr = call.arguments[0];
      if (!expr || expr.kind !== K.Identifier) return undefined;
      const decl = compiler.declaration(symbol(expr)?.valueDeclaration);
      if (!decl) return undefined;
      const init = decl.initializer;
      if (decl.kind !== K.VariableDeclaration || !init || init.kind !== K.CallExpression)
        return undefined;
      const callee = symbol(init.expression?.name ?? init.expression);
      const path = compiler.declarationPath(callee?.declarations?.[0]) ?? "";
      const primitiveRule = rules.find(
        (rule) =>
          rule.kind === "primitive" &&
          callee?.name === rule.symbol &&
          (path.includes("/node_modules/" + rule.package + "/") ||
            path.includes("/node_modules/@types/" + rule.package + "/")),
      );
      if (!primitiveRule) return undefined;
      const declSf = decl.getSourceFile();
      const file = relative(root, declSf.fileName).split(sep).join("/");
      if (file.startsWith("..")) return undefined;
      return `${file}#${init.getStart(declSf)}:${init.end}:primitive`;
    }
    function primitiveOptions(obj: Syntax | undefined) {
      const object = unwrapAll(obj);
      if (!object || object.kind !== K.ObjectLiteralExpression) return {};
      const options: Record<string, { binding: number }> = {};
      for (const prop of object.properties ?? []) {
        const name = propertyName(prop.name);
        const value =
          prop.kind === K.PropertyAssignment
            ? unwrapAll(prop.initializer)
            : prop.kind === K.ShorthandPropertyAssignment
              ? prop.name
              : undefined;
        const at = value && value.kind === K.Identifier ? binding(value) : null;
        if (name && at !== null && at !== undefined) options[name] = { binding: at };
      }
      return options;
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
          const primitiveTarget = primitiveReference(n);
          if (primitiveTarget) {
            facts.push({
              ...base,
              kind: "primitiveInstance",
              label: n.arguments[0].getText(),
              primitive: primitiveTarget,
              options: primitiveOptions(n.arguments[1]),
            });
          }
          if (rule?.kind === "primitive") {
            facts.push(primitiveFact(n, sf, filename, rule.package));
          } else if (
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
          const tagText = tagNode ? tagNode.getText() : "";
          const tagSym = tagNode ? symbol(tagNode.name ?? tagNode) : null;
          const tagPath = compiler.declarationPath(tagSym?.declarations?.[0]) ?? "";
          const controlRule = rules.find(
            (r) =>
              r.kind === "control" &&
              tagSym?.name === r.symbol &&
              (tagPath.includes("/node_modules/" + r.package + "/") ||
                tagPath.includes("/node_modules/@types/" + r.package + "/")),
          );
          const attrNodes = n.attributes?.properties ?? [];
          // A dialog-like element whose open state dui can set: the probe passes the prop
          // through, so a capture can render the dialog open without clicking a trigger.
          const pushOverlay = (pkg: string, prop: string, insertProp: boolean) => {
            if (!tagNode) return;
            const attr = attrNodes.find(
              (a) => a.kind === K.JsxAttribute && a.name?.getText() === prop,
            );
            const init = attr?.initializer;
            const expr =
              init && init.kind === K.JsxExpression && init.expression
                ? init.expression
                : undefined;
            if (attr && !expr) return;
            if (!attr && !insertProp) return;
            facts.push({
              ...base,
              kind: "overlay",
              package: pkg,
              label: tagText,
              prop,
              ...(expr ? { expr: [expr.getStart(sf), expr.end] } : { insertAt: tagNode.end }),
            });
          };
          let ariaRole: string | undefined;
          for (const attr of attrNodes) {
            if (attr.kind !== K.JsxAttribute || !attr.name || attr.name.getText() !== "role") {
              continue;
            }
            const init = attr.initializer;
            if (init?.kind === K.StringLiteral) {
              ariaRole = init.text;
            } else if (
              init?.kind === K.JsxExpression &&
              init.expression?.kind === K.StringLiteral
            ) {
              ariaRole = init.expression.text;
            }
          }
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
              label: tagText || controlRule.symbol,
              ...(ariaRole ? { ariaRole } : {}),
              fields,
            });
            if (
              /dialog|popover/.test(controlRule.package) &&
              /^(Root|Dialog|AlertDialog|Popover)$/.test(controlRule.symbol)
            )
              pushOverlay(controlRule.package, "open", true);
            else if (controlRule.package === "react-native" && controlRule.symbol === "Modal")
              pushOverlay(controlRule.package, "visible", false);
          } else if (tagText) {
            // Framework-level UI control extraction: works for any React component or native
            // HTML/WAI-ARIA element without coupling to a specific UI component library.
            const reactStateProps = new Set([
              "open",
              "defaultOpen",
              "isOpen",
              "visible",
              "checked",
              "defaultChecked",
              "selected",
              "disabled",
              "loading",
              "expanded",
              "value",
              "defaultValue",
              "status",
              "variant",
              "size",
              "mode",
              "activeTab",
              "state",
            ]);
            const ariaStateAttrs = new Set([
              "aria-expanded",
              "aria-selected",
              "aria-checked",
              "aria-pressed",
              "aria-disabled",
              "aria-hidden",
              "aria-invalid",
              "aria-current",
              "aria-busy",
              "data-state",
            ]);
            const htmlStateAttrs: Record<string, Set<string>> = {
              dialog: new Set(["open"]),
              details: new Set(["open"]),
              input: new Set(["checked", "defaultChecked", "disabled", "value", "defaultValue"]),
              select: new Set(["disabled", "value", "defaultValue"]),
              textarea: new Set(["disabled", "value", "defaultValue"]),
              button: new Set(["disabled"]),
              fieldset: new Set(["disabled"]),
            };
            const isReactComponent = /^[A-Z]/.test(tagText);
            const lowerTag = tagText.toLowerCase();
            const allowedHtmlProps = htmlStateAttrs[lowerTag];
            const isStaticLiteral = (node: Syntax): boolean => {
              let cur = node;
              while (cur.kind === K.ParenthesizedExpression && cur.expression) {
                cur = cur.expression;
              }
              return (
                cur.kind === K.StringLiteral ||
                cur.kind === K.NoSubstitutionTemplateLiteral ||
                cur.kind === K.NumericLiteral ||
                cur.kind === K.TrueKeyword ||
                cur.kind === K.FalseKeyword ||
                cur.kind === K.NullKeyword
              );
            };
            const fields: { name: string; type: string; shape: Shape }[] = [];
            const seenProps = new Set<string>();
            for (const attr of attrNodes) {
              if (attr.kind !== K.JsxAttribute || !attr.name) continue;
              const propName = attr.name.getText();
              if (seenProps.has(propName)) continue;
              const matchesProp = isReactComponent
                ? reactStateProps.has(propName) || ariaStateAttrs.has(propName)
                : ariaStateAttrs.has(propName) || Boolean(allowedHtmlProps?.has(propName));
              if (!matchesProp) continue;
              const init = attr.initializer;
              if (!init || init.kind !== K.JsxExpression || !init.expression) continue;
              if (isStaticLiteral(init.expression)) continue;
              seenProps.add(propName);
              const t = checker.getTypeAtLocation(init.expression);
              fields.push({
                name: propName,
                type: t ? checker.typeToString(t) : "unknown",
                shape: t ? shape(t) : { kind: "unknown" },
              });
            }
            if (fields.length > 0) {
              facts.push({
                ...base,
                kind: "control",
                package: isReactComponent ? "react" : "html-aria",
                label: tagText,
                ...(ariaRole ? { ariaRole } : {}),
                fields,
              });
              const openProp = fields.find((f) =>
                isReactComponent ? f.name === "open" || f.name === "isOpen" : f.name === "open",
              );
              if (openProp && (isReactComponent || lowerTag === "dialog"))
                pushOverlay(isReactComponent ? "react" : "html-aria", openProp.name, false);
            }
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
  function factKey(fact: Fact) {
    return `${fact.file}#${fact.start}:${fact.end}:${fact.kind}`;
  }
  function addLimitation(primitive: Fact, reason: string, option?: string) {
    const list = (primitive.limitations ??= []) as Record<string, unknown>[];
    const entry = option ? { option, reason } : { reason };
    if (!list.some((item) => item.option === option && item.reason === reason)) list.push(entry);
  }
  for (const instance of allFacts.filter((fact) => fact.kind === "primitiveInstance")) {
    const primitive = allFacts.find(
      (fact) => fact.kind === "primitive" && factKey(fact) === instance.primitive,
    );
    if (!primitive) continue;
    const state = primitive.state as { controls?: Record<string, string>; shape?: Shape };
    const controls = state.controls ?? {};
    const stateForOption = Object.fromEntries(
      Object.entries(controls).map(([stateKey, option]) => [option, stateKey]),
    );
    const slots = (primitive.slots ?? []) as Record<string, unknown>[];
    for (const [option, target] of Object.entries(instance.options ?? {})) {
      const stateKey = stateForOption[option];
      if (!stateKey) continue;
      const binding = (target as { binding?: number }).binding;
      const candidates = allFacts.filter(
        (fact) => fact.kind === "slot" && fact.file === instance.file && fact.binding === binding,
      );
      if (candidates.length !== 1) {
        addLimitation(
          primitive,
          candidates.length
            ? "controlled option binding is ambiguous"
            : "controlled option is not a compiler-resolved state slot",
          option,
        );
        continue;
      }
      const presence = slots
        .filter((slot) => {
          const present = slot.present as
            | { status?: string; predicate?: { key?: string } }
            | undefined;
          return present?.status === "resolved" && present.predicate?.key === stateKey;
        })
        .map((slot) => ({
          slot: slot.name,
          line: slot.line,
          predicate: (slot.present as { predicate: unknown }).predicate,
        }));
      if (!presence.length) continue;
      const slotFact = candidates[0] as Fact & { primitive?: unknown };
      if (slotFact.primitive) {
        addLimitation(
          primitive,
          "controlled state slot is shared by more than one primitive",
          option,
        );
        continue;
      }
      slotFact.primitive = {
        fact: factKey(primitive),
        name: primitive.primitive,
        state: stateKey,
        stateShape:
          state.shape?.kind === "object"
            ? state.shape.fields.find((field) => field.name === stateKey)?.shape
            : undefined,
        presence,
      };
    }
  }
  for (const primitive of allFacts.filter((fact) => fact.kind === "primitive")) {
    const slots = (primitive.slots ?? []) as Record<string, unknown>[];
    const keys = new Set(
      slots.flatMap((slot) => {
        const present = slot.present as
          | { status?: string; predicate?: { key?: string } }
          | undefined;
        return present?.status === "resolved" && present.predicate?.key
          ? [present.predicate.key]
          : [];
      }),
    );
    for (const stateKey of [...keys].sort()) {
      const linked = allFacts.some(
        (fact) =>
          fact.kind === "slot" &&
          (fact.primitive as { fact?: string; state?: string } | undefined)?.fact ===
            factKey(primitive) &&
          (fact.primitive as { state?: string } | undefined)?.state === stateKey,
      );
      if (!linked) {
        addLimitation(
          primitive,
          `no compiler-resolved usePrimitive instance links controlled state key ${stateKey} to a React state slot`,
        );
      }
    }
  }
  const factsWithoutInstances = allFacts.filter((fact) => fact.kind !== "primitiveInstance");
  allFacts.length = 0;
  allFacts.push(...factsWithoutInstances);
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
      ...(versionNotes.length ? { versionNotes } : {}),
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
