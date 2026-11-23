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
  instances: Record<string, Record<string, { value?: unknown; setter: boolean }>>;
  commit(id: string, instance: string, value: unknown, setter: unknown): () => void;
  input: SlotValues;
  slots: SlotValues;
  hits: Set<string>;
  waitForSample(): Promise<void>;
  branch(id: string, value: unknown): unknown;
  edit(id: string, instance: string, value: unknown): boolean;
  apply(values: SlotValues): boolean;
  slot(id: string, value: unknown, set?: Setter): void;
};

(() => {
  function install(input: SlotValues) {
    const slots = Object.create(null);
    const targets = new Map<string, Setter>();
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
      instances: Object.create(null),
      commit(id, instance, value, setter) {
        const entries = (this.instances[id] ??= Object.create(null));
        const key = JSON.stringify([id, instance]);
        entries[instance] = { setter: typeof setter === "function" };
        if (typeof setter === "function") targets.set(key, setter as Setter);
        if (
          value === null ||
          typeof value === "string" ||
          typeof value === "boolean" ||
          (typeof value === "number" && Number.isFinite(value))
        )
          entries[instance].value = value;
        return () => {
          targets.delete(key);
          delete entries[instance];
        };
      },
      edit(id, instance, value) {
        const setter = targets.get(JSON.stringify([id, instance]));
        if (!setter) return false;
        setter(() => value);
        return true;
      },
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
  function collectRenderTree() {
    const interesting = new Set([
      "h1",
      "h2",
      "h3",
      "h4",
      "h5",
      "h6",
      "a",
      "button",
      "input",
      "select",
      "textarea",
      "img",
      "canvas",
      "svg",
      "section",
      "article",
      "aside",
      "main",
      "nav",
      "header",
      "footer",
      "form",
      "dialog",
      "output",
      "p",
      "li",
      "strong",
      "label",
    ]);
    const interactiveTags = new Set(["a", "button", "input", "select", "textarea", "summary"]);
    const interactiveRoles = new Set([
      "button",
      "link",
      "textbox",
      "checkbox",
      "switch",
      "radio",
      "tab",
      "menuitem",
      "combobox",
    ]);
    const pxInt = (raw: string) => {
      const n = Number.parseFloat(raw);
      return Number.isFinite(n) ? Math.round(n) : 0;
    };
    const isVisuallyHidden = (w: number, h: number, cs: CSSStyleDeclaration) => {
      if (cs.display === "none" || cs.visibility === "hidden" || cs.opacity === "0") return true;
      if (
        w <= 1 &&
        h <= 1 &&
        (cs.overflow === "hidden" ||
          cs.overflow === "clip" ||
          cs.clip === "rect(0px, 0px, 0px, 0px)" ||
          cs.clipPath === "inset(50%)")
      ) {
        return true;
      }
      return false;
    };
    const resolveAccessibleLabel = (el: Element): string => {
      const labelledBy = el.getAttribute("aria-labelledby");
      if (labelledBy) {
        const parts = labelledBy
          .split(/\s+/)
          .map((refId) => document.getElementById(refId)?.textContent?.trim() ?? "")
          .filter(Boolean);
        if (parts.length > 0) return parts.join(" ").slice(0, 120);
      }
      if (
        el instanceof HTMLInputElement ||
        el instanceof HTMLSelectElement ||
        el instanceof HTMLTextAreaElement
      ) {
        if (el.labels && el.labels.length > 0) {
          const labelText = Array.from(el.labels)
            .map((lbl) => (lbl.innerText || lbl.textContent || "").replace(/\s+/g, " ").trim())
            .filter(Boolean)
            .join(" ");
          if (labelText) return labelText.slice(0, 120);
        }
        if (
          el instanceof HTMLInputElement &&
          ["submit", "button", "reset"].includes((el.type || "").toLowerCase()) &&
          el.value.trim()
        ) {
          return el.value.trim().slice(0, 120);
        }
      }
      const desc = el.querySelector("[aria-label], img[alt], svg title");
      if (desc) {
        const fromDesc =
          desc.getAttribute("aria-label") || desc.getAttribute("alt") || desc.textContent || "";
        const cleaned = fromDesc.replace(/\s+/g, " ").trim();
        if (cleaned) return cleaned.slice(0, 120);
      }
      return "";
    };
    const isHorizontallyClipped = (el: Element, viewportWidth: number): boolean => {
      let cur = el.parentElement;
      while (cur && cur !== document.body && cur !== document.documentElement) {
        const pcs = getComputedStyle(cur);
        const ox = pcs.overflowX;
        if (ox === "auto" || ox === "scroll" || ox === "hidden" || ox === "clip") {
          const pr = cur.getBoundingClientRect();
          if (Math.round(pr.x) >= -2 && Math.round(pr.x + pr.width) <= viewportWidth + 2) {
            return true;
          }
        }
        cur = cur.parentElement;
      }
      return false;
    };
    const tagCounts = new Map<string, number>();
    const elToId = new WeakMap<Element, string>();
    const domParentIds = new WeakMap<Element, string>();
    let nextDomParentId = 0;
    const viewportW = Math.round(window.innerWidth || 0);
    const nodes: {
      id: string;
      tag: string;
      role?: string;
      text: string;
      rect: { x: number; y: number; width: number; height: number };
      style: Record<string, string>;
      parentId?: string;
      boxModel?: {
        margin?: { top: number; right: number; bottom: number; left: number };
        padding?: { top: number; right: number; bottom: number; left: number };
        border?: { top: number; right: number; bottom: number; left: number };
        borderRadius?: string;
        gap?: string;
      };
      attributes?: Record<string, string>;
    }[] = [];
    const root = document.body;
    if (!root) return nodes;
    const all = root.querySelectorAll("*");
    for (const el of all) {
      if (nodes.length >= 200) break;
      if (!(el instanceof HTMLElement || el instanceof SVGElement)) continue;
      const tag = el.tagName.toLowerCase();
      const testId = el.getAttribute("data-testid");
      const role = el.getAttribute("role") ?? undefined;
      const ownText = Array.from(el.childNodes)
        .filter((n) => n.nodeType === Node.TEXT_NODE)
        .map((n) => n.textContent?.trim() ?? "")
        .filter(Boolean)
        .join(" ")
        .slice(0, 120);
      if (!testId && !role && !interesting.has(tag) && !ownText) continue;
      const rect = el.getBoundingClientRect();
      const w = Math.round(rect.width);
      const h = Math.round(rect.height);
      if (w <= 0 || h <= 0) continue;
      const cs = getComputedStyle(el);
      if (isVisuallyHidden(w, h, cs)) continue;
      const idx = tagCounts.get(tag) ?? 0;
      tagCounts.set(tag, idx + 1);
      const domId = el.getAttribute("id");
      const id = testId
        ? `[data-testid=${testId}]`
        : domId
          ? `#${domId}`
          : role
            ? `${tag}[role=${role}]#${idx}`
            : `${tag}#${idx}`;
      elToId.set(el, id);
      let parentId: string | undefined;
      let curParent = el.parentElement;
      while (curParent) {
        const mapped = elToId.get(curParent);
        if (mapped) {
          parentId = mapped;
          break;
        }
        curParent = curParent.parentElement;
      }
      const isInteractiveEl =
        interactiveTags.has(tag) || (role !== undefined && interactiveRoles.has(role));
      let text = ownText;
      if (!text && el instanceof HTMLSelectElement) {
        text = (el.selectedOptions?.[0]?.textContent ?? "")
          .replace(/\s+/g, " ")
          .trim()
          .slice(0, 120);
      } else if (
        !text &&
        el instanceof HTMLElement &&
        (el.children.length === 0 || isInteractiveEl)
      ) {
        text = (el.innerText || el.textContent || "").replace(/\s+/g, " ").trim().slice(0, 120);
      }
      const margin = {
        top: pxInt(cs.marginTop),
        right: pxInt(cs.marginRight),
        bottom: pxInt(cs.marginBottom),
        left: pxInt(cs.marginLeft),
      };
      const padding = {
        top: pxInt(cs.paddingTop),
        right: pxInt(cs.paddingRight),
        bottom: pxInt(cs.paddingBottom),
        left: pxInt(cs.paddingLeft),
      };
      const border = {
        top: pxInt(cs.borderTopWidth),
        right: pxInt(cs.borderRightWidth),
        bottom: pxInt(cs.borderBottomWidth),
        left: pxInt(cs.borderLeftWidth),
      };
      const borderRadius =
        cs.borderRadius && cs.borderRadius !== "0px" ? cs.borderRadius : undefined;
      const gap = cs.gap && cs.gap !== "normal" && cs.gap !== "0px" ? cs.gap : undefined;
      const attributes: Record<string, string> = {};
      for (const attrName of [
        "id",
        "type",
        "name",
        "href",
        "aria-label",
        "aria-labelledby",
        "aria-hidden",
        "aria-disabled",
        "placeholder",
        "data-testid",
        "alt",
        "title",
        "disabled",
      ]) {
        const val = el.getAttribute(attrName);
        if (val !== null && val !== "") {
          attributes[attrName] = val.slice(0, 120);
        }
      }
      if (el.hasAttribute("disabled") && !attributes.disabled) {
        attributes.disabled = "true";
      }
      if (
        isInteractiveEl &&
        !text &&
        !attributes["aria-label"] &&
        !attributes.title &&
        !attributes.placeholder
      ) {
        const resolved = resolveAccessibleLabel(el);
        if (resolved) {
          attributes["aria-label"] = resolved;
        }
      }
      const rx = Math.round(rect.x);
      if (
        viewportW > 0 &&
        (rx < -2 || rx + w > viewportW + 2) &&
        isHorizontallyClipped(el, viewportW)
      ) {
        attributes["data-dui-clipped-x"] = "true";
      }
      if (el.parentElement) {
        let dp = domParentIds.get(el.parentElement);
        if (!dp) {
          dp = `p${nextDomParentId++}`;
          domParentIds.set(el.parentElement, dp);
        }
        attributes["data-dui-dom-parent"] = dp;
      }
      const cls = el.getAttribute("class");
      if (cls && cls.trim()) {
        attributes.class = cls.trim().slice(0, 160);
      }
      const style: Record<string, string> = {
        color: cs.color,
        backgroundColor: cs.backgroundColor,
        fontSize: cs.fontSize,
        fontWeight: cs.fontWeight,
        display: cs.display,
      };
      if (cs.lineHeight && cs.lineHeight !== "normal") style.lineHeight = cs.lineHeight;
      if (cs.fontFamily) style.fontFamily = cs.fontFamily.slice(0, 80);
      if (cs.position && cs.position !== "static") style.position = cs.position;
      if (cs.display.includes("flex")) {
        style.flexDirection = cs.flexDirection;
        if (cs.justifyContent && cs.justifyContent !== "normal") {
          style.justifyContent = cs.justifyContent;
        }
        if (cs.alignItems && cs.alignItems !== "normal") {
          style.alignItems = cs.alignItems;
        }
      }
      nodes.push({
        id,
        tag,
        ...(role ? { role } : {}),
        text,
        rect: { x: rx, y: Math.round(rect.y), width: w, height: h },
        style,
        ...(parentId ? { parentId } : {}),
        boxModel: {
          margin,
          padding,
          border,
          ...(borderRadius ? { borderRadius } : {}),
          ...(gap ? { gap } : {}),
        },
        ...(Object.keys(attributes).length > 0 ? { attributes } : {}),
      });
    }
    return nodes;
  }
  async function observe(selector: string, delayed: boolean) {
    const p = globalThis.__duiProbe;
    if (delayed) await (p?.waitForSample() ?? new Promise((resolve) => setTimeout(resolve, 100)));
    if (!p) return null;
    const selectOptions = Array.from(document.querySelectorAll("select"))
      .map((sel) => ({
        current: sel.value,
        values: Array.from(sel.options)
          .map((o) => o.value)
          .filter((v, idx, arr) => arr.indexOf(v) === idx)
          .slice(0, 6),
      }))
      .filter((entry) => entry.values.length > 1);
    const renderTree = collectRenderTree();
    return {
      ready: !!document.querySelector(selector),
      slots: p.slots,
      instances: p.instances,
      hits: [...p.hits].sort(),
      text: document.body?.innerText?.slice(0, 4000),
      path: location.pathname + location.search + location.hash,
      width: innerWidth,
      ...(selectOptions.length > 0 ? { selectOptions } : {}),
      ...(renderTree.length > 0 ? { renderTree } : {}),
    };
  }
  function visibleText(selector: string) {
    const nodes = document.querySelectorAll(selector);
    const node = nodes.length === 1 ? nodes[0] : undefined;
    return node instanceof HTMLElement &&
      node.checkVisibility({ visibilityProperty: true, opacityProperty: true })
      ? node.innerText.trim()
      : null;
  }
  Object.assign(globalThis, {
    __duiRuntime: {
      install,
      visibleText,
      async liveEdit(
        slot: string,
        instance: string,
        value: unknown,
        selector: string | null,
        text: string | null,
      ) {
        const probe = globalThis.__duiProbe;
        if (selector !== null) {
          try {
            visibleText(selector);
          } catch {
            return { error: "invalid-selector" };
          }
        }
        const before = selector === null ? await semantic() : null;
        if (!probe.edit(slot, instance, value)) return { error: "instance-not-found" };
        if (selector === null || text === null) {
          let observed: unknown;
          let after = before!;
          for (let sample = 0; sample < 10; sample++) {
            await probe.waitForSample();
            observed = probe.instances[slot]?.[instance]?.value;
            after = await semantic();
            if (observed === value && (after.hash !== before!.hash || sample >= 1)) break;
          }
          const reason = observed !== value ? "value-mismatch" : "missing-assertion";
          return {
            status: "unverified",
            reason,
            observedValue: observed,
            evidence: {
              mode: "semantic-diff",
              beforeHash: before!.hash,
              afterHash: after.hash,
              changed: before!.hash !== after.hash,
            },
          };
        }
        let observed: unknown;
        let visible: string | null = null;
        for (let sample = 0; sample < 10; sample++) {
          await probe.waitForSample();
          observed = probe.instances[slot]?.[instance]?.value;
          visible = visibleText(selector);
          if (observed === value && visible === text) break;
        }
        const reason =
          observed !== value ? "value-mismatch" : visible !== text ? "semantic-mismatch" : null;
        return {
          status: reason ? "unverified" : "verified",
          reason,
          observedValue: observed,
          evidence: { selector, expectedText: text, visibleText: visible },
        };
      },
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
      auditLocale() {
        function selectorFor(el: Element): string {
          const testId = el.getAttribute("data-testid");
          if (testId) return `[data-testid=${JSON.stringify(testId)}]`;
          if (el.id) return `#${CSS.escape(el.id)}`;
          const parts: string[] = [];
          let cur: Element | null = el;
          while (cur && cur !== document.body && parts.length < 4) {
            let part = cur.localName;
            const parentEl: Element | null = cur.parentElement;
            if (parentEl) {
              const siblings = [...parentEl.children].filter((c) => c.localName === cur!.localName);
              if (siblings.length > 1) {
                part += `:nth-of-type(${siblings.indexOf(cur) + 1})`;
              }
            }
            parts.unshift(part);
            cur = parentEl;
          }
          return parts.join(" > ");
        }

        const overflows: {
          selector: string;
          text: string;
          scrollWidth: number;
          clientWidth: number;
          scrollHeight: number;
          clientHeight: number;
        }[] = [];
        const seenSelectors = new Set<string>();
        const runtimeMissingKeys = new Set<string>();

        if (document.body) {
          for (const el of document.body.querySelectorAll("*")) {
            if (!(el instanceof HTMLElement)) continue;
            const missingKey = el.getAttribute("data-dui-missing-key");
            if (missingKey) runtimeMissingKeys.add(missingKey);
            if (!el.checkVisibility({ visibilityProperty: true, opacityProperty: true })) continue;
            const hasDirectText = [...el.childNodes].some(
              (n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? "").trim().length > 0,
            );
            if (!hasDirectText) continue;
            const style = getComputedStyle(el);
            const horiz = el.clientWidth > 0 && el.scrollWidth > el.clientWidth + 1;
            const vert =
              el.clientHeight > 0 &&
              el.scrollHeight > el.clientHeight + 1 &&
              (style.overflowY === "hidden" || style.overflowY === "clip");
            if (horiz || vert) {
              const selector = selectorFor(el);
              if (seenSelectors.has(selector)) continue;
              seenSelectors.add(selector);
              overflows.push({
                selector,
                text: el.innerText.trim().slice(0, 120),
                scrollWidth: Math.round(el.scrollWidth),
                clientWidth: Math.round(el.clientWidth),
                scrollHeight: Math.round(el.scrollHeight),
                clientHeight: Math.round(el.clientHeight),
              });
            }
          }
        }

        const docStyle = getComputedStyle(document.documentElement);
        return {
          documentDir: docStyle.direction || "ltr",
          htmlLang: document.documentElement.lang || null,
          overflows,
          runtimeMissingKeys: [...runtimeMissingKeys].sort(),
        };
      },
      assertions: (checks: Check[]) => checks.map(check),
    },
  });
})();
