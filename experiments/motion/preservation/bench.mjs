// Input is the same five pinned model/stylesheet pairs for all encodings.
import fs from "node:fs";
import { candidates, raw, ordered, withVariables } from "./extract.mjs";
const [method, input] = process.argv.slice(2);
const cases = JSON.parse(fs.readFileSync(input));
const encode = { raw, ordered, scoped: withVariables }[method];
let bytes;
const start = performance.now();
for (let i = 0; i < 100; i++) {
  bytes = 0;
  for (const c of cases)
    bytes += Buffer.byteLength(JSON.stringify(encode(candidates(c.model, c.css))));
}
console.log(JSON.stringify({ ms: performance.now() - start, bytes }));
