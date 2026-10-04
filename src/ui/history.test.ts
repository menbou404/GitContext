import { describe, expect, it } from "vitest";
import { historyCopy } from "../i18n";
import type { AuditEntry } from "../types";
import { confirmationLabel, filterHistory, historyActor, operationLabel, outcomeLabel } from "./history";

const entry = (overrides: Partial<AuditEntry> = {}): AuditEntry => ({
  at: "2026-10-04T10:00:00Z", tool: "apply_profile", repositoryId: "repo-a", profileId: "profile-a",
  outcome: "success", summary: "Profile applied", client: null, confirmation: null, actor: "gui", ...overrides,
});

describe("history labels", () => {
  it("labels operations, outcomes, and confirmation methods in both languages", () => {
    for (const locale of ["ja", "en"] as const) {
      const copy = historyCopy[locale];
      expect(operationLabel("apply_profile", locale)).toBe(copy.applyProfile);
      expect(operationLabel("create_pull_request", locale)).toBe(copy.createPullRequest);
      expect(operationLabel("push", locale)).toBe(copy.push);
      expect(outcomeLabel("success", locale)).toBe(copy.success);
      expect(outcomeLabel("failed", locale)).toBe(copy.failed);
      expect(outcomeLabel("rejected", locale)).toBe(copy.rejected);
      expect(confirmationLabel("elicitation", locale)).toBe(copy.confirmationScreen);
      expect(confirmationLabel("auto", locale)).toBe(copy.automatic);
      expect(confirmationLabel("client", locale)).toBe(copy.clientApproval);
      expect(confirmationLabel(null, locale)).toBe(copy.none);
    }
    expect(Object.keys(historyCopy.ja).sort()).toEqual(Object.keys(historyCopy.en).sort());
  });
});

describe("history filtering", () => {
  it("combines repository, profile, and actor filters and handles legacy records", () => {
    const records = [entry(), entry({ repositoryId: "repo-b", actor: undefined, client: "Demo AI" }), entry({ profileId: "profile-b", actor: undefined, client: null })];
    expect(historyActor(records[0])).toBe("gui");
    expect(historyActor(records[1])).toBe("ai");
    expect(historyActor(records[2])).toBe("mcp");
    expect(filterHistory(records, "repo-a", "profile-a", "gui")).toEqual([records[0]]);
    expect(filterHistory(records, "", "", "ai")).toEqual(records.slice(1));
    expect(filterHistory(records, "repo-b", "profile-a", "all")).toEqual([records[1]]);
  });
});
