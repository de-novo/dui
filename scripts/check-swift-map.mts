// Map the SwiftUI fixture with `dui map` on macOS with Xcode and check every rule's declarations.
// Rules: docs/en/roadmap.md#swiftui-screen-discovery.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = process.env.DUI_BIN || join(root, "target/debug/dui");
const fixture = join(root, "fixtures/swiftui-map");
const home = mkdtempSync(join(tmpdir(), "dui-swift-map-"));

type Node = {
  label: string;
  nativeSource: {
    name: string;
    source: string;
    line: number;
    dynamic?: boolean;
    condition?: string;
  };
};

try {
  const run = JSON.parse(
    execFileSync(binary, ["map", fixture], {
      encoding: "utf8",
      env: { ...process.env, HOME: home },
      maxBuffer: 16 * 1024 * 1024,
      timeout: 30 * 60 * 1000,
    }),
  );
  assert.deepEqual(run.coverage, { attempted: 0, renderable: 0, unrenderable: 0, verified: 0 });
  const graph = JSON.parse(readFileSync(join(run.output, "graph.json"), "utf8"));
  const screens = (graph.nodes as Node[])
    .map(({ label, nativeSource: n }) =>
      [label, n.source, n.line, n.dynamic ? `dynamic: ${n.condition}` : "static"].join(" "),
    )
    .sort();
  const file = "Sources/SwiftUIMapFixture";
  assert.deepEqual(screens, [
    `SwiftUIMapFixture/HomeView.swift#HomeView ${file}/HomeView.swift 5 static`,
    `SwiftUIMapFixture/LegacyViewController.swift#LegacyViewController ${file}/LegacyViewController.swift 5 static`,
    `SwiftUIMapFixture/RootView.swift#RootView ${file}/RootView.swift 3 static`,
    `SwiftUIMapFixture/Screens.swift#ComposeView ${file}/Screens.swift 16 static`,
    `SwiftUIMapFixture/Screens.swift#DetailView ${file}/Screens.swift 3 static`,
    `SwiftUIMapFixture/Screens.swift#HostedView ${file}/Screens.swift 40 static`,
    `SwiftUIMapFixture/Screens.swift#ItemView ${file}/Screens.swift 11 static`,
    `SwiftUIMapFixture/Screens.swift#LoginView ${file}/Screens.swift 24 dynamic: !(signedIn)`,
    `SwiftUIMapFixture/Screens.swift#ProfileView ${file}/Screens.swift 7 static`,
    `SwiftUIMapFixture/Screens.swift#SettingsView ${file}/Screens.swift 28 static`,
    `SwiftUIMapFixture/Screens.swift#WelcomeView ${file}/Screens.swift 20 dynamic: signedIn`,
  ]);
  assert.deepEqual(run.screenDiscovery.limitations, [
    `${file}/HomeView.swift:23: destination is type-erased with AnyView.`,
  ]);
  assert.equal(run.screenDiscovery.kind, "native-source");
  console.log(
    `SwiftUI map: ${screens.length} declarations from the index store, 1 stated limitation, zero captures.`,
  );
} finally {
  rmSync(home, { recursive: true, force: true });
}
