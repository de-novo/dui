import { Check, MessageSquare, RotateCcw } from "lucide-react";
import { type FormEvent, type KeyboardEvent, useEffect, useState } from "react";
import { Button } from "../../components/ui/button";
import type { Cell, Screen } from "../../data/overview";
import type { Run } from "../../data/run";
import { useI18n } from "../../i18n/LocaleContext";
import {
  type ReviewComment,
  decodeComment,
  decodeCommentList,
  formatCommentTarget,
} from "../../model/comments";

type TargetChoice = "screen" | "cell" | `transition:${string}`;

export function CommentPanel({
  run,
  screen,
  cell,
  outgoingTargets,
  onKeyDown,
}: {
  run: Run;
  screen: Screen;
  cell?: Cell;
  outgoingTargets: string[];
  onKeyDown: (event: KeyboardEvent) => void;
}) {
  const { t } = useI18n();
  const [comments, setComments] = useState<ReviewComment[]>([]);
  const [loadedKey, setLoadedKey] = useState<string>("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [body, setBody] = useState("");
  const [targetChoice, setTargetChoice] = useState<TargetChoice>("screen");
  const currentKey = `${run.projectId}:${screen.path}`;
  const loading = loadedKey !== currentKey;

  useEffect(() => {
    const controller = new AbortController();
    const params = new URLSearchParams({
      projectId: run.projectId,
      path: screen.path,
    });
    fetch(`/api/comments?${params}`, {
      cache: "no-store",
      signal: controller.signal,
    })
      .then((response) => {
        if (!response.ok) throw new Error(t.commentFailed);
        return response.json();
      })
      .then((raw) => {
        if (controller.signal.aborted) return;
        setComments(decodeCommentList(raw).comments);
        setError(null);
        setLoadedKey(currentKey);
      })
      .catch((err) => {
        if (controller.signal.aborted || (err as Error).name === "AbortError") return;
        setError(t.commentFailed);
        setLoadedKey(currentKey);
      });
    return () => controller.abort();
  }, [currentKey, run.projectId, screen.path, t.commentFailed]);

  async function handleAdd(event: FormEvent) {
    event.preventDefault();
    const trimmed = body.trim();
    if (!trimmed || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const target =
        targetChoice === "cell" && cell
          ? { kind: "cell", path: screen.path, width: cell.width, state: cell.state }
          : targetChoice.startsWith("transition:")
            ? {
                kind: "transition",
                path: screen.path,
                to: targetChoice.slice("transition:".length),
              }
            : { kind: "screen", path: screen.path };
      const response = await fetch("/api/comments", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          op: "add",
          run: run.key,
          target,
          body: trimmed,
        }),
      });
      if (!response.ok) throw new Error(t.commentFailed);
      const data = (await response.json()) as { comment?: unknown };
      const added = decodeComment(data.comment);
      if (added) {
        setComments((prev) => [...prev, added]);
        setBody("");
      } else {
        setLoadedKey("");
        setBody("");
      }
    } catch {
      setError(t.commentFailed);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleToggle(comment: ReviewComment) {
    const op = comment.status === "open" ? "resolve" : "reopen";
    setError(null);
    try {
      const response = await fetch("/api/comments", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ op, id: comment.id }),
      });
      if (!response.ok) throw new Error(t.commentFailed);
      const data = (await response.json()) as { comment?: unknown };
      const updated = decodeComment(data.comment);
      if (updated) {
        setComments((prev) => prev.map((c) => (c.id === updated.id ? updated : c)));
      }
    } catch {
      setError(t.commentFailed);
    }
  }

  const openCount = comments.filter((c) => c.status === "open").length;

  return (
    <section className="inspector-section comment-section" aria-label={t.comments}>
      <div className="comment-section-header">
        <h3>
          <MessageSquare aria-hidden="true" className="comment-heading-icon" />
          {t.comments}
        </h3>
        {comments.length > 0 && (
          <span className="comment-count-pill">
            {openCount}/{comments.length}
          </span>
        )}
      </div>

      <form className="comment-form" onSubmit={handleAdd}>
        <label className="comment-target-label">
          <span>{t.commentTarget}</span>
          <select
            aria-label={t.commentTarget}
            value={targetChoice}
            onKeyDown={onKeyDown}
            onChange={(e) => setTargetChoice(e.target.value as TargetChoice)}
          >
            <option value="screen">
              {t.commentTargetScreen} ({screen.path})
            </option>
            {cell && (
              <option value="cell">
                {t.commentTargetWidth} ({cell.width} px)
              </option>
            )}
            {outgoingTargets.map((to) => (
              <option key={to} value={`transition:${to}`}>
                {t.commentTargetTransition} ({screen.path} → {to})
              </option>
            ))}
          </select>
        </label>
        <div className="comment-input-row">
          <input
            type="text"
            className="comment-input"
            value={body}
            placeholder={t.commentPlaceholder}
            aria-label={t.commentPlaceholder}
            disabled={submitting}
            onChange={(e) => setBody(e.target.value)}
          />
          <Button type="submit" size="sm" disabled={submitting || body.trim().length === 0}>
            {t.addComment}
          </Button>
        </div>
      </form>

      {error && (
        <p className="live-unavailable-note" role="alert">
          {error}
        </p>
      )}

      {!loading && comments.length === 0 && <p className="empty-meta">{t.noComments}</p>}

      {comments.length > 0 && (
        <ul className="comment-list">
          {comments.map((comment) => (
            <li
              key={comment.id}
              className={"comment-card" + (comment.status === "resolved" ? " resolved" : "")}
              data-comment-status={comment.status}
            >
              <div className="comment-card-meta">
                <span className="comment-target-badge">
                  {formatCommentTarget(comment.location) || screen.path}
                </span>
                <span className="comment-status-label">
                  {comment.status === "open" ? t.commentOpen : t.commentResolved}
                </span>
              </div>
              <p className="comment-body">{comment.body}</p>
              <div className="comment-card-actions">
                <span className="comment-author">{comment.author}</span>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleToggle(comment)}
                >
                  {comment.status === "open" ? (
                    <>
                      <Check aria-hidden="true" />
                      {t.resolveComment}
                    </>
                  ) : (
                    <>
                      <RotateCcw aria-hidden="true" />
                      {t.reopenComment}
                    </>
                  )}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
