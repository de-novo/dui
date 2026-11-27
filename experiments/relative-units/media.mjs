// Isolate renderer width conditions from extraction with a schema-validated model.
export async function mediaControl(page, source, width) {
  const model = structuredClone(source);
  const node = model.components[0].root.children[0].children[0];
  const decl = node.style.find((d) => d.property === "width");
  decl.value = { kind: "length", value: 20, unit: "px" };
  const limit = width === 390 ? { value: 24, unit: "em" } : { value: 88, unit: "rem" };
  node.style.push({
    ...decl,
    value: { kind: "length", value: 70, unit: "px" },
    when: { minWidth: limit },
  });
  const cdp = await page.context().newCDPSession(page);
  const rows = [];
  for (const initialFont of [16, 20]) {
    await cdp.send("Page.setFontSizes", { fontSizes: { standard: initialFont } });
    await page.setContent(
      `<!doctype html><style>html{font-size:62.5%}#box{width:20px}@media(min-width:${limit.value}${limit.unit}){#box{width:70px}}</style><div id="box"></div>`,
    );
    const truth = await page.locator("#box").evaluate((el) => el.getBoundingClientRect().width);
    const measurements = [];
    for (const variant of ["baseline", "prototype"]) {
      const input = structuredClone(model);
      if (variant === "prototype")
        input.components[0].root.children[0].children[0].style.at(-1).when.minWidth = {
          unit: "px",
          value: limit.value * initialFont,
        };
      const html = await page.evaluate(
        ({ input, width }) =>
          window.baseline.drawModel(window.baseline.decodeModel(input), input.components[0].id, {
            width,
          }).html,
        { input, width },
      );
      await page.setContent(html);
      measurements.push(
        await page
          .locator("main > section")
          .first()
          .locator("div")
          .evaluate((el) => el.getBoundingClientRect().width),
      );
    }
    rows.push({ initialFont, limit, truth, baseline: measurements[0], prototype: measurements[1] });
  }
  await cdp.detach();
  return rows;
}
