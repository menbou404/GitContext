import type { ReactNode } from "react";
import { BranchIcon, FolderIcon } from "../Icons";
import { shellCopy, type Locale } from "../i18n";

export type ShellPage = "repositories" | "profiles";

export function Shell({ page, locale, repositoryCount, notice, noticeLink, onNavigate, onDismiss, children }: {
  page: ShellPage;
  locale: Locale;
  repositoryCount: number;
  notice: string | null;
  noticeLink?: ShellPage | null;
  onNavigate: (page: ShellPage) => void;
  onDismiss: () => void;
  children: ReactNode;
}) {
  const copy = shellCopy[locale];
  return <div className="ui-shell">
    <nav className="ui-sidebar" aria-label={locale === "ja" ? "メインメニュー" : "Main navigation"}>
      <div className="ui-brand"><span className="ui-brand-mark"><BranchIcon /></span><strong>GitContext</strong></div>
      <button type="button" className={page === "repositories" ? "active" : ""} aria-current={page === "repositories" ? "page" : undefined} onClick={() => onNavigate("repositories")}>
        <FolderIcon /> <span>{copy.repositories}</span> <small>{repositoryCount}</small>
      </button>
      <button type="button" className={page === "profiles" ? "active" : ""} aria-current={page === "profiles" ? "page" : undefined} onClick={() => onNavigate("profiles")}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><circle cx="12" cy="8" r="3.5"/><path d="M5 20c1.2-3.5 4-5 7-5s5.8 1.5 7 5"/></svg> <span>{copy.profiles}</span>
      </button>
    </nav>
    <div className="ui-workspace">
      {notice && <div className="ui-notice" role="status"><span title={notice}>{notice}</span>{noticeLink && <button type="button" className="ui-notice-link" onClick={() => onNavigate(noticeLink)}>{copy.openProfiles}</button>}<button type="button" onClick={onDismiss} aria-label={locale === "ja" ? "通知を閉じる" : "Dismiss notice"}>×</button></div>}
      <main className="ui-main">{children}</main>
    </div>
  </div>;
}
