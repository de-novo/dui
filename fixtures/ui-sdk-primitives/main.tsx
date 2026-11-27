import { definePrimitive } from "@ui-sdk/core";
import { usePart, usePrimitive } from "@ui-sdk/react/primitive";
import React from "react";
import { createRoot } from "react-dom/client";

type LookupOptions = {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  open?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
};

type LookupState = {
  value: string;
  open: boolean;
};

const lookupControl = definePrimitive({
  name: "lookup-control",
  options: {
    value: "string",
    defaultValue: "string",
    onValueChange: "callback",
    open: "boolean",
    defaultOpen: "boolean",
    onOpenChange: "callback",
  },
  controlled: ["value", "open"],
  state: (options: LookupOptions): LookupState => ({
    value: options.value ?? options.defaultValue ?? "",
    open: options.open ?? options.defaultOpen ?? false,
  }),
  sync: (options) => {
    const next: Partial<LookupState> = {};
    if (options.value !== undefined) next.value = options.value;
    if (options.open !== undefined) next.open = options.open;
    return next;
  },
  api: (context) => ({
    setValue(value: string) {
      context.options().onValueChange?.(value);
      if (context.options().value === undefined) context.store.set({ value });
    },
    setOpen(open: boolean) {
      context.options().onOpenChange?.(open);
      if (context.options().open === undefined) context.store.set({ open });
    },
  }),
  slots: (slot) => ({
    input: slot({
      attrs: (state) => ({ type: "search", value: state.value }),
      on: {
        input: (event, { api }) => {
          const target = event.target;
          if (target instanceof HTMLInputElement) api.setValue(target.value);
        },
      },
    }),
    clear: slot({
      present: (state) => state.value !== "",
      attrs: () => ({ type: "button" }),
      press: ({ api }) => api.setValue(""),
    }),
    trigger: slot({
      attrs: (state) => ({
        type: "button",
        "aria-expanded": `${state.open}`,
      }),
      press: ({ api, store }) => api.setOpen(!store.get().open),
    }),
    popup: slot({
      present: (state) => state.open,
      attrs: () => ({ role: "status" }),
    }),
  }),
});

const inventory = [
  "Adapter contract",
  "Compiler evidence",
  "Coverage matrix",
  "Rule pack",
  "State planner",
];

function Fixture() {
  const [value, setValue] = React.useState("");
  const [open, setOpen] = React.useState(false);
  const lookup = usePrimitive(lookupControl, {
    value,
    open,
    onValueChange: setValue,
    onOpenChange: setOpen,
  });
  const results = inventory.filter((item) => item.toLowerCase().includes(value.toLowerCase()));
  return (
    <main data-testid="lookup-fixture" className="lookup-panel">
      <h1>Inventory lookup</h1>
      <div className="lookup-bar">
        {usePart(
          lookup,
          "input",
          {},
          {
            "aria-label": "Search inventory",
            placeholder: "Search inventory",
          },
          "input",
        )}
        {usePart(lookup, "clear", {}, { children: "Clear" }, "button")}
        {usePart(
          lookup,
          "trigger",
          {},
          { children: open ? "Hide details" : "Show details" },
          "button",
        )}
      </div>
      <p className="results">
        {results.length ? results.join(" · ") : "No inventory entries match."}
      </p>
      {usePart(
        lookup,
        "popup",
        {},
        {
          children: (
            <div>
              <strong>Capture details</strong>
              <p>
                {value ? `Filtering for “${value}”.` : "Type in the field to filter the inventory."}
              </p>
            </div>
          ),
        },
        "section",
      )}
    </main>
  );
}

createRoot(document.getElementById("app")!).render(<Fixture />);
