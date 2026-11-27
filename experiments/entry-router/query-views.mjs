// Syntax-only research subset: literal comparisons which choose rendered branches.
import { visitorKeys } from "oxc-parser";
function walk(node, fn) {
  if (!node?.type) return;
  fn(node);
  for (const key of visitorKeys[node.type] ?? []) {
    const children = node[key];
    if (Array.isArray(children)) for (const child of children) walk(child, fn);
    else walk(children, fn);
  }
}
export function queryViews(program) {
  const found = new Map();
  function scan(scope) {
    const states = new Set(),
      params = new Set(),
      conditions = new Map();
    const search = (n) =>
      n?.type === "MemberExpression" &&
      n.object.name === "location" &&
      n.property.name === "search";
    const constructor = (n) =>
      n?.type === "NewExpression" &&
      n.callee.name === "URLSearchParams" &&
      (search(n.arguments[0]) || states.has(n.arguments[0]?.name));
    const condition = (n) => {
      if (
        n?.type !== "BinaryExpression" ||
        n.operator !== "===" ||
        typeof n.right.value !== "string"
      )
        return;
      const call = n.left,
        getter = call.callee;
      if (
        call.type !== "CallExpression" ||
        getter?.property?.name !== "get" ||
        typeof call.arguments[0]?.value !== "string"
      )
        return;
      if (!constructor(getter.object) && !params.has(getter.object.name)) return;
      return "?" + new URLSearchParams([[call.arguments[0].value, n.right.value]]);
    };
    const hasRender = (n) => {
      let result = false;
      walk(n, (child) => {
        if (child.type === "CallExpression" && child.callee.property?.name === "render")
          result = true;
      });
      return result;
    };
    function visit(n) {
      if (!n?.type) return;
      if (
        n !== scope &&
        ["FunctionDeclaration", "FunctionExpression", "ArrowFunctionExpression"].includes(n.type)
      ) {
        scan(n);
        return;
      }
      if (n.type === "VariableDeclarator") {
        if (
          n.id.type === "ArrayPattern" &&
          n.init?.callee?.name === "useState" &&
          search(n.init.arguments[0])
        )
          states.add(n.id.elements[0]?.name);
        if (constructor(n.init)) params.add(n.id.name);
        const key = condition(n.init);
        if (key) conditions.set(n.id.name, key);
      }
      if (n.type === "IfStatement" && hasRender(n.consequent)) {
        const key = condition(n.test);
        if (key) {
          found.set(key, n.start);
          if (n.alternate && hasRender(n.alternate)) found.set("/", n.alternate.start);
        }
      }
      if (
        n.type === "ConditionalExpression" &&
        ["JSXElement", "JSXFragment"].includes((n.consequent.expression ?? n.consequent).type)
      ) {
        walk(n.test, (term) => {
          const key = condition(term) ?? (term.type === "Identifier" && conditions.get(term.name));
          if (key) found.set(key, n.start);
        });
      }
      for (const k of visitorKeys[n.type] ?? []) {
        const child = n[k];
        if (Array.isArray(child)) child.forEach(visit);
        else visit(child);
      }
    }
    visit(scope);
  }
  scan(program);
  return Object.fromEntries(found);
}
