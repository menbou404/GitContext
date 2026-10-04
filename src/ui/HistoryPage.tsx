import { useEffect, useState } from "react";
import { listHistory } from "../backend";
import { historyCopy, localizeRuntimeMessage, type Locale } from "../i18n";
import type { AppData, AuditEntry } from "../types";
import { ProfileDot } from "./ProfileDot";
import { confirmationLabel, filterHistory, historyActor, operationLabel, outcomeLabel, type ActorFilter } from "./history";

export function HistoryPage({ data, locale }: { data: AppData; locale: Locale }) {
  const copy = historyCopy[locale];
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [repositoryId, setRepositoryId] = useState("");
  const [profileId, setProfileId] = useState("");
  const [actor, setActor] = useState<ActorFilter>("all");
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    let active = true;
    listHistory().then((items) => {
      if (active) { setEntries(items); setError(null); setLoading(false); }
    }).catch((cause: unknown) => {
      if (active) {
        const message = cause instanceof Error ? cause.message : String(cause);
        setError(localizeRuntimeMessage(message, locale));
        setLoading(false);
      }
    });
    return () => { active = false; };
  }, [reloadKey, locale]);

  const visible = filterHistory(entries, repositoryId, profileId, actor);
  const repositoryOptions = [...new Set(entries.map((entry) => entry.repositoryId).filter((id): id is string => Boolean(id)))];
  const profileOptions = [...new Set(entries.map((entry) => entry.profileId).filter((id): id is string => Boolean(id)))];

  return <div className="ui-page ui-history">
    <div className="ui-page-heading"><h1>{copy.title}</h1><span className="ui-heading-spacer" /><button type="button" className="ui-button" disabled={loading} onClick={() => { setLoading(true); setReloadKey((key) => key + 1); }}>{copy.reload}</button></div>
    <p className="ui-muted ui-history-intro">{copy.description}</p>
    <div className="ui-history-filters">
      <label className="ui-field">{copy.repository}<select value={repositoryId} onChange={(event) => setRepositoryId(event.currentTarget.value)}><option value="">{copy.allRepositories}</option>{repositoryOptions.map((id) => <option key={id} value={id}>{data.repositories.find((item) => item.id === id)?.name ?? copy.deletedRepository}</option>)}</select></label>
      <label className="ui-field">{copy.profile}<select value={profileId} onChange={(event) => setProfileId(event.currentTarget.value)}><option value="">{copy.allProfiles}</option>{profileOptions.map((id) => <option key={id} value={id}>{data.profiles.find((item) => item.id === id)?.label ?? copy.deletedProfile}</option>)}</select></label>
      <label className="ui-field">{copy.actor}<select value={actor} onChange={(event) => setActor(event.currentTarget.value as ActorFilter)}><option value="all">{copy.allActors}</option><option value="gui">{copy.gui}</option><option value="ai">{copy.aiClient}</option></select></label>
    </div>
    {error && <p className="ui-history-error" role="alert">{error} {copy.retry}</p>}
    {loading ? <p role="status">{copy.loading}</p> : !visible.length ? <p className="ui-card" role="status">{copy.empty}</p> :
      <div className="ui-history-scroll"><table className="ui-table ui-history-table"><thead><tr><th>{copy.date}</th><th>{copy.operation}</th><th>{copy.repository}</th><th>{copy.profile}</th><th>{copy.actor}</th><th>{copy.result}</th><th>{copy.confirmation}</th></tr></thead><tbody>{visible.map((entry, index) => {
        const repository = data.repositories.find((item) => item.id === entry.repositoryId);
        const profile = data.profiles.find((item) => item.id === entry.profileId);
        const date = new Date(entry.at);
        const outcome = entry.outcome === "success" ? "success" : entry.outcome === "rejected" ? "rejected" : "failed";
        return <tr key={`${entry.at}-${index}`}><td><time dateTime={entry.at}>{Number.isNaN(date.valueOf()) ? entry.at : new Intl.DateTimeFormat(locale === "ja" ? "ja-JP" : "en-US", { dateStyle: "short", timeStyle: "short" }).format(date)}</time></td><td><strong>{operationLabel(entry.tool, locale)}</strong>{entry.summary && <small>{entry.summary}</small>}</td><td>{entry.repositoryId ? repository?.name ?? copy.deletedRepository : copy.none}</td><td>{profile ? <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}</span> : entry.profileId ? copy.deletedProfile : copy.none}</td><td>{historyActor(entry) === "gui" ? copy.gui : entry.client || copy.unknownClient}</td><td><span className={`ui-history-result ui-history-result--${outcome}`}><span aria-hidden="true">{outcome === "success" ? "✓" : outcome === "rejected" ? "−" : "!"}</span>{outcomeLabel(entry.outcome, locale)}</span></td><td>{confirmationLabel(entry.confirmation, locale)}</td></tr>;
      })}</tbody></table></div>}
  </div>;
}
