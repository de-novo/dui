import { createRoot } from "react-dom/client";
import { choose as useChoice } from "./hooks";
import { z } from "zod";
const Mode = z.enum(["list", "grid"]);
function Catalog() {
  const [mode] = useChoice<z.infer<typeof Mode>>("list");
  const [expanded] = useChoice(false);
  return (
    <main data-testid="catalog">
      <h1>Catalog</h1>
      {mode === "grid" ? <section>Tiles</section> : <section>List</section>}
      {expanded && <aside>Details</aside>}
    </main>
  );
}
createRoot(document.getElementById("app")!).render(<Catalog />);
