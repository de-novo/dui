// Bake the default install ref into the static HTML before uploading Pages.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const target = process.argv[2];
assert.ok(target, "Pass a fresh output directory");
const installRef = execFileSync("sh", ["scripts/install", "--print-ref"], {
  encoding: "utf8",
}).trim();
const destination = resolve(target);
mkdirSync(destination, { recursive: true });
assert.equal(readdirSync(destination).length, 0, "Pages destination must be empty");
cpSync("site", destination, { recursive: true });
copyFileSync("scripts/install", join(destination, "install"));
for (const name of readdirSync(destination).filter((file) => file.endsWith(".html"))) {
  const path = join(destination, name);
  let html = readFileSync(path, "utf8");
  if (name === "index.html") {
    html = html.replace(
      /(data-install-guide\s+href="[^"]+\/blob\/)main(\/README\.md")/,
      `$1${installRef}$2`,
    );
  }
  const marker = "data-install-ref>main</span>";
  assert.equal(html.split(marker).length, 2, `${name} needs one default install ref`);
  writeFileSync(path, html.replace(marker, `data-install-ref>${installRef}</span>`));
}
console.log("Pages install ref:", installRef);
