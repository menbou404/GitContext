import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { demoBootstrap } from "./demoData";
import type {
  AppData,
  ApplyPreview,
  BootstrapResult,
  GhProfileStatus,
  Profile,
  RepositoryRecord,
} from "./types";

let demoState: AppData = structuredClone(demoBootstrap.data);

const inDesktopApp = () => isTauri();

const nextDemoId = (prefix: string) =>
  `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;

export async function bootstrap(): Promise<BootstrapResult> {
  if (!inDesktopApp()) {
    return { ...structuredClone(demoBootstrap), data: structuredClone(demoState) };
  }
  return invoke<BootstrapResult>("bootstrap");
}

export async function chooseRepositoryDirectory(title = "Select a Git repository"): Promise<string | null> {
  if (!inDesktopApp()) {
    return "C:\\Users\\you\\Projects\\new-project";
  }
  const selected = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof selected === "string" ? selected : null;
}

export async function chooseSshKey(title = "Select an existing SSH private key"): Promise<string | null> {
  if (!inDesktopApp()) {
    return "C:\\Users\\you\\.ssh\\id_ed25519_personal";
  }
  const selected = await open({
    directory: false,
    multiple: false,
    title,
  });
  return typeof selected === "string" ? selected : null;
}

export async function chooseGhConfigDirectory(title = "Select an existing GitHub CLI config directory"): Promise<string | null> {
  if (!inDesktopApp()) {
    return "C:\\Users\\you\\.config\\gh-personal";
  }
  const selected = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof selected === "string" ? selected : null;
}

export async function addRepository(path: string): Promise<RepositoryRecord> {
  if (!inDesktopApp()) {
    const pathParts = path.split(/[\\/]/).filter(Boolean);
    const name = pathParts[pathParts.length - 1] ?? "repository";
    const record: RepositoryRecord = {
      id: nextDemoId("repo"),
      name,
      path,
      remoteUrl: "git@github.com:example/new-project.git",
      branch: "main",
      profileId: null,
      lastAppliedAt: null,
    };
    demoState.repositories = [...demoState.repositories, record];
    return structuredClone(record);
  }
  return invoke<RepositoryRecord>("add_repository", { path });
}

export async function removeRepository(id: string): Promise<AppData> {
  if (!inDesktopApp()) {
    demoState.repositories = demoState.repositories.filter((repo) => repo.id !== id);
    return structuredClone(demoState);
  }
  return invoke<AppData>("remove_repository", { id });
}

export async function saveProfile(profile: Profile): Promise<AppData> {
  if (!inDesktopApp()) {
    const next = { ...profile, id: profile.id || nextDemoId("profile") };
    const exists = demoState.profiles.some((item) => item.id === next.id);
    demoState.profiles = exists
      ? demoState.profiles.map((item) => (item.id === next.id ? next : item))
      : [...demoState.profiles, next];
    return structuredClone(demoState);
  }
  return invoke<AppData>("save_profile", { profile });
}

export async function inspectGithubProfile(
  profileId: string,
  ghConfigDir?: string | null,
): Promise<GhProfileStatus> {
  if (!inDesktopApp()) {
    return {
      available: true,
      authenticated: Boolean(ghConfigDir),
      username: ghConfigDir ? "connected-account" : null,
      detail: ghConfigDir ? null : "This Profile has not been connected to GitHub yet.",
      configDir: ghConfigDir || `C:\\Users\\you\\AppData\\Roaming\\app.gitcontext.desktop\\gh\\${profileId}`,
    };
  }
  return invoke<GhProfileStatus>("inspect_github_profile", { profileId, ghConfigDir });
}

export async function connectGithubProfile(
  profileId: string,
  ghConfigDir?: string | null,
): Promise<GhProfileStatus> {
  if (!inDesktopApp()) {
    return {
      available: true,
      authenticated: true,
      username: "connected-account",
      detail: null,
      configDir: ghConfigDir || `C:\\Users\\you\\AppData\\Roaming\\app.gitcontext.desktop\\gh\\${profileId}`,
    };
  }
  return invoke<GhProfileStatus>("connect_github_profile", { profileId, ghConfigDir });
}

export async function previewAssignment(
  repositoryId: string,
  profileId: string,
): Promise<ApplyPreview> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((repo) => repo.id === repositoryId);
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!repository || !profile) throw new Error("Repository or profile was not found.");
    const current = demoState.profiles.find((item) => item.id === repository.profileId);
    return {
      repository: structuredClone(repository),
      profile: structuredClone(profile),
      warnings: demoBootstrap.environment.gh.available
        ? []
        : ["GitHub CLI is unavailable, so gh integration will remain inactive."],
      changes: [
        { key: "user.name", currentValue: current?.gitName ?? null, nextValue: profile.gitName },
        { key: "user.email", currentValue: current?.gitEmail ?? null, nextValue: profile.gitEmail },
        {
          key: "core.sshCommand",
          currentValue: current?.sshKeyPath ? "Managed SSH identity" : null,
          nextValue: profile.sshKeyPath ? `ssh -i \"${profile.sshKeyPath}\" -o IdentitiesOnly=yes` : "No change",
        },
        { key: "gitcontext.profileId", currentValue: repository.profileId ?? null, nextValue: profile.id },
      ],
    };
  }
  return invoke<ApplyPreview>("preview_assignment", { repositoryId, profileId });
}

export async function applyAssignment(
  repositoryId: string,
  profileId: string,
): Promise<AppData> {
  if (!inDesktopApp()) {
    demoState.repositories = demoState.repositories.map((repo) =>
      repo.id === repositoryId
        ? { ...repo, profileId, lastAppliedAt: new Date().toISOString() }
        : repo,
    );
    return structuredClone(demoState);
  }
  return invoke<AppData>("apply_profile", { repositoryId, profileId });
}
