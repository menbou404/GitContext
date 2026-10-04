import { useState } from "react";
import { PlusIcon, SearchIcon } from "../Icons";
import { shellCopy, type Locale } from "../i18n";
import type { Profile, RepositoryRecord, RepositoryStatus } from "../types";
import { ProfileDot } from "./ProfileDot";
import { StatusLabel } from "./StatusLabel";
import { filterRepositories, type RepositoryFilter } from "./status";

export function RepositoryList({ repositories, profiles, statuses, locale, busy, onOpen, onAdd, onClone, onRefresh }: {
  repositories: RepositoryRecord[];
  profiles: Profile[];
  statuses: Record<string, RepositoryStatus>;
  locale: Locale;
  busy: boolean;
  onOpen: (id: string) => void;
  onAdd: () => void;
  onClone: () => void;
  onRefresh: () => void;
}) {
  const copy = shellCopy[locale];
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<RepositoryFilter>("all");
  const visible = filterRepositories(repositories, profiles, statuses, query, filter);
  const counts = {
    all: repositories.length,
    action: repositories.filter((repo) => ["reapply", "attention"].includes(statuses[repo.id]?.state)).length,
    unassigned: repositories.filter((repo) => (statuses[repo.id]?.state ?? (repo.profileId ? "attention" : "unassigned")) === "unassigned").length,
  };
  return <div className="ui-page">
    <div className="ui-page-heading">
      <h1>{copy.repositories}</h1><span className="ui-muted">{copy.repositoryCount(repositories.length)}</span>
      <div className="ui-heading-spacer" />
      <label className="ui-search"><SearchIcon /><span className="ui-sr-only">{copy.search}</span><input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={copy.searchPlaceholder} /></label>
      <button className="ui-button" type="button" onClick={onClone} disabled={busy}>{copy.cloneRepository}</button>
      <button className="ui-button ui-button--primary" type="button" onClick={onAdd} disabled={busy}><PlusIcon />{copy.addRepository}</button>
    </div>
    <div className="ui-filter-row" role="group" aria-label={copy.filterLabel}>
      {(["all", "action", "unassigned"] as const).map((option) => <button type="button" key={option} aria-pressed={filter === option} onClick={() => setFilter(option)}>{copy[option === "action" ? "needsAction" : option === "unassigned" ? "unassignedFilter" : "all"]} {counts[option]}</button>)}
      <button className="ui-refresh" type="button" onClick={onRefresh}>{copy.refresh}</button>
    </div>
    <table className="ui-table" aria-label={copy.repositories}>
      <thead><tr><th scope="col">{copy.name}</th><th scope="col">{copy.profile}</th><th scope="col">{copy.branch}</th><th scope="col">{copy.status}</th><th scope="col">{copy.changes}</th></tr></thead>
      <tbody>
      {visible.map((repository) => {
        const profile = profiles.find((item) => item.id === repository.profileId);
        const status = statuses[repository.id];
        const state = status?.state ?? (profile ? "attention" : "unassigned");
        return <tr className="ui-table-row" key={repository.id} onClick={() => onOpen(repository.id)}>
          <td><button className="ui-repo-name" type="button"><strong>{repository.name}</strong><small title={repository.path}>{repository.path}</small></button></td>
          <td>{profile ? <span className="ui-profile"><ProfileDot profile={profile} />{profile.label}{profile.githubUsername && <small>@{profile.githubUsername}</small>}</span> : <span className="ui-muted">{copy.unassigned}</span>}</td>
          <td className="ui-mono">{status?.branch ?? repository.branch ?? "—"}</td>
          <td><StatusLabel state={state} locale={locale} /></td>
          <td className="ui-muted">{status?.uncommittedChanges ? copy.uncommitted(status.uncommittedChanges) : ""}</td>
        </tr>;
      })}
      {!visible.length && <tr><td className="ui-empty" colSpan={5}>{copy.noRepositories}</td></tr>}
      </tbody>
    </table>
  </div>;
}
