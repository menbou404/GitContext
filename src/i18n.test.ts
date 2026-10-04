import { describe, expect, it } from "vitest";
import { historyCopy, localizeRuntimeMessage, profileCopy, settingsCopy, shellCopy, tabCopy, uiCopy } from "./i18n";

describe("i18n keys", () => {
  it("has matching keys in Japanese and English", () => {
    for (const copy of [shellCopy, uiCopy, tabCopy, profileCopy, settingsCopy, historyCopy]) {
      expect(Object.keys(copy.ja).sort()).toEqual(Object.keys(copy.en).sort());
    }
  });
});

describe("Japanese UI copy", () => {
  it("uses the repository auto-approval wording in both languages", () => {
    expect(uiCopy.ja.autoApproveTitle).toBe("AIによる操作の自動承認");
    expect(uiCopy.ja.autoApprovePush).toBe("作業ブランチへのpush");
    expect(uiCopy.ja.autoApprovePullRequest).toBe("Pull Requestの作成");
    expect(uiCopy.ja.autoApproveDefaultPush).toBe("既定ブランチへのpush");
    expect(uiCopy.ja.autoApproveMerge).toBe("Pull Requestのmerge");
    expect(uiCopy.ja.autoApprovePublish).toBe("GitHubへの公開");
    expect(uiCopy.ja.autoApproveClone).toBe("このプロファイルでのclone");
    expect(uiCopy.en.autoApproveDefaultPushRisk).toContain("without confirmation");
    expect(uiCopy.en.autoApproveMergeRisk).toContain("without confirmation");
    expect(uiCopy.en.autoApprovePublishRisk).toContain("without confirmation");
  });

  it("provides localized labels and dynamic notices", () => {
    expect(uiCopy.ja.addRepository).toBe("リポジトリを追加");
    expect(uiCopy.ja.repositoryAdded("sample")).toContain("sampleを追加しました");
    expect(uiCopy.ja.syncButton).toBe("リポジトリを同期");
    expect(uiCopy.ja.managePullRequestsButton).toBe("PRを確認・merge");
    expect(uiCopy.ja.ciPassed(2)).toContain("2件");
    expect(uiCopy.ja.pullBlockedByChanges(2)).toContain("2件");
  });

  it("localizes known backend messages without changing unknown details", () => {
    expect(localizeRuntimeMessage("Repository was not found.", "ja")).toBe("リポジトリが見つかりません。");
    expect(localizeRuntimeMessage("The current branch does not exist on origin yet.", "ja")).toContain("origin");
    expect(localizeRuntimeMessage("system detail", "ja")).toBe("system detail");
  });
});
