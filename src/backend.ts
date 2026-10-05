import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { demoBootstrap } from "./demoData";
import type {
  AiAction, AiClient, AiInventory, AiPlan, AiTier,
  AppData,
  AuditEntry,
  BackupEntry,
  AppSettings,
  AutoApprove,
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
  MergePullRequestOptions,
  MergePullRequestResult,
  Profile,
  ProfileAutoApprove,
  PublishOptions,
  PublishResult,
  PullRequestCreateOptions,
  PullRequestPreview,
  PullRequestResult,
  PullRequestManagement,
  PushPreview,
  PushResult,
  SyncPreview,
  RepositoryRecord,
  RepositoryStatus,
} from "./types";

const demoAi: AiInventory = {
  server: { path: "C:\\Demo\\GitContext\\gitcontext-mcp.exe", version: "gitcontext-mcp 0.1.0-beta.1", development: false, built: true, registrationName: "gitcontext" },
  clients: [
    { client: "claude_code", state: "connected", tier: "read", trust: false, confirmation: "mixed", configPath: "C:\\Demo\\.claude.json", registrationName: "gitcontext", command: "C:\\Demo\\GitContext\\gitcontext-mcp.exe", args: ["--max-tier", "read"], cliAvailable: true },
    { client: "codex", state: "disconnected", tier: "read", trust: false, confirmation: "unstable", configPath: "C:\\Demo\\.codex\\config.toml", registrationName: "gitcontext", command: null, args: [], cliAvailable: true },
    { client: "claude_desktop", state: "repair", tier: "read", trust: false, confirmation: "unsupported", configPath: "C:\\Demo\\Claude\\claude_desktop_config.json", registrationName: "gitcontext", command: "C:\\Old\\gitcontext-mcp.exe", args: ["--max-tier", "read"], cliAvailable: false },
  ],
};

if (typeof window !== "undefined" && !isTauri() && new URLSearchParams(window.location.search).get("demo") === "ai-intro") {
  for (const client of demoAi.clients) {
    client.state = "disconnected";
    client.command = null;
    client.args = [];
  }
}

export async function listAiClients(): Promise<AiInventory> {
  return inDesktopApp() ? invoke<AiInventory>("list_ai_clients") : structuredClone(demoAi);
}
export async function planAiClient(client: AiClient, action: AiAction, tier: AiTier, trust: boolean): Promise<AiPlan> {
  if (inDesktopApp()) return invoke<AiPlan>("plan_ai_client", { client, action, tier, trust });
  if (trust && tier !== "remote") throw new Error("Trust requires remote access.");
  const info = demoAi.clients.find((item) => item.client === client)!;
  const args = ["--max-tier", tier, ...(trust ? ["--trust-client-approval"] : [])];
  const render = (command: string, entryArgs: string[]) => client === "codex"
    ? `[mcp_servers.${info.registrationName}]
command = '${command}'
args = [${entryArgs.map((arg) => `"${arg}"`).join(", ")}]`
    : JSON.stringify({ command, args: entryArgs }, null, 2);
  const after = action === "disconnect" ? "" : render(demoAi.server.path, args);
  return { client, action, tier, trust, configPath: info.configPath, before: info.command ? render(info.command, info.args) : "", after,
    commandLine: client === "claude_code" ? action === "disconnect" ? "claude mcp remove --scope user gitcontext" : `claude mcp add --scope user gitcontext -- "${demoAi.server.path}" ${args.join(" ")}` : null,
    fileHash: null, registrationName: "gitcontext", manual: client === "claude_code" && !info.cliAvailable };
}
export async function applyAiClient(plan: AiPlan): Promise<string | null> {
  if (inDesktopApp()) return invoke<string | null>("apply_ai_client", { plan });
  const info = demoAi.clients.find((item) => item.client === plan.client)!;
  info.state = plan.action === "disconnect" ? "disconnected" : "connected";
  info.command = plan.action === "disconnect" ? null : demoAi.server.path;
  info.args = plan.action === "disconnect" ? [] : ["--max-tier", plan.tier, ...(plan.trust ? ["--trust-client-approval"] : [])];
  info.tier = plan.action === "disconnect" ? "read" : plan.tier;
  info.trust = plan.action !== "disconnect" && plan.trust;
  return "C:\\Demo\\GitContext\\backups\\ai-clients\\demo.json";
}
export async function verifyAiClient(client: AiClient): Promise<number> {
  if (inDesktopApp()) return invoke<number>("verify_ai_client", { client });
  if (demoAi.clients.find((item) => item.client === client)?.state !== "connected") throw new Error("Client is not connected.");
  return 12;
}

