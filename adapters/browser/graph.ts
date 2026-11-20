(() => {
  const svg = document.querySelector<SVGSVGElement>("#svg")!;
  const detail = document.querySelector<HTMLElement>("#detail")!;
  const nextPage = document.querySelector<HTMLButtonElement>("#nextPage")!;
  const prev = document.querySelector<HTMLButtonElement>("#prev")!;
  const summary = document.querySelector<HTMLElement>("#summary")!;
  const image = document.querySelector<HTMLAnchorElement>("#image")!;
  const choose = document.querySelector<HTMLSelectElement>("#choose")!;

  interface GraphNode {
    id: string;
    kind: string;
    label: string;
    observed?: boolean;
    image?: string;
    reason?: string;
  }
  interface Edge {
    source: string;
    target: string;
    kind: string;
  }
  interface ScreenCapture {
    path: string;
    width: number;
    image?: string;
    status: "verified" | "unverified" | "unrenderable";
    reason?: string;
  }
  const graph: { nodes: GraphNode[]; edges: Edge[]; overview: ScreenCapture[] } = JSON.parse(
    document.getElementById("graph-data")!.textContent!,
  );

  const width = document.querySelector<HTMLSelectElement>("#width")!;
  const gallery = document.querySelector<HTMLElement>("#gallery")!;
  const labels = { verified: "검증됨", unverified: "미검증", unrenderable: "렌더 불가" };
  for (const size of [...new Set(graph.overview.map((cell) => cell.width))].sort((a, b) => b - a)) {
    width.add(new Option(size + " px", String(size)));
  }
  function showScreens() {
    gallery.replaceChildren();
    for (const cell of graph.overview.filter((cell) => cell.width === Number(width.value))) {
      const figure = document.createElement("figure");
      const title = document.createElement("figcaption");
      title.textContent = [cell.path, labels[cell.status], cell.reason].filter(Boolean).join(" · ");
      figure.append(title);
      if (cell.image) {
        const link = document.createElement("a");
        link.href = cell.image;
        const thumbnail = document.createElement("img");
        thumbnail.loading = "lazy";
        thumbnail.src = cell.image;
        thumbnail.alt = cell.path;
        link.append(thumbnail);
        figure.append(link);
      }
      gallery.append(figure);
    }
  }
  width.onchange = showScreens;
  showScreens();

  const nodes = new Map(graph.nodes.map((n) => [n.id, n]));
  const ns = "http://www.w3.org/2000/svg";
  function element<K extends keyof SVGElementTagNameMap>(
    name: K,
    attrs: Record<string, string | number>,
    text?: string,
  ) {
    const el = document.createElementNS(ns, name);
    for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
    if (text != null) el.textContent = text;
    return el;
  }
  for (const node of graph.nodes) {
    const op = document.createElement("option");
    op.value = node.id;
    op.textContent = node.kind + " · " + node.label + " · " + node.id;
    choose.append(op);
  }
  function show(id: string, page = 0) {
    const node = nodes.get(id)!;
    choose.value = id;
    svg.replaceChildren();
    const links = graph.edges.filter((e) => e.source === id || e.target === id);
    const slice = links.slice(page * 12, (page + 1) * 12),
      children = slice.filter((e) => e.source === id),
      parents = slice.filter((e) => e.target === id);
    prev.hidden = !page;
    nextPage.hidden = (page + 1) * 12 >= links.length;
    prev.onclick = () => show(id, page - 1);
    nextPage.onclick = () => show(id, page + 1);
    detail.textContent =
      node.reason ??
      (links.length > 12
        ? page * 12 + 1 + "–" + Math.min(links.length, (page + 1) * 12) + " / " + links.length
        : "");
    summary.textContent =
      graph.nodes.length +
      "개 노드 · " +
      graph.edges.length +
      "개 간선 · 선택한 노드의 연결 " +
      links.length +
      "개";
    image.textContent = node.image ? "캡처 열기" : "";
    if (node.image) image.href = node.image;
    else image.removeAttribute("href");
    const rows = Math.max(parents.length, children.length, 1),
      height = rows * 50 + 48;
    svg.setAttribute("viewBox", "0 0 980 " + height);
    function box(n: GraphNode, x: number, y: number) {
      const g = element("g", { tabindex: 0, role: "button" });
      const color = n.observed === false ? "#f3f1eb" : n.kind === "cell" ? "#f4e5cd" : "#d8eeeb";
      g.append(
        element("rect", {
          x,
          y,
          width: 245,
          height: 32,
          rx: 4,
          fill: color,
          stroke: "#95aaa9",
        }),
        element("title", {}, n.id),
        element("text", { x: x + 8, y: y + 21 }, String(n.label).slice(0, 28)),
      );
      g.onclick = () => show(n.id);
      g.onkeydown = (e) => {
        if (e.key === "Enter") show(n.id);
      };
      svg.append(g);
    }
    const mid = height / 2 - 16;
    function side(es: Edge[], left: boolean) {
      es.forEach((e, i) => {
        const y = i * 50 + 10;
        const start = left ? 250 : 735,
          end = left ? 365 : 610;
        svg.append(
          element("path", {
            d: "M" + start + " " + (y + 16) + " L" + end + " " + (mid + 16),
            stroke: "#657e81",
            fill: "none",
          }),
          element("text", { x: left ? 255 : 646, y: y + 13 }, e.kind),
        );
        box(nodes.get(left ? e.source : e.target)!, left ? 5 : 730, y);
      });
    }
    side(parents, true);
    side(children, false);
    box(node, 365, mid);
  }
  choose.onchange = () => show(choose.value);
  if (nodes.size) show(graph.nodes.find((n) => n.kind === "screen")?.id ?? graph.nodes[0].id);
})();
