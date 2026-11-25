import { en, type Translations } from "./en";
import { vi } from "./vi";

export type LocalePreference = "auto" | "en" | "vi";
export type ResolvedLocale = "en" | "vi";

const STORAGE_KEY = "nanodock-locale";

function detectSystemLocale(): ResolvedLocale {
  if (typeof navigator === "undefined") return "en";
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const lang of langs) {
    if (lang && lang.toLowerCase().startsWith("vi")) {
      return "vi";
    }
  }
  return "en";
}

function loadPreference(): LocalePreference {
  if (typeof localStorage === "undefined") return "auto";
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === "en" || saved === "vi" || saved === "auto") {
    return saved;
  }
  return "auto";
}

class I18nStore {
  preference = $state<LocalePreference>(loadPreference());
  systemLocale = $state<ResolvedLocale>(detectSystemLocale());

  locale = $derived<ResolvedLocale>(
    this.preference === "auto" ? this.systemLocale : this.preference,
  );

  t = $derived<Translations>(this.locale === "vi" ? vi : en);

  setPreference(pref: LocalePreference) {
    this.preference = pref;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(STORAGE_KEY, pref);
    }
  }
}

export const i18n = new I18nStore();
