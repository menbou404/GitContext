import { describe, expect, it } from "vitest";
import { commitRepository, createPullRequest, inspectRepositoryStatuses, listPullRequests, mergePullRequest, previewCommit, previewPullRequest, previewRepositorySync, publishRepository, pullRepository, pushRepository } from "../backend";

describe("browser preview flows", () => {
  it("shows changes, then updates local ahead after commit and push", async () => {
    const before = await previewCommit("repo-personal", "open-source");
    expect(before.changes).toHaveLength(3);
    await commitRepository("repo-personal", "open-source", "Add level 3");
    expect((await previewCommit("repo-personal", "open-source")).changes).toHaveLength(0);
    expect((await inspectRepositoryStatuses()).find((item) => item.repositoryId === "repo-personal")?.ahead).toBe(2);
    await pushRepository("repo-personal", "open-source");
    expect((await inspectRepositoryStatuses()).find((item) => item.repositoryId === "repo-personal")?.ahead).toBe(0);
  });

  it("pulls a branch that is behind and publishes a repository without origin", async () => {
    const before = await previewRepositorySync("repo-school", "university-lab");
    expect([before.ahead, before.behind]).toEqual([0, 2]);
    expect((await pullRepository("repo-school", "university-lab")).behind).toBe(0);
    const published = await publishRepository({ repositoryId: "repo-local", profileId: "open-source", name: "local-sandbox", visibility: "private" });
    expect(published.data.repositories.find((item) => item.id === "repo-local")?.remoteUrl).toContain("local-sandbox.git");
  });

  it("previews, creates and merges Pull Requests", async () => {
    const preview = await previewPullRequest("repo-reapply", "open-source");
    expect(preview.commitsAhead).toBe(1);
    const created = await createPullRequest({ repositoryId: "repo-reapply", profileId: "open-source", baseBranch: preview.baseBranch, title: "Update layout", body: "", draft: false });
    expect(created.number).toBeGreaterThan(0);
    const management = await listPullRequests("repo-reapply", "open-source");
    const ready = management.pullRequests.find((item) => item.mergeStateStatus === "CLEAN");
    expect(ready).toBeDefined();
    const merged = await mergePullRequest({ repositoryId: "repo-reapply", profileId: "open-source", number: ready!.number, strategy: "squash", expectedHeadOid: ready!.headOid });
    expect(merged.number).toBe(ready!.number);
  });
});
