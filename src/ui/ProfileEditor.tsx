import { useEffect, useState, type FormEvent } from "react";
import { chooseGhConfigDirectory, chooseSshKey, connectGithubProfile, inspectGithubProfile, listenForGithubAuthPrompt, openGithubAuthPage } from "../backend";
import { AlertIcon, CheckIcon, ShieldIcon, TerminalIcon } from "../Icons";
import { localizeRuntimeMessage, profileCopy, uiCopy, type Locale } from "../i18n";
import type { GhProfileStatus, GithubAuthPrompt, Profile } from "../types";
import { connectionFor } from "./profileStatus";

const accents = ["#D8A33F", "#56A7D9", "#D97866", "#8E78D4", "#5CA989"];
const messageFrom = (error: unknown, locale: Locale) => localizeRuntimeMessage(error instanceof Error ? error.message : String(error), locale);

export function ProfileEditor({
  initial,
  creating,
  ghAvailable,
  locale,
  onClose,
  onSave,
  onAutoApprove,
  initialStatus,
  onStatus,
}: {
  initial: Profile;
  creating: boolean;
  ghAvailable: boolean;
  locale: Locale;
  onClose: () => void;
  onSave: (profile: Profile) => Promise<void>;
  onAutoApprove: (profileId: string, enabled: boolean) => Promise<void>;
  initialStatus?: GhProfileStatus | null;
  onStatus?: (profileId: string, status: GhProfileStatus) => void;
}) {
  const copy = uiCopy[locale];
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [autoApproveSaving, setAutoApproveSaving] = useState(false);
  const [linking, setLinking] = useState(false);
  const [checking, setChecking] = useState(false);
  const [ghStatus, setGhStatus] = useState<GhProfileStatus | null>(initialStatus ?? null);
  const [authPrompt, setAuthPrompt] = useState<GithubAuthPrompt | null>(null);
  const [codeCopied, setCodeCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialStatus) setGhStatus(initialStatus);
  }, [initialStatus]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listenForGithubAuthPrompt((prompt) => {
      if (!active || prompt.profileId !== initial.id) return;
      setAuthPrompt(prompt);
      setCodeCopied(true);
    })
      .then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      })
      .catch((cause) => {
        if (active) setError(messageFrom(cause, locale));
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [initial.id, locale]);

  const update = (key: keyof Profile, value: string) => {
    if (key === "ghConfigDir") setGhStatus(null);
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const applyGhStatus = (status: GhProfileStatus) => {
    setGhStatus(status);
    onStatus?.(draft.id, status);
    setDraft((current) => ({
      ...current,
      ghConfigDir: status.configDir ?? current.ghConfigDir,
      githubUsername: status.username ?? current.githubUsername,
    }));
  };

  const connectGithub = async () => {
    setError(null);
    setAuthPrompt(null);
    setCodeCopied(false);
    setLinking(true);
    try {
      applyGhStatus(await connectGithubProfile(draft.id, draft.ghConfigDir));
      setAuthPrompt(null);
    } catch (cause) {
      setAuthPrompt(null);
      setError(messageFrom(cause, locale));
    } finally {
      setLinking(false);
    }
  };

  const copyAuthCode = async () => {
    if (!authPrompt) return;
    try {
      await navigator.clipboard.writeText(authPrompt.code);
      setCodeCopied(true);
    } catch {
      setError(copy.couldNotCopyCode);
    }
  };

  const reopenGithubAuth = async () => {
    try {
      await openGithubAuthPage();
    } catch (cause) {
      setError(messageFrom(cause, locale));
    }
  };

  const checkGithub = async () => {
    setError(null);
    setChecking(true);
    try {
      applyGhStatus(await inspectGithubProfile(draft.id, draft.ghConfigDir));
    } catch (cause) {
      setError(messageFrom(cause, locale));
    } finally {
      setChecking(false);
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await onSave(draft);
      onClose();
    } catch (cause) {
      setError(messageFrom(cause, locale));
    } finally {
      setSaving(false);
    }
  };

  const updateCloneApproval = async (enabled: boolean) => {
    setError(null);
    setAutoApproveSaving(true);
    try {
      await onAutoApprove(draft.id, enabled);
      setDraft((current) => ({ ...current, autoApprove: { cloneRepository: enabled } }));
    } catch (cause) {
      setError(messageFrom(cause, locale));
    } finally {
      setAutoApproveSaving(false);
    }
  };

  return (
    <div className="ui-inline-editor">
      <form className="modal profile-modal ui-inline-profile-modal" onSubmit={submit}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">{copy.repositoryIdentity}</p>
            <h1>{creating ? copy.createProfile : copy.editProfile}</h1>
            <p className="modal-lead profile-modal-lead">{copy.createProfileLead}</p>
          </div>

        </div>

        <section className="profile-setup-section">
          <div className="setup-section-head"><span>1</span><div><h3>{copy.identitySection}</h3><p>{copy.identitySectionLead}</p></div></div>
          <div className="profile-form-grid">
            <label className="field field--wide">
              <span>{copy.profileName}</span>
              <input required value={draft.label} onChange={(event) => update("label", event.currentTarget.value)} placeholder={copy.profileNamePlaceholder} />
            </label>
            <fieldset className="accent-field field--wide">
              <legend>{copy.color}</legend>
              <div className="accent-options">
                {accents.map((accent) => (
                  <button key={accent} type="button" className={draft.accent.toLowerCase() === accent.toLowerCase() ? "selected" : ""} style={{ background: accent }} onClick={() => update("accent", accent)} aria-label={copy.useColor(accent)}>
                    {draft.accent.toLowerCase() === accent.toLowerCase() && <CheckIcon />}
                  </button>
                ))}
              </div>
              <label className="ui-color-custom">{copy.customColor}<input type="text" pattern="#[0-9a-fA-F]{6}" value={draft.accent} onChange={(event) => update("accent", event.currentTarget.value)} /></label>
            </fieldset>
            <label className="field">
              <span>{copy.gitAuthorName}</span>
              <input required value={draft.gitName} onChange={(event) => update("gitName", event.currentTarget.value)} placeholder={copy.gitAuthorNamePlaceholder} />
            </label>
            <label className="field">
              <span>{copy.gitAuthorEmail}</span>
              <input required type="email" value={draft.gitEmail} onChange={(event) => update("gitEmail", event.currentTarget.value)} placeholder="you@example.com" />
            </label>
          </div>
        </section>

        <section className="profile-setup-section">
          <div className="setup-section-head"><span>2</span><div><h3>{copy.githubConnection}</h3><p>{copy.githubConnectionLead}</p></div></div>
          <div className={`connection-card ${ghStatus?.authenticated ? "is-connected" : ""}`}>
            <span className="connection-icon"><TerminalIcon /></span>
            <div>
              <strong>{connectionFor(draft, ghStatus) === "different" ? profileCopy[locale].different : ghStatus?.authenticated && ghStatus.username ? copy.connectedAs(ghStatus.username) : copy.githubNotConnectedDetail}</strong>
              <small>{linking ? copy.waitingForGithub : !ghAvailable ? copy.ghCliMissing : ghStatus?.detail ? localizeRuntimeMessage(ghStatus.detail, locale) : copy.githubConnectionLead}</small>
            </div>
            {ghAvailable && (
              <div className="connection-actions">
                {draft.ghConfigDir && <button className="button button--ghost" type="button" onClick={checkGithub} disabled={checking || linking}>{checking ? copy.checkingConnection : copy.checkConnection}</button>}
                <button className="button button--primary" type="button" onClick={connectGithub} disabled={linking || checking}>{linking ? copy.waitingForGithub : ghStatus?.authenticated ? copy.reconnectGithub : copy.connectGithub}</button>
              </div>
            )}
          </div>
          {authPrompt && (
            <div className="github-device-card" role="status" aria-live="polite">
              <div className="github-device-copy">
                <span>{copy.githubOneTimeCode}</span>
                <code>{authPrompt.code}</code>
              </div>
              <p>{copy.githubDeviceInstructions}</p>
              <div className="github-device-actions">
                <button className="button button--ghost" type="button" onClick={copyAuthCode}>{codeCopied ? copy.codeCopied : copy.copyCode}</button>
                <button className="button button--primary" type="button" onClick={reopenGithubAuth}>{copy.openGithubAuthPage}</button>
              </div>
            </div>
          )}
          {!ghAvailable && <code className="install-command">{copy.ghInstallCommand}</code>}
          <div className="profile-form-grid compact-grid">
            <label className="field">
              <span>{copy.githubUsername} <small>{copy.optional}</small></span>
              <div className="input-prefix"><span>@</span><input value={draft.githubUsername ?? ""} onChange={(event) => update("githubUsername", event.currentTarget.value)} placeholder={copy.usernamePlaceholder} /></div>
            </label>
            <label className="field">
              <span>{copy.existingGhDirectory} <small>{copy.optional}</small></span>
              <div className="path-input">
                <input value={draft.ghConfigDir ?? ""} onChange={(event) => update("ghConfigDir", event.currentTarget.value)} placeholder="C:\\Users\\you\\.config\\gh-profile" />
                <button type="button" onClick={async () => { const path = await chooseGhConfigDirectory(copy.selectGhDirectoryDialog); if (path) update("ghConfigDir", path); }}>{copy.browse}</button>
              </div>
            </label>
          </div>
        </section>

        <section className="profile-setup-section">
          <div className="setup-section-head"><span>3</span><div><h3>{copy.sshConnection}</h3><p>{copy.sshConnectionLead}</p></div></div>
          <label className="field">
            <span>{copy.existingSshKey} <small>{copy.referenceOnly}</small></span>
            <div className="path-input">
              <input value={draft.sshKeyPath ?? ""} onChange={(event) => update("sshKeyPath", event.currentTarget.value)} placeholder="C:\\Users\\you\\.ssh\\id_ed25519_profile" />
              <button type="button" onClick={async () => { const path = await chooseSshKey(copy.selectSshKeyDialog); if (path) update("sshKeyPath", path); }}>{copy.browse}</button>
            </div>
          </label>
        </section>

        {!creating && <fieldset className="auto-approve-settings" disabled={autoApproveSaving}>
          <legend>{copy.autoApproveTitle}</legend>
          <label><input type="checkbox" checked={draft.autoApprove.cloneRepository} onChange={(event) => updateCloneApproval(event.currentTarget.checked)} />{copy.autoApproveClone}</label>
        </fieldset>}

        <div className="privacy-note">
          <ShieldIcon />
          <span>{copy.privacyNote}</span>
        </div>
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose}>{copy.cancel}</button>
          <button className="button button--primary" disabled={saving} type="submit">{saving ? copy.saving : copy.saveProfile}</button>
        </div>
      </form>
    </div>
  );
}
