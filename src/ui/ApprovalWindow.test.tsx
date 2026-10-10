import { afterEach, expect, test, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { ApprovalWindow } from "./ApprovalWindow";
import { approvalCopy } from "../i18n";

afterEach(() => vi.unstubAllGlobals());

test("assignment demo shows repository, profile choices, and defaults control", () => {
  vi.stubGlobal("location", { search: "?approval-demo=assignment" });
  vi.stubGlobal("navigator", { language: "ja-JP" });
  const html = renderToStaticMarkup(<ApprovalWindow />);
  expect(html).toContain(approvalCopy.ja.assignmentTitle);
  expect(html).toContain("C:\\Projects\\sample");
  expect(html).toContain("<select");
  expect(html).toContain("Personal (@example-user)");
  expect(html).toContain(approvalCopy.ja.applyDefaults);
  expect(html).toContain("type=\"checkbox\"");
});

test("approval translations have the same keys", () => {
  expect(Object.keys(approvalCopy.ja).sort()).toEqual(Object.keys(approvalCopy.en).sort());
});
