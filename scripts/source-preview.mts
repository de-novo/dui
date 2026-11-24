// Run trusted source locally. The exported preview contains only observed UI and transitions.
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const adapters = resolve(import.meta.dirname, "../adapters/source-preview");
function run(cmd: string, args: string[], cwd: string, timeout = 90000) {
  const r = spawnSync(cmd, args, { cwd, encoding: "utf8", timeout });
  if (r.error || r.status !== 0)
    throw new Error(r.error?.message ?? (r.stderr || r.stdout || "Process terminated: " + cmd));
  return r.stdout;
}

function compile(file: string, entry: string, tmp: string) {
  if (!/^[A-Za-z_][A-Za-z_0-9]*(\.[A-Za-z_][A-Za-z_0-9]*)*$/.test(entry))
    throw Error("Expected a component name, optionally qualified with its package.");
  switch (extname(file)) {
    case ".swift": {
      const check = run("swiftc", ["--version"], tmp);
      if (!/Swift version 6\./i.test(check)) throw Error("Swift 6 is required.");
      run(
        "swiftc",
        [
          "-emit-library",
          "-static",
          "-emit-module",
          "-module-name",
          "SwiftUI",
          join(adapters, "swift/SwiftUI.swift"),
          "-o",
          "libSwiftUI.a",
        ],
        tmp,
      );
      writeFileSync(
        join(tmp, "DuiPreviewDriver.swift"),
        "import SwiftUI\n@main enum DuiPreviewDriver { static func main() { Preview.run(" +
          entry +
          "()) } }\n",
      );
      run(
        "swiftc",
        [
          "-parse-as-library",
          "-I",
          tmp,
          "-L",
          tmp,
          "-lSwiftUI",
          file,
          "DuiPreviewDriver.swift",
          "-o",
          "component",
        ],
        tmp,
      );
      return { command: join(tmp, "component"), args: [], platform: "SwiftUI" };
    }
    case ".kt": {
      // kotlinc writes its version to stderr on some JVMs.
      const v = spawnSync("kotlinc", ["-version"], { encoding: "utf8", timeout: 15000 });
      if (v.status !== 0 || !/kotlinc-jvm 2\./.test(v.stdout + v.stderr))
        throw Error("Kotlin 2 (kotlinc) and a JDK are required.");
      writeFileSync(
        join(tmp, "DuiPreviewMain.kt"),
        "fun main(args: Array<String>) = androidx.compose.runtime.Preview.run(args) { " +
          entry +
          "() }\n",
      );
      run(
        "kotlinc",
        [
          ...["Runtime.kt", "Layout.kt", "Material.kt"].map((f) => join(adapters, "kotlin", f)),
          file,
          "DuiPreviewMain.kt",
          "-include-runtime",
          "-d",
          "component.jar",
        ],
        tmp,
      );
      return {
        command: "java",
        args: ["-cp", join(tmp, "component.jar"), "DuiPreviewMainKt"],
        platform: "Compose",
      };
    }
    default:
      throw Error("Select a .swift or .kt source file.");
  }
}

function escape(text: string) {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

type Frame = { key: string; html: string; controls: number };
function readFrame(output: string): Frame {
  const [header, ...records] = output.replace(/\r?\n$/, "").split(/\r?\n/);
  const match = /^state\t([01]*)$/.exec(header);
  if (!match) throw Error("Missing preview state. Components must keep stdout free.");
  const stack: { kind: string; parts: string[]; switches: number; text: boolean }[] = [
    { kind: "root", parts: [], switches: 0, text: false },
  ];
  let controls = 0;
  for (const record of records) {
    const [kind, a, b] = record.split("\t");
    const top = stack.at(-1)!;
    if (kind === "row" || kind === "column")
      stack.push({ kind, parts: [], switches: 0, text: false });
    else if (kind === "end" && stack.length > 1) {
      stack.pop();
      const tag =
        top.kind === "row" && top.switches === 1 && top.text && top.parts.length === 2
          ? "label"
          : "div";
      stack
        .at(-1)!
        .parts.push(
          "<" + tag + ' class="' + top.kind + '">' + top.parts.join("") + "</" + tag + ">",
        );
    } else if (kind === "text") {
      top.text = true;
      top.parts.push("<span>" + escape(Buffer.from(a, "base64").toString("utf8")) + "</span>");
    } else if (kind === "switch" && (a === "0" || a === "1")) {
      const label = escape(Buffer.from(b ?? "", "base64").toString("utf8"));
      const input =
        '<input type="checkbox" role="switch" data-control="' +
        controls++ +
        '"' +
        (a === "1" ? " checked" : "") +
        ">";
      if (!label) top.switches++;
      top.parts.push(label ? "<label>" + label + " " + input + "</label>" : input);
    } else throw Error("Unsupported preview output: " + kind);
  }
  if (stack.length !== 1) throw Error("Incomplete layout");
  return { key: match[1], html: stack[0].parts.join(""), controls };
}

export function exportPreview(source: string, entry: string) {
  const tmp = mkdtempSync(join(tmpdir(), "dui-source-preview-"));
  try {
    const host = compile(resolve(source), entry, tmp);
    function observe(path: number[]) {
      return readFrame(run(host.command, [...host.args, ...path.map(String)], tmp, 10000));
    }
    const states: { frame: Frame; path: number[]; next: number[] }[] = [];
    const indices = new Map<string, number>();
    function add(path: number[]) {
      const frame = observe(path);
      const existing = indices.get(frame.key);
      if (existing !== undefined) {
        if (states[existing].frame.html !== frame.html)
          throw Error("The same Boolean state produced different content.");
        return existing;
      }
      if (states.length >= 32) throw Error("Source preview exceeded 32 reachable states.");
      if (frame.controls > 16) throw Error("Source preview exceeded 16 switches in one state.");
      const index = states.length;
      indices.set(frame.key, index);
      states.push({ frame, path, next: [] });
      return index;
    }
    add([]);
    for (const state of states) {
      for (let i = 0; i < state.frame.controls; i++) state.next.push(add([...state.path, i]));
    }
    const templates = states
      .map(({ frame }) => "<template>" + frame.html + "</template>")
      .join("\n");
    const transitions = JSON.stringify(states.map((s) => s.next));
    return {
      states: states.length,
      html: `<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${escape(entry)} · dui source preview</title>
<style>body{font:1rem system-ui;color-scheme:light dark;background:Canvas;color:CanvasText;margin:1.5rem}header{font-size:.875rem;margin-bottom:1rem}.column{display:flex;flex-direction:column;gap:.5rem}.row,label{display:flex;align-items:center;gap:.5rem}input{margin:0}</style>
<header>Source preview · ${host.platform} · ${escape(basename(source))}</header>
<main></main>${templates}
<script>
const transitions=${transitions}, templates=document.querySelectorAll("template"), screen=document.querySelector("main");
let state=0;
function render(){screen.replaceChildren(templates[state].content.cloneNode(true))}
screen.addEventListener("change",event=>{
  const id=event.target.dataset.control;
  if(id===undefined)return;
  state=transitions[state][Number(id)];
  render();
  screen.querySelector('[data-control="'+id+'"]')?.focus();
});
render();
</script></html>`,
    };
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const [source, entry, out, ...rest] = process.argv.slice(2);
    if (!source || !entry || !out || rest.length || extname(out) !== ".html")
      throw Error("Usage: node scripts/source-preview.mts SOURCE ENTRY OUTPUT.html");
    const preview = exportPreview(source, entry);
    writeFileSync(out, preview.html);
    console.log(
      JSON.stringify({ source, entry, output: out, states: preview.states, nativeVerified: false }),
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
