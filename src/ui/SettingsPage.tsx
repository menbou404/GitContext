import { useEffect, useState } from "react";
import { AlertIcon, CheckIcon, FolderIcon } from "../Icons";
import { bootstrap, listBackups, openDataFolder, refreshEnvironment, restoreBackup } from "../backend";
import { settingsCopy, type Locale } from "../i18n";
import type { BackupEntry, BootstrapResult, EnvironmentStatus, ToolStatus } from "../types";
import { ConfirmPanel } from "./ConfirmPanel";

const folderOf = (path: string) => path.replace(/[\\/]state\.json$/i, "");

export function BackupList({ backups, locale, selected, restoring, onSelect, onRestore }: {
  backups: BackupEntry[];
  locale: Locale;
  selected: string | null;
  restoring: boolean;
  onSelect: (fileName: string | null) => void;
  onRestore: (fileName: string) => void;
}) {
  const copy = settingsCopy[locale];
  const dateFormat = new Intl.DateTimeFormat(locale === "ja" ? "ja-JP" : "en-US", { dateStyle: "medium", timeStyle: "short" });
  const sizeFormat = new Intl.NumberFormat(locale === "ja" ? "ja-JP" : "en-US");
  if (backups.length === 0) return <p className="ui-muted">{copy.noBackups}</p>;
  return <ul className="ui-backup-list">{backups.map((backup) => <li key={backup.fileName}>
    <div className="ui-backup-row"><time dateTime={backup.createdAt}>{dateFormat.format(new Date(backup.createdAt))}</time><span>{sizeFormat.format(backup.sizeBytes)} B</span><button className="ui-button" type="button" onClick={() => onSelect(selected === backup.fileName ? null : backup.fileName)} aria-expanded={selected === backup.fileName}>{copy.restorePoint}</button></div>
    {selected === backup.fileName && <ConfirmPanel locale={locale} rows={[{ label: copy.restoreTarget, value: <code>{backup.fileName}</code> }, { label: copy.date, value: dateFormat.format(new Date(backup.createdAt)) }, { label: copy.data, value: copy.restoreWarning }]}>
      <button className="ui-button" type="button" onClick={() => onSelect(null)} disabled={restoring}>{copy.cancel}</button>
      <button className="ui-button ui-button--primary" type="button" onClick={() => onRestore(backup.fileName)} disabled={restoring}>{restoring ? copy.restoring : copy.restore}</button>
    </ConfirmPanel>}
  </li>)}</ul>;
}

export function SettingsPage({ locale, result, onLocaleChange, onEnvironment, onRestored }: {
  locale: Locale;
  result: BootstrapResult;
  onLocaleChange: (locale: Locale) => void;
  onEnvironment: (environment: EnvironmentStatus) => void;
  onRestored: (result: BootstrapResult) => void;
}) {
  const copy = settingsCopy[locale];
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    listBackups().then((items) => { if (active) setBackups(items); })
      .catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, []);

  const recheck = async () => {
    setChecking(true);
    setError(null);
    try { onEnvironment(await refreshEnvironment()); }
    catch (reason) { setError(String(reason)); }
    finally { setChecking(false); }
  };

  const openFolder = async () => {
    setError(null);
    setNotice(null);
    try {
      await openDataFolder();
      if (result.demoMode) setNotice(copy.demoFolder);
    } catch (reason) { setError(String(reason)); }
  };

  const restore = async (fileName: string) => {
    setRestoring(true);
    setError(null);
    setNotice(null);
    try {
      await restoreBackup(fileName);
      if (result.demoMode) {
        setNotice(copy.demoRestore);
      } else {
        onRestored(await bootstrap());
        setBackups(await listBackups());
        setNotice(copy.restored);
      }
      setSelected(null);
    } catch (reason) { setError(String(reason)); }
    finally { setRestoring(false); }
  };

  const tools: { key: "git" | "gh" | "ssh"; label: string; status: ToolStatus; help: string }[] = [
    { key: "git", label: copy.git, status: result.environment.git, help: copy.gitMissing },
    { key: "gh", label: copy.gh, status: result.environment.gh, help: copy.ghMissing },
    { key: "ssh", label: copy.ssh, status: result.environment.ssh, help: copy.sshMissing },
  ];

  return <div className="ui-page ui-settings-page">
    <header className="ui-page-heading"><h1>{copy.title}</h1></header>
    {error && <p className="ui-settings-error" role="alert"><AlertIcon />{error}</p>}
    {notice && <p className="ui-result" role="status">{notice}</p>}

    <section className="ui-card" aria-labelledby="settings-environment">
      <div className="ui-settings-heading"><h2 id="settings-environment">{copy.environment}</h2><button className="ui-button" type="button" onClick={recheck} disabled={checking}>{checking ? copy.checking : copy.recheck}</button></div>
      <div className="ui-environment-list">{tools.map(({ key, label, status, help }) => <div className="ui-environment-item" key={key}>
        <strong>{label}</strong>
        <span className={status.available ? "ui-success" : "ui-caution"}>{status.available ? <CheckIcon /> : <AlertIcon />}{status.available ? copy.found : copy.missing}</span>
        {status.available ? <span className="ui-muted">{status.version || status.detail || "—"}</span> : <p>{help}</p>}
      </div>)}</div>
    </section>

    <section className="ui-card" aria-labelledby="settings-language">
      <h2 id="settings-language">{copy.language}</h2>
      <label className="ui-settings-language" htmlFor="settings-locale"><span className="ui-sr-only">{copy.language}</span><select id="settings-locale" value={locale} onChange={(event) => onLocaleChange(event.currentTarget.value as Locale)}><option value="ja">日本語</option><option value="en">English</option></select></label>
    </section>

    <section className="ui-card" aria-labelledby="settings-data">
      <h2 id="settings-data">{copy.data}</h2>
      <div className="ui-data-folder"><div><strong>{copy.saveFolder}</strong><code>{folderOf(result.storagePath || "")}</code></div><button className="ui-button" type="button" onClick={openFolder}><FolderIcon />{copy.openFolder}</button></div>
      <p className="ui-muted">{result.demoMode ? `${copy.demo} — ${copy.demoNote}` : result.developmentData ? `${copy.development} — ${copy.developmentNote}` : copy.release}</p>
      <div className="ui-backup-heading"><h3>{copy.backups}</h3><p className="ui-muted">{copy.backupLimit}</p></div>
      <BackupList backups={backups} locale={locale} selected={selected} restoring={restoring} onSelect={setSelected} onRestore={restore} />
    </section>
  </div>;
}
