// Read the API version exposed by the caller's installed compiler alias.
import { createRequire } from "node:module";
import { join } from "node:path";
const compilerPath = process.argv[1];
const require = createRequire(join(compilerPath, "package.json"));
const compiler: { version: string } = require(compilerPath);
process.stdout.write(compiler.version);