let demoState: AppData = typeof window !== "undefined" && !isTauri() && new URLSearchParams(window.location.search).get("demo") === "empty"
  ? { ...structuredClone(demoBootstrap.data), profiles: [], repositories: [] }
  : structuredClone(demoBootstrap.data);
const demoMergedPullRequests = new Set<number>();
const demoHistory: AuditEntry[] = [
  { at: "2026-10-04T09:25:00Z", tool: "apply_profile", repositoryId: "repo-personal", profileId: "open-source", outcome: "success", summary: "Profile applied", client: null, confirmation: null, actor: "gui" },
  { at: "2026-10-04T08:40:00Z", tool: "create_pull_request", repositoryId: "repo-school", profileId: "university-lab", outcome: "success", summary: "PR #12", client: "Demo AI", confirmation: "elicitation" },
  { at: "2026-10-03T14:05:00Z", tool: "push", repositoryId: "repo-personal", profileId: "open-source", outcome: "rejected", summary: "Remote operation rejected", client: "Demo AI", confirmation: "client" },
  { at: "2026-10-02T11:15:00Z", tool: "commit", repositoryId: "repo-reapply", profileId: "open-source", outcome: "failed", summary: "Operation failed", client: "Demo AI", confirmation: null },
  { at: "2026-10-01T10:00:00Z", tool: "pull", repositoryId: "repo-attention", profileId: "client-work", outcome: "success", summary: "Pulled", client: "Demo AI", confirmation: "auto" },
];
const demoReapplied = new Set<string>();
const demoChanges = new Map<string, CommitPreview["changes"]>([["repo-personal", [
  { status: "M", path: "src/player.ts" },
  { status: "A", path: "assets/levels/level-03.json" },
  { status: "D", path: "docs/old-notes.md" },
]]]);
const demoAhead = new Map<string, number>([["repo-personal", 1], ["repo-school", 0], ["repo-reapply", 1]]);
const demoBehind = new Map<string, number>([["repo-personal", 0], ["repo-school", 2]]);

const inDesktopApp = () => isTauri();
const demoLocaleKey = "gitcontext.locale";

