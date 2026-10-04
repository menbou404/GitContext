import type { GhProfileStatus, Profile, RepositoryRecord, RepositoryStatus } from "../types";

export type ProfileConnection = "connected" | "disconnected" | "different" | "checking";

export function connectionFor(profile: Profile, status?: GhProfileStatus | null): ProfileConnection {
  if (!profile.ghConfigDir || !profile.githubUsername) return "disconnected";
  if (!status) return "checking";
  if (!status.authenticated) return "disconnected";
  return status.username?.toLowerCase() === profile.githubUsername.toLowerCase() ? "connected" : "different";
}

export function assignedCount(profileId: string, repositories: RepositoryRecord[]): number {
  return repositories.filter((repository) => repository.profileId === profileId).length;
}

export function repositoryProfileStatus(profileId: string, statuses: Record<string, RepositoryStatus>, repositories: RepositoryRecord[]): GhProfileStatus | null {
  const repository = repositories.find((item) => item.profileId === profileId && statuses[item.id]?.github);
  return repository ? statuses[repository.id].github ?? null : null;
}

export function firstRunSteps(profiles: Profile[], repositories: RepositoryRecord[], statuses: Record<string, GhProfileStatus>): [boolean, boolean, boolean] {
  return [
    profiles.length > 0,
    profiles.some((profile) => connectionFor(profile, statuses[profile.id]) === "connected"),
    repositories.length > 0,
  ];
}
