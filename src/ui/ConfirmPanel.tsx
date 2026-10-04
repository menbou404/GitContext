import type { ReactNode } from "react";
import { tabCopy, type Locale } from "../i18n";

export interface ConfirmRow { label: string; value: ReactNode }

export function ConfirmPanel({ locale, rows, children }: { locale: Locale; rows: ConfirmRow[]; children: ReactNode }) {
  return <section className="ui-confirm" aria-label={tabCopy[locale].confirmation}>
    <h3>{tabCopy[locale].confirmation}</h3>
    <dl>{rows.map((row, index) => <div key={`${row.label}-${index}`}><dt>{row.label}</dt><dd>{row.value}</dd></div>)}</dl>
    <div className="ui-confirm-actions">{children}</div>
  </section>;
}
