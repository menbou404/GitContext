import { describe, expect, it } from "vitest";
import { resolveLocale } from "./locale";

describe("resolveLocale", () => {
  it("uses a valid saved locale before the browser language", () => {
    expect(resolveLocale("en", "ja-JP")).toBe("en");
    expect(resolveLocale("ja", "en-US")).toBe("ja");
  });

  it("uses Japanese when the browser language starts with ja", () => {
    expect(resolveLocale(null, "ja-JP")).toBe("ja");
    expect(resolveLocale("fr", "ja")).toBe("ja");
  });

  it("defaults to English for other or missing browser languages", () => {
    expect(resolveLocale(undefined, "fr-FR")).toBe("en");
    expect(resolveLocale("invalid", undefined)).toBe("en");
  });
});
