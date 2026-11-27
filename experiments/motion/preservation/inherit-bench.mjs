import fs from "node:fs";
import { scopes, encode } from "./inherited.mjs";
const [mode, input] = process.argv.slice(2),
  cases = JSON.parse(fs.readFileSync(input));
let bytes;
const start = performance.now();
for (let i = 0; i < 100; i++) {
  bytes = 0;
  for (const c of cases)
    bytes += Buffer.byteLength(JSON.stringify(encode(scopes(c.model, c.css), mode)));
}
console.log(JSON.stringify({ ms: performance.now() - start, bytes }));
