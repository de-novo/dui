// The Rust CLI invokes this module only for an explicitly supplied --design manifest.
import { spawnSync } from "node:child_process";
import { readFileSync, realpathSync, statSync } from "node:fs";
import path from "node:path";

function projectFile(root, name) {
  if (typeof name !== "string" || !name || path.isAbsolute(name)) {
    throw new Error("expected a project-relative file path");
  }
  const file = realpathSync(path.resolve(root, name));
  const relative = path.relative(root, file);
  if (
    relative.split(path.sep)[0] === ".." ||
    path.isAbsolute(relative) ||
    !statSync(file).isFile()
  ) {
    throw new Error(`expected a file inside the manifest directory: ${name}`);
  }
  return file;
}

function loadConfig(manifest) {
  const root = path.dirname(realpathSync(manifest));
  const config = JSON.parse(readFileSync(manifest, "utf8"));
  if (!Array.isArray(config.plugins) || !config.plugins.length) {
    throw new Error("design audit requires a non-empty plugins array");
  }
  const plugins = config.plugins.map((name) => ({ name, file: projectFile(root, name) }));
  const spec = readFileSync(projectFile(root, config.designSpec), "utf8");
  const content = config.designSpec.endsWith(".json") ? JSON.parse(spec) : spec;
  return { root, plugins, designSpec: { path: config.designSpec, content } };
}

function checkCell(plugin, input, root) {
  const isModule = plugin.file.endsWith(".mjs");
  const result = spawnSync(
    isModule ? process.execPath : plugin.file,
    isModule ? [plugin.file] : [],
    {
      cwd: root,
      input: JSON.stringify(input),
      encoding: "utf8",
      timeout: 30_000,
      killSignal: "SIGKILL",
      maxBuffer: 8 * 1024 * 1024,
    },
  );
  if (result.error || result.status !== 0) {
    throw new Error(
      `${plugin.name}: ${result.error?.message || result.stderr.trim() || "plugin failed"}`,
    );
  }
  let out;
  try {
    out = JSON.parse(result.stdout);
  } catch {
    throw new Error(`${plugin.name}: expected a JSON response for cell ${input.index}`);
  }
  if (
    !Array.isArray(out?.findings) ||
    (out.unverified !== undefined &&
      !(
        Array.isArray(out.unverified) &&
        out.unverified.every((reason) => typeof reason === "string" && reason.trim())
      ))
  ) {
    throw new Error(`${plugin.name}: expected findings and optional unverified reasons`);
  }
  for (const finding of out.findings) {
    const hasEvidence =
      typeof finding?.nodeId === "string" &&
      input.renderTree.some(
        (node) =>
          node.id === finding.nodeId &&
          node.rect &&
          ["x", "y", "width", "height"].every(
            (k) => Number.isFinite(node.rect[k]) && finding.rect?.[k] === node.rect[k],
          ),
      );
    if (
      !hasEvidence ||
      !["rule", "summary"].every((k) => typeof finding[k] === "string" && finding[k].trim()) ||
      !["info", "warning", "error"].includes(finding.severity) ||
      !("measured" in finding && "expected" in finding) ||
      (finding.specRef !== undefined && typeof finding.specRef !== "string")
    ) {
      throw new Error(`${plugin.name}: invalid finding or missing captured node evidence`);
    }
  }
  return out;
}

function audit(report, { root, plugins, designSpec }) {
  if (!Array.isArray(report.cells)) throw new Error("expected captured cells in report.json");
  const findings = [];
  const unverified = [];
  let auditedCells = 0;
  for (const [index, cell] of report.cells.entries()) {
    const location = {
      index,
      path: cell.path || report.path || "/",
      width: cell.width,
      state: cell.state ?? null,
      locale: cell.locale ?? null,
      scenario: cell.scenario ?? null,
    };
    const tree = cell.observation?.renderTree;
    if (cell.status !== "verified" || !Array.isArray(tree) || !tree.length) {
      unverified.push({ cell: location, reason: cell.reason || "Missing verified render tree" });
      continue;
    }
    const input = { version: 1, ...location, renderTree: tree, designSpec };
    for (const plugin of plugins) {
      const result = checkCell(plugin, input, root);
      for (const finding of result.findings)
        findings.push({ ...finding, plugin: plugin.name, cell: location });
      for (const reason of result.unverified ?? [])
        unverified.push({ plugin: plugin.name, cell: location, reason });
    }
    auditedCells++;
  }
  if (!report.cells.length) unverified.push({ reason: "No captured cells" });
  const status = findings.length ? "findings" : unverified.length ? "unverified" : "passed";
  return { version: 1, status, designSpec: designSpec.path, auditedCells, findings, unverified };
}

try {
  process.exitCode = 2;
  const config = loadConfig(process.argv[1]);
  process.exitCode = 3;
  const result = audit(JSON.parse(readFileSync(process.argv[2], "utf8")), config);
  console.log(JSON.stringify(result));
  process.exitCode = result.status === "passed" ? 0 : 5;
} catch (error) {
  console.error(error.message);
}
