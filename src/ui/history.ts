import { historyCopy, type Locale } from "../i18n";
import type { AuditEntry } from "../types";

export type ActorFilter = "all" | "gui" | "ai";

export function historyActor(entry: AuditEntry): "gui" | "ai" | "mcp" {
  return entry.actor === "gui" ? "gui" : entry.client ? "ai" : "mcp";
}

export function filterHistory(entries: AuditEntry[], repositoryId: string, profileId: string, actor: ActorFilter): AuditEntry[] {
  return entries.filter((entry) =>
    (!repositoryId || entry.repositoryId === repositoryId) &&
    (!profileId || entry.profileId === profileId) &&
    (actor === "all" || (actor === "gui" ? historyActor(entry) === "gui" : historyActor(entry) !== "gui")),
  );
}

export function operationLabel(tool: string, locale: Locale): string {
  const copy = historyCopy[locale];
  const labels: Record<string, string> = {
    apply_profile: copy.applyProfile, create_pull_request: copy.createPullRequest,
    merge_pull_request: copy.mergePullRequest, publish_repository: copy.publishRepository,
    clone_repository: copy.cloneRepository, add_repository: copy.addRepository,
    create_branch: copy.createBranch, pull: copy.pull, push: copy.push, commit: copy.commit,
  };
  return labels[tool] ?? tool.replace(/_/g, " ");
}

export function outcomeLabel(outcome: string, locale: Locale): string {
  const copy = historyCopy[locale];
  return outcome === "success" ? copy.success : outcome === "rejected" ? copy.rejected : copy.failed;
}

export function confirmationLabel(confirmation: string | null, locale: Locale): string {
  const copy = historyCopy[locale];
  return confirmation === "elicitation" ? copy.confirmationScreen
    : confirmation === "auto" ? copy.automatic
    : confirmation === "client" ? copy.clientApproval : copy.none;
}
