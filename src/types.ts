export interface Profile {
  id: string;
  label: string;
  accent: string;
  gitName: string;
  gitEmail: string;
  githubUsername?: string | null;
  sshKeyPath?: string | null;
  ghConfigDir?: string | null;
}

export interface RepositoryRecord {
  id: string;
  name: string;
  path: string;
  remoteUrl?: string | null;
  branch?: string | null;
  profileId?: string | null;
  lastAppliedAt?: string | null;
}

export interface AppData {
  version: number;
  profiles: Profile[];
  repositories: RepositoryRecord[];
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
}

export interface ConfigChange {
  key: string;
  currentValue?: string | null;
  nextValue: string;
}

export interface ApplyPreview {
  repository: RepositoryRecord;
  profile: Profile;
  changes: ConfigChange[];
  warnings: string[];
}

export interface ProfileDraft extends Profile {}

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
