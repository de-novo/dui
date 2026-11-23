import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  base: process.env.DUI_CANVAS_BASE || "/",
  plugins: [
    tailwindcss(),
    {
      name: "dui-build-only-dependencies",
      apply: "build",
      generateBundle(_, bundle) {
        for (const output of Object.values(bundle)) {
          const sources =
            output.type === "chunk" ? Object.keys(output.modules) : output.originalFileNames;
          if (
            sources.some((id) =>
              /(?:^|\/)node_modules\/(?:lightningcss(?:-[^/]+)?|tslib)\//.test(
                id.replaceAll("\\", "/"),
              ),
            )
          ) {
            this.error(`A build-only dependency entered the browser asset ${output.fileName}`);
          }
        }
      },
    },
  ],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
});
