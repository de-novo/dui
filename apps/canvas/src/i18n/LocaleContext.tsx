import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { MESSAGES, normalizeUiLocale, type Messages, type UiLocale } from "./messages";

type I18nValue = {
  locale: UiLocale;
  setLocale: (locale: UiLocale) => void;
  t: Messages;
};

const LocaleContext = createContext<I18nValue>({
  locale: "en",
  setLocale: () => {},
  t: MESSAGES.en,
});

function initialUiLocale(): UiLocale {
  const fromQuery = new URLSearchParams(location.search).get("uiLang");
  if (fromQuery) return normalizeUiLocale(fromQuery);
  const saved = localStorage.getItem("dui.uiLocale");
  if (saved) return normalizeUiLocale(saved);
  return normalizeUiLocale(navigator.language);
}

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocale] = useState<UiLocale>(initialUiLocale);

  useEffect(() => {
    document.documentElement.lang = locale;
    localStorage.setItem("dui.uiLocale", locale);
  }, [locale]);

  return (
    <LocaleContext.Provider value={{ locale, setLocale, t: MESSAGES[locale] }}>
      {children}
    </LocaleContext.Provider>
  );
}

export function useI18n() {
  return useContext(LocaleContext);
}
