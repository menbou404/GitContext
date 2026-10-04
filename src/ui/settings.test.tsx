import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { demoBootstrap } from "../demoData";
import { settingsCopy } from "../i18n";
import { BackupList, SettingsPage } from "./SettingsPage";

const backups = [
  { fileName: "state-20260826T091500.000Z.json", createdAt: "2026-08-26T09:15:00Z", sizeBytes: 1024 },
  { fileName: "state-20260825T143000.000Z.json", createdAt: "2026-08-25T14:30:00Z", sizeBytes: 512 },
];

describe("settings", () => {
  it("renders the three sections and locale choices in both languages", () => {
    for (const locale of ["ja", "en"] as const) {
      const html = renderToStaticMarkup(<SettingsPage locale={locale} result={demoBootstrap} onLocaleChange={() => {}} onEnvironment={() => {}} onRestored={() => {}} />);
      const copy = settingsCopy[locale];
      expect(html).toContain(copy.environment);
      expect(html).toContain(copy.language);
      expect(html).toContain(copy.data);
      expect(html).toContain("日本語");
      expect(html).toContain("English");
      expect(html).toContain(copy.openFolder);
    }
  });

  it("shows one inline confirmation only for the selected backup", () => {
    const props = { backups, locale: "ja" as const, restoring: false, onSelect: () => {}, onRestore: () => {} };
    const closed = renderToStaticMarkup(<BackupList {...props} selected={null} />);
    expect(closed).not.toContain("ui-confirm");
    const opened = renderToStaticMarkup(<BackupList {...props} selected={backups[1].fileName} />);
    expect(opened.match(/class="ui-confirm"/g)).toHaveLength(1);
    expect(opened).toContain(settingsCopy.ja.restoreWarning);
    expect(opened).toContain(backups[1].fileName);
    expect(opened).not.toContain(`<code>${backups[0].fileName}</code>`);
    const unknown = renderToStaticMarkup(<BackupList {...props} selected="missing" />);
    expect(unknown).not.toContain("ui-confirm");
  });
});
