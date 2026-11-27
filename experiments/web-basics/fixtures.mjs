// One row per scenario. The group ID, expected facts and mandatory probes are fixed.
const rows = [];
function add(id, jsx, probes, opts = {}) {
  rows.push({ id, scenario: opts.scenario ?? "default", jsx, probes, ...opts });
}
const text = { section: ["text", "children"] },
  style = { section: ["color", "paddingLeft", "paddingRight"] },
  rect = ["x", "y", "width", "height"],
  pair = { "section > :first-child": rect, "section > :last-child": rect };
const red = "color:#921c36;padding:12px",
  green = "color:#16745c;padding:6px";
for (const flag of [true, false]) {
  const scenario = String(flag),
    prefix = "const flag=" + flag + ";";
  add("and", "<section>A{flag && <strong>B</strong>}</section>", text, {
    scenario,
    prefix,
    expected: flag ? "AB" : "A",
  });
  add("ternary", "<section>{flag?<strong>Yes</strong>:<em>No</em>}</section>", text, {
    scenario,
    prefix,
    expected: flag ? "Yes" : "No",
  });
  for (const id of ["class-composition", "clsx"])
    add(
      id,
      "<section className={" +
        (id === "clsx" ? 'clsx("box",flag && "on")' : '"box "+(flag?"on":"")') +
        "}>Box</section>",
      style,
      {
        scenario,
        prefix: (id === "clsx" ? 'import clsx from "clsx";' : "") + prefix,
        css: ".box{" + green + "}.box.on{" + red + "}",
      },
    );
  add("initial-state", "<section>{active?<strong>Open</strong>:<em>Closed</em>}</section>", text, {
    scenario,
    prefix: 'import {useState} from "react";',
    setup: "const [active]=useState(" + flag + ");",
    expected: flag ? "Open" : "Closed",
  });
  add(
    "checkbox",
    '<section><input aria-label="Done" type="checkbox" defaultChecked={' + flag + "}/></section>",
    { input: ["role", "label", "checked", ...rect] },
    { scenario },
  );
  add(
    "button",
    "<section><button disabled={" + flag + "}>Save</button></section>",
    { button: ["role", "label", "disabled", ...rect] },
    { scenario },
  );
}
for (const a of [[], ["A", "B", "C"], ["C", "A", "B"]])
  add("map-key", "<section>{items.map(item=><span key={item}>{item}</span>)}</section>", text, {
    scenario: a.join("") || "empty",
    prefix: "const items=" + JSON.stringify(a) + ";",
    expected: a.join(""),
    key: true,
  });
add(
  "fragment",
  "<section><><strong>A</strong><em>B</em></></section>",
  { ...text, ...pair },
  { expected: "AB" },
);
add("props", '<section><Label label="Ready"/></section>', text, {
  prefix: "function Label({label}:{label:string}){return <strong>{label}</strong>}",
  expected: "Ready",
});
add("children", "<section><Frame><strong>A</strong><em>B</em></Frame></section>", text, {
  prefix: "function Frame({children}:{children:any}){return <article>{children}</article>}",
  expected: "AB",
});
for (const use of [false, true])
  add("default-prop", "<section><Label " + (use ? 'label="Explicit"' : "") + "/></section>", text, {
    scenario: use ? "explicit" : "omitted",
    prefix: 'function Label({label="Default"}:{label?:string}){return <strong>{label}</strong>}',
    expected: use ? "Explicit" : "Default",
  });
for (const before of [true, false])
  add(
    "spread",
    "<section><Label " +
      (before ? '{...p} label="Override"' : 'label="Override" {...p}') +
      "/></section>",
    text,
    {
      scenario: before ? "before" : "after",
      prefix:
        'const p={label:"Spread"};function Label({label}:{label:string}){return <strong>{label}</strong>}',
      expected: before ? "Override" : "Spread",
    },
  );
