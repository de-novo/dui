import { RotateCcw, Save, X } from "lucide-react";
import { useState, type KeyboardEvent, type RefObject } from "react";
import { Button } from "../../components/ui/button";
import type { Screen } from "../../data/overview";
import { formatControlValue, valuesEqual, type LiveControl } from "../../model/live";
import { routeName } from "../map/CaptureCard";
import type { LiveSessionState } from "./useLiveSession";

function controlKey(control: LiveControl, index: number): string {
  return `${control.slot}:${control.instance ?? index}`;
}

function isBooleanOptions(options: unknown[] | undefined): options is [boolean, boolean] {
  return (
    Array.isArray(options) &&
    options.length === 2 &&
    options.includes(false) &&
    options.includes(true)
  );
}

export function LiveInspector({
  screen,
  width,
  session,
  dismissRef,
  onKeyDown,
  onClose,
  onMutate,
  onSave,
}: {
  screen: Screen;
  width: number;
  session: LiveSessionState;
  dismissRef: RefObject<HTMLButtonElement | null>;
  onKeyDown: (event: KeyboardEvent) => void;
  onClose: () => void;
  onMutate: (
    op: "set" | "reset",
    slot: string,
    instance: string,
    value: unknown,
    expect?: { selector: string; text: string },
  ) => void;
  onSave: () => void;
}) {
  const [selector, setSelector] = useState("");
  const [expectedText, setExpectedText] = useState("");

  const trimmedSelector = selector.trim();
  const assertion =
    trimmedSelector.length > 0
      ? { selector: trimmedSelector, text: expectedText.trim() }
      : undefined;
  const title = routeName(screen.path);
  const showFullPath = title !== screen.path;
  const hasEditableControls =
    session.phase === "ready" && session.controls.some((c) => c.editable && c.instance !== null);

  return (
    <aside className="inspector live-inspector" aria-label="Live state controls">
      <div className="panel-title">
        <div className="panel-heading">
          <h2>{title}</h2>
          {showFullPath && <code className="route-path">{screen.path}</code>}
          <p className="origin-meta">
            Live session · {width} px
            {session.phase === "ready" ? ` · rev ${session.revision}` : ""}
          </p>
        </div>
        <Button
          aria-label="Close live session"
          title="Close live session"
          variant="ghost"
          size="icon"
          ref={dismissRef}
          onKeyDown={onKeyDown}
          onClick={onClose}
        >
          <X aria-hidden="true" />
        </Button>
      </div>

      {session.phase === "ready" && (
        <>
          <div className="live-actions-row">
            <Button
              variant="default"
              className="live-save-button"
              disabled={session.busy || session.revision === 0}
              onKeyDown={onKeyDown}
              onClick={onSave}
            >
              <Save aria-hidden="true" />
              Save revision
            </Button>
          </div>

          {session.lastError && (
            <div className="live-feedback error" role="alert">
              <strong>Edit rejected</strong>
              <span>{session.lastError}</span>
            </div>
          )}

          {session.lastSave && (
            <output className="live-feedback saved" aria-live="polite">
              <strong>
                Saved revision {session.lastSave.revision} ({session.lastSave.status})
              </strong>
              <span>
                {session.lastSave.reason
                  ? `${session.lastSave.run} · ${session.lastSave.reason}`
                  : session.lastSave.run}
              </span>
              {session.lastSave.runKey && (
                <a
                  className="saved-run-link"
                  href={`/?run=${encodeURIComponent(session.lastSave.runKey)}`}
                >
                  Open saved archive
                </a>
              )}
            </output>
          )}

          {session.lastEdit && (
            <section className="inspection-result" aria-label="Latest edit result">
              <div className="section-header-row">
                <h3>Latest edit</h3>
                <span className="provenance-badge">
                  rev {session.lastEdit.revision}
                  {typeof session.lastEdit.elapsedMs === "number"
                    ? ` · ${Math.round(session.lastEdit.elapsedMs)} ms`
                    : ""}
                </span>
              </div>
              <p className="capture-meta">
                <span className="capture-status" data-status={session.lastEdit.status}>
                  {session.lastEdit.status === "verified" ? "Verified" : "Unverified"}
                </span>
                {session.lastEdit.reason && <span>{session.lastEdit.reason}</span>}
              </p>
              <p className="capture-meta">
                <span>Observed: {formatControlValue(session.lastEdit.observedValue)}</span>
                {session.lastEdit.evidence.selector ? (
                  <span>
                    Visible text:{" "}
                    {session.lastEdit.evidence.visibleText === null
                      ? "not matched"
                      : JSON.stringify(session.lastEdit.evidence.visibleText)}
                  </span>
                ) : (
                  <span>
                    {session.lastEdit.evidence.changed
                      ? "Visible DOM updated"
                      : "Visible DOM unchanged"}
                  </span>
                )}
              </p>
            </section>
          )}

          {hasEditableControls && (
            <details className="assertion-box">
              <summary>Verify visible text (optional)</summary>
              <p className="empty-meta">
                Supply a CSS selector and expected visible text to mark edits Verified against the
                rendered DOM.
              </p>
              <label>
                CSS selector
                <input
                  type="text"
                  name="expectSelector"
                  autoComplete="off"
                  spellCheck={false}
                  placeholder="button:first-child…"
                  value={selector}
                  onKeyDown={onKeyDown}
                  onChange={(e) => setSelector(e.target.value)}
                />
              </label>
              <label>
                Expected visible text
                <input
                  type="text"
                  name="expectText"
                  autoComplete="off"
                  placeholder="Close…"
                  value={expectedText}
                  onKeyDown={onKeyDown}
                  onChange={(e) => setExpectedText(e.target.value)}
                />
              </label>
            </details>
          )}

          <section className="inspector-section" aria-label="Component state controls">
            <div className="section-header-row">
              <h3>Controls</h3>
              <span className="provenance-badge">{session.controls.length}</span>
            </div>

            {session.controls.length === 0 ? (
              <p className="empty-meta">
                No instrumented state controls were observed in this cell. The live page is running
                at {width} px.
              </p>
            ) : (
              <ul className="control-list">
                {session.controls.map((control, index) => {
                  const modified =
                    control.hasInitialValue &&
                    control.hasValue &&
                    !valuesEqual(control.value, control.initialValue);
                  const instance = control.instance;
                  const canEdit = control.editable && instance !== null;

                  return (
                    <li key={controlKey(control, index)} className="control-card">
                      <div className="control-header">
                        <div className="control-identity">
                          <strong>
                            {control.owner}.{control.label}
                          </strong>
                          <span className="control-meta">
                            <code>{control.type}</code>
                            {control.source.file && <span>{control.source.file}</span>}
                            {instance && <span>#{instance.slice(0, 6)}</span>}
                          </span>
                        </div>
                        {canEdit && modified && (
                          <Button
                            variant="ghost"
                            size="xs"
                            aria-label={`Reset ${control.owner}.${control.label}`}
                            disabled={session.busy}
                            onKeyDown={onKeyDown}
                            onClick={() =>
                              onMutate("reset", control.slot, instance, undefined, assertion)
                            }
                          >
                            <RotateCcw aria-hidden="true" />
                            Reset
                          </Button>
                        )}
                      </div>

                      {canEdit && isBooleanOptions(control.options) ? (
                        <fieldset className="boolean-toggle">
                          <legend className="sr-only">
                            {control.owner}.{control.label}
                          </legend>
                          {[false, true].map((opt) => {
                            const active = control.value === opt;
                            return (
                              <button
                                key={String(opt)}
                                type="button"
                                className={"segment-button" + (active ? " active" : "")}
                                aria-pressed={active}
                                disabled={session.busy || active}
                                onKeyDown={onKeyDown}
                                onClick={() =>
                                  onMutate("set", control.slot, instance, opt, assertion)
                                }
                              >
                                {String(opt)}
                              </button>
                            );
                          })}
                        </fieldset>
                      ) : canEdit && control.options && control.options.length > 0 ? (
                        <select
                          aria-label={`${control.owner}.${control.label}`}
                          disabled={session.busy}
                          value={control.options.findIndex((o) => valuesEqual(o, control.value))}
                          onKeyDown={onKeyDown}
                          onChange={(e) => {
                            const next = control.options?.[Number(e.target.value)];
                            if (next !== undefined) {
                              onMutate("set", control.slot, instance, next, assertion);
                            }
                          }}
                        >
                          {control.options.map((opt, optIdx) => (
                            <option key={optIdx} value={optIdx}>
                              {formatControlValue(opt)}
                            </option>
                          ))}
                        </select>
                      ) : (
                        <div className="readonly-control">
                          <span className="readonly-value">
                            {control.hasValue ? formatControlValue(control.value) : "Not observed"}
                          </span>
                          <span className="readonly-reason">
                            Read-only{control.reason ? ` · ${control.reason}` : ""}
                          </span>
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        </>
      )}

      {session.phase === "opening" && (
        <p className="empty-meta">Waiting for the live page to commit its initial state…</p>
      )}
    </aside>
  );
}
