import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { decodeOverview, type ScreenMap as Archive } from "./data/overview";
import { ScreenMap } from "./features/map/ScreenMap";
import "./preview.css";

function Preview({ archive }: { archive: Archive }) {
  const widths = [
    ...new Set(archive.screens.flatMap((s) => s.cells.map((c) => c.width))),
  ].sort((a, b) => b - a);
  const [search, setSearch] = useState(location.search);
  const dialog = useRef<HTMLDialogElement>(null);
  const params = new URLSearchParams(search);
  const width = Number(params.get("width") ?? widths[0]);
  const mode = params.get("links") ?? "overview";
  const selectedId = params.get("screen");
  const selected = archive.screens.find((s) => s.id === selectedId);
  const cell = selected?.cells.find((c) => c.width === width);
  useEffect(() => {
    const changed = () => setSearch(location.search);
    window.addEventListener("popstate", changed);
    return () => window.removeEventListener("popstate", changed);
  }, []);
  useEffect(() => {
    if (selectedId !== null && !dialog.current?.open)
      dialog.current?.showModal();
    if (selectedId === null && dialog.current?.open) dialog.current.close();
  }, [selectedId]);
  function navigate(id: string | null, size: number, links = mode) {
    const next = new URL(location.href);
    next.searchParams.set("width", String(size));
    if (links === "overview") next.searchParams.delete("links");
    else next.searchParams.set("links", links);
    if (id === null) next.searchParams.delete("screen");
    else next.searchParams.set("screen", id);
    history.pushState(null, "", next);
    setSearch(location.search);
  }
  const sizes = widths.map((w) => (
    <option key={w} value={w}>
      {w} px
    </option>
  ));
  return (
    <main>
      <header>
        <h1>Screen map</h1>
        <label>
          Connections{" "}
          <select
            value={mode}
            onChange={(e) => navigate(null, width, e.target.value)}
          >
            <option value="overview">Overview</option>
            <option value="all">All links</option>
            <optgroup label="Links to and from a screen">
              {archive.screens.map((s) => (
                <option key={s.id} value={"focus:" + s.id}>
                  {s.path}
                </option>
              ))}
            </optgroup>
          </select>
        </label>
        <label>
          Capture width{" "}
          <select
            value={width}
            onChange={(e) => navigate(null, Number(e.target.value))}
          >
            {sizes}
          </select>
        </label>
      </header>
      <div className="workspace">
        <nav aria-label="Screens">
          {archive.screens.map((s) => (
            <button key={s.id} onClick={() => navigate(s.id, width)}>
              {s.path}
            </button>
          ))}
        </nav>
        <section className="map" aria-label="Screen relationships">
          <ScreenMap
            archive={archive}
            width={width}
            mode={mode}
            select={(id) => navigate(id, width)}
          />
        </section>
      </div>
      <dialog
        ref={dialog}
        aria-labelledby="capture-title"
        onCancel={(e) => {
          e.preventDefault();
          navigate(null, width);
        }}
      >
        <header>
          <h2 id="capture-title">{selected?.path ?? "Screen unavailable"}</h2>
          <button onClick={() => navigate(null, width)}>Close</button>
        </header>
        <label>
          Capture width{" "}
          <select
            value={width}
            onChange={(e) => navigate(selectedId, Number(e.target.value))}
          >
            {sizes}
          </select>
        </label>
        <p>
          {cell
            ? [cell.status, cell.reason].filter(Boolean).join(" · ")
            : "No capture at this width"}
        </p>
        {cell?.image && (
          <img
            alt={"Archived screen at " + width + " px"}
            src={"/archive/" + encodeURIComponent(cell.image)}
          />
        )}
      </dialog>
    </main>
  );
}

const root = createRoot(document.getElementById("root")!);
try {
  const response = await fetch("/archive/graph.json");
  if (!response.ok) throw Error("Could not load archive");
  root.render(<Preview archive={decodeOverview(await response.json())} />);
} catch (error) {
  root.render(<p role="alert">{String(error)}</p>);
}
