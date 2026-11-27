// Restricted syntax evaluator. Unknown constructs abort a projection; never evaluate application JS.
import { parseSync } from "oxc-parser";
export function project(source, modules = {}) {
  const ast = parseSync("App.tsx", source).program,
    env = { ...modules },
    scope = (e, k) => {
      if (!Object.hasOwn(e, k)) throw Error("unknown " + k);
      return e[k];
    };
  function val(n, e) {
    if (!n) return undefined;
    switch (n.type) {
      case "ParenthesizedExpression":
        return val(n.expression, e);
      case "Literal":
        return n.value;
      case "Identifier":
        return scope(e, n.name);
      case "ArrayExpression":
        return n.elements.map((x) => val(x, e));
      case "ObjectExpression":
        return Object.fromEntries(
          n.properties.map((p) => [p.key.name ?? p.key.value, val(p.value, e)]),
        );
      case "MemberExpression":
        return val(n.object, e)[n.property.name];
      case "ConditionalExpression":
        return val(val(n.test, e) ? n.consequent : n.alternate, e);
      case "LogicalExpression":
        if (n.operator === "&&") return val(n.left, e) && val(n.right, e);
        break;
      case "BinaryExpression":
        if (n.operator === "+") return val(n.left, e) + val(n.right, e);
        break;
      case "JSXText":
        return n.value;
      case "JSXExpressionContainer":
        return val(n.expression, e);
      case "JSXFragment":
        return n.children.map((c) => val(c, e));
      case "JSXElement": {
        const op = n.openingElement,
          tag = op.name.name,
          attrs = {};
        for (const a of op.attributes) {
          if (a.type === "JSXSpreadAttribute") Object.assign(attrs, val(a.argument, e));
          else attrs[a.name.name] = a.value ? val(a.value, e) : true;
        }
        const children = n.children
          .map((c) => val(c, e))
          .flat(Infinity)
          .filter((x) => x != null && typeof x !== "boolean");
        if (/^[A-Z]/.test(tag)) return call(scope(e, tag), { ...attrs, children }, e);
        return { tag, attrs, children };
      }
      case "CallExpression":
        if (n.callee.name === "clsx")
          return n.arguments
            .map((a) => val(a, e))
            .filter(Boolean)
            .join(" ");
        if (n.callee.name === "useState") return [val(n.arguments[0], e)];
        if (n.callee.property?.name === "map") {
          const cb = n.arguments[0];
          return val(n.callee.object, e).map((item) =>
            val(cb.body, { ...e, [cb.params[0].name]: item }),
          );
        }
        break;
    }
    throw Error("unsupported " + n.type);
  }
  function vars(d, e) {
    for (const v of d.declarations) {
      const value = val(v.init, e);
      if (v.id.type === "ArrayPattern") v.id.elements.forEach((p, i) => (e[p.name] = value[i]));
      else e[v.id.name] = value;
    }
  }
  function call(f, props, parent) {
    const e = { ...parent };
    for (const p of f.params[0]?.properties ?? []) {
      const k = p.key.name;
      e[k] = props[k] ?? (p.value.type === "AssignmentPattern" ? val(p.value.right, e) : undefined);
    }
    for (const n of f.body.body) {
      if (n.type === "VariableDeclaration") vars(n, e);
      if (n.type === "ReturnStatement") return val(n.argument, e);
    }
    throw Error("missing return");
  }
  let app;
  for (const item of ast.body) {
    const n = item.declaration ?? item;
    if (n.type === "VariableDeclaration") vars(n, env);
    if (n.type === "FunctionDeclaration") {
      env[n.id.name] = n;
      if (item.type === "ExportDefaultDeclaration") app = n;
    }
  }
  return call(app, {}, env);
}
