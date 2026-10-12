import { describe, expect, it } from "vitest";
import { aiCopy } from "../i18n";
import { registrationNeedsUpdate } from "./AiIntegrationPage";
import { loadState, planAiClient, setAiClientPolicy } from "../backend";
import type { AiClientInfo, AiClientPolicies } from "../types";

describe("AI integration flow", () => {
  it("labels every client state and scope in both languages", () => {
    for (const locale of ["ja", "en"] as const) {
      const copy = aiCopy[locale];
      for (const state of ["connected", "repair", "disconnected", "not_found"] as const) expect(copy.states[state]).toBeTruthy();
      for (const tier of ["read", "local", "remote"] as const) expect(copy.tiers[tier]).toBeTruthy();
    }
  });
  it("saves scope immediately in the browser preview and keeps registration at remote", async () => {
    await setAiClientPolicy("claudeCodeDesktop", { maxTier: "local", allowTrustClientApproval: false });
    expect((await loadState()).settings.aiClients?.claudeCodeDesktop.maxTier).toBe("local");
    const plan = await planAiClient("codex", "connect", "read", false);
    expect(plan.after).toContain('"remote"');
    for (const locale of ["ja", "en"] as const) {
      expect(aiCopy[locale].warning).toBeTruthy();
      expect(aiCopy[locale].understand).toBeTruthy();
    }
  });
  it("has matching translation keys", () => {
    const keys = (value: unknown, prefix = ""): string[] => typeof value === "object" && value !== null
      ? Object.entries(value).flatMap(([key, child]) => keys(child, `${prefix}.${key}`)) : [prefix];
    expect(keys(aiCopy.ja).sort()).toEqual(keys(aiCopy.en).sort());
  });
  it("identifies a narrow shared registration and trust upgrade", () => {
    const policies: AiClientPolicies = { claudeCode: { maxTier: "read", allowTrustClientApproval: false }, claudeCodeDesktop: { maxTier: "remote", allowTrustClientApproval: true }, codex: { maxTier: "read", allowTrustClientApproval: false }, other: { maxTier: "read", allowTrustClientApproval: false } };
    const registration: AiClientInfo = { client: "claude_code", state: "connected", tier: "read", trust: false, confirmation: "mixed", configPath: "demo", registrationName: "test", command: "demo", args: [], cliAvailable: true };
    expect(registrationNeedsUpdate(registration, policies)).toBe(true);
    registration.tier = "remote";
    expect(registrationNeedsUpdate(registration, policies)).toBe(true);
    registration.trust = true;
    expect(registrationNeedsUpdate(registration, policies)).toBe(false);
    registration.state = "repair";
    registration.tier = "read";
    expect(registrationNeedsUpdate(registration, policies)).toBe(true);
  });
});
