import { Button } from "../../components/ui/button";
import { useState } from "react";
import { isStaticArchiveMode } from "../../data/archiveUrls";
import type { Run } from "../../data/run";
import { useI18n } from "../../i18n/LocaleContext";

export function WorkPicker({ runs, current }: { runs: Run[]; current?: Run }) {
  const { t } = useI18n();
  const [key, setKey] = useState(current?.key ?? runs[0]?.key);
  const run = runs.find((r) => r.key === key);
  if (!run) return <p>{t.noSavedMaps}</p>;
  const projects = new Map(runs.map((r) => [r.projectId, r.project]));
  const projectRuns = runs.filter((r) => r.projectId === run.projectId);
  const branches = [...new Set(projectRuns.map((r) => r.branch))];
  const versions = projectRuns.filter((r) => r.branch === run.branch);
  return (
    <form action={isStaticArchiveMode() ? "./" : "/"} method="get" className="work-picker">
      <label>
        Project
        <select
          value={run.projectId}
          onChange={(e) => setKey(runs.find((r) => r.projectId === e.target.value)!.key)}
        >
          {Array.from(projects, ([id, name]) => (
            <option key={id} value={id}>
              {name}
            </option>
          ))}
        </select>
      </label>
      <label>
        Branch
        <select
          value={branches.indexOf(run.branch)}
          onChange={(e) =>
            setKey(projectRuns.find((r) => r.branch === branches[Number(e.target.value)])!.key)
          }
        >
          {branches.map((branch, i) => (
            <option key={i} value={i}>
              {branch ?? "Branch not recorded"}
            </option>
          ))}
        </select>
      </label>
      <label>
        Version
        <select name="run" value={key} onChange={(e) => setKey(e.target.value)}>
          {versions.map((r) => (
            <option key={r.key} value={r.key}>
              {r.version}
            </option>
          ))}
        </select>
      </label>
      <Button type="submit" variant="default">
        Open workspace
      </Button>
    </form>
  );
}
