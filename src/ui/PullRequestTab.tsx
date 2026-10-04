import { useEffect, useState } from "react";
import { commitRepository, createBranch, createPullRequest, listPullRequests, mergePullRequest, previewPullRequest, pushRepository } from "../backend";
import { tabCopy, uiCopy, type Locale } from "../i18n";
import type { AppData, ManagedPullRequest, MergeStrategy, Profile, PullRequestManagement, PullRequestPreview, RepositoryRecord } from "../types";
import { ConfirmPanel } from "./ConfirmPanel";
import { ProfileDot } from "./ProfileDot";
import { runtimeError } from "./runtimeError";

const blockReason = (pr: ManagedPullRequest, locale: Locale) => {
  const copy = uiCopy[locale];
  if (pr.isDraft) return copy.draftCannotMerge;
  if (pr.reviewDecision === "CHANGES_REQUESTED") return copy.changesRequestedCannotMerge;
  if (pr.checks.some((check) => check.bucket === "fail" || check.bucket === "cancel")) return copy.ciFailedCannotMerge;
  if (pr.checks.some((check) => check.bucket === "pending")) return copy.ciPendingCannotMerge;
  if (pr.mergeable === "CONFLICTING") return copy.conflictCannotMerge;
  if (pr.mergeable !== "MERGEABLE" || pr.mergeStateStatus !== "CLEAN") return copy.policyCannotMerge;
  return null;
};

