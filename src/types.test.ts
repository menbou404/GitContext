import { describe, expect, it } from "vitest";
import type { Profile } from "./types";
import { compactPath, initials, profileIsComplete } from "./types";

const profile: Profile = {
  id: "personal",
  label: "Personal Work",
  accent: "#d8a33f",
  gitName: "Your Name",
  gitEmail: "you@example.com",
};

describe("profile helpers", () => {
  it("treats author name and email as the minimum complete identity", () => {
    expect(profileIsComplete(profile)).toBe(true);
    expect(profileIsComplete({ ...profile, gitEmail: "" })).toBe(false);
  });

  it("creates stable initials", () => {
    expect(initials(profile.label)).toBe("PW");
    expect(initials("School")).toBe("S");
  });
});

describe("path presentation", () => {
  it("keeps short paths and shortens long paths from the left", () => {
    expect(compactPath("C:\\src\\repo")).toBe("C:\\src\\repo");
    expect(compactPath("C:\\Users\\you\\Projects\\school\\compiler-class", 25)).toBe("…\\school\\compiler-class");
  });
});
