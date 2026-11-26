// Dialogs found by the compiler are opened by setting their open prop, not by clicking guessed
// triggers (#295). The fixture has an uncontrolled Radix dialog reached only from a menu item,
// a native <dialog open>, a component with an `isOpen` prop, and a button that opens nothing.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const toggle = join(root, "fixtures/toggle");
const stage = mkdtempSync(join(tmpdir(), "dui-fact-overlays-"));

type Cell = {
  path: string;
  status: string;
  reason: string | null;
  input: Record<string, unknown>;
  overlay?: {
    method: string;
    parentPath: string;
    title: string;
    source: { file: string; line: number }[];
  };
  timing?: { overlaysMs: number };
};

try {
  const app = join(stage, "app");
  const modules = join(app, "node_modules");
  mkdirSync(modules, { recursive: true });
  for (const entry of readdirSync(join(toggle, "node_modules"))) {
    symlinkSync(join(toggle, "node_modules", entry), join(modules, entry));
  }
  const radix = join(modules, "@radix-ui", "react-dialog");
  mkdirSync(radix, { recursive: true });
  writeFileSync(
    join(radix, "package.json"),
    JSON.stringify({
      name: "@radix-ui/react-dialog",
      version: "1.1.0",
      type: "module",
      main: "index.js",
      types: "index.d.ts",
    }),
  );
  writeFileSync(
    join(radix, "index.d.ts"),
    `import type { ReactNode } from "react";
export interface DialogProps { open?: boolean; defaultOpen?: boolean; children?: ReactNode }
export function Root(props: DialogProps): ReactNode;
export function Content(props: { title: string; children?: ReactNode }): ReactNode;`,
  );
  // A stand-in with Radix's controlled/uncontrolled contract: closed unless `open` is true.
  writeFileSync(
    join(radix, "index.js"),
    `import { createElement } from "react";
export function Root({ open, children }) { return open ? children : null; }
export function Content({ title, children }) {
  return createElement("div", { role: "dialog", "aria-modal": "true", "aria-label": title,
    style: { position: "fixed", top: 40, left: 40, width: 320, height: 180, background: "white", border: "2px solid black" } }, children);
}`,
  );
  writeFileSync(
    join(app, "package.json"),
    JSON.stringify({
      name: "fact-overlays",
      private: true,
      type: "module",
      dependencies: { react: "19.2.8", "react-dom": "19.2.8", "@radix-ui/react-dialog": "1.1.0" },
      devDependencies: { vite: "8.3.8", typescript: "7.0.2" },
    }),
  );
  writeFileSync(join(app, "tsconfig.json"), readFileSync(join(toggle, "tsconfig.json"), "utf8"));
  writeFileSync(
    join(app, "index.html"),
    '<!doctype html><meta charset="utf-8"><div id="app"></div><script type="module" src="/main.tsx"></script>',
  );
  writeFileSync(
    join(app, "main.tsx"),
    `import { useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import * as Dialog from "@radix-ui/react-dialog";

function Sheet({ isOpen, title, children }: { isOpen: boolean; title: string; children: ReactNode }) {
  if (!isOpen) return null;
  return (
    <section role="dialog" aria-modal="true" aria-label={title}
      style={{ position: "fixed", right: 0, top: 0, width: 300, height: 400, background: "#eee" }}>
      {children}
    </section>
  );
}

function App() {
  const [confirmOpen] = useState(false);
  const [sheetOpen] = useState(false);
  return (
    <main>
      <h1>Projects</h1>
      <div role="menu"><div role="menuitem">Invite teammates…</div></div>
      <button type="button" onClick={() => { document.title = "clicked"; }}>Refresh</button>
      <Dialog.Root>
        <Dialog.Content title="Invite teammates"><p>Send an invite link.</p></Dialog.Content>
      </Dialog.Root>
      <dialog open={confirmOpen} style={{ width: 300, height: 150 }}>
        <h2>Delete project</h2>
      </dialog>
      <Sheet isOpen={sheetOpen} title="Filters"><p>Status</p></Sheet>
    </main>
  );
}
createRoot(document.getElementById("app")!).render(<App />);
`,
  );
  writeFileSync(
    join(app, "dui.json"),
    JSON.stringify({ path: "/", widths: [768], readySelector: "main h1", discoverOverlays: true }),
  );

  const out = join(stage, "out");
  let raw: string;
  try {
    raw = execFileSync(binary, ["run", app, join(app, "dui.json"), out], {
      encoding: "utf8",
      env: { ...process.env, HOME: join(stage, "home") },
    });
  } catch (error) {
    raw = (error as { stdout: string }).stdout;
  }
  const report = JSON.parse(raw) as {
    cells: Cell[];
    overlayDiscovery: { method: string; declared: number; rendered: number; captured: number };
  };
  const overlays = report.cells
    .filter((c) => c.overlay)
    .sort((a, b) => a.path.localeCompare(b.path));
  assert.deepEqual(
    overlays.map((c) => [c.path, c.status, c.overlay!.method]),
    [
      ["/#overlay:delete-project", "verified", "compiler"],
      ["/#overlay:filters", "verified", "compiler"],
      ["/#overlay:invite-teammates", "verified", "compiler"],
    ],
    JSON.stringify(report.cells.map((c) => [c.path, c.status, c.reason, c.input])),
  );
  for (const cell of overlays) {
    assert.equal(cell.overlay!.parentPath, "/");
    assert.equal(cell.overlay!.source[0]!.file, "main.tsx");
    assert.ok(cell.overlay!.source[0]!.line > 0);
  }
  assert.deepEqual(report.overlayDiscovery, {
    method: "compiler",
    declared: 3,
    rendered: 3,
    captured: 3,
  });
  // No trigger was clicked: runtime probing never ran on any capture.
  assert.ok(report.cells.every((c) => (c.timing?.overlaysMs ?? 0) === 0));
  console.log(
    JSON.stringify({ ok: true, overlays: overlays.map((c) => c.path), ...report.overlayDiscovery }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
