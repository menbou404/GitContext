import { useEffect, useState } from "react";
import { previewRepositorySync, pullRepository, previewPush, pushRepository, publishRepository } from "../backend";
import { tabCopy, type Locale } from "../i18n";
import type { AppData, Profile, PushPreview, RepositoryRecord, RepositoryVisibility, SyncPreview } from "../types";
import { ConfirmPanel } from "./ConfirmPanel";
import { ProfileDot } from "./ProfileDot";
import { runtimeError } from "./runtimeError";

export function SyncTab({ repository, profile, locale, onData, onFinished }: { repository: RepositoryRecord; profile: Profile | null; locale: Locale; onData: (data: AppData) => void; onFinished: () => void }) {
  const copy = tabCopy[locale];
  const [preview, setPreview] = useState<SyncPreview | null>(null);
  const [pushPreview, setPushPreview] = useState<PushPreview | null>(null);
  const [action, setAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [name, setName] = useState(repository.name);
  const [description, setDescription] = useState("");
  const [visibility, setVisibility] = useState<RepositoryVisibility>("private");

  const fetch = async () => {
    if (!profile || !repository.remoteUrl) return;
    setAction("fetch"); setError(null);
    try {
      const [next, push] = await Promise.all([previewRepositorySync(repository.id, profile.id), previewPush(repository.id, profile.id)]);
      setPreview(next); setPushPreview(push); setResult(copy.fetchDone);
    } catch (cause) { setError(copy.actionFailed("fetch", runtimeError(cause, locale))); }
    finally { setAction(null); }
  };
  useEffect(() => { void fetch(); }, [repository.id, repository.remoteUrl, profile?.id]);

  const run = async (next: "pull" | "push") => {
    if (!profile) return;
    setAction(next); setError(null); setResult(null);
    try {
      if (next === "pull") setPreview(await pullRepository(repository.id, profile.id));
      else { await pushRepository(repository.id, profile.id); if (preview) setPreview(await previewRepositorySync(repository.id, profile.id).catch(() => preview)); }
      setResult(next === "pull" ? copy.pullDone : copy.pushDone); onFinished();
    } catch (cause) { setError(copy.actionFailed(next, runtimeError(cause, locale))); }
    finally { setAction(null); }
  };
  const publish = async () => {
    if (!profile) return;
    setAction("publish"); setError(null); setResult(null);
    try {
      const published = await publishRepository({ repositoryId: repository.id, profileId: profile.id, name, visibility, description });
      onData(published.data); onFinished(); setResult(copy.publishDone);
    } catch (cause) { setError(copy.actionFailed(copy.publish, runtimeError(cause, locale))); }
    finally { setAction(null); }
  };

  if (!profile) return <p className="ui-warning">{copy.appliedRequired}</p>;
  if (!repository.remoteUrl) return <section className="ui-card ui-publish"><h2>{copy.publish}</h2>
    <div className="ui-form-grid"><label className="ui-field"><span>{copy.repositoryName}</span><input required maxLength={100} pattern="[A-Za-z0-9._-]+" value={name} onChange={(event) => setName(event.target.value)} disabled={Boolean(action)} /></label>
      <label className="ui-field"><span>{copy.visibility}</span><select value={visibility} onChange={(event) => setVisibility(event.target.value as RepositoryVisibility)} disabled={Boolean(action)}><option value="private">{copy.private}</option><option value="public">{copy.public}</option></select></label>
      <label className="ui-field"><span>{copy.description}</span><input value={description} maxLength={350} onChange={(event) => setDescription(event.target.value)} disabled={Boolean(action)} /></label></div>
    <ConfirmPanel locale={locale} rows={[
      { label: copy.profile, value: <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}</span> },
      { label: copy.author, value: `${profile.gitName} <${profile.gitEmail}>` },
      { label: copy.branch, value: <code>{repository.branch ?? "—"}</code> },
      { label: copy.destination, value: <code>@{profile.githubUsername ?? "—"}/{name || "—"} ({visibility === "public" ? copy.public : copy.private})</code> },
    ]}><button className="ui-button ui-button--primary" type="button" onClick={() => void publish()} disabled={Boolean(action) || !profile.githubUsername || !/^[A-Za-z0-9._-]{1,100}$/.test(name)}>{action === "publish" ? copy.loading : copy.publish}</button></ConfirmPanel>
    {error && <p className="ui-warning" role="alert">{error}</p>}{result && <p className="ui-success" role="status">{result}</p>}
  </section>;

  const diverged = Boolean(preview && preview.ahead > 0 && preview.behind > 0);
  const dirty = Boolean(preview?.changes.length);
  const canPull = Boolean(preview && preview.behind > 0 && preview.ahead === 0 && !dirty && preview.remoteBranch);
  const canPush = Boolean(preview && !diverged && (preview.ahead > 0 || !preview.remoteBranch));
  return <div className="ui-tab-main"><section className="ui-card"><div className="ui-card-heading"><h2>{copy.sync}</h2><button className="ui-button" type="button" onClick={() => void fetch()} disabled={Boolean(action)}>{action === "fetch" ? copy.loading : copy.fetch}</button></div>
    {preview ? <><dl className="ui-sync-details"><div><dt>{copy.branch}</dt><dd><code>{preview.branch}</code></dd></div><div><dt>{copy.tracking}</dt><dd><code>{preview.upstream ?? preview.remoteBranch ?? copy.unknown}</code></dd></div><div><dt>{copy.lastFetched}</dt><dd>{new Date(preview.fetchedAt).toLocaleString(locale === "ja" ? "ja-JP" : "en-US")}</dd></div></dl>
      <div className="ui-sync-counts"><div><strong>{preview.ahead}</strong>{copy.ahead}</div><div><strong>{preview.behind}</strong>{copy.behind}</div></div>
      {diverged && <p className="ui-warning">{copy.diverged}</p>}{dirty && <p className="ui-warning">{copy.dirty}</p>}{!preview.remoteBranch && <p className="ui-warning">{copy.noRemoteBranch}</p>}
      <ConfirmPanel locale={locale} rows={[
        { label: copy.profile, value: <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}</span> },
        { label: copy.author, value: `${profile.gitName} <${profile.gitEmail}>` },
        { label: copy.branch, value: <code>{preview.branch}</code> },
        { label: copy.target, value: `${preview.behind} ${copy.behind} / ${preview.ahead} ${copy.ahead}` },
        { label: copy.destination, value: <code>{pushPreview?.remoteUrl ?? preview.remoteUrl} → {pushPreview?.upstream ?? `origin/${preview.branch}`}</code> },
      ]}><button className="ui-button" type="button" onClick={() => void run("pull")} disabled={Boolean(action) || !canPull}>{action === "pull" ? copy.loading : copy.pull}</button><button className="ui-button ui-button--primary" type="button" onClick={() => void run("push")} disabled={Boolean(action) || !canPush || !pushPreview}>{action === "push" ? copy.pushing : copy.push}</button></ConfirmPanel>
    </> : <p>{action === "fetch" ? copy.loading : copy.refresh}</p>}
    {error && <p className="ui-warning" role="alert">{error}</p>}{result && <p className="ui-success" role="status">{result}</p>}
  </section></div>;
}
