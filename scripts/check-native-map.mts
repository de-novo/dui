import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { verifyNativeMapViewer } from "./check-native-map-viewer.mts";

const binary = process.env.DUI_BIN || resolve(import.meta.dirname, "../target/debug/dui");

type RouteNode = {
  id: string;
  label: string;
  nativeSource: { source: string; name: string; line: number };
};

// Compare the three pinned upstream examples with their own declaration forms.
// The runtime adapters never use these fixture conventions or expected screen counts.
function upstreamDeclarations(sample: string, project: string, nodes: RouteNode[]) {
  if (sample === "expo") {
    const files = readdirSync(join(project, "app"), { recursive: true, encoding: "utf8" }).filter(
      (name) =>
        name.endsWith(".tsx") && !name.endsWith(".web.tsx") && basename(name) !== "_layout.tsx",
    );
    assert.deepEqual(
      nodes.map((n) => n.nativeSource.source).sort(),
      files.map((f) => "app/" + f).sort(),
    );
  } else if (sample === "navigation") {
    const files = readdirSync(join(project, "src"), { recursive: true, encoding: "utf8" }).filter(
      (f) => f.endsWith(".tsx"),
    );
    const expected: string[] = [];
    for (const file of files) {
      const source = readFileSync(join(project, "src", file), "utf8");
      for (const match of source.matchAll(
        /(?:^\s*(\w+): createNativeStackScreen\(|<\w+\.Screen\s+name="([^"]+)")/gm,
      )) {
        expected.push("src/" + file + "#" + (match[1] ?? match[2]));
      }
    }
    assert.deepEqual(
      nodes.map((n) => n.nativeSource.source + "#" + n.nativeSource.name).sort(),
      expected.sort(),
    );
  } else {
    const source = readFileSync(join(project, "lib/routing/router.dart"), "utf8").split(/\r?\n/);
    const lines = source.flatMap((line, i) => (/^\s*GoRoute\(/.test(line) ? [i + 1] : []));
    assert.deepEqual(
      nodes.map((n) => n.nativeSource.line).sort((a, b) => a - b),
      lines,
    );
  }
}

const samples = ["navigation", "expo", "flutter"];
const projects = process.argv.slice(2);
assert.equal(projects.length, samples.length, "Pass T3 Code, Orca and Compass app directories");
for (const [index, arg] of projects.entries()) {
  const project = realpathSync(arg);
  const report = JSON.parse(execFileSync(binary, ["map", project], { encoding: "utf8" }));
  const graph = JSON.parse(readFileSync(join(report.output, "graph.json"), "utf8"));
  upstreamDeclarations(samples[index]!, project, graph.nodes);
  assert.equal(report.screenDiscovery.kind, "native-source");
  assert.equal(graph.nodes.length, report.screenDiscovery.declared);
  assert(graph.nodes.length > 0, JSON.stringify(report.screenDiscovery));
  assert.deepEqual(report.cells, []);
  assert.equal(report.coverage.attempted, 0);
  assert.equal(report.coverage.verified, 0);
  assert.deepEqual(graph.edges, []);
  assert.equal(
    new Set(graph.nodes.map((node: { id: string }) => node.id)).size,
    graph.nodes.length,
  );

  const db = new DatabaseSync(join(homedir(), ".dui", "dui.db"), { readOnly: true });
  try {
    const rows = db
      .prepare(
        "SELECT s.id, s.path, s.origin_json FROM screens s JOIN run_screens rs ON s.id = rs.screen_id WHERE rs.run_id = ?",
      )
      .all(report.snapshot.id);
    const byId = new Map(rows.map((row) => [row.id, row]));
    assert.equal(byId.size, graph.nodes.length);
    for (const node of graph.nodes) {
      const row = byId.get(node.id);
      assert(row);
      assert.equal(row.path, node.label);
      assert.deepEqual(JSON.parse(String(row.origin_json)), {
        kind: "native",
        ...node.nativeSource,
      });
      const source = readFileSync(join(project, node.nativeSource.source), "utf8").split(/\r?\n/);
      assert(source[node.nativeSource.line - 1]?.trim(), JSON.stringify(node));
    }
  } finally {
    db.close();
  }
  await verifyNativeMapViewer(report.output);
  console.log(
    `Verified ${graph.nodes.length} declared screens and zero native captures in ${project}`,
  );
}
