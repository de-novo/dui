/**
 * Resolves archive and API URLs for both the embedded `dui open` loopback server
 * and static GitHub Pages demo exports (`<meta name="dui-static-archive" content="./archive">`).
 */

function staticArchiveBase(): string | null {
  if (typeof document === "undefined") return null;
  const meta = document.querySelector<HTMLMetaElement>('meta[name="dui-static-archive"]');
  const value = meta?.content?.trim();
  return value ? value.replace(/\/+$/, "") : null;
}

export function isStaticArchiveMode(): boolean {
  return staticArchiveBase() !== null;
}

export function runsIndexUrl(): string {
  const base = staticArchiveBase();
  return base ? `${base}/runs.json` : "/archive/runs.json";
}

export function archiveFileUrl(name: string, runKey: string): string {
  const base = staticArchiveBase();
  if (base) {
    return `${base}/${encodeURIComponent(runKey)}/${encodeURIComponent(name)}`;
  }
  return `/archive/${encodeURIComponent(name)}?run=${encodeURIComponent(runKey)}`;
}

export function auditApiUrl(runKey: string): string {
  const base = staticArchiveBase();
  if (base) {
    return `${base}/${encodeURIComponent(runKey)}/audit.json`;
  }
  return `/api/audit?run=${encodeURIComponent(runKey)}`;
}

export function sourceModelApiUrl(runKey: string): string {
  return `/api/source-model?run=${encodeURIComponent(runKey)}`;
}

export function diffApiUrl(beforeKey: string, afterKey: string, pixels: boolean): string {
  const base = staticArchiveBase();
  if (base) {
    return `${base}/diffs/${encodeURIComponent(beforeKey)}__${encodeURIComponent(afterKey)}.json`;
  }
  return `/api/diff?before=${encodeURIComponent(beforeKey)}&after=${encodeURIComponent(afterKey)}&pixels=${pixels ? "1" : "0"}`;
}

export function diffOverlayUrl(file: string): string {
  const base = staticArchiveBase();
  if (base) {
    return `${base}/diffs/${encodeURIComponent(file)}`;
  }
  return `/api/diff-overlay?file=${encodeURIComponent(file)}`;
}
