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
          branch(id: string, truth: boolean): void;
          slot(id: string, value: unknown, setter: unknown): void;
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
    initial<T>(id: string, fallback: () => T): T {
      const inputs = probe()?.input;
      return inputs && Object.prototype.hasOwnProperty.call(inputs, id)
        ? (inputs[id] as T)
        : fallback();
    },
  };
}
