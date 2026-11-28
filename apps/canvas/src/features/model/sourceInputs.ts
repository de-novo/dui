export type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

export type SourceInputState = {
  slots?: Record<string, Json>;
  expressions?: Record<string, Json>;
};

export type SourceInputs = {
  defaultState?: string;
  states: Record<string, SourceInputState>;
};

export type SourceInputRenderContext = {
  state?: string;
  slotValues?: Record<string, Json>;
  expressionValues?: Record<string, Json>;
};

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function jsonValues(value: unknown, context: string): Record<string, Json> | undefined {
  if (value === undefined) return undefined;
  if (!record(value)) throw Error(`Invalid source input ${context}`);
  for (const key of Object.keys(value))
    if (!key) throw Error(`Invalid empty source input ${context} key`);
  return value as Record<string, Json>;
}

/** Decode the immutable reviewer-supplied input archive. Unknown or malformed input is not repaired. */
export function decodeSourceInputs(raw: unknown): SourceInputs {
  if (!record(raw) || !record(raw.states)) throw Error("Invalid source inputs");
  const states: Record<string, SourceInputState> = {};
  for (const [state, entry] of Object.entries(raw.states)) {
    if (!state || !record(entry)) throw Error("Invalid source input state");
    states[state] = {
      slots: jsonValues(entry.slots, "slot"),
      expressions: jsonValues(entry.expressions, "expression"),
    };
  }
  const defaultState = typeof raw.defaultState === "string" ? raw.defaultState : undefined;
  if (defaultState !== undefined && !states[defaultState])
    throw Error("Invalid default source input state");
  return { defaultState, states };
}

const emptyContext: SourceInputRenderContext = Object.freeze({});
const contextCache = new WeakMap<SourceInputs, Map<string, SourceInputRenderContext>>();

/** Resolve the viewer's selected state exactly. An unknown explicit state selects no values;
 * the optional default applies only when the URL does not name a state. Results are cached by
 * immutable input document so stable card props do not invalidate unrelated map renders.
 */
export function sourceInputContext(
  inputs: SourceInputs | undefined,
  requestedState: string | null,
): SourceInputRenderContext {
  if (!inputs) return emptyContext;
  const state = requestedState === null ? inputs.defaultState : requestedState;
  if (!state || !inputs.states[state]) return emptyContext;
  const cached = contextCache.get(inputs);
  const hit = cached?.get(state);
  if (hit) return hit;
  const selected = inputs.states[state];
  const context = {
    state,
    slotValues: selected.slots,
    expressionValues: selected.expressions,
  };
  if (cached) cached.set(state, context);
  else contextCache.set(inputs, new Map([[state, context]]));
  return context;
}