add("expression-text", "<section>Count: {n} items</section>", text, {
  prefix: "const n=7;",
  expected: "Count: 7 items",
});
add("inline-style", '<section style={{padding:12,color:"#921c36"}}>Box</section>', style);
add("tailwind", '<section className="p-[12px] text-[#921c36]">Box</section>', style, {
  tailwind: ["p-[12px]", "text-[#921c36]"],
});
add("stylesheet", "<section>Box</section>", style, { css: "section{" + red + "}" });
add(
  "css-module",
  "<section><strong className={a.tile}>A</strong><em className={b.tile}>B</em></section>",
  { strong: style.section, em: style.section },
  { modules: { a: ".tile{" + red + "}", b: ".tile{" + green + "}" } },
);
add(
  "custom-property",
  "<section><strong>A</strong><em>B</em></section>",
  { strong: style.section, em: style.section },
  {
    css: "section{--c:#921c36;--p:12px}em{--c:#16745c;--p:6px}strong,em{color:var(--c);padding:var(--p)}",
  },
);
add("media-query", "<section>Box</section>", style, {
  css: "section{" + green + "}@media(min-width:700px){section{" + red + "}}",
});
add(
  "block",
  "<section><div style={{width:80,height:20}}/><div style={{width:45,height:10}}/></section>",
  pair,
);
add(
  "inline",
  "<section style={{width:70}}><span>one two </span><span>three four</span></section>",
  { ...pair, section: ["lineRects"] },
);
add(
  "flex",
  '<section style={{display:"flex",gap:12,alignItems:"center"}}><div style={{width:20,height:28}}/><div style={{width:30,height:10}}/></section>',
  pair,
);
add(
  "grid",
  '<section style={{display:"grid",gridTemplateColumns:"40px 60px",gap:8}}><div>A</div><div>B</div></section>',
  { ...pair, section: ["gridTemplateColumns"] },
);
add(
  "position",
  '<section style={{position:"relative",width:100,height:50}}><div style={{position:"absolute",left:14,top:8,width:20,height:10}}/></section>',
  { "section > div": rect },
);
for (const mode of ["hidden", "auto"])
  add(
    "overflow",
    '<section style={{width:50,height:20,overflow:"' +
      mode +
      '"}}><div style={{width:90,height:40}}/></section>',
    { section: [...rect, "scrollWidth", "scrollHeight", "outsideChild", "overflowX"] },
    { scenario: mode },
  );
for (const [id, css] of Object.entries({
  percent: "section{width:160px}strong{width:50%}",
  rem: "html{font-size:62.5%}strong{width:8rem}",
  em: "section{font-size:20px}strong{font-size:1.5em;width:2em}",
  vh: "strong{width:10vh}",
  vw: "strong{width:10vw}",
}))
  add(
    id,
    "<section><strong/></section>",
    { strong: rect },
    { css: "strong{display:block;height:12px}" + css },
  );
add(
  "text-wrap",
  '<section style={{width:100,fontSize:16,lineHeight:"24px"}}>One small box with five words.</section>',
  { section: ["text", "lineRects", ...rect] },
);
add(
  "font",
  "<section>AAA AAA</section>",
  {
    section: [
      "text",
      "lineRects",
      "fontLoaded",
      "fontFamily",
      "fontWeight",
      "fontSize",
      "lineHeight",
    ],
  },
  {
    css: "@font-face{font-family:FixtureTriangle;src:url(./triangle.ttf)}section{font:400 20px/28px FixtureTriangle}",
  },
);
add(
  "image",
  '<section><img src="./mark.png" alt="Green square" width={32} height={24}/></section>',
  { img: ["label", "loaded", "ratio", ...rect, "pixel"] },
);
add(
  "icon",
  '<section><svg role="img" aria-label="Green square" width="32" height="24" viewBox="0 0 32 24" style={{color:"#16745c"}}><rect width="32" height="24" fill="currentColor"/></svg></section>',
  { svg: ["label", "children", ...rect, "pixel"] },
);
add("input", '<section><input aria-label="Name" defaultValue="Ada"/></section>', {
  input: ["role", "label", "value", ...rect],
});
add(
  "select",
  '<section><select aria-label="Choice" defaultValue="b"><option value="a">A</option><option value="b">B</option></select></section>',
  { select: ["role", "label", "value", "options", ...rect] },
);
export const fixtures = rows;
// Direction supplement, outside the frozen #441 denominator.
add(
  "inherit-style",
  '<section style={{color:"#16745c",fontSize:18}}><span>Child</span></section>',
  { span: ["color", "fontSize", "text"] },
);
add("cascade", '<section className="box" id="target">Box</section>', style, {
  css: "#target{" + red + "}.box{" + green + "}",
});
add(
  "color",
  '<section style={{width:40,height:20,backgroundColor:"#16745c",color:"#ffffff"}}>A</section>',
  { section: ["color", "backgroundColor"] },
);
add("border", '<section style={{width:80,height:40,border:"3px solid #921c36"}}/>', {
  section: ["borderLeftWidth", "borderLeftStyle", "borderLeftColor", ...rect],
});
add("radius", '<section style={{width:80,height:40,backgroundColor:"#16745c",borderRadius:12}}/>', {
  section: ["borderTopLeftRadius", "borderBottomRightRadius", ...rect],
});
add("translate", '<section style={{width:80,height:24,transform:"translate(12px,6px)"}}/>', {
  section: rect,
});
add(
  "transform-order",
  '<section style={{width:80,height:24,transform:"translateX(18px) rotate(20deg) scale(1.2)"}}/>',
  { section: [...rect, "transform"] },
);
