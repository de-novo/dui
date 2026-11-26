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
  pendingTimers(): number;
  branch(id: string, value: unknown): unknown;
  edit(id: string, instance: string, value: unknown): boolean;
  apply(values: SlotValues): boolean;
  slot(id: string, value: unknown, set?: Setter): void;
  overlay(id: string, open: boolean): void;
  overlays: Set<string>;
};

(() => {
  function install(input: SlotValues) {
    const slots = Object.create(null);
    const targets = new Map<string, Setter>();
    const hits = new Set<string>();
    const setters = new Map<string, Set<Setter>>();
    const overlays = new Set<string>();
    function recordable(value: unknown): boolean {
      return (
        value === null ||
        ["string", "boolean", "number"].includes(typeof value) ||
        (Array.isArray(value) && value.every(recordable))
      );
    }
    const timer = globalThis.setTimeout.bind(globalThis);
    // Short timers the page schedules while it mounts (a delayed render, a deferred fetch) can
    // still change the screen, so settling waits for them. Timers scheduled later, such as a
    // polling loop, do not hold it back.
    const nativeSetTimeout = globalThis.setTimeout as unknown as (
      handler: TimerHandler,
      delay?: number,
      ...args: unknown[]
    ) => unknown;
    const nativeClearTimeout = globalThis.clearTimeout;
    const mounted = performance.now();
    const mountTimers = new Set<unknown>();
    // The callback of a tracked timer while it runs: scheduling itself again is a loop.
    let running: unknown;
    globalThis.setTimeout = function (handler: TimerHandler, delay?: number, ...args: unknown[]) {
      const track =
        typeof handler === "function" &&
        handler !== running &&
        (delay ?? 0) <= 1000 &&
        performance.now() - mounted < 1500;
      let id: unknown;
      const run = track
        ? (...values: unknown[]) => {
            mountTimers.delete(id);
            running = handler;
            try {
              (handler as (...v: unknown[]) => void)(...values);
            } finally {
              running = undefined;
            }
          }
        : handler;
      id = nativeSetTimeout.call(globalThis, run, delay, ...args);
      if (track) mountTimers.add(id);
      return id;
    } as typeof setTimeout;
    globalThis.clearTimeout = function (id?: Parameters<typeof clearTimeout>[0]) {
      mountTimers.delete(id);
      nativeClearTimeout.call(globalThis, id);
    } as typeof clearTimeout;
    // A short interval started while mounting (a feed, a ticker) is waited for until it first
    // runs, so a change it makes is seen before the screen can count as stable.
    const nativeSetInterval = globalThis.setInterval as unknown as (
      handler: TimerHandler,
      delay?: number,
      ...args: unknown[]
    ) => unknown;
    const nativeClearInterval = globalThis.clearInterval;
    globalThis.setInterval = function (handler: TimerHandler, delay?: number, ...args: unknown[]) {
      const track =
        typeof handler === "function" && (delay ?? 0) <= 1000 && performance.now() - mounted < 1500;
      let id: unknown;
      const run = track
        ? (...values: unknown[]) => {
            mountTimers.delete(id);
            (handler as (...v: unknown[]) => void)(...values);
          }
        : handler;
      id = nativeSetInterval.call(globalThis, run, delay, ...args);
      if (track) mountTimers.add(id);
      return id;
    } as typeof setInterval;
    globalThis.clearInterval = function (id?: Parameters<typeof clearInterval>[0]) {
      mountTimers.delete(id);
      nativeClearInterval.call(globalThis, id);
    } as typeof clearInterval;
    globalThis.__duiProbe = {
      pendingTimers: () => mountTimers.size,
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
      waitForSample: () => new Promise((resolve) => timer(resolve, 40)),
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
      overlays,
      // A dialog from the compiler's facts rendered; its open state is a slot dui can set.
      overlay(id, open) {
        overlays.add(id);
        slots[id] = open;
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
  /** Tag, label and rectangle rounded like render-tree nodes, so audits can match the two. */
  function keyboardRecord(el: Element) {
    const r = el.getBoundingClientRect();
    const label = (el.getAttribute("aria-label") || (el as HTMLElement).innerText || "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 60);
    return {
      tag: el.tagName.toLowerCase(),
      ...(el.getAttribute("role") ? { role: el.getAttribute("role")! } : {}),
      label,
      // Page coordinates: focusing a control can scroll it into view, while render-tree nodes are
      // measured at the top of the page.
      rect: {
        x: Math.round(r.x + window.scrollX),
        y: Math.round(r.y + window.scrollY),
        width: Math.round(r.width),
        height: Math.round(r.height),
      },
    };
  }

  function collectRenderTree(
    scopeRoot?: Element | null,
    measureText = false,
    onNode?: (el: Element, id: string) => void,
  ) {
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
      if ("labels" in el) {
        const lbls = (el as HTMLInputElement).labels;
        if (lbls && lbls.length > 0) {
          const labelText = Array.from(lbls)
            .map((lbl) => (lbl.innerText || lbl.textContent || "").replace(/\s+/g, " ").trim())
            .filter(Boolean)
            .join(" ");
          if (labelText) return labelText.slice(0, 120);
        }
      }
      if (el.id) {
        try {
          const linkedLabels = document.querySelectorAll(`label[for="${CSS.escape(el.id)}"]`);
          if (linkedLabels.length > 0) {
            const labelText = Array.from(linkedLabels)
              .map((lbl) => (lbl.textContent || "").replace(/\s+/g, " ").trim())
              .filter(Boolean)
              .join(" ");
            if (labelText) return labelText.slice(0, 120);
          }
        } catch {
          // Ignore invalid selector edge cases.
        }
      }
      const wrappingLabel = el.closest("label");
      if (wrappingLabel) {
        const labelText = (wrappingLabel.innerText || wrappingLabel.textContent || "")
          .replace(/\s+/g, " ")
          .trim();
        if (labelText) return labelText.slice(0, 120);
      }
      if (
        el instanceof HTMLInputElement &&
        ["submit", "button", "reset"].includes((el.type || "").toLowerCase()) &&
        el.value.trim()
      ) {
        return el.value.trim().slice(0, 120);
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
    const tagCounts = new Map<string, number>();
    const elToId = new WeakMap<Element, string>();
    const domParentIds = new WeakMap<Element, string>();
    let nextDomParentId = 0;
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
    const root = scopeRoot ?? document.body;
    if (!root) return nodes;
    const all: Element[] = scopeRoot
      ? [scopeRoot, ...Array.from(root.querySelectorAll("*"))]
      : Array.from(root.querySelectorAll("*"));
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
      onNode?.(el, id);
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
      const textLines = new Set<number>();
      if (measureText) {
        const range = document.createRange();
        for (const child of el.childNodes) {
          if (child.nodeType !== Node.TEXT_NODE || !child.textContent?.trim()) continue;
          range.selectNodeContents(child);
          for (const rect of range.getClientRects()) textLines.add(rect.top);
        }
      }
      nodes.push({
        ...(measureText ? { textLines: textLines.size } : {}),
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
  function collectScreenLinks(paths: Set<string>, selector: string) {
    const asset =
      /\.(?:json|xml|rss|atom|pdf|zip|svg|png|jpe?g|webp|gif|csv|txt|ico|woff2?|avif|mp[34]|css|m?js)$/i;
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
        !url.pathname.startsWith("//") &&
        !asset.test(url.pathname)
      ) {
        paths.add(url.pathname + url.search + url.hash);
      }
    }
  }

  function visibleDialogs(): HTMLElement[] {
    const found: HTMLElement[] = [];
    for (const el of document.querySelectorAll(
      '[role="dialog"], [role="alertdialog"], dialog[open], [aria-modal="true"], [popover]:popover-open',
    )) {
      if (!(el instanceof HTMLElement)) continue;
      const cs = getComputedStyle(el);
      if (cs.display === "none" || cs.visibility === "hidden" || cs.opacity === "0") continue;
      const r = el.getBoundingClientRect();
      const isExplicitModal = el.matches('dialog[open], [role="alertdialog"], [aria-modal="true"]');
      if (isExplicitModal) {
        if (r.width >= 120 && r.height >= 80) found.push(el);
        continue;
      }
      // Non-modal dialogs/popovers: skip tiny filter dropdowns/listboxes without headings or forms.
      if (r.width < 260 || r.height < 160) continue;
      const hasListboxOrMenu = Boolean(el.querySelector('[role="listbox"], [role="menu"]'));
      const hasHeadingOrForm = Boolean(el.querySelector('h1, h2, h3, h4, [role="heading"], form'));
      if (hasListboxOrMenu && !hasHeadingOrForm) continue;
      found.push(el);
    }
    return found;
  }

  /** Title and slug of an open dialog: its label, its first heading, or `fallback`. */
  function dialogTitle(dialog: HTMLElement, fallback: string) {
    let title = (dialog.getAttribute("aria-label") || "").trim();
    if (!title) {
      const lby = dialog.getAttribute("aria-labelledby");
      if (lby) {
        title = lby
          .split(/\s+/)
          .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
          .filter(Boolean)
          .join(" ");
      }
    }
    if (!title) {
      const heading = dialog.querySelector('h1, h2, h3, h4, [role="heading"]');
      if (heading) title = (heading.textContent || "").replace(/\s+/g, " ").trim();
    }
    title = (title || fallback).slice(0, 80);
    const slug = title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 40);
    return { title, slug };
  }

  /**
   * A self-contained, script-free copy of the page as rendered now (see "HTML cards" in
   * docs/en/web-viewer.md): the live DOM with open shadow roots, the parsed CSS rules, form
   * state, and same-origin images, fonts and readable canvases inlined as data: URLs.
   * Live elements get temporary attributes for state that only exists as properties; they are
   * removed again before returning.
   */
  async function htmlSnapshot(width: number) {
    const omitted: { resource: string; reason: string }[] = [];
    const MAX_ASSET = 3 * 1024 * 1024;
    const MAX_TOTAL = 24 * 1024 * 1024;
    let inlinedBytes = 0;
    const dataUrls = new Map<string, Promise<string | null>>();
    const toDataUrl = (raw: string) => {
      let url: URL;
      try {
        url = new URL(raw, document.baseURI);
      } catch {
        return Promise.resolve(null);
      }
      if (url.protocol === "data:") return Promise.resolve(url.href);
      if (url.origin !== location.origin) {
        omitted.push({ resource: url.href, reason: "cross-origin" });
        return Promise.resolve(null);
      }
      const key = url.href;
      if (!dataUrls.has(key))
        dataUrls.set(
          key,
          (async () => {
            try {
              const response = await fetch(key);
              if (!response.ok) throw Error("HTTP " + response.status);
              const blob = await response.blob();
              if (blob.size > MAX_ASSET || inlinedBytes + blob.size > MAX_TOTAL) {
                omitted.push({ resource: key, reason: "too large" });
                return null;
              }
              inlinedBytes += blob.size;
              return await new Promise<string>((done, fail) => {
                const reader = new FileReader();
                reader.onload = () => done(String(reader.result));
                reader.onerror = () => fail(reader.error);
                reader.readAsDataURL(blob);
              });
            } catch (error) {
              omitted.push({ resource: key, reason: String((error as Error)?.message ?? error) });
              return null;
            }
          })(),
        );
      return dataUrls.get(key)!;
    };
    // url(...) references in CSS text, resolved against `base` and inlined.
    const inlineCssUrls = async (css: string, base: string) => {
      const found = [...css.matchAll(/url\(\s*(['"]?)([^'")]+)\1\s*\)/g)];
      let out = css;
      for (const [whole, , ref] of found) {
        if (ref.startsWith("data:") || ref.startsWith("#")) continue;
        let absolute: string;
        try {
          absolute = new URL(ref, base).href;
        } catch {
          continue;
        }
        const data = await toDataUrl(absolute);
        out = out.replace(whole, data ? `url("${data}")` : "none");
      }
      return out;
    };
    const sheetCss = async (sheet: CSSStyleSheet): Promise<string> => {
      let rules: CSSRuleList;
      try {
        rules = sheet.cssRules;
      } catch {
        omitted.push({ resource: sheet.href ?? "style sheet", reason: "cross-origin style sheet" });
        return "";
      }
      const base = sheet.href ?? document.baseURI;
      let text = "";
      for (const rule of Array.from(rules)) {
        if (rule instanceof CSSImportRule) {
          if (rule.styleSheet) text += await sheetCss(rule.styleSheet);
          continue;
        }
        text += (await inlineCssUrls(rule.cssText, base)) + "\n";
      }
      const media = sheet.media?.mediaText;
      return media && media !== "all" ? `@media ${media} {\n${text}}\n` : text;
    };

    const restore: (() => void)[] = [];
    const setAttr = (el: Element, name: string, value: string | null) => {
      const had = el.hasAttribute(name);
      const before = el.getAttribute(name);
      if (value === null) el.removeAttribute(name);
      else el.setAttribute(name, value);
      restore.push(() => (had ? el.setAttribute(name, before!) : el.removeAttribute(name)));
    };
    const roots: (Document | ShadowRoot)[] = [document];
    const shadowRoots: ShadowRoot[] = [];
    for (let i = 0; i < roots.length; i++) {
      for (const el of roots[i]!.querySelectorAll("*")) {
        const root = (el as HTMLElement).shadowRoot;
        if (root) {
          roots.push(root);
          shadowRoots.push(root);
        }
      }
    }
    try {
      for (const root of roots) {
        for (const el of root.querySelectorAll("input, textarea, select option, canvas, img")) {
          if (el instanceof HTMLInputElement) {
            if (el.type === "checkbox" || el.type === "radio")
              setAttr(el, "checked", el.checked ? "" : null);
            else if (el.type !== "password" && el.type !== "file") setAttr(el, "value", el.value);
          } else if (el instanceof HTMLTextAreaElement) {
            setAttr(el, "data-dui-value", el.value);
          } else if (el instanceof HTMLOptionElement) {
            setAttr(el, "selected", el.selected ? "" : null);
          } else if (el instanceof HTMLCanvasElement) {
            try {
              setAttr(el, "data-dui-canvas", el.toDataURL());
            } catch {
              omitted.push({ resource: "canvas", reason: "canvas cannot be read" });
            }
          } else if (el instanceof HTMLImageElement && el.currentSrc) {
            const data = await toDataUrl(el.currentSrc);
            setAttr(el, "data-dui-src", data);
          }
        }
      }
      // Adopted style sheets of shadow roots travel inside their template.
      for (const root of shadowRoots) {
        if (!root.adoptedStyleSheets.length) continue;
        const style = document.createElement("style");
        style.setAttribute("data-dui-adopted", "");
        let css = "";
        for (const sheet of root.adoptedStyleSheets) css += await sheetCss(sheet);
        style.textContent = css;
        root.prepend(style);
        restore.push(() => style.remove());
      }
      let css = "";
      for (const sheet of [...Array.from(document.styleSheets), ...document.adoptedStyleSheets])
        css += await sheetCss(sheet as CSSStyleSheet);
      const html = document.documentElement.getHTML({
        serializableShadowRoots: true,
        shadowRoots,
      });
      const parsed = new DOMParser().parseFromString(
        "<!doctype html><html>" + html.replace(/^<html[^>]*>/, ""),
        "text/html",
      );
      const out = parsed.documentElement;
      for (const attr of Array.from(document.documentElement.attributes))
        out.setAttribute(attr.name, attr.value);
      // Declarative shadow roots are templates here; walk them as well as the document.
      const scopes: ParentNode[] = [parsed];
      for (let i = 0; i < scopes.length; i++)
        for (const t of scopes[i]!.querySelectorAll("template[shadowrootmode]"))
          scopes.push((t as HTMLTemplateElement).content);
      for (const scope of scopes) {
        for (const el of scope.querySelectorAll(
          'script, noscript, link[rel~="stylesheet" i], link[rel~="preload" i], link[rel~="modulepreload" i], link[rel~="prefetch" i], base',
        ))
          el.remove();
        for (const el of scope.querySelectorAll("style:not([data-dui-adopted])"))
          if (scope === parsed) el.remove();
        for (const el of scope.querySelectorAll("iframe, object, embed")) {
          omitted.push({ resource: el.tagName.toLowerCase(), reason: "embedded document" });
          el.replaceWith(parsed.createElement("div"));
        }
        for (const el of scope.querySelectorAll("*")) {
          for (const attr of Array.from(el.attributes))
            if (/^on/i.test(attr.name) || /^\s*javascript:/i.test(attr.value))
              el.removeAttribute(attr.name);
          if (el.hasAttribute("style"))
            el.setAttribute(
              "style",
              await inlineCssUrls(el.getAttribute("style")!, document.baseURI),
            );
        }
        for (const el of scope.querySelectorAll("textarea[data-dui-value]")) {
          el.textContent = el.getAttribute("data-dui-value");
          el.removeAttribute("data-dui-value");
        }
        for (const el of scope.querySelectorAll("canvas[data-dui-canvas]")) {
          const img = parsed.createElement("img");
          img.src = el.getAttribute("data-dui-canvas")!;
          for (const name of ["class", "style", "width", "height", "id"])
            if (el.hasAttribute(name)) img.setAttribute(name, el.getAttribute(name)!);
          el.replaceWith(img);
        }
        for (const el of scope.querySelectorAll("img")) {
          const data = el.getAttribute("data-dui-src");
          el.removeAttribute("data-dui-src");
          el.removeAttribute("srcset");
          el.removeAttribute("loading");
          if (data) el.setAttribute("src", data);
          else if (el.getAttribute("src") && !el.getAttribute("src")!.startsWith("data:"))
            el.removeAttribute("src");
        }
        for (const el of scope.querySelectorAll("a[href]")) {
          try {
            el.setAttribute("href", new URL(el.getAttribute("href")!, document.baseURI).href);
          } catch {
            // Keep the href as written.
          }
        }
      }
      const head = parsed.head;
      if (!head.querySelector("meta[charset]")) {
        const meta = parsed.createElement("meta");
        meta.setAttribute("charset", "utf-8");
        head.prepend(meta);
      }
      const viewport = parsed.createElement("meta");
      viewport.setAttribute("name", "viewport");
      viewport.setAttribute("content", `width=${width}`);
      head.append(viewport);
      const style = parsed.createElement("style");
      style.setAttribute("data-dui-snapshot", "");
      style.textContent = css;
      head.append(style);
      const text = "<!doctype html>\n" + out.outerHTML;
      return { html: text, omitted };
    } finally {
      for (const undo of restore.reverse()) undo();
    }
  }

  /** Resolve as soon as `done()` holds (checked every 16 ms), or after `ms`. */
  async function waitUntil(done: () => boolean, ms: number) {
    const deadline = performance.now() + ms;
    while (!done() && performance.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 16));
    }
    return done();
  }

  function overlayClosed(active: HTMLElement) {
    return (
      !active.isConnected ||
      (active instanceof HTMLDialogElement && !active.open) ||
      !active.checkVisibility({ visibilityProperty: true, opacityProperty: true })
    );
  }

  function isImagePreview(dialog: HTMLElement) {
    if (
      !dialog.querySelector("img") ||
      dialog.querySelector("form, input, select, textarea, a[href], [contenteditable=true]")
    )
      return false;
    const copy = dialog.cloneNode(true) as HTMLElement;
    for (const control of copy.querySelectorAll("button, [role], [tabindex]")) {
      if (control.matches('button, [role="button"]')) {
        const name =
          control.getAttribute("aria-label") ||
          control.getAttribute("title") ||
          control.textContent ||
          "";
        if (
          !/^(?:close|dismiss|next|prev(?:ious)?)(?: (?:image(?: preview)?|preview|dialog))?$|^[×✕x]$/i.test(
            name.trim(),
          )
        )
          return false;
        control.remove();
      } else if (!control.matches('img, [role="img"], [role="presentation"], [role="none"]'))
        return false;
    }
    for (const decoration of copy.querySelectorAll("svg, script, style")) decoration.remove();
    return !copy.textContent?.trim();
  }

  /** Resolves once the page has loaded (styles, images), its fonts are ready and two frames
   * have painted, or after `limit` ms: the first settle sample then sees the finished page, so a
   * page that never changes afterwards settles on the fast path. */
  async function whenLoaded(limit: number) {
    const loaded = (async () => {
      if (document.readyState !== "complete")
        await new Promise((resolve) => addEventListener("load", resolve, { once: true }));
      await document.fonts?.ready;
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    })();
    await Promise.race([loaded, new Promise((resolve) => setTimeout(resolve, limit))]);
    return true;
  }

  async function observe(selector: string, delayed: boolean) {
    const p = globalThis.__duiProbe;
    if (delayed) await (p?.waitForSample() ?? new Promise((resolve) => setTimeout(resolve, 40)));
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
    // Regions that change on their own (busy or progress indicators, timers, endlessly
    // animated spinners) are reported so the settle loop does not wait for them.
    const busyRoots: Element[] = Array.from(
      document.querySelectorAll(
        '[aria-busy="true"], [role="progressbar"], [role="timer"], progress, marquee',
      ),
    );
    for (const animation of document.getAnimations()) {
      const target = (animation.effect as KeyframeEffect | null)?.target;
      if (
        animation.playState === "running" &&
        animation.effect?.getComputedTiming().iterations === Infinity &&
        target instanceof Element
      )
        busyRoots.push(target);
    }
    const volatile: string[] = [];
    const renderTree = collectRenderTree(null, false, (el, id) => {
      if (busyRoots.some((busy) => busy === el || busy.contains(el))) volatile.push(id);
    });
    const root = document.querySelector("#root, #app, #__next");
    const navigation = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
    const path = location.pathname + location.search + location.hash;
    const entryRedirect =
      navigation?.name === location.origin + "/" &&
      navigation.responseStatus === 200 &&
      location.pathname !== "/";
    const canonicalLinks = document.head?.querySelectorAll<HTMLLinkElement>(
      'link[rel~="canonical" i]',
    );
    const declared =
      canonicalLinks?.length === 1 ? canonicalLinks[0].getAttribute("href")?.trim() : undefined;
    const canonical = declared ? URL.parse(declared, document.baseURI) : null;
    const canonicalPath =
      canonical?.origin === location.origin
        ? canonical.pathname + canonical.search + canonical.hash
        : null;
    const matches = document.querySelectorAll(selector);
    return {
      ready: matches.length > 0,
      selectorVisible: Array.from(matches).some(
        (node) =>
          node.checkVisibility({ visibilityProperty: true, opacityProperty: true }) &&
          Array.from(node.getClientRects()).some((rect) => rect.width > 0 && rect.height > 0),
      ),
      documentStatus: navigation?.responseStatus || null,
      canonicalPath,
      ...(root &&
      !root.firstElementChild &&
      !root.textContent?.trim() &&
      !document.body?.innerText.trim()
        ? { emptyMount: true }
        : {}),
      ...(entryRedirect ? { redirectedTo: path, documentOrigin: location.origin } : {}),
      slots: p.slots,
      instances: p.instances,
      hits: [...p.hits].sort(),
      text: document.body?.innerText?.slice(0, 4000),
      path,
      width: innerWidth,
      ...(selectOptions.length > 0 ? { selectOptions } : {}),
      ...(renderTree.length > 0 ? { renderTree } : {}),
      ...(volatile.length > 0 ? { volatile } : {}),
      ...(p.pendingTimers?.() ? { pendingTimers: p.pendingTimers() } : {}),
      ...(p.overlays?.size ? { overlaySlots: [...p.overlays].sort() } : {}),
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
  // Full-page screenshots can hide a classic scrollbar and reflow the page after
  // its rectangles have been measured. Reserve the existing gutter while painting.
  let restoreGutter: (() => void) | undefined;
  // A blinking text caret makes otherwise identical screenshots differ; hide it while painting.
  function hideCaret(hidden: boolean) {
    const id = "__dui-hide-caret";
    document.getElementById(id)?.remove();
    if (!hidden) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = "*, *::before, *::after { caret-color: transparent !important; }";
    document.head?.append(style);
  }
  function setCaptureGutter(enabled: boolean) {
    if (!enabled) {
      restoreGutter?.();
      restoreGutter = undefined;
      return;
    }
    const root = document.documentElement;
    if (root.clientWidth >= innerWidth || getComputedStyle(root).scrollbarGutter !== "auto") return;
    const property = "scrollbar-gutter";
    const value = root.style.getPropertyValue(property);
    const priority = root.style.getPropertyPriority(property);
    root.style.setProperty(property, "stable", "important");
    restoreGutter = () => root.style.setProperty(property, value, priority);
  }
  const liveIds = new WeakMap<HTMLElement, number>();
  let nextLiveId = 1;
  function liveContainers() {
    return Array.from(document.querySelectorAll<HTMLElement>("*")).filter(
      (el) =>
        el instanceof HTMLElement &&
        el.getClientRects().length > 0 &&
        getComputedStyle(el)
          .containerType.split(/\s+/)
          .some((type) => type === "size" || type === "inline-size"),
    );
  }
  function liveId(el: HTMLElement) {
    const existing = liveIds.get(el);
    if (existing !== undefined) return existing;
    const id = nextLiveId++;
    liveIds.set(el, id);
    return id;
  }
  function liveFields() {
    return Array.from(
      document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>(
        "input, textarea, select",
      ),
    ).filter(
      (el) =>
        !el.disabled &&
        el.getClientRects().length &&
        (el instanceof HTMLSelectElement
          ? !el.multiple
          : !el.readOnly &&
            (el instanceof HTMLTextAreaElement || ["text", "search"].includes(el.type))),
    );
  }
  function prepareLiveField(id: number, value: string) {
    const field = liveFields().find((el) => liveId(el) === id);
    if (!field) return "field-unavailable";
    if (field instanceof HTMLSelectElement) {
      if (
        !Array.from(field.options).some(
          (option) => option.value === value && !option.matches(":disabled"),
        )
      )
        return "option-unavailable";
      field.value = value;
      field.dispatchEvent(new Event("input", { bubbles: true }));
      field.dispatchEvent(new Event("change", { bubbles: true }));
      return "selected";
    }
    field.focus();
    field.select();
    return document.activeElement === field ? "focused" : "field-unavailable";
  }
  async function liveLayout(target?: number, width?: number, height?: number) {
    if (target !== undefined) {
      const el = liveContainers().find((node) => liveId(node) === target);
      if (!el) return { error: "container-unavailable" };
      if (width !== undefined) el.style.setProperty("width", width + "px", "important");
      if (height !== undefined) el.style.setProperty("height", height + "px", "important");
    }
    await new Promise<void>((done) =>
      requestAnimationFrame(() => requestAnimationFrame(() => done())),
    );
    const root = document.documentElement;
    return {
      width: innerWidth,
      height: innerHeight,
      path: location.pathname + location.search + location.hash,
      scrollWidth: root.scrollWidth,
      scrollHeight: root.scrollHeight,
      fields: liveFields().map((el) => ({
        id: liveId(el),
        label:
          el.labels?.[0]?.textContent?.trim() ||
          el.getAttribute("aria-label") ||
          el.name ||
          el.id ||
          el.tagName.toLowerCase(),
        value: el.value,
        ...(el instanceof HTMLSelectElement
          ? {
              options: Array.from(el.options).map((option) => ({
                value: option.value,
                label: option.label,
                disabled: option.matches(":disabled"),
              })),
            }
          : {}),
      })),
      containers: liveContainers().map((el) => ({
        id: liveId(el),
        label:
          el.id ||
          getComputedStyle(el).containerName.replace(/^none$/, "") ||
          el.tagName.toLowerCase(),
        width: parseFloat(getComputedStyle(el).width),
        height: parseFloat(getComputedStyle(el).height),
      })),
      renderTree: collectRenderTree(null, true),
    };
  }
  Object.assign(globalThis, {
    __duiRuntime: {
      liveLayout,
      prepareLiveField,
      containerStyle(id: number, axis: string) {
        const el = liveContainers().find((node) => liveId(node) === id);
        return el
          ? {
              id,
              axis,
              value: el.style.getPropertyValue(axis),
              priority: el.style.getPropertyPriority(axis),
            }
          : null;
      },
      restoreContainerStyle(style: { id: number; axis: string; value: string; priority: string }) {
        const el = liveContainers().find((node) => liveId(node) === style.id);
        if (!el) return false;
        el.style.setProperty(style.axis, style.value, style.priority);
        return true;
      },
      setCaptureGutter,
      hideCaret,
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
      whenLoaded,
      apply,
      restore,
      reference,
      links(selector: string) {
        const paths = new Set<string>();
        collectScreenLinks(paths, selector);
        return [...paths].sort();
      },
      async discoverLinks(selector: string) {
        const paths = new Set<string>();
        collectScreenLinks(paths, selector);
        collectScreenLinks(paths, "a[href]");

        // Framework-agnostic WAI-ARIA navigation disclosure expansion:
        // safely expand collapsed nav accordions/trees (aria-expanded="false") after capture
        // to uncover submenu <a href> links without relying on any router-specific syntax.
        const navRoots = document.querySelectorAll('nav, aside, [role="navigation"]');
        const triggers: HTMLElement[] = [];
        const seen = new Set<Element>();
        const destructiveRe =
          /\b(logout|log out|sign out|delete|remove|reset|destroy)\b|로그아웃|삭제|제거|초기화|탈퇴|退出|删除|移除|重置|ログアウト|削除|リセット/i;
        for (const root of navRoots) {
          for (const el of root.querySelectorAll(
            'button[aria-expanded="false"], [role="button"][aria-expanded="false"], [role="treeitem"][aria-expanded="false"]',
          )) {
            if (!(el instanceof HTMLElement) || seen.has(el)) continue;
            if (el.hasAttribute("disabled") || el.getAttribute("aria-disabled") === "true")
              continue;
            if (el.closest("form")) continue;
            const label = (
              el.innerText ||
              el.getAttribute("aria-label") ||
              el.textContent ||
              ""
            ).trim();
            if (destructiveRe.test(label)) continue;
            seen.add(el);
            triggers.push(el);
            if (triggers.length >= 24) break;
          }
          if (triggers.length >= 24) break;
        }
        if (triggers.length > 0) {
          const startHref = location.href;
          const opened: HTMLElement[] = [];
          for (const trigger of triggers) {
            if (!trigger.isConnected || trigger.getAttribute("aria-expanded") !== "false") continue;
            try {
              trigger.click();
              opened.push(trigger);
            } catch {
              // Ignore click failures on detached or guarded triggers.
            }
            if (location.href !== startHref) {
              try {
                history.back();
              } catch {
                // Ignore history errors.
              }
              break;
            }
          }
          await new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          );
          collectScreenLinks(paths, selector);
          collectScreenLinks(paths, "a[href]");
          for (const trigger of opened.reverse()) {
            if (trigger.isConnected && trigger.getAttribute("aria-expanded") === "true") {
              try {
                trigger.click();
              } catch {
                // Ignore restore click failures.
              }
            }
          }
        }
        return [...paths].sort();
      },
      listOverlayTriggers() {
        const destructiveRe =
          /\b(logout|log out|sign out|delete|remove|reset|destroy|clear|cancel|close|submit|save|back|next|prev|previous|download|export)\b|로그아웃|삭제|제거|초기화|취소|닫기|저장|탈퇴|다운로드|내보내기|退出|删除|移除|重置|取消|关闭|保存|下载|导出|ログアウト|削除|リセット|キャンセル|閉じる|保存/i;
        const skipRoles = new Set([
          "tab",
          "checkbox",
          "radio",
          "switch",
          "combobox",
          "menuitem",
          "option",
          "treeitem",
        ]);
        const triggers: { el: HTMLElement; label: string; scope: "header" | "main" }[] = [];
        const seen = new Set<Element>();
        const pushCandidate = (el: Element, explicit: boolean) => {
          if (triggers.length >= 12 || seen.has(el) || !(el instanceof HTMLElement)) return;
          if (el.hasAttribute("disabled") || el.getAttribute("aria-disabled") === "true") return;
          if (el.closest("form")) return;
          if (el.closest('nav, aside, [role="navigation"]')) return;
          const role = (el.getAttribute("role") || "").toLowerCase();
          if (skipRoles.has(role)) return;
          const popup = (el.getAttribute("aria-haspopup") || "").toLowerCase();
          if (!explicit && (popup === "menu" || popup === "listbox")) return;
          if (el instanceof HTMLButtonElement && (el.type || "").toLowerCase() === "submit") return;
          const rect = el.getBoundingClientRect();
          if (rect.width < 16 || rect.height < 16) return;
          const cs = getComputedStyle(el);
          if (cs.display === "none" || cs.visibility === "hidden" || cs.opacity === "0") return;
          const label = (
            el.getAttribute("aria-label") ||
            el.getAttribute("title") ||
            el.innerText ||
            el.textContent ||
            ""
          )
            .replace(/\s+/g, " ")
            .trim()
            .slice(0, 60);
          if (!explicit && (!label || /^\d+$/.test(label) || destructiveRe.test(label))) return;
          if (explicit && destructiveRe.test(label)) return;
          const scope: "header" | "main" = el.closest("header") ? "header" : "main";
          seen.add(el);
          triggers.push({ el, label: label || "Dialog", scope });
        };

        // Priority 1: explicit standard HTML / WAI-ARIA dialog & popover triggers
        for (const el of document.querySelectorAll(
          'button[aria-haspopup="dialog"], [role="button"][aria-haspopup="dialog"], button[popovertarget], button[commandfor]',
        )) {
          pushCandidate(el, true);
        }
        // Priority 2: safe visible action buttons inside main/header (e.g. Create, Import, Search)
        for (const el of document.querySelectorAll(
          'main button, [role="main"] button, header button',
        )) {
          pushCandidate(el, false);
        }
        (globalThis as Record<string, unknown>).__duiOverlayTriggers = triggers;
        // The rectangle identifies a shared header trigger across screens (probed once per run).
        return triggers.map((t, index) => {
          const r = t.el.getBoundingClientRect();
          const rect = [r.x, r.y, r.width, r.height].map(Math.round);
          return { index, label: t.label, scope: t.scope, rect };
        });
      },
      async openOverlayTrigger(index: number) {
        const triggers = ((globalThis as Record<string, unknown>).__duiOverlayTriggers ?? []) as {
          el: HTMLElement;
          label: string;
          scope: "header" | "main";
        }[];
        const entry = triggers[index];
        if (!entry || !entry.el.isConnected) return { opened: false };

        const before = new Set(visibleDialogs());
        const startHref = location.href;
        try {
          // A pointer or keyboard user focuses the trigger before activating it; dismissal checks
          // whether focus comes back here.
          entry.el.focus({ preventScroll: true });
          entry.el.click();
        } catch {
          return { opened: false };
        }
        // Continue as soon as a dialog appears or the page navigates; a trigger that does
        // neither within 380 ms opened nothing.
        await waitUntil(
          () => location.href !== startHref || visibleDialogs().some((d) => !before.has(d)),
          380,
        );
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        if (location.href !== startHref) {
          try {
            history.back();
          } catch {
            // Ignore history restore errors.
          }
          await new Promise((resolve) => setTimeout(resolve, 100));
          return { opened: false };
        }
        const newlyOpened = visibleDialogs().filter((d) => !before.has(d));
        if (newlyOpened.length === 0) return { opened: false };

        const dialog = newlyOpened[newlyOpened.length - 1]!;
        (globalThis as Record<string, unknown>).__duiActiveOverlay = dialog;
        (globalThis as Record<string, unknown>).__duiOverlayTrigger = entry.el;
        if (isImagePreview(dialog)) return { opened: true, imagePreview: true };
        // Wait for CSS slide-in / fade-in animations on the dialog (e.g. Tailwind duration-500
        // drawers) to finish before measuring renderTree or taking the screenshot.
        try {
          const anims = typeof dialog.getAnimations === "function" ? dialog.getAnimations() : [];
          if (anims.length > 0) {
            await Promise.race([
              Promise.allSettled(anims.map((a) => a.finished)),
              new Promise((resolve) => setTimeout(resolve, 650)),
            ]);
          }
        } catch {
          // Ignore animation API errors.
        }
        // Content that arrives after the dialog opens (lazy sections): wait until the dialog's
        // markup is unchanged across two checks, for at most 300 ms.
        let markup = dialog.innerHTML;
        let unchanged = 0;
        await waitUntil(() => {
          const next = dialog.innerHTML;
          unchanged = next === markup ? unchanged + 1 : 0;
          markup = next;
          return unchanged >= 2;
        }, 300);
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        const { title, slug: named } = dialogTitle(dialog, entry.label);
        const slug = named || `dialog-${index}`;
        // Scope renderTree to the active dialog/overlay subtree so obscured background page
        // elements behind the modal backdrop do not generate false overlap findings.
        const renderTree = collectRenderTree(dialog);
        return {
          opened: true,
          slug,
          title,
          triggerLabel: entry.label,
          scope: entry.scope,
          renderTree,
        };
      },
      /** After a trusted Escape: did the overlay close, and did focus go back to its trigger? */
      async overlayDismissal() {
        const active = (globalThis as Record<string, unknown>).__duiActiveOverlay as
          | HTMLElement
          | undefined;
        const trigger = (globalThis as Record<string, unknown>).__duiOverlayTrigger as
          | HTMLElement
          | undefined;
        if (!active) return null;
        // An exit animation can keep the overlay visible briefly; wait up to 320 ms for it.
        const closed = await waitUntil(() => overlayClosed(active), 320);
        if (closed) await new Promise((resolve) => requestAnimationFrame(resolve));
        const focused = document.activeElement;
        return {
          closedByEscape: closed,
          focusReturned: closed
            ? Boolean(trigger && focused && (focused === trigger || trigger.contains(focused)))
            : null,
        };
      },
      /** Elements a keyboard user should reach with Tab, in render-tree coordinates. */
      keyboardTargets() {
        const selector =
          'a[href], button, input:not([type="hidden"]), select, textarea, summary, [tabindex], [contenteditable="true"]';
        return Array.from(document.querySelectorAll<HTMLElement>(selector))
          .filter((el) => {
            if (el.matches(":disabled, [inert] *, [aria-hidden='true'], [aria-hidden='true'] *"))
              return false;
            if (el.tabIndex < 0) return false;
            const r = el.getBoundingClientRect();
            return (
              r.width > 0 &&
              r.height > 0 &&
              el.checkVisibility({ visibilityProperty: true, opacityProperty: true })
            );
          })
          .map((el) => keyboardRecord(el));
      },
      /** The element Tab focused, with whether a focus indicator is painted on it. */
      focusStop() {
        const el = document.activeElement as HTMLElement | null;
        if (!el || el === document.body || el === document.documentElement) return null;
        const outlined = (node: Element) => {
          const cs = getComputedStyle(node);
          return (
            cs.outlineStyle !== "none" &&
            (cs.outlineStyle === "auto" || Number.parseFloat(cs.outlineWidth) > 0)
          );
        };
        const ring = getComputedStyle(el).boxShadow !== "none";
        // A wrapper can draw the indicator with :focus-within (a styled select inside its label).
        let wrapper: Element | null = el.parentElement;
        for (let depth = 0; wrapper && depth < 3 && !outlined(wrapper); depth++)
          wrapper = wrapper.parentElement;
        const around = Boolean(wrapper && wrapper.matches(":focus-within") && outlined(wrapper));
        return {
          ...keyboardRecord(el),
          focusVisible: outlined(el) || ring || around,
          indicator: outlined(el)
            ? "outline"
            : ring
              ? "box-shadow"
              : around
                ? "outline on a :focus-within wrapper"
                : "",
        };
      },
      resetFocus() {
        (document.activeElement as HTMLElement | null)?.blur?.();
        window.scrollTo(0, 0);
      },
      htmlSnapshot,
      /** The dialog dui opened from a compiler fact: its title, slug and render tree. */
      openDialog() {
        const dialog = visibleDialogs().at(-1);
        if (!dialog) return null;
        const { title, slug } = dialogTitle(dialog, "Dialog");
        return { title, slug: slug || "dialog", renderTree: collectRenderTree(dialog) };
      },
      async closeOverlay() {
        const open = (globalThis as Record<string, unknown>).__duiActiveOverlay as
          | HTMLElement
          | undefined;
        if (!open || overlayClosed(open)) {
          (globalThis as Record<string, unknown>).__duiActiveOverlay = undefined;
          return;
        }
        const target = (document.activeElement as HTMLElement | null) ?? document.body;
        try {
          target?.dispatchEvent(
            new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }),
          );
          document.dispatchEvent(
            new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }),
          );
        } catch {
          // Ignore synthetic keyboard event failures.
        }
        if (!(await waitUntil(() => overlayClosed(open), 260))) {
          if (open instanceof HTMLDialogElement && open.open) {
            open.close();
          } else {
            const closeBtn = open.querySelector<HTMLElement>(
              'button[aria-label*="close" i], button[title*="close" i]',
            );
            closeBtn?.click();
          }
          await waitUntil(() => overlayClosed(open), 220);
        }
        (globalThis as Record<string, unknown>).__duiActiveOverlay = undefined;
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
            const horiz =
              style.textOverflow !== "ellipsis" &&
              el.clientWidth > 0 &&
              el.scrollWidth > el.clientWidth + 1;
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
      async revealForFullPageCapture(maxHeight: number) {
        const limit = Math.max(800, Math.round(maxHeight || 12_000));
        const vpH = Math.max(1, Math.round(window.innerHeight || 800));
        const measureDocH = () =>
          Math.ceil(
            Math.max(
              vpH,
              document.documentElement ? document.documentElement.scrollHeight : 0,
              document.body ? document.body.scrollHeight : 0,
            ),
          );
        const measureShellH = (docH: number) => {
          let shellH = 0;
          if (docH <= vpH + 8) {
            const vpW = Math.max(1, Math.round(window.innerWidth || 0));
            const candidates = document.querySelectorAll(
              'main, [role="main"], [data-dui-scroll-root], div, section, ul, ol, article',
            );
            for (const el of candidates) {
              if (!(el instanceof HTMLElement)) continue;
              const cs = getComputedStyle(el);
              if (cs.overflowY !== "auto" && cs.overflowY !== "scroll") continue;
              if (
                el.clientHeight >= vpH * 0.35 &&
                el.clientWidth >= vpW * 0.4 &&
                el.scrollHeight > el.clientHeight + 40
              ) {
                const needed = Math.ceil(vpH + (el.scrollHeight - el.clientHeight));
                if (needed > shellH) shellH = needed;
              }
            }
          }
          return shellH;
        };

        const initialDocH = measureDocH();
        const sweepCeiling = Math.min(initialDocH, limit);

        // Promote lazy images inside the initial document height so full-page captures do not show empty boxes.
        for (const img of document.querySelectorAll('img[loading="lazy"]')) {
          if (img instanceof HTMLImageElement) {
            img.loading = "eager";
          }
        }

        let infiniteScrollDetected = false;
        let growthEvents = 0;
        let prevDocH = initialDocH;
        const startScrollY = window.scrollY;
        const growthThreshold = Math.max(400, Math.round(vpH * 0.6));
        const burstThreshold = Math.max(1600, vpH * 2);

        if (sweepCeiling > vpH + 8) {
          const maxScrollY = Math.max(0, sweepCeiling - vpH);
          const step = Math.max(Math.round(vpH * 0.85), Math.ceil(maxScrollY / 12));
          for (let stepIdx = 1, y = step; stepIdx <= 12; stepIdx++, y += step) {
            const targetY = Math.min(y, maxScrollY);
            window.scrollTo(0, targetY);
            await new Promise((resolve) =>
              requestAnimationFrame(() => requestAnimationFrame(resolve)),
            );
            await new Promise((resolve) => setTimeout(resolve, 45));

            const curDocH = measureDocH();
            if (
              curDocH > prevDocH + Math.max(200, Math.round(vpH * 0.4)) &&
              curDocH > initialDocH + growthThreshold
            ) {
              growthEvents += 1;
              prevDocH = curDocH;
            }
            if (
              growthEvents >= 2 ||
              curDocH > initialDocH + burstThreshold ||
              (initialDocH <= limit && curDocH > limit)
            ) {
              infiniteScrollDetected = true;
              break;
            }
            if (targetY >= maxScrollY) break;
          }
          window.scrollTo(0, startScrollY);
          await new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          );
        }

        // Wait briefly for eager/lazy images and scroll-reveal animations to settle (strictly bounded).
        try {
          const pendingImgs: Promise<unknown>[] = [];
          for (const img of document.querySelectorAll("img")) {
            if (!(img instanceof HTMLImageElement) || !img.currentSrc) continue;
            if (!img.complete) {
              pendingImgs.push(
                new Promise((resolve) => {
                  img.addEventListener("load", resolve, { once: true });
                  img.addEventListener("error", resolve, { once: true });
                }),
              );
            } else if (typeof img.decode === "function") {
              pendingImgs.push(img.decode().catch(() => {}));
            }
          }
          if (pendingImgs.length > 0) {
            await Promise.race([
              Promise.allSettled(pendingImgs),
              new Promise((resolve) => setTimeout(resolve, 1200)),
            ]);
          }
        } catch {
          // Ignore image decode errors.
        }

        try {
          const anims =
            typeof document.getAnimations === "function"
              ? document
                  .getAnimations()
                  .filter(
                    (a) =>
                      a.playState === "running" &&
                      (a.effect?.getComputedTiming().iterations ?? 1) === 1,
                  )
              : [];
          if (anims.length > 0) {
            await Promise.race([
              Promise.allSettled(anims.map((a) => a.finished)),
              new Promise((resolve) => setTimeout(resolve, 650)),
            ]);
          }
        } catch {
          // Ignore Web Animations API errors.
        }
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));

        const finalDocH = measureDocH();
        const shellH = measureShellH(finalDocH);
        const effectiveDocH = infiniteScrollDetected ? initialDocH : finalDocH;
        const expandedShell = !infiniteScrollDetected && shellH > effectiveDocH;
        const captureH = Math.max(vpH, Math.min(expandedShell ? shellH : effectiveDocH, limit));
        const truncated =
          infiniteScrollDetected || effectiveDocH > limit || (expandedShell && shellH > limit);
        return {
          vpH,
          docH: effectiveDocH,
          initialDocH,
          shellH,
          captureH,
          expandedShell,
          infiniteScrollDetected,
          truncated,
        };
      },
      assertions: (checks: Check[]) => checks.map(check),
    },
  });
})();
