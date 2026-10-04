import { localizeRuntimeMessage, type Locale } from "../i18n";

export const runtimeError = (cause: unknown, locale: Locale) => localizeRuntimeMessage(cause instanceof Error ? cause.message : String(cause), locale);
