import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { GhProfileStatus, Profile, RepositoryRecord } from "../types";
import { profileCopy } from "../i18n";
import { ProfileList } from "./ProfileList";
import { FirstRunGuide } from "./FirstRunGuide";
import { connectionFor, firstRunSteps } from "./profileStatus";

const profile: Profile = {
  id: "sample", label: "Sample", accent: "#D8A33F", gitName: "Example", gitEmail: "person@example.com",
  githubUsername: "sample-account", ghConfigDir: "C:\\example\\gh", autoApprove: { cloneRepository: false },
};
const repository: RepositoryRecord = {
  id: "repository", name: "sample", path: "C:\\example\\sample", profileId: profile.id,
  autoApprove: { pushWorkBranch: false, pushDefaultBranch: false, createPullRequest: false, mergePullRequest: false, publishRepository: false },
};
const connected: GhProfileStatus = { available: true, authenticated: true, username: "sample-account" };
const different: GhProfileStatus = { available: true, authenticated: true, username: "other-account" };

describe("profiles", () => {
  it("distinguishes connected, disconnected, and different accounts", () => {
    expect(connectionFor(profile, connected)).toBe("connected");
    expect(connectionFor(profile, { available: true, authenticated: false })).toBe("disconnected");
    expect(connectionFor(profile, different)).toBe("different");
    expect(connectionFor(profile)).toBe("checking");
  });

  it("renders account state with an icon and assigned count in both languages", () => {
    for (const locale of ["ja", "en"] as const) {
      for (const [status, label] of [[connected, profileCopy[locale].connected], [different, profileCopy[locale].different]] as const) {
        const html = renderToStaticMarkup(<ProfileList profiles={[profile]} repositories={[repository]} statuses={{ sample: status }} locale={locale} refreshing={false} onRefresh={() => {}} onCreate={() => {}} onOpen={() => {}} />);
        expect(html).toContain(label);
        expect(html).toContain(profileCopy[locale].repositoryCount(1));
        expect(html).toContain("<svg");
      }
    }
  });

  it("advances the guide after creating and connecting a profile", () => {
    expect(firstRunSteps([], [], {})).toEqual([false, false, false]);
    expect(firstRunSteps([profile], [], {})).toEqual([true, false, false]);
    expect(firstRunSteps([profile], [], { sample: connected })).toEqual([true, true, false]);
    expect(firstRunSteps([profile], [repository], { sample: connected })).toEqual([true, true, true]);
    const html = renderToStaticMarkup(<FirstRunGuide profiles={[profile]} repositories={[]} statuses={{ sample: connected }} locale="ja" onCreate={() => {}} onConnect={() => {}} onAdd={() => {}} />);
    expect(html.match(/完了/g)).toHaveLength(2);
    expect(html).toContain("リポジトリを追加");
    expect(html).toContain("ui-button--primary");
  });
});
