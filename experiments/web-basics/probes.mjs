export function probe(root, probes) {
  const out = {};
  for (const [selector, fields] of Object.entries(probes)) {
    const el = root.querySelector(selector);
    if (!el) {
      out[selector] = null;
      continue;
    }
    const v = (out[selector] = {}),
      r = el.getBoundingClientRect(),
      cs = getComputedStyle(el);
    for (const k of fields) {
      if (["x", "y", "width", "height"].includes(k)) v[k] = r[k];
      else if (k === "lineRects") {
        const walker = el.ownerDocument.createTreeWalker(el, NodeFilter.SHOW_TEXT),
          lines = new Map();
        let n;
        while ((n = walker.nextNode())) {
          const range = el.ownerDocument.createRange();
          range.selectNodeContents(n);
          for (const b of range.getClientRects()) {
            if (!b.width) continue;
            const k = b.y;
            const old = lines.get(k) ?? [b.x, b.right, b.height];
            lines.set(k, [Math.min(old[0], b.x), Math.max(old[1], b.right), b.height]);
          }
        }
        v[k] = [...lines].map(([y, [x, right, h]]) => [x, y, right - x, h]);
      } else if (k === "outsideChild") {
        const x = r.right + 2,
          y = r.top + 2;
        const hit = root.elementFromPoint?.(x, y) ?? el.ownerDocument.elementFromPoint(x, y);
        v[k] = !!hit && el.contains(hit) && hit !== el;
      } else if (k === "role")
        v[k] =
          el.getAttribute("role") ??
          {
            SELECT: "combobox",
            BUTTON: "button",
            INPUT: el.type === "checkbox" ? "checkbox" : "textbox",
          }[el.tagName];
      else if (k === "label")
        v[k] =
          el.getAttribute("aria-label") ??
          el.getAttribute("alt") ??
          (el.tagName === "BUTTON" ? el.textContent : "");
      else if (k === "children")
        v[k] = Array.from(el.querySelectorAll("*"), (n) => n.tagName.toLowerCase()).join("/");
      else if (k === "text") v[k] = el.textContent;
      else if (k === "loaded") v[k] = el.complete && el.naturalWidth > 0;
      else if (k === "ratio") v[k] = el.naturalHeight ? el.naturalWidth / el.naturalHeight : null;
      else if (k === "fontLoaded")
        v[k] = [...el.ownerDocument.fonts].some(
          (f) => f.family === "FixtureTriangle" && f.status === "loaded",
        );
      else if (k === "options") v[k] = [...el.options].map((o) => o.value);
      else if (k === "pixel") v[k] = [r.x + r.width / 2, r.y + r.height / 2];
      else if (["value", "checked", "disabled", "scrollWidth", "scrollHeight"].includes(k))
        v[k] = el[k];
      else v[k] = cs[k];
    }
  }
  return out;
}
export function deviations(ref, candidate, id) {
  const diff = {};
  function delta(a, b) {
    if (Array.isArray(a) && Array.isArray(b) && a.length === b.length)
      return Math.max(0, ...a.map((v, i) => delta(v, b[i])));
    if (typeof a === "number" && typeof b === "number") return Math.abs(a - b);
    return a === b ? 0 : Infinity;
  }
  for (const [s, fields] of Object.entries(ref)) {
    if (!fields) throw Error("Reference selector missing " + s);
    if (!candidate?.[s]) {
      diff[s] = "missing";
      continue;
    }
    for (const [k, a] of Object.entries(fields)) {
      const b = candidate[s][k];
      const geo = [
        "x",
        "y",
        "width",
        "height",
        "scrollWidth",
        "scrollHeight",
        "lineRects",
        "paddingLeft",
        "paddingRight",
      ].includes(k);
      const tol =
        k === "pixel"
          ? 2
          : geo
            ? ["input", "checkbox", "select", "button"].includes(id)
              ? 2
              : 1
            : 0;
      const d = k.startsWith("padding") ? Math.abs(parseFloat(a) - parseFloat(b)) : delta(a, b);
      if (!(d <= tol)) diff[s + "." + k] = Number.isFinite(d) ? d : [a, b];
    }
  }
  return diff;
}