const nextDemoId = (prefix: string) =>
  `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;

export async function bootstrap(): Promise<BootstrapResult> {
  if (!inDesktopApp()) {
    try {
      demoState.settings = { ...demoState.settings, locale: localStorage.getItem(demoLocaleKey) };
    } catch {
      // Browser storage can be unavailable in private or restricted contexts.
    }
    return { ...structuredClone(demoBootstrap), data: structuredClone(demoState) };
  }
  return invoke<BootstrapResult>("bootstrap");
}

export async function setLocale(locale: "ja" | "en"): Promise<AppSettings> {
  if (!inDesktopApp()) {
    demoState.settings = { ...demoState.settings, locale };
    try {
      localStorage.setItem(demoLocaleKey, locale);
    } catch {
      // Keep the selection for this preview session when storage is unavailable.
    }
    return demoState.settings;
  }
  return invoke<AppSettings>("set_locale", { locale });
}

export async function dismissAiIntegrationNotice(): Promise<AppSettings> {
  if (!inDesktopApp()) {
    demoState.settings = { ...demoState.settings, aiIntegrationNoticeDismissed: true };
    return demoState.settings;
  }
  return invoke<AppSettings>("dismiss_ai_integration_notice");
}

export async function refreshEnvironment(): Promise<BootstrapResult["environment"]> {
  if (!inDesktopApp()) return structuredClone(demoBootstrap.environment);
  return invoke<BootstrapResult["environment"]>("refresh_environment");
}

export async function listBackups(): Promise<BackupEntry[]> {
  if (!inDesktopApp()) return [
    { fileName: "state-20260826T091500.000Z.json", createdAt: "2026-08-26T09:15:00Z", sizeBytes: 4096 },
    { fileName: "state-20260825T143000.000Z.json", createdAt: "2026-08-25T14:30:00Z", sizeBytes: 3584 },
  ];
  return invoke<BackupEntry[]>("list_backups");
}

export async function restoreBackup(fileName: string): Promise<AppData> {
  if (!inDesktopApp()) return structuredClone(demoState);
  return invoke<AppData>("restore_backup", { fileName });
}

export async function openDataFolder(): Promise<void> {
  if (!inDesktopApp()) return;
  return invoke<void>("open_data_folder");
}

export async function inspectRepositoryStatuses(): Promise<RepositoryStatus[]> {
  if (!inDesktopApp()) {
    return demoState.repositories.map((repository) => {
      const profile = demoState.profiles.find((item) => item.id === repository.profileId);
      const state = !profile ? "unassigned"
        : repository.id === "repo-reapply" && !demoReapplied.has(repository.id) ? "reapply"
        : repository.id === "repo-attention" && profile.id === "client-work" ? "attention" : "ready";
      const github: GhProfileStatus | null = profile ? {
        available: true,
        authenticated: state !== "attention",
        username: state === "attention" ? null : profile.githubUsername,
        configDir: profile.ghConfigDir,
      } : null;
      return {
        repositoryId: repository.id,
        state,
        branch: repository.branch,
        uncommittedChanges: demoChanges.get(repository.id)?.length ?? 0,
        ahead: repository.remoteUrl ? demoAhead.get(repository.id) ?? 0 : null,
        identityInSync: state === "ready" || state === "attention",
        mismatchedKeys: state === "reapply" ? ["user.email"] : [],
        github,
        error: null,
      };
    });
  }
  return invoke<RepositoryStatus[]>("inspect_repository_statuses");
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
      autoApprove: { pushWorkBranch: false, pushDefaultBranch: false, createPullRequest: false, mergePullRequest: false, publishRepository: false },
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

export async function setRepositoryAutoApprove(repositoryId: string, autoApprove: AutoApprove): Promise<AppData> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === repositoryId);
    if (!repository) throw new Error("Repository was not found.");
    repository.autoApprove = { ...autoApprove };
    return structuredClone(demoState);
  }
  return invoke<AppData>("set_repository_auto_approve", { repositoryId, autoApprove });
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
      autoApprove: { pushWorkBranch: false, pushDefaultBranch: false, createPullRequest: false, mergePullRequest: false, publishRepository: false },
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
    const existing = demoState.profiles.find((item) => item.id === next.id);
    if (existing) next.autoApprove = { ...existing.autoApprove };
    const exists = demoState.profiles.some((item) => item.id === next.id);
    demoState.profiles = exists
      ? demoState.profiles.map((item) => (item.id === next.id ? next : item))
      : [...demoState.profiles, next];
    return structuredClone(demoState);
  }
  return invoke<AppData>("save_profile", { profile });
}

export async function setProfileAutoApprove(profileId: string, autoApprove: ProfileAutoApprove): Promise<AppData> {
  if (!inDesktopApp()) {
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!profile) throw new Error("Profile was not found.");
    profile.autoApprove = { ...autoApprove };
    return structuredClone(demoState);
  }
  return invoke<AppData>("set_profile_auto_approve", { profileId, autoApprove });
}

export async function inspectGithubProfile(
  profileId: string,
  ghConfigDir?: string | null,
): Promise<GhProfileStatus> {
  if (!inDesktopApp()) {
    const profile = demoState.profiles.find((item) => item.id === profileId);
    return {
      available: true,
      authenticated: Boolean(ghConfigDir),
      username: ghConfigDir ? profile?.githubUsername || "connected-account" : null,
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
        ...(profile.sshKeyPath || current?.sshKeyPath ? [{
          key: "core.sshCommand",
          currentValue: current?.sshKeyPath ? "Managed SSH identity" : null,
          nextValue: profile.sshKeyPath ? `ssh -i \"${profile.sshKeyPath}\" -o IdentitiesOnly=yes` : null,
        }] : []),
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
    demoHistory.unshift({ at: new Date().toISOString(), tool: "apply_profile", repositoryId, profileId, outcome: "success", summary: "Profile applied", client: null, confirmation: null, actor: "gui" });
    demoReapplied.add(repositoryId);
    demoState.repositories = demoState.repositories.map((repo) =>
      repo.id === repositoryId
        ? { ...repo, profileId, lastAppliedAt: new Date().toISOString() }
        : repo,
    );
    return structuredClone(demoState);
  }
  return invoke<AppData>("apply_profile", { repositoryId, profileId });
}

export async function listHistory(): Promise<AuditEntry[]> {
  if (!inDesktopApp()) return structuredClone(demoHistory);
  return invoke<AuditEntry[]>("list_history");
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
    demoAhead.set(repositoryId, 0);
    return {
      branch: preview.branch,
      remoteUrl: preview.remoteUrl,
      detail: "Everything up-to-date",
    };
  }
  return invoke<PushResult>("push_repository", { repositoryId, profileId });
}

