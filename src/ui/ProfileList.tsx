import { AlertIcon, CheckIcon, PlusIcon } from "../Icons";
import { profileCopy, shellCopy, type Locale } from "../i18n";
import type { GhProfileStatus, Profile, RepositoryRecord } from "../types";
import { ProfileDot } from "./ProfileDot";
import { assignedCount, connectionFor } from "./profileStatus";

export function ProfileList({ profiles, repositories, statuses, locale, refreshing, onRefresh, onCreate, onOpen }: {
  profiles: Profile[];
  repositories: RepositoryRecord[];
  statuses: Record<string, GhProfileStatus>;
  locale: Locale;
  refreshing: boolean;
  onRefresh: () => void;
  onCreate: () => void;
  onOpen: (profile: Profile) => void;
}) {
  const copy = profileCopy[locale];
  return <div className="ui-page">
    <div className="ui-page-heading"><h1>{shellCopy[locale].profiles}</h1><div className="ui-heading-spacer" />
      <button className="ui-button" type="button" onClick={onRefresh} disabled={refreshing}>{refreshing ? copy.refreshing : copy.refresh}</button>
      <button className="ui-button ui-button--primary" type="button" onClick={onCreate}><PlusIcon />{copy.create}</button>
    </div>
    <table className="ui-table ui-profile-table" aria-label={shellCopy[locale].profiles}>
      <thead><tr><th scope="col">{copy.name}</th><th scope="col">{copy.account}</th><th scope="col">{copy.connection}</th><th scope="col">{copy.assigned}</th></tr></thead>
      <tbody>{profiles.map((profile) => {
        const state = connectionFor(profile, statuses[profile.id]);
        return <tr className="ui-table-row" key={profile.id} onClick={() => onOpen(profile)}>
          <td><button className="ui-repo-name ui-profile-name" type="button" onClick={(event) => { event.stopPropagation(); onOpen(profile); }}><ProfileDot profile={profile} /><strong>{profile.label}</strong></button></td>
          <td>{profile.githubUsername ? `@${profile.githubUsername}` : <span className="ui-muted">{copy.noAccount}</span>}</td>
          <td><span className={`ui-status ui-status--${state === "connected" ? "ready" : state === "different" ? "attention" : "unassigned"}`}>
            {state === "connected" ? <CheckIcon /> : <AlertIcon />}<span>{copy[state]}</span>
          </span></td>
          <td>{copy.repositoryCount(assignedCount(profile.id, repositories))}</td>
        </tr>;
      })}
      {!profiles.length && <tr><td className="ui-empty" colSpan={4}>{copy.empty}</td></tr>}</tbody>
    </table>
  </div>;
}
