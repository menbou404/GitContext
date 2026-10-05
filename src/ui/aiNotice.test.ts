import { describe, expect, it } from "vitest";
import { shellCopy } from "../i18n";
import type { AiInventory } from "../types";
import { shouldShowAiNotice } from "./aiNotice";

const inventory = (states: AiInventory["clients"][number]["state"][]): AiInventory => ({
  server: { path: "gitcontext-mcp.exe", version: null, development: false, built: true, registrationName: "gitcontext" },
  clients: states.map((state) => ({ client: "codex", state, tier: "read", trust: false, confirmation: "unstable", configPath: "config.toml", registrationName: "gitcontext", command: null, args: [], cliAvailable: true })),
});

describe("AI integration introduction notice", () => {
  it("shows only for existing users with a successfully loaded, unconnected client list", () => {
    expect(shouldShowAiNotice(1, false, inventory(["disconnected", "repair"]))).toBe(true);
    expect(shouldShowAiNotice(0, false, inventory(["disconnected"]))).toBe(false);
    expect(shouldShowAiNotice(1, true, inventory(["disconnected"]))).toBe(false);
    expect(shouldShowAiNotice(1, false, inventory(["connected", "disconnected"]))).toBe(false);
    expect(shouldShowAiNotice(1, false, null)).toBe(false);
  });

  it("has both notice labels in both languages", () => {
    for (const locale of ["ja", "en"] as const) {
      expect(shellCopy[locale].aiIntegrationAvailable).toBeTruthy();
      expect(shellCopy[locale].openAiIntegration).toBeTruthy();
    }
  });
});
