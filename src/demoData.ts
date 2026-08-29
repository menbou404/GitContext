import type { BootstrapResult } from "./types";

export const demoBootstrap: BootstrapResult = {
  demoMode: true,
  storagePath: "Browser preview — no settings are written",
  environment: {
    git: { available: true, version: "git version 2.53.0.windows.2" },
    gh: {
      available: false,
      detail: "GitHub CLI is not installed. Git identity management still works.",
    },
    ssh: { available: true, version: "OpenSSH for Windows" },
    sshDirectory: "C:\\Users\\you\\.ssh",
  },
  data: {
    version: 1,
    profiles: [
      {
        id: "personal",
        label: "Personal",
        accent: "#d8a33f",
        gitName: "Your Name",
        gitEmail: "personal@example.com",
        githubUsername: "your-personal",
        sshKeyPath: "C:\\Users\\you\\.ssh\\id_ed25519_personal",
        ghConfigDir: "C:\\Users\\you\\.config\\gh-personal",
      },
      {
        id: "school",
        label: "School",
        accent: "#56a7d9",
        gitName: "Your Name",
        gitEmail: "student@example.ac.jp",
        githubUsername: "your-school",
        sshKeyPath: "C:\\Users\\you\\.ssh\\id_ed25519_school",
        ghConfigDir: "C:\\Users\\you\\.config\\gh-school",
      },
    ],
    repositories: [
      {
        id: "repo-personal",
        name: "weekend-game",
        path: "C:\\Users\\you\\Projects\\personal\\weekend-game",
        remoteUrl: "git@github.com:your-personal/weekend-game.git",
        branch: "main",
        profileId: "personal",
        lastAppliedAt: "2026-08-26T00:20:00Z",
      },
      {
        id: "repo-school",
        name: "compiler-class",
        path: "C:\\Users\\you\\Projects\\school\\compiler-class",
        remoteUrl: "git@github.com:your-school/compiler-class.git",
        branch: "main",
        profileId: "school",
        lastAppliedAt: "2026-08-25T11:05:00Z",
      },
      {
        id: "repo-unassigned",
        name: "new-research",
        path: "C:\\Users\\you\\Projects\\new-research",
        remoteUrl: "git@github.com:example/new-research.git",
        branch: "develop",
        profileId: null,
        lastAppliedAt: null,
      },
    ],
  },
};
