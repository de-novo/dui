import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { chromium } from "playwright-core";
import { candidates, raw, ordered, withVariables } from "./extract.mjs";
const [extractor, chrome, out] = process.argv.slice(2);
const cases = {
  offset: "translate(12px,6px)",
  relative: "translate(50%,-25%)",
  compound: "translateX(12px) rotate(30deg) scale(1.2)",
  reordered: "scale(1.2) rotate(30deg) translateX(12px)",
  dependent: "translateX(var(--shift))",
};
const results = {};
const b = await chromium.launch({ executablePath: chrome });
try {
  for (const [name, value] of Object.entries(cases)) {
    const src =
      "#box{width:80px;height:24px;" +
      (name === "dependent" ? "--shift:18px;" : "") +
      "transform:" +
      value +
      "}";
    fs.writeFileSync(path.join(out, "App.tsx"), 'export function App(){return <div id="box"/>}');
    fs.writeFileSync(path.join(out, "built.css"), src);
    const model = JSON.parse(execFileSync(extractor, [out]));
    const c = candidates(model, src);
    const sidecars = { raw: raw(c), ordered: ordered(c), scoped: withVariables(c) };
    const base = Object.fromEntries(
      model.components[0].root.style
        .filter((d) => d.value.kind === "length" && d.value.unit === "px")
        .map((d) => [d.property, d.value.value]),
    );
    const row = (results[name] = { sidecars, base, views: {} });
    for (const width of [1440, 390]) {
      const p = await b.newPage({ viewport: { width, height: width === 390 ? 844 : 920 } });
      for (const mode of ["Chrome", "v1", "raw", "ordered", "scoped"]) {
        await p.setContent(
          "<style>body{margin:0;padding:32px}#box{background:#135c52}" +
            (mode === "Chrome"
              ? src
              : "#box{width:" + base.width + "px;height:" + base.height + "px}") +
            '</style><div id="box"></div>',
        );
        const measured = await p.locator("#box").evaluate(
          (el, { mode, sidecars }) => {
            let unresolved = false;
            if (mode === "scoped") {
              const t = sidecars.scoped;
              for (const [k, v] of Object.entries(t.variables)) el.style.setProperty(k, v);
              el.style.transform = t.text;
              unresolved = !!t.missing.length;
            }
            if (mode === "raw") {
              el.style.transform = sidecars.raw.text;
              unresolved = sidecars.raw.text.includes("var(");
            }
            if (mode === "ordered") {
              const m = new DOMMatrix();
              if (!sidecars.ordered.operations) unresolved = true;
              else {
                for (const { op, values } of sidecars.ordered.operations) {
                  const [a, h] = values.map((v, i) =>
                    v.unit === "%"
                      ? (v.n / 100) * (i || op === "translateY" ? el.offsetHeight : el.offsetWidth)
                      : v.n,
                  );
                  if (op === "translate") m.translateSelf(a, h ?? 0);
                  else if (op === "translateX") m.translateSelf(a, 0);
                  else if (op === "translateY") m.translateSelf(0, a);
                  else if (op === "rotate") m.rotateSelf(a);
                  else m.scaleSelf(a, h ?? a);
                }
                el.style.transform = m.toString();
              }
            }
            const { x, y, width, height } = el.getBoundingClientRect();
            return { rect: [x, y, width, height], unresolved };
          },
          { mode, sidecars },
        );
        (row.views[width] ??= {})[mode] = measured;
        await p.screenshot({
          path: path.join(out, name + "-" + width + "-" + mode + ".png"),
          clip: { x: 0, y: 0, width: 175, height: 110 },
        });
      }
      await p.close();
    }
  }
} finally {
  await b.close();
}
fs.writeFileSync(path.join(out, "preservation.json"), JSON.stringify(results, null, 2));
