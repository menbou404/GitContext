import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { demoBootstrap } from "./demoData";
import type {
  AppData,
  ApplyPreview,
  BootstrapResult,
  BranchResult,
  CloneOptions,
  CloneResult,
  CommitPreview,
  CommitResult,
  GhProfileStatus,
  GithubRepository,
  GithubAuthPrompt,
  Profile,
  PublishOptions,
  PublishResult,
  PullRequestCreateOptions,
  PullRequestPreview,
  PullRequestResult,
  PushPreview,
  PushResult,
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

export async function chooseCloneDestinationDirectory(title = "Select a clone destination"): Promise<string | null> {
  if (!inDesktopApp()) {
    return "C:\\Users\\you\\Projects";
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
      remoteUrl: null,
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

export async function listGithubRepositories(profileId: string): Promise<GithubRepository[]> {
  if (!inDesktopApp()) {
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!profile) throw new Error("Profile was not found.");
    const owner = profile.githubUsername || "connected-account";
    return [
      {
        name: "example-project",
        nameWithOwner: `${owner}/example-project`,
        description: "Repository available to the selected Profile",
        isPrivate: true,
        sshUrl: `git@github.com:${owner}/example-project.git`,
        updatedAt: new Date().toISOString(),
      },
    ];
  }
  return invoke<GithubRepository[]>("list_github_repositories", { profileId });
}

export async function cloneGithubRepository(options: CloneOptions): Promise<CloneResult> {
  if (!inDesktopApp()) {
    const profile = demoState.profiles.find((item) => item.id === options.profileId);
    if (!profile) throw new Error("Profile was not found.");
    const match = options.repositoryUrl.trim().match(/^git@github\.com:[A-Za-z0-9-]+\/([A-Za-z0-9._-]+)\.git$/);
    if (!match) throw new Error("Use a GitHub SSH URL in the form git@github.com:owner/repository.git.");
    const name = match[1];
    const separator = options.destinationParent.includes("\\") ? "\\" : "/";
    const repository: RepositoryRecord = {
      id: nextDemoId("repo"),
      name,
      path: `${options.destinationParent.replace(/[\\/]$/, "")}${separator}${name}`,
      remoteUrl: options.repositoryUrl.trim(),
      branch: "main",
      profileId: profile.id,
      lastAppliedAt: new Date().toISOString(),
    };
    demoState.repositories = [...demoState.repositories, repository];
    return { data: structuredClone(demoState), repository: structuredClone(repository) };
  }
  return invoke<CloneResult>("clone_repository", {
    profileId: options.profileId,
    repositoryUrl: options.repositoryUrl,
    destinationParent: options.destinationParent,
  });
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

export async function listenForGithubAuthPrompt(
  handler: (prompt: GithubAuthPrompt) => void,
): Promise<UnlistenFn> {
  if (!inDesktopApp()) return () => undefined;
  return listen<GithubAuthPrompt>("github-auth-prompt", (event) => handler(event.payload));
}

export async function openGithubAuthPage(): Promise<void> {
  if (!inDesktopApp()) {
    window.open("https://github.com/login/device", "_blank", "noopener,noreferrer");
    return;
  }
  return invoke<void>("open_github_auth_page");
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

export async function publishRepository(options: PublishOptions): Promise<PublishResult> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === options.repositoryId);
    const profile = demoState.profiles.find((item) => item.id === options.profileId);
    if (!repository || !profile?.githubUsername) throw new Error("Repository or profile was not found.");
    const repositoryUrl = `https://github.com/${profile.githubUsername}/${options.name}`;
    demoState.repositories = demoState.repositories.map((item) =>
      item.id === options.repositoryId
        ? { ...item, remoteUrl: `git@github.com:${profile.githubUsername}/${options.name}.git` }
        : item,
    );
    return { data: structuredClone(demoState), repositoryUrl };
  }
  return invoke<PublishResult>("publish_repository", {
    repositoryId: options.repositoryId,
    profileId: options.profileId,
    name: options.name,
    visibility: options.visibility,
    description: options.description,
  });
}

export async function previewPush(repositoryId: string, profileId: string): Promise<PushPreview> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === repositoryId);
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!repository || !profile || !repository.remoteUrl) throw new Error("Repository or profile was not found.");
    return {
      repository: structuredClone(repository),
      profile: structuredClone(profile),
      branch: repository.branch || "main",
      remoteUrl: repository.remoteUrl,
      upstream: repository.branch ? `origin/${repository.branch}` : null,
      hasUncommittedChanges: false,
    };
  }
  return invoke<PushPreview>("preview_push", { repositoryId, profileId });
}

