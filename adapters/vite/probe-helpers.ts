// Appended after application code. Hoisted declarations also work during module initialization.
var __duiToolsCache: ReturnType<typeof __duiToolsCreate> | undefined;
function __duiTools() {
  return (__duiToolsCache ??= __duiToolsCreate());
}
function __duiToolsCreate() {
  function probe() {
    return (
      globalThis as {
        __duiProbe?: {
          input: Record<string, unknown>;
          commit(id: string, instance: string, value: unknown, setter: unknown): () => void;
          branch(id: string, truth: boolean): void;
          slot(id: string, value: unknown, setter: unknown): void;
          overlay?(id: string, open: boolean): void;
        };
      }
    ).__duiProbe;
  }
  // Membership never inspects an application object, including a Proxy.
  const boxes = new WeakSet<object>();
  function right<T>(value: T) {
    const box = { value };
    boxes.add(box);
    return box;
  }
  return {
    probe,
    right,
    logical<L, R>(id: string, and: boolean, value: L | ReturnType<typeof right<R>>): L | R {
      const evaluated = typeof value === "object" && value !== null && boxes.has(value);
      probe()?.branch(id, evaluated ? and : !and);
      return evaluated ? (value as ReturnType<typeof right<R>>).value : (value as L);
    },
    // A dialog's open prop: dui's requested value when it set one, otherwise the app's own.
    // Rendering the element registers the dialog with the probe.
    overlay<T>(id: string, fallback: () => T): T {
      const p = probe();
      const inputs = p?.input;
      const value =
        inputs && Object.prototype.hasOwnProperty.call(inputs, id) ? (inputs[id] as T) : fallback();
      p?.overlay?.(id, Boolean(value));
      return value;
    },
    initial<T>(id: string, fallback: () => T): T {
      const inputs = probe()?.input;
      return inputs && Object.prototype.hasOwnProperty.call(inputs, id)
        ? (inputs[id] as T)
        : fallback();
    },
  };
}
