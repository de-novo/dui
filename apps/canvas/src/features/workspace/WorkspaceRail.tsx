import {
  ChevronDown,
  ChevronsUpDown,
  FolderClosed,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Sun,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { Run } from "../../data/run";

type Theme = "system" | "light" | "dark";
export function WorkspaceRail({ run, open }: { run?: Run; open: () => void }) {
  const [collapsed, setCollapsed] = useState(
    () =>
      localStorage.getItem("dui.rail") === "collapsed" ||
      (localStorage.getItem("dui.rail") === null &&
        matchMedia("(max-width: 600px)").matches),
  );
  const [theme, setTheme] = useState<Theme>(() => {
    const saved = localStorage.getItem("dui.theme");
    return saved === "dark" || saved === "light" ? saved : "system";
  });
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
    };
    apply();
    media.addEventListener("change", apply);
    localStorage.setItem("dui.theme", theme);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  function toggle() {
    localStorage.setItem("dui.rail", collapsed ? "expanded" : "collapsed");
    setCollapsed(!collapsed);
  }
  const ThemeIcon = { system: Monitor, light: Sun, dark: Moon }[theme];
  const CollapseIcon = collapsed ? PanelLeftOpen : PanelLeftClose;
  return (
    <nav
      className={"rail" + (collapsed ? " collapsed" : "")}
      aria-label="Workspace"
    >
      <strong className="wordmark" aria-label="dui">
        dui
        <span className="brand-period" aria-hidden="true">
          .
        </span>
      </strong>
      <button
        type="button"
        aria-label="Select workspace"
        className="workspace-switch"
        title={
          run
            ? [run.project, run.branch, run.version].filter(Boolean).join(" / ")
            : "Select workspace"
        }
        onClick={open}
      >
        <FolderClosed aria-hidden="true" />
        <span className="rail-text">
          <strong>{run?.project ?? "Select workspace"}</strong>
          {run && (
            <small>
              {[run.branch, run.version.split("-").at(-1)]
                .filter(Boolean)
                .join(" · ")}
            </small>
          )}
        </span>
        <ChevronsUpDown className="rail-disclosure" aria-hidden="true" />
      </button>
      <label className="rail-theme" title={`Theme: ${theme}`}>
        <ThemeIcon aria-hidden="true" />
        <select
          aria-label="Theme"
          value={theme}
          onChange={(e) => setTheme(e.target.value as Theme)}
        >
          <option value="system">System theme</option>
          <option value="light">Light theme</option>
          <option value="dark">Dark theme</option>
        </select>
        <ChevronDown className="rail-disclosure" aria-hidden="true" />
      </label>
      <button
        type="button"
        aria-label={collapsed ? "Expand rail" : "Collapse rail"}
        title={collapsed ? "Expand rail" : "Collapse rail"}
        aria-expanded={!collapsed}
        onClick={toggle}
      >
        <CollapseIcon aria-hidden="true" />
        <span className="rail-text">Collapse rail</span>
      </button>
    </nav>
  );
}
