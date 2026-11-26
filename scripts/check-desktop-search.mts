// Exercise the native search control with routes from a real browser capture.
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const home = mkdtempSync(join(tmpdir(), "dui-search-"));
try {
  execFileSync(resolve("target/debug/dui"), ["map", "examples/dispatch", "--capture"], {
    env: { ...process.env, HOME: home },
    stdio: ["ignore", "ignore", "inherit"],
  });
  execFileSync(
    "cargo",
    [
      "test",
      "--locked",
      "--manifest-path",
      "apps/desktop/Cargo.toml",
      "screen_search::tests::search_captured_screens",
      "--",
      "--exact",
      "--ignored",
    ],
    {
      env: { ...process.env, DUI_DESKTOP_SEARCH_HOME: join(home, ".dui") },
      stdio: "inherit",
    },
  );
} finally {
  rmSync(home, { recursive: true, force: true });
}
