import { describe, expect, it } from "vitest";
import { localizeRuntimeMessage, uiCopy } from "./i18n";

describe("Japanese UI copy", () => {
  it("provides localized labels and dynamic notices", () => {
    expect(uiCopy.ja.addRepository).toBe("リポジトリを追加");
    expect(uiCopy.ja.repositoryAdded("sample")).toContain("sampleを追加しました");
    expect(uiCopy.ja.syncButton).toBe("リポジトリを同期");
    expect(uiCopy.ja.pullBlockedByChanges(2)).toContain("2件");
  });

  it("localizes known backend messages without changing unknown details", () => {
    expect(localizeRuntimeMessage("Repository was not found.", "ja")).toBe("リポジトリが見つかりません。");
    expect(localizeRuntimeMessage("The current branch does not exist on origin yet.", "ja")).toContain("origin");
    expect(localizeRuntimeMessage("system detail", "ja")).toBe("system detail");
  });
});
