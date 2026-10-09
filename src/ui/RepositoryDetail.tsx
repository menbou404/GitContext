import { useRef, useState, type KeyboardEvent } from "react";
import { BranchIcon } from "../Icons";
import { shellCopy, tabCopy, type Locale } from "../i18n";
import type { AppData, ApplyPreview, AutoApprove, Profile, RepositoryRecord, RepositoryStatus } from "../types";
import { ChangesTab } from "./ChangesTab";
import { ProfileDot } from "./ProfileDot";
import { PullRequestTab } from "./PullRequestTab";
import { RepositoryOverview } from "./RepositoryOverview";
import { StatusLabel } from "./StatusLabel";
import { SyncTab } from "./SyncTab";

type DetailTab = "overview" | "changes" | "sync" | "pr";
const tabs: DetailTab[] = ["overview", "changes", "sync", "pr"];
export function nextDetailTab(index: number, key: string): DetailTab | null {
  if (key === "ArrowRight") return tabs[(index + 1) % tabs.length];
  if (key === "ArrowLeft") return tabs[(index - 1 + tabs.length) % tabs.length];
  if (key === "Home") return tabs[0];
  if (key === "End") return tabs[tabs.length - 1];
  return null;
}

export function RepositoryDetail({ repository, profiles, status, pendingProfileId, preview, locale, busy, removing, onBack, onPendingProfile, onReview, onCancelReview, onApply, onAutoApprove, onStartRemove, onCancelRemove, onRemove, onData, onFinished }: {
  repository: RepositoryRecord; profiles: Profile[]; status: RepositoryStatus | undefined;
  pendingProfileId: string; preview: ApplyPreview | null; locale: Locale; busy: boolean; removing: boolean;
  onBack: () => void; onPendingProfile: (id: string) => void; onReview: () => void; onCancelReview: () => void;
  onApply: () => Promise<void>; onAutoApprove: (field: keyof AutoApprove, enabled: boolean | "private" | "any") => void;
  onStartRemove: () => void; onCancelRemove: () => void; onRemove: () => void;
  onData: (data: AppData) => void; onFinished: () => void;
}) {
  const copy = shellCopy[locale];
  const labels = tabCopy[locale];
  const [active, setActive] = useState<DetailTab>("overview");
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const profile = profiles.find((item) => item.id === repository.profileId) ?? null;
  const state = status?.state ?? (profile ? "attention" : "unassigned");
  const appliedProfile = repository.lastAppliedAt && state !== "reapply" ? profile : null;
  const onTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const next = nextDetailTab(index, event.key);
    if (!next) return;
    event.preventDefault(); setActive(next); tabRefs.current[tabs.indexOf(next)]?.focus();
  };
  const tabLabel = (tab: DetailTab) => tab === "overview" ? copy.overview : tab === "changes" ? labels.changes : tab === "sync" ? labels.sync : labels.pullRequests;
  return <div className="ui-page ui-detail">
    <div className="ui-breadcrumb"><button type="button" onClick={onBack}>{copy.back}</button><span>/</span>{repository.name}</div>
    <div className="ui-detail-title"><h1>{repository.name}</h1>{profile && <span className="ui-profile ui-profile--badge"><ProfileDot profile={profile} />{profile.label}{profile.githubUsername && <small>@{profile.githubUsername}</small>}</span>}<StatusLabel state={state} locale={locale} /></div>
    <div className="ui-detail-path" title={repository.path}>{repository.path}</div>
    <p className="ui-branch"><BranchIcon />{status?.branch ?? repository.branch ?? copy.noBranch}</p>
    <div className="ui-tabs" role="tablist" aria-label={labels.tabList}>
      {tabs.map((tab, index) => <button key={tab} ref={(element) => { tabRefs.current[index] = element; }} type="button" role="tab" id={`repo-tab-${tab}`} aria-controls={`repo-panel-${tab}`} aria-selected={active === tab} tabIndex={active === tab ? 0 : -1} onClick={() => setActive(tab)} onKeyDown={(event) => onTabKeyDown(event, index)}>{tabLabel(tab)}{tab === "changes" && status?.uncommittedChanges ? <small>{status.uncommittedChanges}</small> : null}{tab === "sync" && status?.ahead ? <small>{status.ahead}</small> : null}</button>)}
    </div>
    {tabs.map((tab) => <div key={tab} id={`repo-panel-${tab}`} role="tabpanel" aria-labelledby={`repo-tab-${tab}`} tabIndex={0} hidden={active !== tab}>
      {active === tab && (tab === "overview" ? <RepositoryOverview repository={repository} profiles={profiles} profile={profile} status={status} pendingProfileId={pendingProfileId} preview={preview} locale={locale} busy={busy} removing={removing} onPendingProfile={onPendingProfile} onReview={onReview} onCancelReview={onCancelReview} onApply={onApply} onAutoApprove={onAutoApprove} onStartRemove={onStartRemove} onCancelRemove={onCancelRemove} onRemove={onRemove} />
        : tab === "changes" ? <ChangesTab repository={repository} profile={appliedProfile} status={status} locale={locale} onFinished={onFinished} onOverview={() => setActive("overview")} />
        : tab === "sync" ? <SyncTab repository={repository} profile={appliedProfile} locale={locale} onData={onData} onFinished={onFinished} />
        : <PullRequestTab repository={repository} profile={appliedProfile} locale={locale} onData={onData} onFinished={onFinished} />)}
    </div>)}
  </div>;
}
