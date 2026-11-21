// Verify multi-locale capture (screen × state × width × locale), missing translation key
// detection, text overflow flagging with cell + selector, RTL mismatch detection, and
// viewer language switching (en / zh-CN / ko).
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

type LocaleOverflow = {
  selector: string;
  text: string;
  scrollWidth: number;
  clientWidth: number;
};

type LocaleAudit = {
  locale: string;
  missingKeys: string[];
  overflows: LocaleOverflow[];
  rtlExpected: boolean;
  documentDir: string;
  rtlMismatch: { expected: string; observed: string } | null;
};

type I18nReport = {
  cells: {
    locale?: string;
    width: number;
    status: string;
    localeAudit?: LocaleAudit;
  }[];
  locales: Record<
    string,
    {
      cells: number;
      verified: number;
      unverified: number;
      unrenderable: number;
      missingKeys: string[];
      overflowCount: number;
      rtlMismatches: number;
    }
  >;
};

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-i18n-check-"));

function chromeExecutable(): string {
  const candidates = [
    process.env.DUI_CHROMIUM,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium-browser",
  ];
  for (const path of candidates) {
    if (path && existsSync(path)) return path;
  }
  throw new Error("Chrome/Chromium executable not found");
}

try {
  const appDir = join(stage, "app");
  const homeDir = join(stage, "home");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");
  mkdirSync(join(appDir, "locales"), { recursive: true });

  writeFileSync(
    join(appDir, "locales", "en.json"),
    JSON.stringify({
      title: "Settings",
      subtitle: "Manage preferences",
      action: "Save",
    }),
  );
  writeFileSync(
    join(appDir, "locales", "de.json"),
    JSON.stringify({
      title: "Einstellungen",
      // Intentionally omit `subtitle` to test missing-key reporting in `de` only.
      action: "Einstellungen speichern und synchronisieren",
    }),
  );
  writeFileSync(
    join(appDir, "locales", "ar.json"),
    JSON.stringify({
      title: "الإعدادات",
      subtitle: "إدارة التفضيلات",
      action: "حفظ",
    }),
  );

  writeFileSync(
    join(appDir, "main.tsx"),
    [
      'import { createRoot } from "react-dom/client";',
      'import en from "./locales/en.json";',
      'import de from "./locales/de.json";',
      'import ar from "./locales/ar.json";',
      "",
      "const catalogs: Record<string, Record<string, string>> = { en, de, ar };",
      "const active = (navigator.language || 'en').split('-')[0];",
      "const msgs = catalogs[active] ?? en;",
      "",
      "function Settings() {",
      "  return (",
      '    <main data-testid="settings-screen" style={{ fontFamily: "sans-serif", padding: "16px" }}>',
      "      <h1>{msgs.title}</h1>",
      '      <p data-testid="subtitle">{msgs.subtitle ?? "missing"}</p>',
      "      <button",
      '        type="button"',
      '        data-testid="save-action"',
      '        style={{ width: "100px", overflow: "hidden", whiteSpace: "nowrap", display: "block" }}',
      "      >",
      "        {msgs.action}",
      "      </button>",
      "    </main>",
      "  );",
      "}",
      'createRoot(document.getElementById("app")!).render(<Settings />);',
    ].join("\n"),
  );

  writeFileSync(
    join(appDir, "dui.json"),
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/",
        widths: [375],
        locales: ["en", "de", "ar"],
        readySelector: "[data-testid=settings-screen]",
        stateBudget: 4,
      },
      null,
      2,
    ),
  );

  const runOut = join(stage, "run-out");
  const res = spawnSync(binary, ["run", appDir, join(appDir, "dui.json"), runOut], {
    env: { ...process.env, HOME: homeDir },
    encoding: "utf8",
  });
  assert.equal(res.status, 0, `dui run failed (${res.status}): ${res.stdout}\n${res.stderr}`);
  const report = JSON.parse(readFileSync(join(runOut, "report.json"), "utf8")) as I18nReport;

  // AC 1: Produces cells for each configured locale and groups report.locales by locale.
  assert.deepEqual(Object.keys(report.locales).sort(), ["ar", "de", "en"]);
  assert.equal(report.locales.en.cells, 1);
  assert.equal(report.locales.de.cells, 1);
  assert.equal(report.locales.ar.cells, 1);

  // AC 2: Missing key `subtitle` is reported for `de` only.
  assert.deepEqual(report.locales.en.missingKeys, []);
  assert.deepEqual(report.locales.de.missingKeys, ["subtitle"]);
  assert.deepEqual(report.locales.ar.missingKeys, []);

  // AC 3: Text overflow in `de` is flagged with the cell and selector `[data-testid="save-action"]`.
  const enCell = report.cells.find((c) => c.locale === "en");
  const deCell = report.cells.find((c) => c.locale === "de");
  const arCell = report.cells.find((c) => c.locale === "ar");
  assert(enCell && deCell && arCell);
  assert.equal(enCell.localeAudit?.overflows.length, 0);
  assert.equal(report.locales.en.overflowCount, 0);
  assert(
    deCell.localeAudit?.overflows.some((o) => o.selector === '[data-testid="save-action"]'),
    `expected overflow on [data-testid="save-action"], got ${JSON.stringify(deCell.localeAudit)}`,
  );
  assert(report.locales.de.overflowCount >= 1);
  // RTL mismatch detected on `ar` rendered as LTR.
  assert.equal(arCell.localeAudit?.rtlMismatch?.expected, "rtl");
  assert.equal(arCell.localeAudit?.rtlMismatch?.observed, "ltr");
  assert.equal(report.locales.ar.rtlMismatches, 1);

  // AC 4: Viewer switches its own UI language across en, zh-CN, and ko in real Chrome.
  execFileSync("npm", ["--prefix", join(root, "apps/canvas"), "run", "build"], {
    stdio: "pipe",
  });
  const server = spawn(process.execPath, [join(root, "apps/canvas/preview.mts"), runOut], {
    env: { ...process.env, HOME: homeDir, PORT: "0" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const url = await new Promise<string>((resolveUrl, rejectUrl) => {
      let buf = "";
      server.stdout.on("data", (chunk) => {
        buf += chunk.toString();
        const match = buf.match(/http:\/\/127\.0\.0\.1:\d+/);
        if (match) resolveUrl(match[0]);
      });
      server.on("exit", (code) => rejectUrl(new Error(`preview exited early (${code})`)));
    });

    const chrome = chromeExecutable();
    const expectedOpenLive: Record<string, string> = {
      en: "Open live",
      "zh-CN": "打开实时会话",
      ko: "라이브 열기",
    };
    for (const [uiLang, expectedLabel] of Object.entries(expectedOpenLive)) {
      const dom = execFileSync(
        chrome,
        [
          "--headless",
          "--disable-gpu",
          "--no-sandbox",
          "--virtual-time-budget=3000",
          "--dump-dom",
          `${url}/?screen=route%3A%2F&uiLang=${encodeURIComponent(uiLang)}`,
        ],
        { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
      );
      assert(
        dom.includes(`lang="${uiLang}"`),
        `expected html lang="${uiLang}" in rendered viewer DOM`,
      );
      assert(
        dom.includes(expectedLabel),
        `expected viewer label "${expectedLabel}" for uiLang=${uiLang}`,
      );
    }

    console.log(
      JSON.stringify({
        locales: report.locales,
        deOverflowSelector: deCell.localeAudit?.overflows[0]?.selector ?? null,
        viewerLanguagesVerified: ["en", "zh-CN", "ko"],
      }),
    );
  } finally {
    server.kill();
  }
} finally {
  rmSync(stage, { recursive: true, force: true });
}
