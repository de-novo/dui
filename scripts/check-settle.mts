// A screen settles in tens of milliseconds, yet never before the page has finished what it
// started while mounting (#334): a render delayed by a short timer is captured after it runs,
// and a polling loop does not hold the capture back.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-settle-"));

type Report = {
  cells: {
    status: string;
    observation: { text?: string; settled?: boolean; pendingTimers?: number };
    timing: { settleMs: number };
  }[];
};

try {
  const site = join(stage, "site");
  mkdirSync(site);
  const page = (name: string, script: string) =>
    writeFileSync(
      join(site, name),
      `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${name}</title></head>
<body><main><h1>Orders</h1><ul id="list"></ul></main><script>${script}</script></body></html>`,
    );
  // The list appears 250 ms after mount, longer than the quiet window.
  page(
    "delayed.html",
    `setTimeout(() => { document.getElementById("list").innerHTML = "<li>Loaded order</li>"; }, 250);`,
  );
  // A poll every 400 ms that never changes the screen.
  page(
    "polling.html",
    `(function poll() { setTimeout(poll, 400); })(); document.getElementById("list").innerHTML = "<li>Static order</li>";`,
  );
  const capture = (path: string) => {
    const manifest = join(site, "dui.json");
    writeFileSync(
      manifest,
      JSON.stringify({ host: "static", path, widths: [800], readySelector: "main h1" }),
    );
    const run = spawnSync(binary, ["run", site, manifest], {
      encoding: "utf8",
      env: { ...process.env, HOME: join(stage, "home" + path.replace(/\W/g, "-")) },
    });
    assert.equal(run.status, 0, run.stdout.slice(0, 2000) + run.stderr.slice(0, 2000));
    return (JSON.parse(run.stdout) as Report).cells[0]!;
  };

  const delayed = capture("/delayed.html");
  assert.match(delayed.observation.text ?? "", /Loaded order/, "captured after the delayed render");
  assert.equal(delayed.observation.settled, true);
  assert.equal(delayed.observation.pendingTimers, undefined);

  const polling = capture("/polling.html");
  assert.match(polling.observation.text ?? "", /Static order/);
  assert.equal(polling.observation.settled, true);
  assert.ok(
    polling.timing.settleMs < 1000,
    `a poll does not hold the capture back: ${polling.timing.settleMs} ms`,
  );
  console.log(
    JSON.stringify({
      ok: true,
      delayedSettleMs: delayed.timing.settleMs,
      pollingSettleMs: polling.timing.settleMs,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