export function PullRequestTab({ repository, profile, locale, onData, onFinished }: { repository: RepositoryRecord; profile: Profile | null; locale: Locale; onData: (data: AppData) => void; onFinished: () => void }) {
  const copy = tabCopy[locale];
  const old = uiCopy[locale];
  const [preview, setPreview] = useState<PullRequestPreview | null>(null);
  const [management, setManagement] = useState<PullRequestManagement | null>(null);
  const [branchName, setBranchName] = useState("");
  const [commitMessage, setCommitMessage] = useState("");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [draft, setDraft] = useState(false);
  const [selectedNumber, setSelectedNumber] = useState<number | null>(null);
  const [strategy, setStrategy] = useState<MergeStrategy>("squash");
  const [confirmed, setConfirmed] = useState(false);
  const [running, setRunning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const refresh = async () => {
    if (!profile || !repository.remoteUrl) return;
    setRunning("refresh"); setError(null);
    const loaded = await Promise.allSettled([previewPullRequest(repository.id, profile.id), listPullRequests(repository.id, profile.id)]);
    if (loaded[0].status === "fulfilled") setPreview(loaded[0].value);
    if (loaded[1].status === "fulfilled") setManagement(loaded[1].value);
    const failure = loaded.find((item) => item.status === "rejected");
    if (failure?.status === "rejected") setError(copy.actionFailed("Pull Request", runtimeError(failure.reason, locale)));
    setRunning(null);
  };
  useEffect(() => { void refresh(); }, [repository.id, repository.remoteUrl, profile?.id]);

  const hasChanges = Boolean(preview?.changes.length);
  const hasProposal = Boolean(preview && (hasChanges || preview.commitsAhead > 0));
  const branchValid = Boolean(preview && (!preview.requiresNewBranch || (branchName.trim().length > 0 && branchName.trim().length <= 200)));
  const messageValid = !hasChanges || (commitMessage.trim().length > 0 && commitMessage.trim().length <= 200 && !/[\r\n]/.test(commitMessage));
  const titleValid = title.trim().length > 0 && title.trim().length <= 256 && !/[\r\n]/.test(title);
  const create = async () => {
    if (!preview || !profile || !hasProposal || !branchValid || !messageValid || !titleValid) return;
    setError(null); setResult(null); setRunning("branch");
    try {
      if (preview.requiresNewBranch) {
        const created = await createBranch(repository.id, profile.id, branchName.trim());
        onData(created.data);
      }
      if (hasChanges) { setRunning("commit"); await commitRepository(repository.id, profile.id, commitMessage.trim()); }
      if (preview.requiresNewBranch || hasChanges || !preview.branchPushed) { setRunning("push"); await pushRepository(repository.id, profile.id); }
      setRunning("pr");
      const created = await createPullRequest({ repositoryId: repository.id, profileId: profile.id, baseBranch: preview.baseBranch, title: title.trim(), body, draft });
      setResult(created.existing ? copy.existingDone : copy.createDone); onFinished();
      await Promise.allSettled([
        listPullRequests(repository.id, profile.id).then(setManagement),
        previewPullRequest(repository.id, profile.id).then(setPreview),
      ]);
    } catch (cause) {
      setError(copy.actionFailed("Pull Request", runtimeError(cause, locale)));
      await previewPullRequest(repository.id, profile.id).then(setPreview).catch(() => undefined);
    } finally { setRunning(null); }
  };

  const selected = management?.pullRequests.find((item) => item.number === selectedNumber) ?? management?.pullRequests[0] ?? null;
  const blocked = selected ? blockReason(selected, locale) : null;
  const merge = async () => {
    if (!selected || !profile || blocked || !confirmed) return;
    setRunning("merge"); setError(null); setResult(null);
    try {
      await mergePullRequest({ repositoryId: repository.id, profileId: profile.id, number: selected.number, strategy, expectedHeadOid: selected.headOid });
      setResult(copy.mergeDone); setConfirmed(false); onFinished();
      await listPullRequests(repository.id, profile.id).then(setManagement).catch(() => undefined);
    } catch (cause) { setError(copy.actionFailed("merge", runtimeError(cause, locale))); setConfirmed(false); }
    finally { setRunning(null); }
  };

  if (!profile || !repository.remoteUrl) return <p className="ui-warning">{copy.unavailable}</p>;
  return <div className="ui-tab-main ui-pr-tab">
    <section className="ui-card"><h2>{copy.create}</h2>
      {preview ? <>
        {preview.requiresNewBranch && <label className="ui-field"><span>{copy.newBranch}</span><input value={branchName} maxLength={200} onChange={(event) => setBranchName(event.target.value)} disabled={Boolean(running)} /></label>}
        {hasChanges && <><div className="ui-file-list">{preview.changes.map((change, index) => <div key={`${change.path}-${index}`}><code>{change.status}</code><code>{change.path}</code></div>)}</div><label className="ui-field"><span>{copy.commitMessage}</span><input value={commitMessage} maxLength={200} onChange={(event) => setCommitMessage(event.target.value)} disabled={Boolean(running)} /></label></>}
        {!hasProposal && <p className="ui-warning">{copy.noProposal}</p>}
        {preview.existingPullRequest && <p className="ui-warning">{old.existingPullRequestFound(preview.existingPullRequest.number)}</p>}
        <label className="ui-field"><span>{copy.title}</span><input value={title} maxLength={256} onChange={(event) => setTitle(event.target.value)} disabled={Boolean(running)} /></label>
        <label className="ui-field"><span>{copy.body}</span><textarea value={body} maxLength={65536} rows={4} onChange={(event) => setBody(event.target.value)} disabled={Boolean(running)} /></label>
        <label className="ui-check"><input type="checkbox" checked={draft} onChange={(event) => setDraft(event.target.checked)} disabled={Boolean(running)} />{copy.draft}</label>
        <ConfirmPanel locale={locale} rows={[
          { label: copy.profile, value: <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}</span> },
          { label: copy.author, value: `${profile.gitName} <${profile.gitEmail}>` },
          { label: copy.branch, value: <code>{preview.requiresNewBranch ? branchName.trim() || "—" : preview.currentBranch}</code> },
          { label: copy.target, value: `${copy.filesCount(preview.changes.length)}, ${preview.commitsAhead} commit` },
          { label: copy.destination, value: <code>{preview.repositoryNameWithOwner}: {preview.requiresNewBranch ? branchName.trim() || "—" : preview.currentBranch} → {preview.baseBranch}</code> },
        ]}><button className="ui-button ui-button--primary" type="button" disabled={Boolean(running) || !hasProposal || !branchValid || !messageValid || !titleValid} onClick={() => void create()}>{running && running !== "refresh" && running !== "merge" ? copy.createProgress : copy.create}</button></ConfirmPanel>
      </> : <p>{running === "refresh" ? copy.loading : copy.unavailable}</p>}
    </section>
    <section className="ui-card"><div className="ui-card-heading"><h2>{copy.open}</h2><button className="ui-button" type="button" onClick={() => void refresh()} disabled={Boolean(running)}>{copy.refresh}</button></div>
      {management?.pullRequests.length ? <div className="ui-pr-layout"><div className="ui-pr-list">{management.pullRequests.map((pr) => <button className="ui-button" type="button" key={pr.number} aria-pressed={selected?.number === pr.number} onClick={() => { setSelectedNumber(pr.number); setConfirmed(false); setError(null); }} disabled={Boolean(running)}><strong>#{pr.number} {pr.title}</strong><small>{pr.headBranch} → {pr.baseBranch}</small></button>)}</div>
        {selected && <div className="ui-pr-detail"><h3>#{selected.number} {selected.title}</h3><p><code>{selected.headBranch} → {selected.baseBranch}</code></p><p>{copy.review}: {selected.reviewDecision || "—"}</p><h4>{copy.checks}</h4>{selected.checks.length ? <ul>{selected.checks.map((check, index) => <li key={`${check.name}-${index}`}>{check.name}: {check.bucket} {check.link && <a href={check.link} target="_blank" rel="noreferrer">{copy.viewGithub}</a>}</li>)}</ul> : <p>{old.ciNotConfigured}</p>}
          {blocked && <p className="ui-warning">{blocked}</p>}
          <label className="ui-field"><span>{copy.strategy}</span><select value={strategy} onChange={(event) => { setStrategy(event.target.value as MergeStrategy); setConfirmed(false); }} disabled={Boolean(running)}><option value="squash">{copy.squash}</option><option value="merge">{copy.mergeCommit}</option><option value="rebase">{copy.rebase}</option></select></label>
          <ConfirmPanel locale={locale} rows={[
            { label: copy.profile, value: <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}</span> },
            { label: copy.author, value: selected.author ?? "GitHub" },
            { label: copy.target, value: `#${selected.number} ${selected.title}` },
            { label: copy.branch, value: <code>{selected.headBranch}</code> },
            { label: copy.destination, value: <code>{selected.baseBranch}</code> },
            { label: copy.strategy, value: strategy },
          ]}><label className="ui-check"><input type="checkbox" checked={confirmed} disabled={Boolean(blocked) || Boolean(running)} onChange={(event) => setConfirmed(event.target.checked)} />{copy.confirmMerge}</label><button className="ui-button ui-button--primary" type="button" disabled={Boolean(blocked) || !confirmed || Boolean(running)} onClick={() => void merge()}>{running === "merge" ? copy.merging : copy.merge}</button></ConfirmPanel>
          <a href={selected.url} target="_blank" rel="noreferrer">{copy.viewGithub}</a>
        </div>}</div> : <p>{running === "refresh" ? copy.loading : copy.noOpen}</p>}
    </section>
    {error && <p className="ui-warning" role="alert">{error}</p>}{result && <p className="ui-success" role="status">{result}</p>}
  </div>;
}
