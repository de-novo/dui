// Execute the README examples against the default install and the PR binary in separate archives.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { withChromiumPage } from "./chromium-page.mts";

function recipe(file: string) {
  const markdown = readFileSync(file, "utf8");
  function block(name: string, language: string) {
    const marker = "<!-- quickstart:" + name + " -->\n```" + language + "\n";
    const parts = markdown.split(marker);
    assert.equal(parts.length, 2, file + ": missing or duplicate " + name);
    const end = parts[1]!.indexOf("\n```");
    assert.ok(end > 0, file + ": unterminated " + name);
    return parts[1]!.slice(0, end);
  }
  function commands(name: string) {
    return block(name, "sh")
      .split("\n")
      .map((line) => {
        const [program, ...args] = line.trim().split(/\s+/);
        assert.equal(program, "dui", file + ": expected a dui command");
        return args;
      });
  }
  return {
    manifest: JSON.parse(block("manifest", "json")),
    first: commands("first-run"),
    built: commands("static"),
  };
}

const guide = recipe("README.md");
for (const file of ["README.ko.md", "README.zh-CN.md"]) {
  assert.deepEqual(recipe(file), guide, file + ": translated steps differ");
}
assert.equal(guide.first.length, 3, "expected version, capture and viewer steps");
assert.equal(guide.built.length, 1, "expected a static capture step");

interface Cell {
  image?: string;
  status: string;
  width: number;
  observation?: { renderTree?: unknown[] };
}

async function verifyViewer(url: string, archive: string, images: Set<string>) {
  const runs = (await (await fetch(new URL("/archive/runs.json", url))).json()) as {
    key: string;
  }[];
  assert.equal(runs.length, 1, "expected just this capture in the isolated home");
  const query = "?run=" + encodeURIComponent(runs[0]!.key);
  const graph = (await (await fetch(new URL("/archive/graph.json" + query, url))).json()) as {
    cells: Cell[];
  };
  assert.deepEqual(new Set(graph.cells.flatMap((c) => (c.image ? [c.image] : []))), images);
  await withChromiumPage(async ({ call }) => {
    await call("Page.navigate", { url });
    const deadline = Date.now() + 20000;
    while (Date.now() < deadline) {
      const { result } = await call("Runtime.evaluate", {
        expression: `Array.from(document.images).filter(img => {
          const r = img.getBoundingClientRect();
          return img.complete && img.naturalWidth && r.width && r.height &&
            r.right > 0 && r.bottom > 0 && r.left < innerWidth && r.top < innerHeight;
        }).map(img => img.currentSrc)`,
        returnByValue: true,
      });
      for (const src of (result as { value?: string[] })?.value ?? []) {
        const parsed = new URL(src);
        const name = decodeURIComponent(basename(parsed.pathname));
        if (parsed.origin !== new URL(url).origin || !images.has(name)) continue;
        const response = await fetch(src);
        assert.equal(response.status, 200);
        const bytes = Buffer.from(await response.arrayBuffer());
        assert.ok(
          bytes.equals(readFileSync(join(archive, name))),
          "displayed image differs from capture",
        );
        assert.ok(bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])));
        return;
      }
      await delay(100);
    }
    assert.fail("a captured PNG did not become visible in the viewer");
  });
}

const [installed, development] = process.argv.slice(2);
assert.ok(installed && development, "Pass the default-installed and development dui executables");
const stableRef = execFileSync("sh", ["scripts/install", "--print-ref"], {
  encoding: "utf8",
}).trim();
const root = mkdtempSync(join(tmpdir(), "dui-guide-"));
const app = join(root, "app");
try {
  cpSync("fixtures/toggle", app, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(resolve("fixtures/toggle/node_modules"), join(app, "node_modules"), "dir");
  writeFileSync(join(app, "dui.json"), JSON.stringify(guide.manifest));
  execFileSync(join(app, "node_modules/.bin/vite"), ["build"], { cwd: app, stdio: "inherit" });
  const deployment = join(root, "static-site");
  mkdirSync(deployment);
  cpSync(join(app, "dist"), join(deployment, "dist"), { recursive: true });
  cpSync(join(app, "dui.json"), join(deployment, "dui.json"));
  for (const [name, executable, cwd, capture] of [
    ["installed", installed, app, guide.first[1]!],
    ["development", development, app, guide.first[1]!],
    ["development-static", development, deployment, guide.built[0]!],
  ] as const) {
    const binary = resolve(executable);
    const home = join(root, name);
    mkdirSync(home);
    let options = { cwd, env: { ...process.env, HOME: home } };
    function run(args: string[]) {
      return JSON.parse(
        execFileSync(binary, args, {
          ...options,
          encoding: "utf8",
          timeout: 120000,
          maxBuffer: 32 * 1024 * 1024,
        }),
      );
    }
    const version = run(guide.first[0]!);
    assert.equal(version.name, "dui");
    assert.equal(typeof version.version, "string");
    if (name === "installed") assert.equal(version.tag, stableRef === "main" ? null : stableRef);
    else if (version.tag === null) {
      const head = execFileSync("git", ["rev-parse", "--short=12", "HEAD"], {
        encoding: "utf8",
      }).trim();
      assert.ok(
        version.version.endsWith("-dev+" + head),
        "development binary must match the checkout",
      );
    } else {
      const tags = execFileSync("git", ["tag", "--points-at", "HEAD"], { encoding: "utf8" })
        .trim()
        .split("\n");
      assert.ok(tags.includes(version.tag), "release binary must match the checkout");
    }
    let report = run(capture) as { output: string; cells: Cell[] };
    // From the development build on, `dui map` plans cells without capturing them
    // (docs/en/web-viewer.md#shared-screen-maps); the viewer is then checked on a captured map.
    if (report.cells.length && report.cells.every((c) => c.status === "unverified" && !c.image)) {
      assert.notEqual(name, "installed", "the default release still captures");
      assert.ok(
        report.cells.every((c) => (c as { reason?: string }).reason === "not-rendered"),
        "a map without capture plans its cells",
      );
      // A home of its own, so the viewer lists just the captured run.
      const captured = home + "-captured";
      mkdirSync(captured);
      options = { cwd, env: { ...process.env, HOME: captured } };
      report = run([...capture, "--capture"]) as { output: string; cells: Cell[] };
    }
    assert.ok(
      report.cells.length &&
        report.cells.every(
          (c) => c.status === "verified" && c.image && c.observation?.renderTree?.length,
        ),
      "expected verified captures with render evidence",
    );
    const viewer = spawn(binary, guide.first[2]!, {
      ...options,
      stdio: ["ignore", "pipe", "inherit"],
    });
    const lines = createInterface({ input: viewer.stdout });
    try {
      const [url] = await once(lines, "line", { signal: AbortSignal.timeout(15000) });
      assert.match(url, /^http:\/\/127\.0\.0\.1:\d+\/?$/);
      await verifyViewer(url, report.output, new Set(report.cells.map((c) => c.image!)));
    } finally {
      const stopped =
        viewer.exitCode === null && viewer.signalCode === null ? once(viewer, "exit") : undefined;
      lines.close();
      viewer.kill();
      await stopped;
    }
    console.log(
      name,
      version.version,
      report.cells.length,
      "verified cells; displayed captured PNG",
    );
  }
} finally {
  rmSync(root, { recursive: true, force: true });
}
