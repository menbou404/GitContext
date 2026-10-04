import type { Locale } from "./i18n";

export function resolveLocale(savedLocale: unknown, browserLanguage: string | undefined): Locale {
  if (savedLocale === "ja" || savedLocale === "en") return savedLocale;
  return browserLanguage?.toLowerCase().startsWith("ja") ? "ja" : "en";
}
