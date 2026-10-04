import type { RepositoryRecord, RepositoryState, RepositoryStatus, Profile } from "../types";

export type RepositoryFilter = "all" | "action" | "unassigned";

export function filterRepositories(
  repositories: RepositoryRecord[],
  profiles: Profile[],
  statuses: Record<string, RepositoryStatus>,
  query: string,
  filter: RepositoryFilter,
): RepositoryRecord[] {
  const term = query.trim().toLocaleLowerCase();
  return repositories.filter((repository) => {
    const state: RepositoryState = statuses[repository.id]?.state ?? (repository.profileId ? "attention" : "unassigned");
    if (filter === "action" && state !== "reapply" && state !== "attention") return false;
    if (filter === "unassigned" && state !== "unassigned") return false;
    const profile = profiles.find((item) => item.id === repository.profileId);
    return !term || [repository.name, repository.path, profile?.label ?? "", profile?.githubUsername ?? ""]
      .some((value) => value.toLocaleLowerCase().includes(term));
  });
}
