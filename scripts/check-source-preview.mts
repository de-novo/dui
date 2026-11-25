import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { withChromiumPage } from "./chromium-page.mts";
import { exportPreview } from "./source-preview.mts";

const dir = mkdtempSync(join(tmpdir(), "dui-preview-check-"));
try {
  await withChromiumPage(async ({ call }) => {
    await call("Network.enable");
    await call("Network.emulateNetworkConditions", {
      offline: true,
      latency: 0,
      downloadThroughput: 0,
      uploadThroughput: 0,
    });
    async function evaluate(expression: string) {
      const { result, exceptionDetails } = await call("Runtime.evaluate", {
        expression,
        returnByValue: true,
      });
      assert.ok(!exceptionDetails, JSON.stringify(exceptionDetails));
      return (result as { value: unknown }).value;
    }
    for (const [ext, entry] of [
      ["swift", "Settings"],
      ["kt", "example.Settings"],
    ]) {
      const original = readFileSync(
        resolve(import.meta.dirname, "../fixtures/source-preview/Settings." + ext),
        "utf8",
      );
      const file = join(dir, "Settings." + ext);
      for (const shared of [false, true]) {
        let source = original;
        if (shared) {
          const rows = source.split("\n");
          const first = rows.findIndex((line) => line.includes('"Notifications"'));
          const second = rows.findIndex((line) => line.includes('"Reminders"'));
          assert.ok(first >= 0 && second > first);
          [rows[first], rows[second]] = [
            rows[second]
              .replaceAll("reminders", "notifications")
              .replace("Reminders", "<Changed & shared>"),
            rows[first],
          ];
          source = rows.join("\n");
        }
        writeFileSync(file, source);
        const preview = exportPreview(file, entry);
        assert.equal(preview.states, shared ? 2 : 4);
        const out = join(dir, ext + "-" + shared + ".html");
        writeFileSync(out, preview.html);
        await call("Page.navigate", { url: pathToFileURL(out).href });
        const selector = "main input[role=switch]";
        for (let i = 0; i < 100; i++) {
          if (
            await evaluate(
              "location.href===" +
                JSON.stringify(pathToFileURL(out).href) +
                " && document.querySelectorAll('" +
                selector +
                "').length===2",
            )
          )
            break;
          await new Promise((r) => setTimeout(r, 25));
        }
        const controls = "Array.from(document.querySelectorAll('" + selector + "'))";
        assert.deepEqual(
          await evaluate(controls + ".map(input=>input.labels[0].textContent.trim())"),
          shared ? ["<Changed & shared>", "Notifications"] : ["Notifications", "Reminders"],
        );
        const values = controls + ".map(input=>input.checked)";
        assert.deepEqual(await evaluate(values), [false, !shared]);
        await evaluate(controls + "[0].click()");
        assert.deepEqual(await evaluate(values), [true, true]);
        assert.ok(
          await evaluate('document.querySelector("main").textContent.includes("Notifications on")'),
        );
        await evaluate(controls + "[1].click()");
        assert.deepEqual(await evaluate(values), shared ? [false, false] : [true, false]);
        await evaluate(controls + "[0].click()");
        assert.deepEqual(await evaluate(values), shared ? [true, true] : [false, false]);
      }
      writeFileSync(file, original.replace('"Notifications off"', "unsupportedNativeCall()"));
      assert.throws(() => exportPreview(file, entry), /unsupportedNativeCall/);
      if (ext === "kt") {
        // Material 3 binds a trailing lambda to interactionSource, so real Compose rejects this.
        writeFileSync(
          file,
          original.replace(
            "Switch(checked = notifications, onCheckedChange = { notifications = it })",
            "Switch(notifications) { notifications = it }",
          ),
        );
        assert.throws(() => exportPreview(file, entry), /onCheckedChange/);
      }
      console.log(
        ext +
          ": source edits, independent and shared state, offline interaction and unsupported call passed",
      );
    }
  });
} finally {
  rmSync(dir, { recursive: true, force: true });
}
