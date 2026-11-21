import {
  ChevronDown,
  ChevronsUpDown,
  FolderClosed,
  Globe,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Sun,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { Run } from "../../data/run";
import { useI18n } from "../../i18n/LocaleContext";
import { UI_LOCALE_OPTIONS, type UiLocale } from "../../i18n/messages";

type Theme = "system" | "light" | "dark";
export function WorkspaceRail({ run, open }: { run?: Run; open: () => void }) {
  const { locale, setLocale, t } = useI18n();
  const [collapsed, setCollapsed] = useState(
    () =>
      localStorage.getItem("dui.rail") === "collapsed" ||
      (localStorage.getItem("dui.rail") === null && matchMedia("(max-width: 600px)").matches),
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
    <nav className={"rail" + (collapsed ? " collapsed" : "")} aria-label={t.workspace}>
      <strong className="wordmark" aria-label="dui">
        dui
        <span className="brand-period" aria-hidden="true">
          .
        </span>
      </strong>
      <button
        type="button"
        aria-label={t.selectWorkspace}
        className="workspace-switch"
        title={
          run
            ? [run.project, run.branch, run.version].filter(Boolean).join(" / ")
            : t.selectWorkspace
        }
        onClick={open}
      >
        <FolderClosed aria-hidden="true" />
        <span className="rail-text">
          <strong>{run?.project ?? t.selectWorkspace}</strong>
          {run && (
            <small>{[run.branch, run.version.split("-").at(-1)].filter(Boolean).join(" · ")}</small>
          )}
        </span>
        <ChevronsUpDown className="rail-disclosure" aria-hidden="true" />
      </button>
      <label className="rail-theme" title={`${t.language}: ${locale}`}>
        <Globe aria-hidden="true" />
        <select
          aria-label={t.language}
          value={locale}
          onChange={(e) => setLocale(e.target.value as UiLocale)}
        >
          {UI_LOCALE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
        <ChevronDown className="rail-disclosure" aria-hidden="true" />
      </label>
      <label className="rail-theme" title={`${t.theme}: ${theme}`}>
        <ThemeIcon aria-hidden="true" />
        <select
          aria-label={t.theme}
          value={theme}
          onChange={(e) => setTheme(e.target.value as Theme)}
        >
          <option value="system">{t.themeSystem}</option>
          <option value="light">{t.themeLight}</option>
          <option value="dark">{t.themeDark}</option>
        </select>
        <ChevronDown className="rail-disclosure" aria-hidden="true" />
      </label>
      <button
        type="button"
        aria-label={collapsed ? t.expandRail : t.collapseRail}
        title={collapsed ? t.expandRail : t.collapseRail}
        aria-expanded={!collapsed}
        onClick={toggle}
      >
        <CollapseIcon aria-hidden="true" />
        <span className="rail-text">{t.collapseRail}</span>
      </button>
    </nav>
  );
}
