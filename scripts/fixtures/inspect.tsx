import { useState } from "react";
import { createRoot } from "react-dom/client";

function Menu({ initial }: { initial: boolean }) {
  const [open, setOpen] = useState(initial);
  const [mode, setMode] = useState<"view" | "edit">("view");
  const [metadata] = useState<unknown>(null);
  return (
    <section>
      <button onClick={() => setOpen(!open)}>{open ? "Close" : "Open"}</button>
      <button onClick={() => setMode(mode === "view" ? "edit" : "view")}>{mode}</button>
      <output>{String(metadata)}</output>
    </section>
  );
}

createRoot(document.getElementById("app")!).render(
  <main aria-label="Notebook">
    <Menu initial={false} />
    <Menu initial={true} />
  </main>,
);
