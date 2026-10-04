import { describe, expect, it } from "vitest";
import { aiCopy } from "../i18n";
import { canReviewAiPlan, requiresAiConsent } from "./AiIntegrationPage";

describe("AI integration flow", () => {
  it("labels every client state and scope in both languages", () => {
    for (const locale of ["ja", "en"] as const) {
      const copy = aiCopy[locale];
      for (const state of ["connected", "repair", "disconnected", "not_found"] as const) expect(copy.states[state]).toBeTruthy();
      for (const tier of ["read", "local", "remote"] as const) expect(copy.tiers[tier]).toBeTruthy();
    }
  });
  it("requires acknowledgement before reviewing remote access on unstable clients", () => {
    for (const client of ["codex", "claude_desktop"] as const) {
      expect(requiresAiConsent(client, "remote")).toBe(true);
      expect(canReviewAiPlan(client, "remote", false)).toBe(false);
      expect(canReviewAiPlan(client, "remote", true)).toBe(true);
      expect(canReviewAiPlan(client, "read", false)).toBe(true);
    }
    expect(requiresAiConsent("claude_code", "remote")).toBe(false);
    expect(canReviewAiPlan("claude_code", "remote", false)).toBe(true);
  });
  it("has matching translation keys", () => {
    const keys = (value: unknown, prefix = ""): string[] => typeof value === "object" && value !== null
      ? Object.entries(value).flatMap(([key, child]) => keys(child, `${prefix}.${key}`)) : [prefix];
    expect(keys(aiCopy.ja).sort()).toEqual(keys(aiCopy.en).sort());
  });
});