export async function pushRepository(repositoryId: string, profileId: string): Promise<PushResult> {
  if (!inDesktopApp()) {
    const preview = await previewPush(repositoryId, profileId);
    return {
      branch: preview.branch,
      remoteUrl: preview.remoteUrl,
      detail: "Everything up-to-date",
    };
  }
  return invoke<PushResult>("push_repository", { repositoryId, profileId });
}

export async function previewCommit(repositoryId: string, profileId: string): Promise<CommitPreview> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === repositoryId);
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!repository || !profile) throw new Error("Repository or profile was not found.");
    return {
      repository: structuredClone(repository),
      profile: structuredClone(profile),
      branch: repository.branch || "main",
      changes: [
        { status: "M", path: "src/App.tsx" },
        { status: "??", path: "notes.md" },
      ],
      pushRemoteUrl: repository.remoteUrl || null,
      pushUnavailableReason: repository.remoteUrl ? null : "This repository does not have an origin remote.",
    };
  }
  return invoke<CommitPreview>("preview_commit", { repositoryId, profileId });
}

export async function commitRepository(
  repositoryId: string,
  profileId: string,
  message: string,
): Promise<CommitResult> {
  if (!inDesktopApp()) {
    const preview = await previewCommit(repositoryId, profileId);
    if (!message.trim()) throw new Error("Commit message must contain 1 to 200 characters on one line.");
    return { branch: preview.branch, commitId: "a1b2c3d", message: message.trim() };
  }
  return invoke<CommitResult>("commit_repository", { repositoryId, profileId, message });
}

export async function previewPullRequest(
  repositoryId: string,
  profileId: string,
): Promise<PullRequestPreview> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === repositoryId);
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!repository || !profile || !repository.remoteUrl) throw new Error("Repository or profile was not found.");
    const currentBranch = repository.branch || "main";
    const baseBranch = "main";
    return {
      repository: structuredClone(repository),
      profile: structuredClone(profile),
      currentBranch,
      baseBranch,
      remoteUrl: repository.remoteUrl,
      repositoryNameWithOwner: repository.remoteUrl.replace("git@github.com:", "").replace(/\.git$/, ""),
      changes: [
        { status: "M", path: "src/App.tsx" },
        { status: "??", path: "docs/pr-workflow.md" },
      ],
      commitsAhead: currentBranch === baseBranch ? 0 : 1,
      branchPushed: currentBranch !== baseBranch,
      requiresNewBranch: currentBranch === baseBranch,
      existingPullRequest: null,
    };
  }
  return invoke<PullRequestPreview>("preview_pull_request", { repositoryId, profileId });
}

export async function createBranch(
  repositoryId: string,
  profileId: string,
  branchName: string,
): Promise<BranchResult> {
  if (!inDesktopApp()) {
    demoState.repositories = demoState.repositories.map((repository) =>
      repository.id === repositoryId ? { ...repository, branch: branchName } : repository,
    );
    return { data: structuredClone(demoState), branch: branchName };
  }
  return invoke<BranchResult>("create_branch", { repositoryId, profileId, branchName });
}

export async function createPullRequest(options: PullRequestCreateOptions): Promise<PullRequestResult> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === options.repositoryId);
    const branch = repository?.branch || "feature/example";
    return {
      number: 12,
      url: "https://github.com/example/example/pull/12",
      title: options.title,
      branch,
      baseBranch: options.baseBranch,
      existing: false,
    };
  }
  return invoke<PullRequestResult>("create_pull_request", { input: options });
}
