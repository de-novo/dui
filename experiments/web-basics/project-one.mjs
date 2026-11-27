import fs from "node:fs";
import path from "node:path";
import { project } from "./static-tree.mjs";
import { jsx } from "./lower.mjs";
import { execFileSync } from "node:child_process";
const [mode, dir, dui, width] = process.argv.slice(2);
const read = (p) => fs.readFileSync(path.join(dir, p), "utf8");
const tree = project(read("App.tsx"), JSON.parse(read("bindings.json"))),
  css = read("built.css");
if (mode === "sidecar") {
  const input = JSON.parse(read("sidecar.json"));
  process.stdout.write(JSON.stringify({ ...input, tree, css }));
} else {
  fs.writeFileSync(
    path.join(dir, "normalized/App.tsx"),
    "export default function App(){return (" + jsx(tree) + ")}",
  );
  const a = execFileSync(
    dui,
    [
      "model",
      path.join(dir, "normalized"),
      "App.tsx",
      ...(css ? ["--built-css", "built.css", "--viewport-width", width] : []),
    ],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  process.stdout.write(a);
}
