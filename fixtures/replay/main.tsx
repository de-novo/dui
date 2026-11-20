import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";

function useOptions() {
  const [expanded, setExpanded] = useState(false);
  const [draft, setDraft] = useState(false);
  useEffect(() => { setExpanded(false); }, []);
  return { expanded, draft, setExpanded, setDraft };
}

function Example() {
  const options = useOptions();
  return <main aria-label="Notebook">
    <h1>Notebook</h1>
    {options.expanded ? <section>Meeting notes</section> : <section>Closed</section>}
  </main>;
}

createRoot(document.getElementById("app")!).render(<Example />);
