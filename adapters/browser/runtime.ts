type SlotValues = Record<string, unknown>;
type Setter = (update: () => unknown) => void;
type Baseline = { slots: SlotValues; semantic: { hash: string } };
type Check = {
  hit?: string;
  shown?: string;
  hidden?: string;
  selector?: string;
  present?: boolean;
  absentWhen?: { slot: string; value: unknown };
};

declare var __duiProbe: {
  input: SlotValues;
  slots: SlotValues;
  hits: Set<string>;
  waitForSample(): Promise<void>;
  branch(id: string, value: unknown): unknown;
  apply(values: SlotValues): boolean;
  slot(id: string, value: unknown, set?: Setter): void;
};

(() => {
  function install(input: SlotValues) {
    const slots = Object.create(null);
    const hits = new Set<string>();
    const setters = new Map<string, Set<Setter>>();
    function recordable(value: unknown): boolean {
      return (
        value === null ||
        ["string", "boolean", "number"].includes(typeof value) ||
        (Array.isArray(value) && value.every(recordable))
      );
    }
    const timer = globalThis.setTimeout.bind(globalThis);
    globalThis.__duiProbe = {
      waitForSample: () => new Promise((resolve) => timer(resolve, 100)),
      input,
      slots,
      hits,
      branch(id, value) {
        hits.add(id + ":" + Boolean(value));
        return value;
      },
      apply(values) {
        for (const id of Object.keys(values)) {
          if (setters.get(id)?.size !== 1) return false;
        }
        for (const [id, value] of Object.entries(values)) {
          const [set] = setters.get(id)!;
          set(() => value);
        }
        return true;
      },
      slot(id, value, set) {
        if (typeof set === "function") {
          if (!setters.has(id)) setters.set(id, new Set());
          setters.get(id)!.add(set);
        }
        if (recordable(value)) slots[id] = value;
      },
    };
  }
  function check(check: Check) {
    const probe = globalThis.__duiProbe;
    if (check.hit && !probe.hits.has(check.hit)) return false;
    if (check.shown && !document.body.innerText.includes(check.shown)) return false;
    if (check.hidden && document.body.innerText.includes(check.hidden)) return false;
    if (!check.selector) return true;
    let present = check.present;
    const mask = check.absentWhen;
    if (mask) {
      if (!mask.slot || !Object.hasOwn(probe.slots, mask.slot)) return false;
      if (probe.slots[mask.slot] === mask.value) present = false;
    }
    return Boolean(document.querySelector(check.selector)) === present;
  }
  async function semantic() {
    const features: unknown[][] = [];
    function walk(node: Node) {
      if (node.nodeType === Node.TEXT_NODE) {
        const text = (node.textContent ?? "").replace(/\s+/g, " ").trim();
        if (text) {
          const range = new Range();
          range.selectNodeContents(node);
          if (range.getClientRects().length) features.push(["text", text]);
        }
        return;
      }
      if (
        !(node instanceof Element) ||
        node.matches("script,style,template,[hidden],[aria-hidden=true]")
      )
        return;
      const style = getComputedStyle(node);
      if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0")
        return;
      const tag = node.localName;
      const role = node.getAttribute("role");
      const attrs: [string, string | string[]][] = [...node.attributes]
        .filter(
          (a) =>
            a.name.startsWith("aria-") &&
            ![
              "aria-controls",
              "aria-labelledby",
              "aria-describedby",
              "aria-owns",
              "aria-activedescendant",
            ].includes(a.name),
        )
        .map((a): [string, string] => [a.name, a.value])
        .sort();
      for (const attr of ["aria-labelledby", "aria-describedby"]) {
        if (node.hasAttribute(attr))
          attrs.push([
            attr,
            node
              .getAttribute(attr)!
              .trim()
              .split(/\s+/)
              .map((id) => document.getElementById(id)?.textContent?.trim() ?? ""),
          ]);
      }
      if (
        node.checkVisibility() &&
        (role ||
          attrs.length ||
          /^(button|input|select|textarea|a|img|h[1-6]|dialog|option)$/.test(tag))
      ) {
        features.push([
          tag,
          role,
          attrs,
          node instanceof HTMLImageElement ? node.alt : null,
          node instanceof HTMLElement ? node.title || null : null,
          "disabled" in node ? node.disabled : null,
          node instanceof HTMLInputElement ? node.checked : null,
          (node instanceof HTMLInputElement ||
            node instanceof HTMLTextAreaElement ||
            node instanceof HTMLSelectElement) &&
          node.type !== "password"
            ? node.value
            : null,
          node.matches("img") ? node.getAttribute("src") : null,
        ]);
      }
      for (const child of node.childNodes) walk(child);
    }
    walk(document.body);
    const digest = await crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(JSON.stringify(features)),
    );
    return {
      count: features.length,
      hash: Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join(""),
    };
  }
  async function reference(
    inputs: SlotValues,
    baseline: Baseline,
    selector: string,
    checks: Check[],
  ) {
    const probe = globalThis.__duiProbe;
    const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
    const keys = Object.keys(inputs);
    const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));
    const valuesMatch = (values: SlotValues) =>
      Object.entries(values).every(
        ([id, value]) => Object.hasOwn(probe.slots, id) && same(probe.slots[id], value),
      );
    async function settle(values: SlotValues) {
      let previous;
      for (let poll = 0; poll < 12; poll++) {
        await wait(45);
        const snapshot = await semantic();
        if (snapshot.count > 0 && previous === snapshot.hash && valuesMatch(values))
          return snapshot;
        previous = snapshot.hash;
      }
      return null;
    }
    // Restore all observable baseline slots, even when an effect updated a dependent field.
    if (!valuesMatch(baseline.slots) || (await semantic()).hash !== baseline.semantic.hash)
      return { restore: false, reason: "dirty-control" };
    if (!keys.every((k) => Object.hasOwn(baseline.slots, k)))
      return { restore: true, reason: "not-mounted" };
    const changed = Object.fromEntries(
      Object.entries(inputs).filter(([k, v]) => !same(probe.slots[k], v)),
    );
    if (!probe.apply(changed)) return { restore: true, reason: "no-unique-setter" };
    const observed = await settle(inputs);
    const evidence = {
      semantic: observed,
      ready: !!document.querySelector(selector),
      path: location.pathname + location.search + location.hash,
      width: innerWidth,
      slots: Object.fromEntries(keys.map((k) => [k, probe.slots[k]])),
      assertions: checks.map(check),
      applied: true,
      restore: false,
    };
    const resets = Object.fromEntries(
      Object.entries(baseline.slots).filter(([k, v]) => !same(v, probe.slots[k])),
    );
    const reset = probe.apply(resets) && (await settle(baseline.slots));
    evidence.restore = !!reset && reset.hash === baseline.semantic.hash;
    return evidence;
  }
  async function restore(baseline: Baseline) {
    const p = globalThis.__duiProbe;
    const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
    const changes = Object.fromEntries(
      Object.entries(baseline.slots).filter(([k, v]) => !equal(p.slots[k], v)),
    );
    if (!p.apply(changes)) return false;
    let stable = 0;
    for (let i = 0; i < 14; i++) {
      await new Promise((r) => setTimeout(r, 45));
      const valid =
        Object.entries(baseline.slots).every(
          ([k, v]) => Object.hasOwn(p.slots, k) && equal(p.slots[k], v),
        ) && (await semantic()).hash === baseline.semantic.hash;
      stable = valid ? stable + 1 : 0;
      if (stable >= 2) return true;
    }
    return false;
  }

  async function apply(input: SlotValues, baseline: Baseline) {
    const p = globalThis.__duiProbe;
    const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
    if (
      (await semantic()).hash !== baseline.semantic.hash ||
      !Object.entries(baseline.slots).every(
        ([k, v]) => Object.hasOwn(p.slots, k) && equal(p.slots[k], v),
      )
    )
      return false;
    p.hits.clear();
    return p.apply(
      Object.fromEntries(
        Object.entries(input).filter(
          ([k, v]) => !Object.hasOwn(p.slots, k) || !equal(v, p.slots[k]),
        ),
      ),
    );
  }
  async function observe(selector: string, delayed: boolean) {
    const p = globalThis.__duiProbe;
    if (delayed) await (p?.waitForSample() ?? new Promise((resolve) => setTimeout(resolve, 100)));
    if (!p) return null;
    return {
      ready: !!document.querySelector(selector),
      slots: p.slots,
      hits: [...p.hits].sort(),
      text: document.body?.innerText?.slice(0, 4000),
      path: location.pathname + location.search + location.hash,
      width: innerWidth,
    };
  }
  Object.assign(globalThis, {
    __duiRuntime: {
      install,
      semantic,
      observe,
      apply,
      restore,
      reference,
      links(selector: string) {
        const paths = new Set<string>();
        for (const link of document.querySelectorAll(selector)) {
          if (
            !(link instanceof HTMLAnchorElement) ||
            !link.hasAttribute("href") ||
            link.hasAttribute("download")
          )
            continue;
          const url = URL.parse(link.href);
          if (
            url &&
            url.protocol === location.protocol &&
            url.origin === location.origin &&
            !url.pathname.startsWith("//")
          ) {
            paths.add(url.pathname + url.search + url.hash);
          }
        }
        return [...paths].sort();
      },
      assertions: (checks: Check[]) => checks.map(check),
    },
  });
})();
