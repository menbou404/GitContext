import type { ReactNode } from "react";
import { BranchIcon, FolderIcon } from "../Icons";
import { shellCopy, type Locale } from "../i18n";

export type ShellPage = "repositories" | "profiles" | "ai" | "history" | "settings";

export function Shell({ page, locale, repositoryCount, notice, noticeLink, noticeLinkLabel, onNavigate, onDismiss, children }: {
  page: ShellPage;
  locale: Locale;
  repositoryCount: number;
  notice: string | null;
  noticeLink?: ShellPage | null;
  noticeLinkLabel?: string;
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
      <button type="button" className={page === "ai" ? "active" : ""} aria-current={page === "ai" ? "page" : undefined} onClick={() => onNavigate("ai")}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><path d="M9 3v4M15 3v4M7 7h10v4a5 5 0 0 1-10 0zM12 16v5"/></svg><span>{copy.aiIntegration}</span>
      </button>
      <button type="button" className={page === "history" ? "active" : ""} aria-current={page === "history" ? "page" : undefined} onClick={() => onNavigate("history")}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg> <span>{copy.history}</span>
      </button>
      <button type="button" className={page === "settings" ? "active" : ""} aria-current={page === "settings" ? "page" : undefined} onClick={() => onNavigate("settings")}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M10 2h4l.6 2.3 2 .9 2.1-1.2 2.8 2.8-1.2 2.1.9 2L23 11v4l-2.3.6-.9 2 1.2 2.1-2.8 2.8-2.1-1.2-2 .9L14 24h-4l-.6-2.3-2-.9-2.1 1.2-2.8-2.8 1.2-2.1-.9-2L1 15v-4l2.3-.6.9-2L3 6.3l2.8-2.8 2.1 1.2 2-.9L10 2Z" transform="translate(0 -1) scale(1 .92)"/></svg> <span>{copy.settings}</span>
      </button>
    </nav>
    <div className="ui-workspace">
      {notice && <div className="ui-notice" role="status"><span title={notice}>{notice}</span>{noticeLink && <button type="button" className="ui-notice-link" onClick={() => onNavigate(noticeLink)}>{noticeLinkLabel ?? copy.openProfiles}</button>}<button type="button" onClick={onDismiss} aria-label={locale === "ja" ? "通知を閉じる" : "Dismiss notice"}>×</button></div>}
      <main className="ui-main">{children}</main>
    </div>
  </div>;
}
