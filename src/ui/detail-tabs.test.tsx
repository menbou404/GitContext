import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { demoBootstrap } from "../demoData";
import { ConfirmPanel } from "./ConfirmPanel";
import { canCommit } from "./ChangesTab";
import { nextDetailTab, RepositoryDetail } from "./RepositoryDetail";

describe("repository detail tabs", () => {
  it("moves with arrow, Home and End keys and wraps", () => {
    expect(nextDetailTab(0, "ArrowRight")).toBe("changes");
    expect(nextDetailTab(0, "ArrowLeft")).toBe("pr");
    expect(nextDetailTab(3, "ArrowRight")).toBe("overview");
    expect(nextDetailTab(2, "Home")).toBe("overview");
    expect(nextDetailTab(0, "End")).toBe("pr");
    expect(nextDetailTab(1, "Enter")).toBeNull();
  });

  it("renders a tablist with linked panels", () => {
    const repository = demoBootstrap.data.repositories[0];
    const html = renderToStaticMarkup(createElement(RepositoryDetail, {
      repository, profiles: demoBootstrap.data.profiles, status: undefined, pendingProfileId: repository.profileId ?? "", preview: null,
      locale: "ja", busy: false, removing: false, onBack: () => {}, onPendingProfile: () => {}, onReview: () => {}, onCancelReview: () => {},
      onApply: async () => {}, onAutoApprove: () => {}, onStartRemove: () => {}, onCancelRemove: () => {}, onRemove: () => {}, onData: () => {}, onFinished: () => {},
    }));
    expect(html).toContain('role="tablist"');
    expect(html.match(/role="tab"/g)).toHaveLength(4);
    expect(html.match(/role="tabpanel"/g)).toHaveLength(4);
    expect(html).toContain('aria-controls="repo-panel-changes"');
    expect(html).toContain('aria-selected="true"');
  });
});

describe("confirmation", () => {
  it("requires files, a valid message and a destination for commit and push", () => {
    expect(canCommit(0, "Update", false)).toBe(false);
    expect(canCommit(1, " ", false)).toBe(false);
    expect(canCommit(1, "first\nsecond", false)).toBe(false);
    expect(canCommit(1, "Update", true)).toBe(false);
    expect(canCommit(1, "Update", true, "git@github.com:example/repo.git")).toBe(true);
  });

  it("places the primary action inside the confirmation section", () => {
    const html = renderToStaticMarkup(createElement(ConfirmPanel, { locale: "ja", rows: [{ label: "ブランチ", value: "main" }], children: createElement("button", { type: "button" }, "commit") }));
    expect(html).toContain('aria-label="実行前に内容を確認"');
    expect(html).toMatch(/<section[^>]+>.*<button[^>]*>commit<\/button>.*<\/section>/);
  });
});
