// Chrome exports typed computed values separately from used pixel dimensions.
export function snapshot(root) {
  function walk(node, id) {
    if (node.nodeType === Node.TEXT_NODE) return { text: node.data, id };
    const resolved = getComputedStyle(node),
      used = {},
      typed = {};
    for (const k of resolved) used[k] = resolved.getPropertyValue(k);
    for (const [k] of node.computedStyleMap()) {
      const value = node.computedStyleMap().get(k);
      if (value) typed[k] = value.toString();
    }
    const pseudo = {};
    for (const p of ["::before", "::after"]) {
      const style = getComputedStyle(node, p);
      if (!["none", "normal"].includes(style.content))
        pseudo[p] = Object.fromEntries(Array.from(style, (k) => [k, style.getPropertyValue(k)]));
    }
    return {
      id,
      tag: node.localName,
      attrs: Object.fromEntries(Array.from(node.attributes, (a) => [a.name, a.value])),
      used,
      typed,
      pseudo,
      children: Array.from(node.childNodes)
        .filter((n) => [1, 3].includes(n.nodeType))
        .map((n, i) => walk(n, id + "." + i)),
    };
  }
  return walk(root, "r");
}
export function observe(root) {
  const boxes = {},
    runs = {};
  function walk(n, id) {
    if (n.nodeType === Node.TEXT_NODE) {
      const range = document.createRange(),
        p = n.parentElement,
        css = getComputedStyle(p),
        chars = [];
      for (let i = 0; i < n.length; i++) {
        range.setStart(n, i);
        range.setEnd(n, i + 1);
        const r = range.getBoundingClientRect();
        chars.push([r.x, r.y, r.width, r.height]);
      }
      if (n.data.trim())
        runs[id] = {
          text: n.data,
          chars,
          font: css.fontFamily,
          size: parseFloat(css.fontSize),
          weight: css.fontWeight,
          spacing: parseFloat(css.letterSpacing) || 0,
          color: css.color,
        };
      return;
    }
    const b = n.getBoundingClientRect();
    boxes[id] = [b.x, b.y, b.width, b.height];
    Array.from(n.childNodes)
      .filter((n) => [1, 3].includes(n.nodeType))
      .forEach((n, i) => walk(n, id + "." + i));
  }
  walk(root, "r");
  return { boxes, runs };
}
export function render({ tree, model, mode, width, height }) {
  const old = document.querySelector("[data-route-host]") ?? document.querySelector(".h-svh");
  const host = document.createElement("div");
  host.dataset.routeHost = "";
  old.replaceWith(host);
  const shadow = host.attachShadow({ mode: "open" });
  const out = window.drawer.drawModel(window.drawer.decodeModel(model), model.components[0].id, {
    width,
    height,
  });
  shadow.innerHTML = out.markup;
  const root = shadow.querySelector(".dui-document-body").firstElementChild;
  if (mode === "identity") {
    function restore(n, e) {
      if (!n.tag) return;
      for (const a of Array.from(e.attributes)) e.removeAttribute(a.name);
      for (const [k, v] of Object.entries(n.attrs)) e.setAttribute(k, v);
      n.children.forEach((c, i) => restore(c, e.childNodes[i]));
    }
    restore(tree, root);
    host.replaceWith(root);
    return root;
  }
  if (mode === "shadow-only") {
    function apply(n, e) {
      if (!n.tag) return;
      e.style.boxShadow = n.used["box-shadow"];
      n.children.forEach((c, i) => apply(c, e.childNodes[i]));
    }
    apply(tree, root);
    return root;
  }
  if (mode !== "stock") {
    const rules = [];
    function style(n, e) {
      if ("text" in n) {
        if (e.data !== n.text) throw Error("Text mismatch " + n.id);
        return;
      }
      if (e.localName !== n.tag || e.childNodes.length !== n.children.length)
        throw Error("DOM mismatch " + n.id);
      for (const [k, v] of Object.entries(n.attrs)) e.setAttribute(k, v);
      e.dataset.probe = n.id;
      const list = mode === "used" ? n.used : n.typed;
      // Typed OM has list-valued properties. Keep resolved serialization for
      // multi-value paint/text properties; preserve typed sizing and offsets.
      const flow =
        /^(width|height|min-|max-|block-size|inline-size|margin|inset|top$|bottom$|left$|right$|padding|flex-basis|grid-template|line-height)/;
      const source =
        mode === "used"
          ? list
          : {
              ...n.used,
              ...Object.fromEntries(Object.entries(list).filter(([k]) => flow.test(k))),
            };
      for (const [k, v] of Object.entries(source)) e.style.setProperty(k, v);
      for (const [p, values] of Object.entries(n.pseudo))
        rules.push(
          '[data-probe="' +
            n.id +
            '"]' +
            p +
            "{" +
            Object.entries(values)
              .map(([k, v]) => k + ":" + v)
              .join(";") +
            "}",
        );
      n.children.forEach((c, i) => style(c, e.childNodes[i]));
    }
    style(tree, root);
    if (rules.length)
      shadow.append(
        Object.assign(document.createElement("style"), { textContent: rules.join("\n") }),
      );
    if (mode === "yoga") window.arrange(host, width);
  }
  return root;
}
