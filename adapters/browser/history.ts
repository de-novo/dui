(() => {
  interface Cell {
    path: string; state: string; scenario: string; width: number;
    status: string; image: string | null; reason: string | null;
  }
  interface Snapshot {
    projectId: string; projectRoot: string; worktree: string | null; app: string;
    id: string; timestampMs: number; branch: string | null; commit: string | null;
    dirty: boolean | null; output: string; cells: Cell[];
  }
  const element = (id: string) => document.getElementById(id)!;
  const select = (id: string) => element(id) as HTMLSelectElement;
  const project = select("project"), before = select("before"), after = select("after");
  const runs: Snapshot[] = JSON.parse(element("data").textContent!);
  const projectKey = (run: Snapshot) => JSON.stringify([run.projectId, run.app]);
  const file = (dir: string, name: string) =>
    "file://" + [...dir.split("/"), name].map(encodeURIComponent).join("/");
  const selected = (input: HTMLSelectElement) => runs.find(r => r.id === input.value && projectKey(r) === project.value);
  function cells(run?: Snapshot) {
    return new Map(run?.cells.map(c => [JSON.stringify([c.path, c.state, c.width]), c]));
  }
  function figure(run?: Snapshot, cell?: Cell) {
    const el = document.createElement("figure");
    const caption = document.createElement("figcaption");
    caption.textContent = cell ? [cell.status, cell.reason].filter(Boolean).join(" · ") : "해당 칸 없음";
    el.append(caption);
    if (run && cell?.image && /^[\da-f]{64}-\d+\.png$/.test(cell.image)) {
      const a = document.createElement("a"), img = document.createElement("img");
      a.href = img.src = file(run.output, cell.image);
      img.alt = cell.path + " · " + cell.width + "px";
      img.loading = "lazy"; a.append(img); el.append(a);
    }
    return el;
  }
  function render() {
    const base = selected(before), current = selected(after);
    const left = cells(base), right = cells(current);
    const keys = new Set([...left.keys(), ...right.keys()]);
    const grid = element("cells"), links = element("links");
    grid.replaceChildren(); links.replaceChildren();
    for (const [run, label] of [[base, "이전 갤러리"], [current, "현재 갤러리"]] as const) {
      if (!run) continue;
      const link = document.createElement("a"); link.href = file(run.output, "graph.html");
      link.textContent = label; links.append(link);
    }
    for (const key of keys) {
      const l = left.get(key), r = right.get(key), cell = (r ?? l)!;
      const change = !base ? "" : !l ? "추가" : !r ? "삭제" : !l.image || !r.image ? "이미지 비교 불가" :
        l.image === r.image ? "동일 이미지" : "이미지 변경";
      const card = document.createElement("article"), title = document.createElement("h2");
      title.textContent = [cell.path, cell.scenario, cell.width + "px", change].filter(Boolean).join(" · ");
      const pair = document.createElement("div"); pair.className = "pair";
      if (base) pair.append(figure(base, l));
      pair.append(figure(current, r)); card.append(title, pair); grid.append(card);
    }
    element("summary").textContent = current ? keys.size + "칸" : "저장된 캡처가 없습니다.";
  }
  function choose() {
    before.replaceChildren(new Option("선택 안 함", "")); after.replaceChildren();
    const scoped = runs.filter(r => projectKey(r) === project.value);
    for (const input of [before, after]) {
      const groups = new Map<string, HTMLOptGroupElement>();
      for (const run of scoped) {
        const branch = run.branch ?? (run.worktree ? "detached HEAD" : "Git 없음");
        const location = [branch, run.worktree].filter(Boolean).join(" · ");
        let group = groups.get(location);
        if (!group) {
          group = document.createElement("optgroup");
          group.label = location;
          groups.set(location, group);
          input.append(group);
        }
        const label = [branch, run.commit?.slice(0, 8), run.dirty ? "미커밋 변경" : "",
          new Date(run.timestampMs).toLocaleString()].filter(Boolean).join(" · ");
        group.append(new Option(label, run.id));
      }
    }
    if (scoped.length > 1) before.value = scoped[1].id;
    render();
  }
  const projects = new Map(runs.map(r => [projectKey(r), r.projectRoot + (r.app ? "/" + r.app : "")]));
  for (const [id, label] of projects) project.add(new Option(label, id));
  project.onchange = choose; before.onchange = after.onchange = render;
  choose();
})();
