import { RotateCcw, Save, X } from "lucide-react";
import type { KeyboardEvent, RefObject } from "react";
import { Button } from "../../components/ui/button";
import type { Screen } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import {
  formatControlValue,
  partitionControls,
  valuesEqual,
  type LiveControl,
} from "../../model/live";
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
  onMutate: (op: "set" | "reset", slot: string, instance: string, value: unknown) => void;
  onSave: () => void;
}) {
  const { t } = useI18n();
  const title = routeName(screen.path);
  const showFullPath = title !== screen.path;
  const { editableGroups, readOnly } =
    session.phase === "ready"
      ? partitionControls(session.controls)
      : { editableGroups: [], readOnly: [] };

  const editFailed =
    session.phase === "ready" &&
    session.lastEdit !== null &&
    session.lastEdit.reason !== null &&
    session.lastEdit.reason !== "missing-assertion";

  return (
    <aside className="inspector live-inspector glass-panel" aria-label={t.liveControls}>
      <div className="panel-title">
        <div className="panel-heading">
          <h2>{title}</h2>
          {showFullPath && <code className="route-path">{screen.path}</code>}
          <p className="origin-meta">Live · {width} px</p>
        </div>
        <Button
          aria-label={t.closeLiveSession}
          title={t.closeLiveSession}
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
          <div className="live-inspector-body">
            {session.lastError && (
              <div className="live-feedback error" role="alert">
                <strong>{t.couldNotApply}</strong>
                <span>{session.lastError}</span>
              </div>
            )}

            {editFailed && session.lastEdit && (
              <div className="live-feedback error" role="alert">
                <strong>{t.valueDidNotStick}</strong>
                <span>
                  Observed {formatControlValue(session.lastEdit.observedValue)} (
                  {session.lastEdit.reason})
                </span>
              </div>
            )}

            {editableGroups.length === 0 ? (
              <p className="empty-meta">{t.noEditableStates}</p>
            ) : (
              <div className="control-groups">
                {editableGroups.map((group) => (
                  <section key={group.id} className="control-group" aria-label={group.title}>
                    <h3>{group.title}</h3>
                    <ul className="control-list">
                      {group.controls.map((control, index) => {
                        const instance = control.instance!;
                        const modified =
                          control.hasInitialValue &&
                          control.hasValue &&
                          !valuesEqual(control.value, control.initialValue);

                        return (
                          <li key={controlKey(control, index)} className="control-row">
                            <div className="control-row-header">
                              <span className="control-label">{control.label}</span>
                              {modified && (
                                <Button
                                  variant="ghost"
                                  size="xs"
                                  aria-label={`${t.reset} ${group.title} ${control.label}`}
                                  disabled={session.busy}
                                  onKeyDown={onKeyDown}
                                  onClick={() =>
                                    onMutate("reset", control.slot, instance, undefined)
                                  }
                                >
                                  <RotateCcw aria-hidden="true" />
                                  {t.reset}
                                </Button>
                              )}
                            </div>

                            {isBooleanOptions(control.options) ? (
                              <fieldset className="boolean-toggle">
                                <legend className="sr-only">
                                  {group.title} {control.label}
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
                                      onClick={() => onMutate("set", control.slot, instance, opt)}
                                    >
                                      {opt ? t.on : t.off}
                                    </button>
                                  );
                                })}
                              </fieldset>
                            ) : control.options && control.options.length > 0 ? (
                              <select
                                aria-label={`${group.title} ${control.label}`}
                                disabled={session.busy}
                                value={control.options.findIndex((o) =>
                                  valuesEqual(o, control.value),
                                )}
                                onKeyDown={onKeyDown}
                                onChange={(e) => {
                                  const next = control.options?.[Number(e.target.value)];
                                  if (next !== undefined) {
                                    onMutate("set", control.slot, instance, next);
                                  }
                                }}
                              >
                                {control.options.map((opt, optIdx) => (
                                  <option key={optIdx} value={optIdx}>
                                    {formatControlValue(opt)}
                                  </option>
                                ))}
                              </select>
                            ) : null}
                          </li>
                        );
                      })}
                    </ul>
                  </section>
                ))}
              </div>
            )}

            {readOnly.length > 0 && (
              <details className="readonly-disclosure">
                <summary>{t.readOnlyCount(readOnly.length)}</summary>
                <ul className="readonly-list">
                  {readOnly.map((control, index) => (
                    <li key={controlKey(control, index)} className="readonly-item">
                      <span>
                        {control.owner}.{control.label}
                      </span>
                      <span className="readonly-meta">
                        {control.hasValue ? formatControlValue(control.value) : t.notShown}
                        {control.reason ? ` · ${control.reason}` : ""}
                      </span>
                    </li>
                  ))}
                </ul>
              </details>
            )}
          </div>

          {(editableGroups.length > 0 || session.revision > 0 || session.lastSave) && (
            <footer className="live-inspector-footer">
              {session.lastSave && (
                <output className="live-feedback saved" aria-live="polite">
                  <span>{t.savedSnapshot}</span>
                  {session.lastSave.runKey && (
                    <a
                      className="saved-run-link"
                      href={`/?run=${encodeURIComponent(session.lastSave.runKey)}`}
                    >
                      {t.openSaved}
                    </a>
                  )}
                </output>
              )}
              <Button
                variant="default"
                className="live-save-button"
                disabled={session.busy || session.revision === 0}
                onKeyDown={onKeyDown}
                onClick={onSave}
              >
                <Save aria-hidden="true" />
                {t.saveSnapshot}
              </Button>
            </footer>
          )}
        </>
      )}

      {session.phase === "opening" && <p className="empty-meta">{t.loadingStates}</p>}
    </aside>
  );
}
