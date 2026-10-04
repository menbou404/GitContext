import { useState } from "react";
import { AlertIcon, ChevronIcon } from "../Icons";
import { localizeRuntimeMessage, shellCopy, uiCopy, type Locale } from "../i18n";
import type { ApplyPreview, AutoApprove, Profile, RepositoryRecord, RepositoryStatus } from "../types";
import { ProfileDot } from "./ProfileDot";

export function RepositoryOverview({ repository, profiles, profile, status, pendingProfileId, preview, locale, busy, removing, onPendingProfile, onReview, onCancelReview, onApply, onAutoApprove, onStartRemove, onCancelRemove, onRemove }: {
  repository: RepositoryRecord;
  profiles: Profile[];
  profile: Profile | null;
  status: RepositoryStatus | undefined;
  pendingProfileId: string;
  preview: ApplyPreview | null;
  locale: Locale;
  busy: boolean;
  removing: boolean;
  onPendingProfile: (id: string) => void;
  onReview: () => void;
  onCancelReview: () => void;
  onApply: () => Promise<void>;
  onAutoApprove: (field: keyof AutoApprove, enabled: boolean) => void;
  onStartRemove: () => void;
  onCancelRemove: () => void;
  onRemove: () => void;
}) {
  const copy = shellCopy[locale];
  const old = uiCopy[locale];
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);
  const mismatches = new Set(status?.mismatchedKeys ?? []);
  const githubMatches = Boolean(status?.github?.authenticated && profile?.githubUsername && status.github.username?.toLowerCase() === profile.githubUsername.toLowerCase());
  const doApply = async () => {
    setApplying(true);
    setApplyError(null);
    try { await onApply(); } catch (error) { setApplyError(localizeRuntimeMessage(error instanceof Error ? error.message : String(error), locale)); } finally { setApplying(false); }
  };
  return <div className="ui-overview">
    <section className="ui-card">
      <h2>{copy.assignedProfile}</h2>
      <div className="ui-profile-selection">
        <div>{profile ? <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}{profile.githubUsername && <small>@{profile.githubUsername}</small>}</span> : <span className="ui-muted">{copy.unassigned}</span>}</div>
        <label><span>{copy.changeProfile}</span><select value={pendingProfileId} onChange={(event) => { onCancelReview(); onPendingProfile(event.target.value); }}><option value="">{old.selectProfile}</option>{profiles.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
      </div>
      <button className="ui-button ui-button--primary" type="button" disabled={!pendingProfileId || busy} onClick={onReview}>{copy.reviewApply}</button>
    </section>

    {preview?.repository.id === repository.id && <section className="ui-card ui-review" aria-label={copy.applyConfirmation}>
      <h2>{copy.applyConfirmation}</h2><p>{old.applyLead}</p>
      <p className="ui-profile"><ProfileDot profile={preview.profile} />{preview.profile.label} → {repository.name}</p>
      {preview.changes.length ? <div className="ui-changes">{preview.changes.map((change) => <div key={change.key}><code>{change.key}</code><span>{change.currentValue || old.notSet} <ChevronIcon /> <strong>{change.nextValue ?? old.removedValue}</strong></span></div>)}</div> : <p>{copy.noConfigChanges}</p>}
      {preview.warnings.map((warning) => <p className="ui-warning" key={warning}><AlertIcon />{localizeRuntimeMessage(warning, locale)}</p>)}
      {applyError && <p className="ui-warning">{applyError}</p>}
      <div className="ui-actions"><button className="ui-button" type="button" onClick={onCancelReview}>{copy.cancel}</button><button className="ui-button ui-button--primary" type="button" onClick={doApply} disabled={applying}>{applying ? copy.applying : copy.apply}</button></div>
    </section>}

    <section className="ui-card">
      <h2>{copy.localSettings}</h2>
      <dl className="ui-settings">
        <div><dt>{copy.author}</dt><dd>{profile ? `${profile.gitName} <${profile.gitEmail}>` : "—"}</dd><dd className={mismatches.has("user.name") || mismatches.has("user.email") ? "ui-caution" : "ui-success"}>{profile ? (mismatches.has("user.name") || mismatches.has("user.email") ? copy.differs : copy.matches) : "—"}</dd></div>
        <div><dt>{copy.sshKey}</dt><dd className="ui-mono">{profile?.sshKeyPath ?? copy.notManaged}</dd><dd className={mismatches.has("core.sshCommand") ? "ui-caution" : "ui-success"}>{profile ? (mismatches.has("core.sshCommand") ? copy.differs : copy.matches) : "—"}</dd></div>
        <div><dt>{copy.github}</dt><dd>{profile?.githubUsername ? `@${profile.githubUsername}` : copy.notManaged}</dd><dd className={githubMatches ? "ui-success" : "ui-caution"}>{profile ? (githubMatches ? copy.connected : status?.github?.username ? `${copy.authenticatedAs(status.github.username)} · ${copy.differs}` : copy.notConnected) : "—"}</dd></div>
        <div><dt>{copy.origin}</dt><dd className="ui-mono">{repository.remoteUrl ?? copy.noOrigin}</dd></div>
      </dl>
      {mismatches.has("gitcontext.profileId") && <p className="ui-warning">{copy.assignmentMismatch}</p>}
      {status?.github?.detail && <p className="ui-warning">{localizeRuntimeMessage(status.github.detail, locale)} {copy.notConnected}</p>}
      {status?.error && <p className="ui-warning">{copy.statusError(repository.name, localizeRuntimeMessage(status.error, locale))}</p>}
    </section>

    <fieldset className="ui-card ui-auto-approve" disabled={busy}>
      <legend>{old.autoApproveTitle}</legend>
      <label><input type="checkbox" checked={repository.autoApprove.pushWorkBranch} onChange={(event) => onAutoApprove("pushWorkBranch", event.target.checked)} />{old.autoApprovePush}</label>
      <label><input type="checkbox" checked={repository.autoApprove.pushDefaultBranch} onChange={(event) => onAutoApprove("pushDefaultBranch", event.target.checked)} />{old.autoApproveDefaultPush}<span className="ui-risk">{old.autoApproveDefaultPushRisk}</span></label>
      <label><input type="checkbox" checked={repository.autoApprove.createPullRequest} onChange={(event) => onAutoApprove("createPullRequest", event.target.checked)} />{old.autoApprovePullRequest}</label>
      <label><input type="checkbox" checked={repository.autoApprove.mergePullRequest} onChange={(event) => onAutoApprove("mergePullRequest", event.target.checked)} />{old.autoApproveMerge}<span className="ui-risk">{old.autoApproveMergeRisk}</span></label>
      <label><input type="checkbox" checked={repository.autoApprove.publishRepository} onChange={(event) => onAutoApprove("publishRepository", event.target.checked)} />{old.autoApprovePublish}<span className="ui-risk">{old.autoApprovePublishRisk}</span></label>
    </fieldset>

    <section className="ui-remove">
      {removing ? <div className="ui-card"><p>{copy.removeConfirm(repository.name)}</p><div className="ui-actions"><button className="ui-button ui-button--danger" type="button" onClick={onRemove}>{copy.removeAction}</button><button className="ui-button" type="button" onClick={onCancelRemove}>{copy.cancel}</button></div></div>
        : <button className="ui-link-danger" type="button" onClick={onStartRemove}>{copy.remove}</button>}
    </section>
  </div>;
}
