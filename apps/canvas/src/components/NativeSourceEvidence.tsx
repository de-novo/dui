import type { NativeSource } from "../data/overview";
import type { SourcePreview } from "../data/sourcePreview";
import { useI18n } from "../i18n/LocaleContext";

/**
 * Map card body for a declaration without a capture. A portable preview is not a native
 * screen, so the card never draws it; the inspector shows it with that disclaimer.
 */
export function SourceCardBody({
  source,
  preview,
}: {
  source: NativeSource;
  preview?: SourcePreview;
}) {
  const { t } = useI18n();
  return (
    <>
      <NativeSourceEvidence source={source} sourceOnly />
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
}: {
  source: NativeSource;
  sourceOnly: boolean;
}) {
  const { t } = useI18n();
  return (
    <div className="native-source-evidence">
      <span>
        {t.sourceDeclaration}
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
