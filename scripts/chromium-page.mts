import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

export function findChrome(): string {
  const candidates = [
    process.env.DUI_CHROMIUM,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter((c): c is string => Boolean(c));
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  throw new Error("Chrome/Chromium binary not found; set DUI_CHROMIUM");
}

export type ChromiumPage = {
  ws: WebSocket;
  call: (method: string, params?: Record<string, unknown>) => Promise<Record<string, unknown>>;
};

export async function withChromiumPage<T>(verify: (page: ChromiumPage) => Promise<T>): Promise<T> {
  const chromeBin = findChrome();
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "dui-viewer-verify-"));
  const chrome = spawn(
    chromeBin,
    [
      "--headless",
      "--disable-gpu",
      "--no-sandbox",
      "--no-first-run",
      "--no-startup-window",
      "--disable-dev-shm-usage",
      "--remote-debugging-port=0",
      `--user-data-dir=${userDataDir}`,
    ],
    { stdio: ["ignore", "ignore", "pipe"] },
  );

  try {
    const endpoint = await new Promise<URL>((resolve, reject) => {
      let stderr = "";
      const timer = setTimeout(() => fail("timed out starting DevTools"), 30_000);
      function fail(reason: string) {
        clearTimeout(timer);
        reject(new Error(`${chromeBin}: ${reason}\n${stderr.trim()}`));
      }
      chrome.once("error", (error) => fail(error.message));
      chrome.once("close", (code, signal) => fail(`Chrome exited (${signal ?? code})`));
      createInterface({ input: chrome.stderr }).on("line", (line) => {
        stderr = (stderr + line + "\n").slice(-4096);
        const match = line.match(/^DevTools listening on (ws:\/\/\S+)/);
        if (match) {
          clearTimeout(timer);
          resolve(new URL(match[1]));
        }
      });
    });
    const response = await fetch(`http://${endpoint.host}/json/new?about:blank`, {
      method: "PUT",
      signal: AbortSignal.timeout(10_000),
    });
    if (!response.ok) throw new Error(`Could not create Chrome page: ${response.status}`);
    const { webSocketDebuggerUrl } = (await response.json()) as { webSocketDebuggerUrl: string };

    const ws = new WebSocket(webSocketDebuggerUrl);
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Chrome page WebSocket timed out")), 10_000);
      ws.addEventListener(
        "open",
        () => {
          clearTimeout(timer);
          resolve();
        },
        { once: true },
      );
      ws.addEventListener(
        "error",
        () => {
          clearTimeout(timer);
          reject(new Error("Chrome page WebSocket failed"));
        },
        { once: true },
      );
    });
    let msgId = 1;
    const call = (method: string, params: Record<string, unknown> = {}) =>
      new Promise<Record<string, unknown>>((resolve, reject) => {
        const cur = msgId++;
        const onMsg = (ev: MessageEvent) => {
          const msg = JSON.parse(String(ev.data)) as {
            id?: number;
            result?: Record<string, unknown>;
            error?: { message?: string };
          };
          if (msg.id !== cur) return;
          ws.removeEventListener("message", onMsg);
          if (msg.error) reject(new Error(msg.error.message || method));
          else resolve(msg.result ?? {});
        };
        ws.addEventListener("message", onMsg);
        ws.send(JSON.stringify({ id: cur, method, params }));
      });

    await call("Page.enable");

    try {
      return await verify({ ws, call });
    } finally {
      ws.close();
    }
  } finally {
    chrome.kill("SIGTERM");
    await new Promise((r) => setTimeout(r, 250));
    try {
      fs.rmSync(userDataDir, { recursive: true, force: true, maxRetries: 3 });
    } catch {
      // Ignore temp profile cleanup race on exit.
    }
  }
}
