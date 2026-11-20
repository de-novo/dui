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
  return (
    <nav
      className={"rail" + (collapsed ? " collapsed" : "")}
      aria-label="Workspace"
    >
      <strong className="wordmark" aria-label="dui">
        {collapsed ? "d" : "dui"}
      </strong>
      <button
        type="button"
        aria-label="Select workspace"
        title="Select workspace"
        onClick={open}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M3 6h6l2 2h10v11H3z" />
        </svg>
        <span>
          {run?.project ?? "Select workspace"}
          {run && (
            <small>
              {run.branch ?? "Branch not recorded"} · {run.version}
            </small>
          )}
        </span>
      </button>
      <label className="rail-theme" title="Theme">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="8" />
          <path d="M12 4v16a8 8 0 0 0 0-16" fill="currentColor" />
        </svg>
        <span>Theme</span>
        <select
          aria-label="Theme"
          value={theme}
          onChange={(e) => setTheme(e.target.value as Theme)}
        >
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>
      <button
        type="button"
        aria-label={collapsed ? "Expand rail" : "Collapse rail"}
        title={collapsed ? "Expand rail" : "Collapse rail"}
        aria-expanded={!collapsed}
        onClick={toggle}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M9 4v16" />
        </svg>
        <span>Collapse rail</span>
      </button>
    </nav>
  );
}
