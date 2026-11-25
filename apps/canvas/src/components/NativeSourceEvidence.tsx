import type { NativeSource } from "../data/overview";
import type { SourcePreview } from "../data/sourcePreview";
import { useI18n } from "../i18n/LocaleContext";
import { SourcePreviewSnapshot } from "./SourcePreviewPanel";

/** Map card body for a declaration without a capture: its preview snapshot, if any, then its evidence. */
export function SourceCardBody({
  source,
  preview,
}: {
  source: NativeSource;
  preview?: SourcePreview;
}) {
  const { t } = useI18n();
  const frame = preview?.status === "ready" ? preview.frames[0] : undefined;
  return (
    <>
      {frame && <SourcePreviewSnapshot items={frame.items} />}
      <NativeSourceEvidence source={source} sourceOnly preview={Boolean(frame)} />
      {preview && preview.status !== "ready" && (
        <p className="source-preview-reason">
          <strong>{t.previewStatus[preview.status]}</strong> · <code>{preview.reason}</code>
        </p>
      )}
    </>
  );
}

export function NativeSourceEvidence({
  source,
  sourceOnly,
  preview = false,
}: {
  source: NativeSource;
  sourceOnly: boolean;
  preview?: boolean;
}) {
  const { t } = useI18n();
  return (
    <div className="native-source-evidence">
      <span>
        {preview ? t.sourcePreview : t.sourceDeclaration}
        {sourceOnly && (
          <>
            {" "}
            ·{" "}
            <span className="capture-status" data-status="unverified">
              {t.unverified}
            </span>
          </>
        )}
      </span>
      <code>
        {source.source}
        {source.line === undefined ? "" : ":" + source.line}
      </code>
      {source.dynamic && <span>{t.dynamicRoute}</span>}
      {source.condition && (
        <span>
          {t.routeCondition} · <code>{source.condition}</code>
        </span>
      )}
    </div>
  );
}

export function DiscoveryLimitations({
  limitations,
  open,
}: {
  limitations?: string[];
  open?: boolean;
}) {
  const { t } = useI18n();
  if (!limitations?.length) return null;
  return (
    <details className="readonly-disclosure discovery-limitations" open={open}>
      <summary>
        {t.discoveryLimitations} ({limitations.length})
      </summary>
      <ul>
        {limitations.map((reason, index) => (
          <li key={index}>{reason}</li>
        ))}
      </ul>
    </details>
  );
}
