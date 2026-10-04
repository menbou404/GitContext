import { AlertIcon, CheckIcon } from "../Icons";
import { shellCopy, type Locale } from "../i18n";
import type { RepositoryState } from "../types";

export function StatusLabel({ state, locale }: { state: RepositoryState; locale: Locale }) {
  const copy = shellCopy[locale];
  return <span className={`ui-status ui-status--${state}`}>
    {state === "ready" ? <CheckIcon /> : <AlertIcon />}
    <span>{copy[state]}</span>
  </span>;
}