export async function previewRepositorySync(repositoryId: string, profileId: string): Promise<SyncPreview> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === repositoryId);
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!repository || !profile || !repository.remoteUrl) throw new Error("Repository or profile was not found.");
    const branch = repository.branch || "main";
    return {
      repository: structuredClone(repository),
      profile: structuredClone(profile),
      branch,
      remoteUrl: repository.remoteUrl,
      upstream: `origin/${branch}`,
      remoteBranch: `origin/${branch}`,
      changes: demoChanges.get(repositoryId) ?? [],
      ahead: demoAhead.get(repositoryId) ?? 0,
      behind: demoBehind.get(repositoryId) ?? 0,
      fetchedAt: new Date().toISOString(),
    };
  }
  return invoke<SyncPreview>("preview_repository_sync", { repositoryId, profileId });
}

export async function pullRepository(repositoryId: string, profileId: string): Promise<SyncPreview> {
  if (!inDesktopApp()) {
    const preview = await previewRepositorySync(repositoryId, profileId);
    demoBehind.set(repositoryId, 0);
    return { ...preview, ahead: 0, behind: 0, fetchedAt: new Date().toISOString() };
  }
  return invoke<SyncPreview>("pull_repository", { repositoryId, profileId });
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
      changes: demoChanges.get(repositoryId) ?? [],
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
    demoChanges.set(repositoryId, []);
    demoAhead.set(repositoryId, (demoAhead.get(repositoryId) ?? 0) + 1);
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
      changes: demoChanges.get(repositoryId) ?? [],
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

export async function listPullRequests(
  repositoryId: string,
  profileId: string,
): Promise<PullRequestManagement> {
  if (!inDesktopApp()) {
    const repository = demoState.repositories.find((item) => item.id === repositoryId);
    const profile = demoState.profiles.find((item) => item.id === profileId);
    if (!repository || !profile || !repository.remoteUrl) throw new Error("Repository or profile was not found.");
    const repositoryNameWithOwner = repository.remoteUrl
      .replace("git@github.com:", "")
      .replace(/\.git$/, "");
    const pullRequests: PullRequestManagement["pullRequests"] = [
      {
        number: 18,
        url: `https://github.com/${repositoryNameWithOwner}/pull/18`,
        title: "Add pull request management",
        state: "OPEN",
        isDraft: false,
        baseBranch: "main",
        headBranch: "codex/pr-management",
        headOid: "0123456789abcdef0123456789abcdef01234567",
        mergeable: "MERGEABLE",
        mergeStateStatus: "CLEAN",
        reviewDecision: "APPROVED",
        author: profile.githubUsername || "connected-account",
        updatedAt: new Date().toISOString(),
        checks: [
          { name: "Frontend tests", state: "SUCCESS", bucket: "pass" as const, workflow: "CI", link: null },
          { name: "Rust tests", state: "SUCCESS", bucket: "pass" as const, workflow: "CI", link: null },
        ],
      },
      {
        number: 19,
        url: `https://github.com/${repositoryNameWithOwner}/pull/19`,
        title: "Update documentation",
        state: "OPEN",
        isDraft: false,
        baseBranch: "main",
        headBranch: "docs/update-guide",
        headOid: "89abcdef0123456789abcdef0123456789abcdef",
        mergeable: "MERGEABLE",
        mergeStateStatus: "BLOCKED",
        reviewDecision: "",
        author: profile.githubUsername || "connected-account",
        updatedAt: new Date(Date.now() - 3_600_000).toISOString(),
        checks: [{ name: "Verify on Windows", state: "IN_PROGRESS", bucket: "pending" as const, workflow: "CI", link: null }],
      },
    ].filter((item) => !demoMergedPullRequests.has(item.number));
    return {
      repository: structuredClone(repository),
      profile: structuredClone(profile),
      repositoryNameWithOwner,
      pullRequests,
    };
  }
  return invoke<PullRequestManagement>("list_pull_requests", { repositoryId, profileId });
}

export async function mergePullRequest(options: MergePullRequestOptions): Promise<MergePullRequestResult> {
  if (!inDesktopApp()) {
    demoMergedPullRequests.add(options.number);
    return {
      number: options.number,
      url: `https://github.com/example/example/pull/${options.number}`,
      title: "Add pull request management",
      strategy: options.strategy,
      mergedAt: new Date().toISOString(),
    };
  }
  return invoke<MergePullRequestResult>("merge_pull_request", { input: options });
}
