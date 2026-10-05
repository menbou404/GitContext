export interface Profile {
  id: string;
  label: string;
  accent: string;
  gitName: string;
  gitEmail: string;
  githubUsername?: string | null;
  sshKeyPath?: string | null;
  ghConfigDir?: string | null;
  autoApprove: ProfileAutoApprove;
}

export interface AuditEntry {
  at: string;
  tool: string;
  repositoryId: string | null;
  profileId: string | null;
  outcome: string;
  summary: string;
  client: string | null;
  confirmation: string | null;
  actor?: "gui" | "mcp" | null;
}

export interface ProfileAutoApprove {
  cloneRepository: boolean;
}

export interface RepositoryRecord {
  id: string;
  name: string;
  path: string;
  remoteUrl?: string | null;
  branch?: string | null;
  profileId?: string | null;
  lastAppliedAt?: string | null;
  autoApprove: AutoApprove;
}

export type RepositoryState = "ready" | "reapply" | "unassigned" | "attention";

export interface RepositoryStatus {
  repositoryId: string;
  state: RepositoryState;
  branch?: string | null;
  uncommittedChanges?: number | null;
  ahead?: number | null;
  identityInSync: boolean;
  mismatchedKeys: string[];
  github?: GhProfileStatus | null;
  error?: string | null;
}

export interface AutoApprove {
  pushWorkBranch: boolean;
  pushDefaultBranch: boolean;
  createPullRequest: boolean;
  mergePullRequest: boolean;
  publishRepository: boolean;
}

export interface AppData {
  version: number;
  profiles: Profile[];
  repositories: RepositoryRecord[];
  settings: AppSettings;
}

export interface AppSettings {
  locale?: string | null;
  aiIntegrationNoticeDismissed?: boolean;
}

export interface ToolStatus {
  available: boolean;
  version?: string | null;
  detail?: string | null;
}

export interface EnvironmentStatus {
  git: ToolStatus;
  gh: ToolStatus;
  ssh: ToolStatus;
  sshDirectory?: string | null;
}

export interface BootstrapResult {
  data: AppData;
  environment: EnvironmentStatus;
  storagePath?: string | null;
  demoMode: boolean;
  developmentData?: boolean;
}

export interface BackupEntry {
  fileName: string;
  createdAt: string;
  sizeBytes: number;
}

export interface GhProfileStatus {
  available: boolean;
  authenticated: boolean;
  username?: string | null;
  detail?: string | null;
  configDir?: string | null;
}

export interface GithubAuthPrompt {
  profileId: string;
  code: string;
  verificationUrl: string;
}

export type RepositoryVisibility = "private" | "public";

export interface PublishOptions {
  repositoryId: string;
  profileId: string;
  name: string;
  visibility: RepositoryVisibility;
  description?: string | null;
}

export interface PublishResult {
  data: AppData;
  repositoryUrl: string;
}

export interface GithubRepository {
  name: string;
  nameWithOwner: string;
  description?: string | null;
  isPrivate: boolean;
  sshUrl: string;
  updatedAt: string;
}

export interface CloneOptions {
  profileId: string;
  repositoryUrl: string;
  destinationParent: string;
}

export interface CloneResult {
  data: AppData;
  repository: RepositoryRecord;
}

export interface PushPreview {
  repository: RepositoryRecord;
  profile: Profile;
  branch: string;
  remoteUrl: string;
  upstream?: string | null;
  hasUncommittedChanges: boolean;
}

export interface PushResult {
  branch: string;
  remoteUrl: string;
  detail?: string | null;
}

export interface SyncPreview {
  repository: RepositoryRecord;
  profile: Profile;
  branch: string;
  remoteUrl: string;
  upstream?: string | null;
  remoteBranch?: string | null;
  changes: WorkingTreeChange[];
  ahead: number;
  behind: number;
  fetchedAt: string;
}

export interface WorkingTreeChange {
  status: string;
  path: string;
}

