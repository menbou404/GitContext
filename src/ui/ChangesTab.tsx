import { useEffect, useState } from "react";
import { commitRepository, previewCommit, pushRepository } from "../backend";
import { shellCopy, tabCopy, type Locale } from "../i18n";
import type { CommitPreview, Profile, RepositoryRecord, RepositoryStatus } from "../types";
import { ConfirmPanel } from "./ConfirmPanel";
import { ProfileDot } from "./ProfileDot";
import { runtimeError } from "./runtimeError";

export function canCommit(changes: number, message: string, push: boolean, remoteUrl?: string | null): boolean {
  return changes > 0 && message.trim().length > 0 && message.trim().length <= 200 && !/[\r\n]/.test(message) && (!push || Boolean(remoteUrl));
}

export function ChangesTab({ repository, profile, status, locale, onFinished, onOverview }: { repository: RepositoryRecord; profile: Profile | null; status: RepositoryStatus | undefined; locale: Locale; onFinished: () => void; onOverview: () => void }) {
  const copy = tabCopy[locale];
  const shell = shellCopy[locale];
  const [preview, setPreview] = useState<CommitPreview | null>(null);
  const [message, setMessage] = useState("");
  const [running, setRunning] = useState<"commit" | "push" | "refresh" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const refresh = async () => {
    if (!profile) return;
    setRunning("refresh"); setError(null);
    try { setPreview(await previewCommit(repository.id, profile.id)); }
    catch (cause) { setError(copy.actionFailed("commit", runtimeError(cause, locale))); }
    finally { setRunning(null); }
  };
  useEffect(() => { void refresh(); }, [repository.id, profile?.id]);

  const valid = canCommit(preview?.changes.length ?? 0, message, false);
  const commit = async (push: boolean) => {
    if (!preview || !profile || !canCommit(preview.changes.length, message, push, preview.pushRemoteUrl)) return;
    setRunning(push ? "push" : "commit"); setError(null); setResult(null);
    let committed = false;
    try {
      await commitRepository(repository.id, profile.id, message.trim());
      committed = true;
      if (push) await pushRepository(repository.id, profile.id);
      setResult(push ? copy.commitPushDone : copy.commitDone);
      setMessage("");
      setPreview(await previewCommit(repository.id, profile.id));
      onFinished();
    } catch (cause) {
      setError(copy.actionFailed(push && committed ? "push" : "commit", runtimeError(cause, locale)));
      if (committed) { setResult(copy.commitDone); onFinished(); await previewCommit(repository.id, profile.id).then(setPreview).catch(() => undefined); }
    } finally { setRunning(null); }
  };

  return <div className="ui-detail-columns">
    <div className="ui-tab-main">
      <section className="ui-card ui-file-card"><div className="ui-card-heading"><h2>{copy.changedFiles}</h2><span>{copy.branch} <code>{preview?.branch ?? repository.branch ?? "—"}</code></span></div>
        {preview?.changes.length ? <div className="ui-file-list">{preview.changes.map((change, index) => <div key={`${change.path}-${index}`}><code className={`ui-file-status ui-file-status--${change.status[0] === "?" ? "added" : change.status[0] === "D" ? "deleted" : "modified"}`}>{change.status === "??" ? "A" : change.status.trim() || "M"}</code><code>{change.path}</code></div>)}</div> : <p className="ui-muted">{running === "refresh" ? copy.loading : copy.noChanges}</p>}
      </section>
      <section className="ui-card ui-commit-card"><h2>commit</h2>
        <label className="ui-field"><span>{copy.commitMessage}</span><input value={message} maxLength={200} onChange={(event) => setMessage(event.target.value)} disabled={Boolean(running)} /><small>{copy.messageHint}</small></label>
        {preview && <ConfirmPanel locale={locale} rows={[
          { label: copy.profile, value: <span className="ui-profile"><ProfileDot profile={preview.profile} />{preview.profile.label}{preview.profile.githubUsername && <small>@{preview.profile.githubUsername}</small>}</span> },
          { label: copy.author, value: `${preview.profile.gitName} <${preview.profile.gitEmail}>` },
          { label: copy.branch, value: <code>{preview.branch}</code> },
          { label: copy.target, value: copy.filesCount(preview.changes.length) },
          { label: copy.destination, value: <code>{preview.pushRemoteUrl ? `${preview.pushRemoteUrl} → origin/${preview.branch}` : copy.noOrigin}</code> },
        ]}>
          <button className="ui-button" type="button" onClick={() => void commit(false)} disabled={Boolean(running) || !valid || !preview.changes.length}>{running === "commit" ? copy.committing : copy.commitOnly}</button>
          <button className="ui-button ui-button--primary" type="button" onClick={() => void commit(true)} disabled={Boolean(running) || !valid || !preview.changes.length || !preview.pushRemoteUrl}>{running === "push" ? copy.pushing : copy.commitAndPush}</button>
        </ConfirmPanel>}
        {!profile && <p className="ui-warning">{copy.appliedRequired}</p>}
        {error && <p className="ui-warning" role="alert">{error}</p>}{result && <p className="ui-success" role="status">{result}</p>}
        <button className="ui-button" type="button" onClick={() => void refresh()} disabled={Boolean(running) || !profile}>{copy.refresh}</button>
      </section>
    </div>
    <aside className="ui-card ui-tab-aside"><h2>{copy.repositorySettings}</h2>{profile ? <div className="ui-aside-settings">
      <div><strong>{shell.author}</strong><span>{status?.identityInSync ? shell.matches : shell.differs}</span></div>
      <div><strong>{shell.sshKey}</strong><code>{profile.sshKeyPath ?? shell.notManaged}</code></div>
      <div><strong>{shell.github}</strong><span>{status?.github?.authenticated ? status.github.username ? shell.authenticatedAs(status.github.username) : shell.connected : shell.notConnected}</span></div>
      <div><strong>{shell.origin}</strong><code>{repository.remoteUrl ?? shell.noOrigin}</code></div>
    </div> : <p>{copy.appliedRequired}</p>}<button className="ui-button" type="button" onClick={onOverview}>{copy.viewOverview}</button></aside>
  </div>;
}
