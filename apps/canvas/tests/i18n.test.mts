import assert from "node:assert/strict";
import { test } from "node:test";
import { decodeOverview } from "../src/data/overview.ts";
import { MESSAGES, normalizeUiLocale, UI_LOCALE_OPTIONS } from "../src/i18n/messages.ts";
import { parseSelection, resolveCell } from "../src/model/selection.ts";

test("viewer UI supports English, Simplified Chinese, and Korean message catalogs", () => {
  assert.deepEqual(
    UI_LOCALE_OPTIONS.map((o) => o.value),
    ["en", "zh-CN", "ko"],
  );
  assert.equal(normalizeUiLocale("en-US"), "en");
  assert.equal(normalizeUiLocale("zh-Hans-CN"), "zh-CN");
  assert.equal(normalizeUiLocale("ko-KR"), "ko");
  assert.equal(normalizeUiLocale("fr"), "en");

  assert.equal(MESSAGES.en.verified, "Verified");
  assert.equal(MESSAGES["zh-CN"].verified, "已验证");
  assert.equal(MESSAGES.ko.verified, "검증됨");
  assert.notEqual(MESSAGES.ko.unverified, MESSAGES.ko.unrenderable);
  assert.notEqual(MESSAGES["zh-CN"].unverified, MESSAGES["zh-CN"].unrenderable);
});

test("overview decoding and selection filter captured cells by locale and preserve audit evidence", () => {
  const archive = decodeOverview({
    nodes: [{ id: "route:/", kind: "screen", label: "/" }],
    overview: [],
    edges: [],
    cells: [
      {
        path: "/",
        width: 375,
        state: "en-state",
        locale: "en",
        status: "verified",
        image: "en.png",
        localeAudit: {
          locale: "en",
          missingKeys: [],
          overflows: [],
          rtlExpected: false,
          documentDir: "ltr",
          rtlMismatch: null,
        },
      },
      {
        path: "/",
        width: 375,
        state: "de-state",
        locale: "de",
        status: "verified",
        image: "de.png",
        localeAudit: {
          locale: "de",
          missingKeys: ["subtitle"],
          overflows: [
            {
              selector: '[data-testid="save-action"]',
              text: "Einstellungen speichern",
              scrollWidth: 220,
              clientWidth: 120,
              scrollHeight: 32,
              clientHeight: 32,
            },
          ],
          rtlExpected: false,
          documentDir: "ltr",
          rtlMismatch: null,
        },
      },
    ],
  });

  const sel = parseSelection("?screen=route%3A%2F&width=375&locale=de", archive);
  assert.deepEqual(sel.locales, ["de", "en"]);
  assert.equal(sel.locale, "de");
  const { cells, cell } = resolveCell(archive.screens[0], 375, sel.state, sel.locale);
  assert.equal(cells.length, 1);
  assert.equal(cell?.locale, "de");
  assert.deepEqual(cell?.localeAudit?.missingKeys, ["subtitle"]);
  assert.equal(cell?.localeAudit?.overflows[0]?.selector, '[data-testid="save-action"]');
});
