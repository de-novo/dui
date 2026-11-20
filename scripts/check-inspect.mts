import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = join(root, "target/debug/dui");
const fixture = join(root, "fixtures/replay");
const temp = mkdtempSync(join(tmpdir(), "dui-inspect-"));
const project = join(temp, "project");
const directory = join(temp, "capture");
try {
  mkdirSync(project);
  for (const file of [
    "index.html",
    "tsconfig.json",
    "dui.json",
    "package.json",
    "package-lock.json",
  ])
    copyFileSync(join(fixture, file), join(project, file));
  copyFileSync(join(import.meta.dirname, "fixtures/inspect.tsx"), join(project, "main.tsx"));
  symlinkSync(join(fixture, "node_modules"), join(project, "node_modules"), "dir");
  const env = { ...process.env, HOME: temp };
  const run = spawnSync(binary, ["run", project, join(project, "dui.json"), directory], {
    env,
    encoding: "utf8",
  });
  assert.ok(run.status === 0 || run.status === 5, run.stderr + run.stdout);
  const [cell] = JSON.parse(readFileSync(join(directory, "report.json"), "utf8")).cells;
  const base = { state: cell.state, width: cell.width };
  function inspect(extra: Record<string, unknown> = {}, reason?: string) {
    const selection = { ...base, ...extra };
    const cli = spawnSync(binary, ["inspect", directory, JSON.stringify(selection)], {
      env,
      encoding: "utf8",
    });
    const value = JSON.parse(cli.stdout);
    assert.equal(cli.status, reason ? 2 : 0, cli.stdout);
    if (reason) assert.equal(value.reason, reason);
    const request = {
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "inspect", arguments: { directory, selection } },
    };
    const mcp = spawnSync(binary, ["mcp"], {
      input: JSON.stringify(request) + "\n",
      env,
      encoding: "utf8",
    });
    const result = JSON.parse(mcp.stdout).result;
    assert.equal(result.isError, Boolean(reason));
    assert.deepEqual(JSON.parse(result.content[0].text), value);
    return value.controls as {
      slot: string;
      instance: string;
      owner: string;
      label: string;
      value: unknown;
      editable: boolean;
      type: string;
      source: { file: string };
    }[];
  }
  const controls = inspect();
  const menus = controls.filter((c) => c.label === "open");
  assert.equal(menus.length, 2);
  assert.deepEqual(new Set(menus.map((c) => c.value)), new Set([true, false]));
  assert.equal(new Set(menus.map((c) => c.instance)).size, 2);
  assert.equal(new Set(controls.map((c) => c.instance)).size, 2);
  for (const menu of menus) {
    assert.equal(menu.owner, "Menu");
    assert.equal(menu.source.file, "main.tsx");
    assert.equal(menu.type, "boolean");
    assert.deepEqual(inspect({ slot: menu.slot, instance: menu.instance, value: !menu.value }), [
      menu,
    ]);
  }
  inspect({ slot: menus[0].slot }, "ambiguous-instance");
  const mode = controls.find((c) => c.label === "mode")!;
  inspect({ slot: mode.slot, instance: mode.instance, value: "edit" });
  inspect({ slot: mode.slot, instance: mode.instance, value: "invalid" }, "invalid-value");
  const unresolved = controls.find((c) => c.label === "metadata")!;
  assert.equal(unresolved.editable, false);
  inspect({ slot: unresolved.slot, instance: unresolved.instance, value: null }, "read-only");
  assert.deepEqual(inspect(), controls);
  console.log(
    "CLI and MCP agree on two Menu instances, boolean and enum values, and read-only state.",
  );
} finally {
  rmSync(temp, { recursive: true, force: true });
}
