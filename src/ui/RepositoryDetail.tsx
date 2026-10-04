import { BranchIcon } from "../Icons";
import { shellCopy, uiCopy, type Locale } from "../i18n";
import type { ApplyPreview, AutoApprove, Profile, RepositoryRecord, RepositoryStatus } from "../types";
import { ProfileDot } from "./ProfileDot";
import { RepositoryOverview } from "./RepositoryOverview";
import { StatusLabel } from "./StatusLabel";

export function RepositoryDetail({ repository, profiles, status, pendingProfileId, preview, locale, busy, removing, onBack, onPendingProfile, onReview, onCancelReview, onApply, onAutoApprove, onStartRemove, onCancelRemove, onRemove, onCommit, onPush, onSync, onPullRequest, onManagePullRequests, onPublish }: {
  repository: RepositoryRecord; profiles: Profile[]; status: RepositoryStatus | undefined;
  pendingProfileId: string; preview: ApplyPreview | null; locale: Locale; busy: boolean; removing: boolean;
  onBack: () => void; onPendingProfile: (id: string) => void; onReview: () => void; onCancelReview: () => void;
  onApply: () => Promise<void>; onAutoApprove: (field: keyof AutoApprove, enabled: boolean) => void;
  onStartRemove: () => void; onCancelRemove: () => void; onRemove: () => void;
  onCommit: () => void; onPush: () => void; onSync: () => void; onPullRequest: () => void;
  onManagePullRequests: () => void; onPublish: () => void;
}) {
  const copy = shellCopy[locale];
  const old = uiCopy[locale];
  const profile = profiles.find((item) => item.id === repository.profileId) ?? null;
  const state = status?.state ?? (profile ? "attention" : "unassigned");
  const applied = Boolean(profile && repository.lastAppliedAt);
  const githubReady = Boolean(applied && profile?.githubUsername && profile.ghConfigDir);
  return <div className="ui-page ui-detail">
    <div className="ui-breadcrumb"><button type="button" onClick={onBack}>{copy.back}</button><span>/</span>{repository.name}</div>
    <div className="ui-detail-title"><h1>{repository.name}</h1>{profile && <span className="ui-profile ui-profile--badge"><ProfileDot profile={profile} />{profile.label}{profile.githubUsername && <small>@{profile.githubUsername}</small>}</span>}<StatusLabel state={state} locale={locale} /></div>
    <div className="ui-detail-path" title={repository.path}>{repository.path}</div>
    <p className="ui-branch"><BranchIcon />{status?.branch ?? repository.branch ?? copy.noBranch}</p>
    {(state === "reapply" || state === "unassigned") && <button className="ui-button ui-button--primary" type="button" disabled={!pendingProfileId || busy} onClick={onReview}>{copy.reviewApply}</button>}
    <section className="ui-operations" aria-label={copy.actions}><h2>{copy.actions}</h2><div className="ui-operation-buttons">
      <button className="ui-button" type="button" onClick={onCommit} disabled={!applied || busy}>{old.commitButton}</button>
      {repository.remoteUrl ? <>
        <button className="ui-button" type="button" onClick={onSync} disabled={!applied || busy}>{old.syncButton}</button>
        <button className="ui-button" type="button" onClick={onPush} disabled={!applied || busy}>{old.pushButton}</button>
        <button className="ui-button" type="button" onClick={onPullRequest} disabled={!githubReady || busy}>{old.pullRequestButton}</button>
        <button className="ui-button" type="button" onClick={onManagePullRequests} disabled={!githubReady || busy}>{old.managePullRequestsButton}</button>
      </> : <button className="ui-button" type="button" onClick={onPublish} disabled={!githubReady || busy}>{old.publishToGithub}</button>}
    </div></section>
    <div className="ui-tab"><h2>{copy.overview}</h2></div>
    <RepositoryOverview repository={repository} profiles={profiles} profile={profile} status={status} pendingProfileId={pendingProfileId} preview={preview} locale={locale} busy={busy} removing={removing} onPendingProfile={onPendingProfile} onReview={onReview} onCancelReview={onCancelReview} onApply={onApply} onAutoApprove={onAutoApprove} onStartRemove={onStartRemove} onCancelRemove={onCancelRemove} onRemove={onRemove} />
  </div>;
}
