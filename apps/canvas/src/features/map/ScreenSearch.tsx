import { useRef, useState } from "react";
import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";
import type { Screen } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import { routeName } from "./CaptureCard";

export function ScreenSearch({
  screens,
  close,
  select,
  restoreFocus,
}: {
  screens: Screen[];
  close: () => void;
  select: (id: string) => void;
  restoreFocus: () => HTMLElement | null;
}) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const term = query.trim().toLowerCase();
  const matches = screens.filter((s) =>
    [s.path, routeName(s.path), s.nativeSource?.name, s.nativeSource?.source]
      .join(" ")
      .toLowerCase()
      .includes(term),
  );

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) close();
      }}
    >
      <DialogContent
        initialFocus={input}
        finalFocus={restoreFocus}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return;
          if (event.key === "Enter" && event.target === input.current && matches[0]) {
            event.preventDefault();
            select(matches[0].id);
          }
          if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
          const buttons = Array.from(list.current?.querySelectorAll("button") ?? []);
          if (!buttons.length) return;
          event.preventDefault();
          const index = buttons.findIndex((b) => b === event.target);
          buttons[
            event.key === "ArrowDown"
              ? Math.min(index + 1, buttons.length - 1)
              : Math.max(index - 1, 0)
          ]?.focus();
        }}
      >
        <DialogTitle>{t.findScreen}</DialogTitle>
        <input
          className="screen-search-input"
          ref={input}
          type="search"
          name="screen"
          autoComplete="off"
          spellCheck={false}
          aria-label={t.findScreen}
          placeholder="/…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="screen-search-results" ref={list}>
          {matches.map((s) => (
            <Button key={s.id} variant="ghost" onClick={() => select(s.id)}>
              {s.path}
            </Button>
          ))}
          {matches.length === 0 && <output>{t.noMatchingScreens}</output>}
        </div>
      </DialogContent>
    </Dialog>
  );
}
