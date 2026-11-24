import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

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
      "--headless=new",
      "--disable-gpu",
      "--no-sandbox",
      "--remote-debugging-port=0",
      `--user-data-dir=${userDataDir}`,
    ],
    { stdio: "ignore" },
  );

  try {
    let wsUrl: string | undefined;
    for (let i = 0; i < 40; i++) {
      try {
        const [debugPort] = fs
          .readFileSync(path.join(userDataDir, "DevToolsActivePort"), "utf8")
          .split("\n");
        const targets = (await (
          await fetch(`http://127.0.0.1:${debugPort}/json`)
        ).json()) as Array<{
          type: string;
          webSocketDebuggerUrl?: string;
        }>;
        wsUrl = targets.find((t) => t.type === "page")?.webSocketDebuggerUrl;
        if (wsUrl) break;
      } catch {
        // Chrome still booting.
      }
      await new Promise((r) => setTimeout(r, 200));
    }
    if (!wsUrl) throw new Error("Failed to attach to headless Chrome CDP");

    const ws = new WebSocket(wsUrl);
    await new Promise((r) => ws.addEventListener("open", r, { once: true }));
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
