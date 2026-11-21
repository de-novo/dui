export type CommentStatus = "open" | "resolved";

export type CommentLocation = {
  kind: "screen" | "state" | "cell" | "transition";
  path?: string | null;
  stateId?: string | null;
  width?: number | null;
  to?: string | null;
};

export type ReviewComment = {
  id: string;
  projectId: string;
  runId: string;
  author: string;
  body: string;
  status: CommentStatus;
  createdAtMs: number;
  updatedAtMs: number;
  location: CommentLocation;
};

export type CommentListResponse = {
  summary: {
    total: number;
    open: number;
    resolved: number;
  };
  comments: ReviewComment[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function decodeComment(raw: unknown): ReviewComment | null {
  if (!isRecord(raw)) return null;
  if (
    typeof raw.id !== "string" ||
    typeof raw.projectId !== "string" ||
    typeof raw.runId !== "string" ||
    typeof raw.author !== "string" ||
    typeof raw.body !== "string" ||
    (raw.status !== "open" && raw.status !== "resolved")
  ) {
    return null;
  }
  const loc = isRecord(raw.location) ? raw.location : {};
  const kind =
    loc.kind === "state" || loc.kind === "cell" || loc.kind === "transition" ? loc.kind : "screen";
  return {
    id: raw.id,
    projectId: raw.projectId,
    runId: raw.runId,
    author: raw.author,
    body: raw.body,
    status: raw.status,
    createdAtMs: typeof raw.createdAtMs === "number" ? raw.createdAtMs : 0,
    updatedAtMs: typeof raw.updatedAtMs === "number" ? raw.updatedAtMs : 0,
    location: {
      kind,
      path: typeof loc.path === "string" ? loc.path : null,
      stateId: typeof loc.stateId === "string" ? loc.stateId : null,
      width: typeof loc.width === "number" ? loc.width : null,
      to: typeof loc.to === "string" ? loc.to : null,
    },
  };
}

export function decodeCommentList(raw: unknown): CommentListResponse {
  if (!isRecord(raw) || !Array.isArray(raw.comments)) {
    return { summary: { total: 0, open: 0, resolved: 0 }, comments: [] };
  }
  const comments = raw.comments
    .map(decodeComment)
    .filter((item): item is ReviewComment => item !== null);
  const open = comments.filter((c) => c.status === "open").length;
  const resolved = comments.length - open;
  return {
    summary: { total: comments.length, open, resolved },
    comments,
  };
}

export function formatCommentTarget(location: CommentLocation): string {
  if (location.kind === "transition" && location.to) {
    return `${location.path ?? ""} → ${location.to}`.trim();
  }
  if (location.kind === "cell" && location.width) {
    return `${location.width} px`;
  }
  if (location.kind === "state" && location.stateId) {
    return location.stateId;
  }
  return location.path ?? "";
}