export interface CommitPreview {
  repository: RepositoryRecord;
  profile: Profile;
  branch: string;
  changes: WorkingTreeChange[];
  pushRemoteUrl?: string | null;
  pushUnavailableReason?: string | null;
}

export interface CommitResult {
  branch: string;
  commitId: string;
  message: string;
}

export interface BranchResult {
  data: AppData;
  branch: string;
}

export interface PullRequestSummary {
  number: number;
  url: string;
  title: string;
}

export interface PullRequestPreview {
  repository: RepositoryRecord;
  profile: Profile;
  currentBranch: string;
  baseBranch: string;
  remoteUrl: string;
  repositoryNameWithOwner: string;
  changes: WorkingTreeChange[];
  commitsAhead: number;
  branchPushed: boolean;
  requiresNewBranch: boolean;
  existingPullRequest?: PullRequestSummary | null;
}

export interface PullRequestResult {
  number: number;
  url: string;
  title: string;
  branch: string;
  baseBranch: string;
  existing: boolean;
}

export interface PullRequestCreateOptions {
  repositoryId: string;
  profileId: string;
  baseBranch: string;
  title: string;
  body: string;
  draft: boolean;
}

export type PullRequestCheckBucket = "pass" | "fail" | "pending" | "skipping" | "cancel";

export interface PullRequestCheck {
  name: string;
  state: string;
  bucket: PullRequestCheckBucket;
  link?: string | null;
  workflow?: string | null;
}

export interface ManagedPullRequest {
  number: number;
  url: string;
  title: string;
  state: string;
  isDraft: boolean;
  baseBranch: string;
  headBranch: string;
  headOid: string;
  mergeable: string;
  mergeStateStatus: string;
  reviewDecision: string;
  author?: string | null;
  updatedAt: string;
  checks: PullRequestCheck[];
}

export interface PullRequestManagement {
  repository: RepositoryRecord;
  profile: Profile;
  repositoryNameWithOwner: string;
  pullRequests: ManagedPullRequest[];
}

export type MergeStrategy = "squash" | "merge" | "rebase";

export interface MergePullRequestOptions {
  repositoryId: string;
  profileId: string;
  number: number;
  strategy: MergeStrategy;
  expectedHeadOid: string;
}

export interface MergePullRequestResult {
  number: number;
  url: string;
  title: string;
  strategy: MergeStrategy;
  mergedAt?: string | null;
}

export interface ConfigChange {
  key: string;
  currentValue?: string | null;
  nextValue: string | null;
}

export interface ApplyPreview {
  repository: RepositoryRecord;
  profile: Profile;
  changes: ConfigChange[];
  warnings: string[];
}

export interface ProfileDraft extends Profile {}

export type AiClient = "claude_code" | "codex" | "claude_desktop";
export type AiTier = "read" | "local" | "remote";
export type AiAction = "connect" | "change" | "repair" | "disconnect";
export interface AiServer { path: string; version: string | null; development: boolean; built: boolean; registrationName: string }
export interface AiClientInfo { client: AiClient; state: "connected" | "repair" | "disconnected" | "not_found"; tier: AiTier; trust: boolean; confirmation: "mixed" | "unstable" | "unsupported"; configPath: string; registrationName: string; command: string | null; args: string[]; cliAvailable: boolean }
export interface AiInventory { server: AiServer; clients: AiClientInfo[] }
export interface AiPlan { client: AiClient; action: AiAction; tier: AiTier; trust: boolean; configPath: string; before: string; after: string; commandLine: string | null; fileHash: string | null; registrationName: string; manual: boolean }

export const profileIsComplete = (profile: Profile) =>
  profile.gitName.trim().length > 0 && profile.gitEmail.trim().length > 0;

export const initials = (label: string) =>
  label
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("") || "?";

export const compactPath = (path: string, maxLength = 48) => {
  if (path.length <= maxLength) return path;
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length < 3) return `…${path.slice(-(maxLength - 1))}`;
  const tail = parts.slice(-2).join("\\");
  return `…\\${tail}`;
};
