import type { Run } from "../data/run.ts";

export type DiffPair = {
  before: Run;
  after: Run;
};

function shortenOpaqueVersion(version: string): string {
  const trimmed = version.trim();
  if (!trimmed || /\s/.test(trimmed) || trimmed.length <= 16) return trimmed;
  const segments = trimmed.split(/[-_]/).filter(Boolean);
  const tail = segments.at(-1);
  if (tail && tail.length >= 6 && tail.length <= 12) return tail;
  return trimmed.slice(0, 12);
}

/** Label for the top showcase project selector; distinguishes multiple captures of the same project. */
export function formatShowcaseRunLabel(run: Run, runs: readonly Run[]): string {
  const sameProject = runs.filter((candidate) => candidate.project === run.project).length > 1;
  if (sameProject) {
    const version = shortenOpaqueVersion(run.version);
    return version ? `${run.project} · ${version}` : run.project;
  }
  const detail = run.branch ?? shortenOpaqueVersion(run.version);
  return detail ? `${run.project} · ${detail}` : run.project;
}

/** Label for the inline compare selector without slicing words mid-token. */
export function formatDiffCompareLabel(run: Run, candidates: readonly Run[]): string {
  const version = shortenOpaqueVersion(run.version);
  const branches = new Set(candidates.map((c) => c.branch ?? ""));
  const projects = new Set(candidates.map((c) => c.project));
  const parts: string[] = [];
  if (projects.size > 1) parts.push(run.project);
  if (run.branch && branches.size > 1) parts.push(run.branch);
  if (version) parts.push(version);
  else if (run.branch && parts.length === 0) parts.push(run.branch);
  return parts.length > 0 ? parts.join(" · ") : run.key.slice(0, 12);
}

/** Runs that can serve as a "before" baseline for `run`. */
export function comparableRunsFor(
  run: Run,
  runs: readonly Run[],
  staticArchiveMode: boolean,
): Run[] {
  if (staticArchiveMode) {
    const hasExplicitPairs = runs.some((candidate) => Array.isArray(candidate.diffWith));
    if (hasExplicitPairs) {
      if (!run.diffWith?.length) return [];
      const allowed = new Set(run.diffWith);
      return runs.filter((candidate) => candidate.key !== run.key && allowed.has(candidate.key));
    }
  }
  return runs.filter(
    (candidate) => candidate.key !== run.key && candidate.projectId === run.projectId,
  );
}

/** Valid `(before, after)` capture pairs for full diff view. */
export function availableDiffPairs(runs: readonly Run[], staticArchiveMode: boolean): DiffPair[] {
  const byKey = new Map(runs.map((r) => [r.key, r]));
  if (staticArchiveMode) {
    const hasExplicitPairs = runs.some((candidate) => Array.isArray(candidate.diffWith));
    if (hasExplicitPairs) {
      const pairs: DiffPair[] = [];
      for (const after of runs) {
        for (const beforeKey of after.diffWith ?? []) {
          const before = byKey.get(beforeKey);
          if (before && before.key !== after.key) {
            pairs.push({ before, after });
          }
        }
      }
      return pairs;
    }
  }
  const pairs: DiffPair[] = [];
  for (const after of runs) {
    for (const before of runs) {
      if (
        before.key !== after.key &&
        (!staticArchiveMode || before.projectId === after.projectId)
      ) {
        pairs.push({ before, after });
      }
    }
  }
  return pairs;
}
