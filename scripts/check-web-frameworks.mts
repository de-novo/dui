import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdtemp, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

interface Page {
  path: string;
  heading: string;
  next: string;
}

interface Report {
  output: string;
  snapshot: { projectRoot: string };
  screenDiscovery: { sources: Record<string, { kind: string; from?: string }> };
  cells: {
    path: string;
    width: number;
    status: string;
    image?: string;
    observation: { renderTree: { tag: string; text: string }[] };
  }[];
}

interface Graph {
  edges: { source: string; target: string; kind: string; observedWidths?: number[] }[];
}

const astroToolchain = process.argv[2];
assert.ok(astroToolchain, "Pass the path to an installed Astro toolchain (see web-frameworks.yml)");
const fixtures = resolve(import.meta.dirname, "../fixtures/web-frameworks");
const dui = resolve(import.meta.dirname, "../target/debug/dui");
const temp = await mkdtemp(join(tmpdir(), "dui-web-frameworks-"));
const widths = [375, 1280];
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

try {
  // Keep SvelteKit's optional TypeScript peer isolated from dui's own compiler.
  await cp(join(fixtures, "package.json"), join(temp, "package.json"));
  await cp(join(fixtures, "package-lock.json"), join(temp, "package-lock.json"));
  execFileSync("npm", ["ci", "--no-audit", "--no-fund"], { cwd: temp, stdio: "inherit" });
  await cp(join(fixtures, "screen.css"), join(temp, "screen.css"));
  for (const name of ["vue", "preact", "sveltekit", "astro"]) {
    const app = join(temp, name);
    await cp(join(fixtures, name), app, { recursive: true });
    await writeFile(
      join(app, "package.json"),
      JSON.stringify({ name, private: true, type: "module" }),
    );
    const modules = resolve(name === "astro" ? astroToolchain : temp, "node_modules");
    await symlink(modules, join(app, "node_modules"), "dir");
    const builder = name === "astro" ? "astro" : "vite";
    execFileSync(join(modules, ".bin", builder), ["build"], {
      cwd: app,
      stdio: "inherit",
      env: { ...process.env, ASTRO_TELEMETRY_DISABLED: "1" },
    });
    const buildDir = name === "sveltekit" ? "build" : "dist";
    const report: Report = JSON.parse(
      execFileSync(dui, ["map", app, "--build-dir", buildDir], {
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
        env: { ...process.env, HOME: temp },
      }),
    );
    const pages: Page[] = JSON.parse(await readFile(join(app, "pages.json"), "utf8"));
    const graph: Graph = JSON.parse(await readFile(join(report.output, "graph.json"), "utf8"));
    assert.equal(report.snapshot.projectRoot, await realpath(app));
    assert.equal(report.screenDiscovery.sources["/"]?.kind, "entry");
    assert.equal(report.cells.length, pages.length * widths.length, name);
    const digests = new Set<string>();
    for (const page of pages) {
      const edge = graph.edges.find(
        (e) =>
          e.kind === "linksTo" &&
          e.source === "route:" + page.path &&
          e.target === "route:" + page.next,
      );
      assert.deepEqual(edge?.observedWidths, widths, `${name}: ${page.path} -> ${page.next}`);
      if (page.next !== "/") {
        const source = report.screenDiscovery.sources[page.next];
        assert.equal(source?.kind, "anchor");
        assert.equal(source?.from, page.path);
      }
      for (const width of widths) {
        const label = `${name} ${page.path} at ${width}px`;
        const cell = report.cells.find((c) => c.path === page.path && c.width === width);
        assert.equal(cell?.status, "verified", label);
        assert.ok(
          cell?.observation.renderTree.some((n) => n.tag === "h1" && n.text === page.heading),
          label,
        );
        assert.ok(cell.image, label);
        const png = await readFile(join(report.output, cell.image));
        assert.ok(png.subarray(0, 8).equals(pngSignature), label);
        assert.equal(png.readUInt32BE(16), width, label);
        assert.ok(png.readUInt32BE(20) > 0, label);
        digests.add(createHash("sha256").update(png).digest("hex"));
      }
    }
    assert.equal(digests.size, report.cells.length, `${name}: expected distinct screen captures`);
    if (process.env.DUI_WEB_EVIDENCE) {
      await cp(report.output, join(process.env.DUI_WEB_EVIDENCE, name), { recursive: true });
    }
    console.log(
      JSON.stringify({
        framework: name,
        pages: pages.length,
        captures: report.cells.length,
        widths,
      }),
    );
  }
} finally {
  await rm(temp, { recursive: true, force: true });
}
