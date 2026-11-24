// Capture two pinned Git checkouts and compare only archives produced by this run.
import { execFileSync, spawnSync } from "node:child_process";
import {
  appendFileSync,
  closeSync,
  existsSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, posix, resolve } from "node:path";

function git(root: string, args: string[]) {
  return execFileSync("git", ["-C", root, ...args], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
}

function projects(root: string, sha: string) {
  if (!/^[0-9a-f]{40}$/.test(sha) || git(root, ["rev-parse", "HEAD"]).trim() !== sha) {
    throw new Error("Checkout does not match the requested commit " + sha);
  }
  if (git(root, ["status", "--porcelain", "--untracked-files=no"]).length) {
    throw new Error("Tracked files differ from commit " + sha);
  }
  const files = new Set(git(root, ["ls-tree", "-rz", "--name-only", "HEAD"]).split("\0"));
  const roots = new Set<string>();
  for (const file of files) {
    const dir = posix.dirname(file);
    // A dui.json without an entry point or package can also be a profile for an external app.
    if (
      posix.basename(file) === "index.html" ||
      (posix.basename(file) === "dui.json" && files.has(posix.join(dir, "package.json")))
    ) {
      roots.add(dir);
    }
  }
  // Nested static index pages belong to the enclosing app. A separate package or
  // capture profile declares an independent app within the same source tree.
  return new Set(
    [...roots].filter((dir) => {
      if (files.has(posix.join(dir, "dui.json")) || files.has(posix.join(dir, "package.json")))
        return true;
      let ancestor = dir;
      while (ancestor !== ".") {
        ancestor = posix.dirname(ancestor);
        if (roots.has(ancestor)) return false;
      }
      return true;
    }),
  );
}

function capture(binary: string, root: string, app: string, sha: string, stage: string) {
  const project = join(root, app);
  const env: NodeJS.ProcessEnv = { ...process.env, HOME: stage };
  // Project build and capture subprocesses do not produce Actions step outputs.
  for (const key of [
    "GITHUB_ENV",
    "GITHUB_PATH",
    "GITHUB_OUTPUT",
    "GITHUB_STATE",
    "GITHUB_STEP_SUMMARY",
  ])
    delete env[key];
  console.log("Capturing", app, "at", sha);
  if (existsSync(join(project, "package.json"))) {
    execFileSync("npm", ["ci", "--no-audit", "--no-fund"], {
      cwd: project,
      env,
      stdio: ["ignore", "inherit", "inherit"],
    });
  }
  // Large reports contain render trees and can exceed a subprocess pipe buffer.
  const file = join(stage, "capture.json");
  const fd = openSync(file, "w");
  let run;
  try {
    run = spawnSync(binary, ["run", project], {
      cwd: root,
      env,
      stdio: ["ignore", fd, "inherit"],
    });
  } finally {
    closeSync(fd);
  }
  if (run.error) throw run.error;
  const { output, error, cells } = JSON.parse(readFileSync(file, "utf8"));
  if (run.status !== 0 && run.status !== 5)
    throw new Error(
      `Capture at ${sha} exited with ${run.status ?? run.signal}: ${String(error ?? "no archive").slice(0, 600)}`,
    );
  if (typeof output !== "string") throw new Error("Capture produced no archive");
  const snapshot = JSON.parse(readFileSync(join(output, "snapshot.json"), "utf8"));
  if (snapshot.commit !== sha || snapshot.app !== (app === "." ? "" : app) || snapshot.dirty) {
    throw new Error("Archive provenance does not match the clean requested app and commit");
  }
  return {
    output,
    rendered:
      Array.isArray(cells) &&
      cells.some((cell) => typeof cell.image === "string" && cell.image.length > 0),
  };
}

function escaped(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replace(/[\r\n]/g, " ");
}

function compare(
  binary: string,
  base: string,
  head: string,
  baseSha: string,
  headSha: string,
  stage: string,
) {
  const baseApps = projects(base, baseSha);
  const headApps = projects(head, headSha);
  const summary = [`Base \`${baseSha}\` · Head \`${headSha}\``];
  let regressions = false;
  let inconclusive = false;
  let common = 0;
  for (const app of [...new Set([...baseApps, ...headApps])].sort()) {
    const label = `<code>${escaped(app)}</code>`;
    if (!baseApps.has(app) || !headApps.has(app)) {
      summary.push(`${label} — present only on the ${baseApps.has(app) ? "base" : "head"} commit.`);
      continue;
    }
    common++;
    console.log("Comparing", app);
    try {
      const before = capture(binary, base, app, baseSha, mkdtempSync(join(stage, "base-")));
      const after = capture(binary, head, app, headSha, mkdtempSync(join(stage, "head-")));
      if (!before.rendered && !after.rendered)
        throw new Error("Neither commit produced a rendered cell");
      const args = ["diff", before.output, after.output, "--regressions-only"];
      const options = {
        encoding: "utf8" as const,
        env: { ...process.env, HOME: stage },
        maxBuffer: 64 * 1024 * 1024,
      };
      const diff = spawnSync(binary, args, options);
      if (diff.error) throw diff.error;
      const { verification, error } = JSON.parse(diff.stdout);
      if (!verification || ![0, 2, 5].includes(diff.status ?? -1))
        throw new Error(`Archive comparison failed: ${String(error ?? diff.status).slice(0, 600)}`);
      let detail =
        `${verification.regressedCells} regressed cells across ${verification.comparedChecks} compared checks. ` +
        `${verification.unmatchedBaseCells} base and ${verification.unmatchedHeadCells} head cells were unmatched.`;
      if (diff.status === 5) {
        regressions = true;
        // Use the CLI's PR formatter for the affected states and their recorded evidence.
        const formatted = spawnSync(binary, [...args, "--markdown"], options);
        if (formatted.error) throw formatted.error;
        if (formatted.status !== 5) throw new Error("Unable to format verification regressions");
        detail = formatted.stdout.trim();
      } else if (diff.status === 2) {
        inconclusive = true;
        detail = `Inconclusive — ${detail}`;
      }
      summary.push(`### ${label}\n\n${detail}`);
    } catch (err) {
      inconclusive = true;
      summary.push(`### ${label}\n\nInconclusive — <code>${escaped(String(err))}</code>`);
    }
  }
  if (!common) {
    inconclusive = true;
    summary.push("Inconclusive — no shared web capture projects.");
  }
  return { code: regressions ? 5 : inconclusive ? 2 : 0, summary };
}

const [binary, base, head, baseSha, headSha] = process.argv.slice(2);
if (!binary || !base || !head || !baseSha || !headSha)
  throw new Error("Expected DUI BASE_DIR HEAD_DIR BASE_SHA HEAD_SHA");
const stage = mkdtempSync(join(tmpdir(), "dui-pr-captures-"));
try {
  let result;
  try {
    result = compare(resolve(binary), resolve(base), resolve(head), baseSha, headSha, stage);
  } catch (err) {
    result = { code: 2, summary: [escaped(String(err))] };
  }
  const verdict =
    result.code === 5
      ? "New verification regressions"
      : result.code === 2
        ? "Inconclusive"
        : "No new verification regressions";
  const markdown = [`# ${verdict}`, ...result.summary].join("\n\n") + "\n";
  console.log(markdown);
  if (process.env.GITHUB_STEP_SUMMARY) {
    // GitHub caps each step summary at 1 MiB. Full details are already in the job log.
    const bytes = Buffer.from(markdown);
    const limit = 1_000_000;
    const visible =
      bytes.length <= limit
        ? markdown
        : bytes.subarray(0, limit).toString("utf8") + "\n\nFull report in the job log.\n";
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, visible);
  }
  if (process.env.GITHUB_ACTIONS && result.code !== 0)
    console.log(`::error title=dui PR verification::${verdict}. See the job summary for evidence.`);
  process.exitCode = result.code;
} finally {
  rmSync(stage, { recursive: true, force: true });
}
