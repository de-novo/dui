import { useEffect, useState } from "react";
import { sourceModelApiUrl } from "../../data/archiveUrls";
import type { ScreenMap } from "../../data/overview";
import { decodeModel, type Model } from "./modelDocument";
import {
  applyModelPatch,
  applySourceGraphPatch,
  replaceSourceGraph,
  type SourceGraphPatch,
  type SourceModelPatch,
} from "./hotReload";

type HotState = { archive: ScreenMap; model?: Model };

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Subscribe to the embedded server's in-memory source-model overlay. Archive files themselves
 * are never rewritten; static Pages exports simply do not expose this endpoint and keep the
 * frozen archived model.
 */
export function useSourceModelHotReload(
  runKey: string,
  initialArchive: ScreenMap,
  initialModel: Model | undefined,
  enabled: boolean,
): HotState {
  const [state, setState] = useState<HotState>({ archive: initialArchive, model: initialModel });
  useEffect(() => {
    if (!enabled || !initialModel) return;
    let stopped = false;
    let archive = initialArchive;
    let model: Model = initialModel;
    let revision: number | undefined;
    let statusRevision: number | undefined;

    async function poll() {
      let retry = 0;
      while (!stopped) {
        try {
          const params = new URLSearchParams();
          if (revision !== undefined) {
            params.set("revision", String(revision));
            params.set("statusRevision", String(statusRevision ?? 0));
            params.set("wait", "1");
          }
          const separator = params.size ? "&" : "";
          const response = await fetch(`${sourceModelApiUrl(runKey)}${separator}${params}`, {
            cache: "no-store",
          });
          if (!response.ok) return;
          const raw = await response.json();
          if (typeof raw?.revision !== "number") return;
          const nextRevision = raw.revision as number;
          const nextStatusRevision =
            typeof raw.statusRevision === "number" ? raw.statusRevision : 0;
          let changed = false;
          if (raw.kind === "snapshot" && raw.model) {
            const nextModel = decodeModel(raw.model);
            if (raw.patch && raw.graphPatch) {
              model = applyModelPatch(model, raw.patch as SourceModelPatch);
              archive = applySourceGraphPatch(
                archive,
                raw.graphPatch as SourceGraphPatch,
                (raw.patch as SourceModelPatch).affectedScreens ?? [],
              );
              changed = true;
            } else if (nextRevision !== 0) {
              model = nextModel;
              archive = replaceSourceGraph(archive, raw.graph ?? {});
              changed = true;
            }
          } else if (raw.kind === "patch" && raw.patch) {
            model = applyModelPatch(model, raw.patch as SourceModelPatch);
            archive = applySourceGraphPatch(
              archive,
              (raw.graphPatch ?? {}) as SourceGraphPatch,
              (raw.patch as SourceModelPatch).affectedScreens ?? [],
            );
            changed = true;
          }
          revision = nextRevision;
          statusRevision = nextStatusRevision;
          if (changed && !stopped) setState({ archive, model });
          retry = 0;
        } catch {
          if (stopped) return;
          retry = Math.min(retry + 1, 4);
          await sleep(250 * retry);
        }
      }
    }
    void poll();
    return () => {
      stopped = true;
    };
  }, [runKey, initialArchive, initialModel, enabled]);
  return state;
}
