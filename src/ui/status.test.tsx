import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { shellCopy } from "../i18n";
import type { Profile, RepositoryRecord, RepositoryStatus, RepositoryState } from "../types";
import { StatusLabel } from "./StatusLabel";
import { filterRepositories } from "./status";

const profile: Profile = {
  id: "p", label: "Sample", accent: "#D8A33F", gitName: "Example", gitEmail: "person@example.com", autoApprove: { cloneRepository: false },
};
const states: RepositoryState[] = ["ready", "reapply", "unassigned", "attention"];
const repositories: RepositoryRecord[] = states.map((state) => ({
  id: state, name: state, path: `C:\\example\\${state}`,
  profileId: state === "unassigned" ? null : "p", branch: "main",
  autoApprove: { pushWorkBranch: false, pushDefaultBranch: false, createPullRequest: false, mergePullRequest: false, publishRepository: false },
}));
const statuses = Object.fromEntries(states.map((state): [string, RepositoryStatus] => [state, {
  repositoryId: state, state, identityInSync: state === "ready" || state === "attention", mismatchedKeys: [],
}]));

describe("repository list status", () => {
  it("renders a readable icon and translated label for every state", () => {
    for (const state of states) {
      for (const locale of ["ja", "en"] as const) {
        const html = renderToStaticMarkup(<StatusLabel state={state} locale={locale} />);
        expect(html).toContain(shellCopy[locale][state]);
        expect(html).toContain("<svg");
      }
    }
  });

  it("filters needs action and unassigned separately, and searches profiles", () => {
    expect(filterRepositories(repositories, [profile], statuses, "", "action").map((repo) => repo.id)).toEqual(["reapply", "attention"]);
    expect(filterRepositories(repositories, [profile], statuses, "", "unassigned").map((repo) => repo.id)).toEqual(["unassigned"]);
    expect(filterRepositories(repositories, [profile], statuses, "sample", "all")).toHaveLength(3);
  });

  it("has matching UI keys in Japanese and English", () => {
    expect(Object.keys(shellCopy.ja).sort()).toEqual(Object.keys(shellCopy.en).sort());
    for (const state of states) {
      expect(shellCopy.ja[state].length).toBeGreaterThan(0);
      expect(shellCopy.en[state].length).toBeGreaterThan(0);
    }
  });
});
