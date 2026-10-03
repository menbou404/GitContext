import { describe, expect, it } from "vitest";
import { localizeRuntimeMessage, uiCopy } from "./i18n";

describe("Japanese UI copy", () => {
  it("uses the repository auto-approval wording in both languages", () => {
    expect(uiCopy.ja.autoApproveTitle).toBe("AIによる操作の自動承認");
    expect(uiCopy.ja.autoApprovePush).toBe("作業ブランチへのpush");
    expect(uiCopy.ja.autoApprovePullRequest).toBe("Pull Requestの作成");
    expect(uiCopy.ja.autoApproveConfirm).toContain("mergeと既定ブランチへのpushは、常に確認します。");
    expect(uiCopy.en.autoApproveConfirm).toContain("default branch");
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
