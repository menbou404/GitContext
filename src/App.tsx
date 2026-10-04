import { useEffect, useMemo, useState, type CSSProperties, type FormEvent } from "react";
import {
  addRepository,
  applyAssignment,
  bootstrap,
  chooseCloneDestinationDirectory,
  chooseGhConfigDirectory,
  chooseRepositoryDirectory,
  chooseSshKey,
  cloneGithubRepository,
  connectGithubProfile,
  inspectGithubProfile,
  inspectRepositoryStatuses,
  listenForGithubAuthPrompt,
  listGithubRepositories,
  openGithubAuthPage,
  previewAssignment,
  removeRepository,
  saveProfile,
  setLocale,
  setProfileAutoApprove,
  setRepositoryAutoApprove,
} from "./backend";
import {
  AlertIcon,
  BranchIcon,
  CheckIcon,
  CloseIcon,
  PlusIcon,
  SearchIcon,
  ShieldIcon,
  TerminalIcon,
} from "./Icons";
import type { AppData, ApplyPreview, AutoApprove, BootstrapResult, CloneOptions, GhProfileStatus, GithubAuthPrompt, GithubRepository, Profile } from "./types";
import type { RepositoryStatus } from "./types";
import { initials, profileIsComplete } from "./types";
import { localizeRuntimeMessage, shellCopy, uiCopy, type Locale } from "./i18n";
import { resolveLocale } from "./locale";
import { Shell, type ShellPage } from "./ui/Shell";
import { RepositoryList } from "./ui/RepositoryList";
import { RepositoryDetail } from "./ui/RepositoryDetail";
import { ProfileDot } from "./ui/ProfileDot";
import "./App.css";
import "./ui/ui.css";

const accents = ["#d8a33f", "#56a7d9", "#d97866", "#8e78d4", "#5ca989"];

const emptyProfile = (): Profile => ({
  id: crypto.randomUUID(),
  label: "",
  accent: accents[0],
  gitName: "",
  gitEmail: "",
  githubUsername: "",
  sshKeyPath: "",
  ghConfigDir: "",
  autoApprove: { cloneRepository: false },
});

interface EditingProfile {
  profile: Profile;
  creating: boolean;
}

const messageFrom = (error: unknown, locale: Locale) => {
  const message = error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : uiCopy[locale].somethingWentWrong;
  return localizeRuntimeMessage(message, locale);
};

function ProfileAvatar({ profile, size = "normal" }: { profile: Profile; size?: "small" | "normal" | "large" }) {
  return (
    <span
      className={`profile-avatar profile-avatar--${size}`}
      style={{ "--profile-accent": profile.accent } as CSSProperties}
      aria-hidden="true"
    >
      {initials(profile.label)}
    </span>
  );
}

function ProfileEditor({
  initial,
  creating,
  ghAvailable,
  locale,
  onClose,
  onSave,
  onAutoApprove,
  inline = false,
}: {
  initial: Profile;
  creating: boolean;
  ghAvailable: boolean;
  locale: Locale;
  onClose: () => void;
  onSave: (profile: Profile) => Promise<void>;
  onAutoApprove: (profileId: string, enabled: boolean) => Promise<void>;
  inline?: boolean;
}) {
  const copy = uiCopy[locale];
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [autoApproveSaving, setAutoApproveSaving] = useState(false);
  const [linking, setLinking] = useState(false);
  const [checking, setChecking] = useState(false);
  const [ghStatus, setGhStatus] = useState<GhProfileStatus | null>(null);
  const [authPrompt, setAuthPrompt] = useState<GithubAuthPrompt | null>(null);
  const [codeCopied, setCodeCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  useEffect(() => {
    if (!initial.ghConfigDir) return;
    let active = true;
    setChecking(true);
    inspectGithubProfile(initial.id, initial.ghConfigDir)
      .then((status) => {
        if (active) setGhStatus(status);
      })
      .catch(() => undefined)
      .finally(() => {
        if (active) setChecking(false);
      });
    return () => {
      active = false;
    };
  }, [initial.ghConfigDir, initial.id]);

  const update = (key: keyof Profile, value: string) => {
    if (key === "ghConfigDir") setGhStatus(null);
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const applyGhStatus = (status: GhProfileStatus) => {
    setGhStatus(status);
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
    <div className={inline ? "ui-inline-editor" : "modal-layer"} role={inline ? undefined : "presentation"} onMouseDown={inline ? undefined : onClose}>
      <form className={`modal profile-modal ${inline ? "ui-inline-profile-modal" : ""}`} onSubmit={submit} onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">{copy.repositoryIdentity}</p>
            <h2>{creating ? copy.createProfile : copy.editProfile}</h2>
            <p className="modal-lead profile-modal-lead">{copy.createProfileLead}</p>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}>
            <CloseIcon />
          </button>
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
                  <button key={accent} type="button" className={draft.accent === accent ? "selected" : ""} style={{ background: accent }} onClick={() => update("accent", accent)} aria-label={copy.useColor(accent)}>
                    {draft.accent === accent && <CheckIcon />}
                  </button>
                ))}
              </div>
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
              <strong>{ghStatus?.authenticated && ghStatus.username ? copy.connectedAs(ghStatus.username) : copy.githubNotConnectedDetail}</strong>
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

function CloneDialog({
  profiles,
  locale,
  onClose,
  onClone,
}: {
  profiles: Profile[];
  locale: Locale;
  onClose: () => void;
  onClone: (options: CloneOptions) => Promise<void>;
}) {
  const copy = uiCopy[locale];
  const firstProfile = profiles.find((profile) => profileIsComplete(profile) && profile.sshKeyPath) ?? profiles[0];
  const [profileId, setProfileId] = useState(firstProfile?.id ?? "");
  const [source, setSource] = useState<"list" | "url">("list");
  const [repositories, setRepositories] = useState<GithubRepository[]>([]);
  const [selectedUrl, setSelectedUrl] = useState("");
  const [manualUrl, setManualUrl] = useState("");
  const [destinationParent, setDestinationParent] = useState("");
  const [repositoryQuery, setRepositoryQuery] = useState("");
  const [loadingRepositories, setLoadingRepositories] = useState(false);
  const [repositoryError, setRepositoryError] = useState<string | null>(null);
  const [cloning, setCloning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const profile = profiles.find((item) => item.id === profileId) ?? null;

  const loadRepositories = async () => {
    if (!profileId || !profile?.githubUsername || !profile.ghConfigDir) {
      setRepositories([]);
      setSelectedUrl("");
      setRepositoryError(null);
      return;
    }
    setLoadingRepositories(true);
    setRepositoryError(null);
    try {
      const next = await listGithubRepositories(profileId);
      setRepositories(next);
      setSelectedUrl((current) => next.some((item) => item.sshUrl === current) ? current : "");
    } catch (cause) {
      setRepositories([]);
      setSelectedUrl("");
      setRepositoryError(messageFrom(cause, locale));
    } finally {
      setLoadingRepositories(false);
    }
  };

  useEffect(() => {
    if (source === "list") void loadRepositories();
    // The selected Profile is the complete dependency for this one-shot fetch.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profileId, source]);

  const filteredRepositories = useMemo(() => {
    const term = repositoryQuery.trim().toLowerCase();
    if (!term) return repositories;
    return repositories.filter((repository) =>
      [repository.nameWithOwner, repository.description ?? ""].some((value) => value.toLowerCase().includes(term)),
    );
  }, [repositories, repositoryQuery]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!profile) return;
    setError(null);
    setCloning(true);
    try {
      await onClone({
        profileId: profile.id,
        repositoryUrl: source === "list" ? selectedUrl : manualUrl,
        destinationParent,
      });
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setCloning(false);
    }
  };

  const profileReady = Boolean(profile && profileIsComplete(profile) && profile.sshKeyPath);
  const sourceUrl = source === "list" ? selectedUrl : manualUrl.trim();
  const canSubmit = profileReady && Boolean(sourceUrl) && Boolean(destinationParent.trim()) && !cloning;

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <form className="modal clone-modal" onSubmit={submit} onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">GitHub</p>
            <h2>{copy.cloneRepositoryTitle}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.cloneRepositoryLead}</p>

        <label className="field">
          <span>{copy.cloneProfile}</span>
          <select value={profileId} onChange={(event) => setProfileId(event.currentTarget.value)}>
            <option value="" disabled>{copy.selectProfile}</option>
            {profiles.map((item) => (
              <option value={item.id} key={item.id} disabled={!profileIsComplete(item) || !item.sshKeyPath}>
                {item.label}{item.githubUsername ? ` — @${item.githubUsername}` : ""}
              </option>
            ))}
          </select>
        </label>
        {!profiles.some((item) => profileIsComplete(item) && item.sshKeyPath) && <div className="warning-note"><AlertIcon />{copy.noCloneProfiles}</div>}
        {profile && !profile.sshKeyPath && <div className="warning-note"><AlertIcon />{copy.profileNeedsSsh}</div>}

        <div className="clone-source-tabs" role="tablist">
          <button className={source === "list" ? "active" : ""} type="button" onClick={() => setSource("list")}>{copy.chooseFromGithub}</button>
          <button className={source === "url" ? "active" : ""} type="button" onClick={() => setSource("url")}>{copy.enterSshUrl}</button>
        </div>

        {source === "list" ? (
          <section className="clone-source-panel">
            {!profile?.githubUsername || !profile.ghConfigDir ? (
              <div className="warning-note"><AlertIcon />{copy.profileNeedsGithub}</div>
            ) : (
              <>
                <div className="clone-list-toolbar">
                  <div className="search-box"><SearchIcon /><input value={repositoryQuery} onChange={(event) => setRepositoryQuery(event.currentTarget.value)} placeholder={copy.cloneSearch} /></div>
                  <button className="button button--ghost" type="button" onClick={loadRepositories} disabled={loadingRepositories}>{copy.refreshRepositories}</button>
                </div>
                {loadingRepositories ? (
                  <p className="clone-list-message">{copy.loadingRepositories}</p>
                ) : repositoryError ? (
                  <div className="inline-error"><AlertIcon />{repositoryError}</div>
                ) : filteredRepositories.length ? (
                  <div className="github-repository-list">
                    {filteredRepositories.map((repository) => (
                      <button className={selectedUrl === repository.sshUrl ? "selected" : ""} type="button" key={repository.sshUrl} onClick={() => setSelectedUrl(repository.sshUrl)}>
                        <span><strong>{repository.nameWithOwner}</strong><small>{repository.description || repository.sshUrl}</small></span>
                        <em>{repository.isPrivate ? copy.privateLabel : copy.publicLabel}</em>
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="clone-list-message">{copy.noGithubRepositories}</p>
                )}
              </>
            )}
          </section>
        ) : (
          <section className="clone-source-panel">
            <label className="field">
              <span>{copy.sshCloneUrl}</span>
              <input required={source === "url"} value={manualUrl} onChange={(event) => setManualUrl(event.currentTarget.value)} placeholder="git@github.com:owner/repository.git" spellCheck={false} />
              <small className="field-help">{copy.sshCloneUrlHelp}</small>
            </label>
          </section>
        )}

        <label className="field clone-destination-field">
          <span>{copy.cloneDestination}</span>
          <div className="path-input">
            <input required value={destinationParent} onChange={(event) => setDestinationParent(event.currentTarget.value)} placeholder="C:\\Users\\you\\Projects" />
            <button type="button" onClick={async () => { const path = await chooseCloneDestinationDirectory(copy.selectCloneDestinationDialog); if (path) setDestinationParent(path); }}>{copy.selectCloneDestination}</button>
          </div>
        </label>

        {profileReady && sourceUrl && destinationParent && (
          <div className="clone-summary">
            <ProfileAvatar profile={profile!} />
            <div><strong>{profile!.label}{profile!.githubUsername ? ` · @${profile!.githubUsername}` : ""}</strong><span>{sourceUrl}</span><small>{destinationParent}</small></div>
          </div>
        )}
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose} disabled={cloning}>{copy.cancel}</button>
          <button className="button button--primary" type="submit" disabled={!canSubmit}>{cloning ? copy.cloning : copy.cloneAndApply}</button>
        </div>
      </form>
    </div>
  );
}

function App({ previewLocale }: { previewLocale?: Locale }) {
  const [locale, updateLocale] = useState<Locale>(() => previewLocale ?? resolveLocale(undefined, navigator.language));
  const copy = uiCopy[locale];
  const [result, setResult] = useState<BootstrapResult | null>(null);
  const [loadingError, setLoadingError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [errorNotice, setErrorNotice] = useState<string | null>(null);
  const [environmentDismissed, setEnvironmentDismissed] = useState(false);
  const [statuses, setStatuses] = useState<Record<string, RepositoryStatus>>({});
  const [page, setPage] = useState<ShellPage>("repositories");
  const [selectedRepositoryId, setSelectedRepositoryId] = useState<string | null>(null);
  const [pendingProfileId, setPendingProfileId] = useState("");
  const [editingProfile, setEditingProfile] = useState<EditingProfile | null>(null);
  const [cloneOpen, setCloneOpen] = useState(false);
  const [preview, setPreview] = useState<ApplyPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [removeConfirmingId, setRemoveConfirmingId] = useState<string | null>(null);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = copy.documentTitle;
  }, [copy.documentTitle, locale]);

  useEffect(() => {
    bootstrap()
      .then((value) => {
        updateLocale(previewLocale ?? resolveLocale(value.data.settings?.locale, navigator.language));
        setResult(value);
        setSelectedRepositoryId(null);
        setPendingProfileId(value.data.repositories[0]?.profileId ?? value.data.profiles[0]?.id ?? "");
      })
      .catch((error) => setLoadingError(messageFrom(error, locale)));
  }, [previewLocale]);

  const changeLocale = async (nextLocale: Locale) => {
    if (nextLocale === locale) return;
    const previousLocale = locale;
    updateLocale(nextLocale);
    setNotice(null);
    setErrorNotice(null);
    try {
      await setLocale(nextLocale);
    } catch (error) {
      updateLocale(previousLocale);
      setErrorNotice(messageFrom(error, previousLocale));
    }
  };

  const data = result?.data;
  const environment = result?.environment;
  const selectedRepository = data?.repositories.find((repo) => repo.id === selectedRepositoryId) ?? null;
  const pendingProfile = data?.profiles.find((profile) => profile.id === pendingProfileId) ?? null;

  const refreshStatuses = () => {
    inspectRepositoryStatuses()
      .then((items) => setStatuses(Object.fromEntries(items.map((item) => [item.repositoryId, item]))))
      .catch((error) => setErrorNotice(`${messageFrom(error, locale)} ${shellCopy[locale].refresh}`));
  };

  useEffect(() => {
    if (data) refreshStatuses();
  }, [data]);

  const updateData = (nextData: AppData) => setResult((current) => current ? { ...current, data: nextData } : current);

  const selectRepository = (id: string) => {
    const repository = data?.repositories.find((repo) => repo.id === id);
    setSelectedRepositoryId(id);
    setRemoveConfirmingId(null);
    setNotice(null);
    setPendingProfileId(repository?.profileId ?? data?.profiles[0]?.id ?? "");
  };

  const addRepo = async () => {
    setNotice(null);
    setErrorNotice(null);
    try {
      const path = await chooseRepositoryDirectory(copy.selectRepositoryDialog);
      if (!path) return;
      setBusy(true);
      const repository = await addRepository(path);
      if (data) updateData({ ...data, repositories: [...data.repositories.filter((repo) => repo.id !== repository.id), repository] });
      setSelectedRepositoryId(repository.id);
      setPendingProfileId(data?.profiles[0]?.id ?? "");
      setNotice(copy.repositoryAdded(repository.name));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const saveProfileAction = async (profile: Profile) => {
    const nextData = await saveProfile(profile);
    updateData(nextData);
    setNotice(copy.profileSaved(profile.label));
  };

  const updateProfileAutoApprove = async (profileId: string, enabled: boolean) => {
    const nextData = await setProfileAutoApprove(profileId, { cloneRepository: enabled });
    updateData(nextData);
  };

  const updateAutoApprove = async (field: keyof AutoApprove, enabled: boolean) => {
    if (!selectedRepository || busy) return;
    setBusy(true);
    setNotice(null);
    setErrorNotice(null);
    try {
      const nextData = await setRepositoryAutoApprove(selectedRepository.id, {
        ...selectedRepository.autoApprove,
        [field]: enabled,
      });
      updateData(nextData);
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const reviewAssignment = async () => {
    if (!selectedRepository || !pendingProfile) return;
    setNotice(null);
    setErrorNotice(null);
    setBusy(true);
    try {
      setPreview(await previewAssignment(selectedRepository.id, pendingProfile.id));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const applyProfileAction = async () => {
    if (!preview) return;
    const nextData = await applyAssignment(preview.repository.id, preview.profile.id);
    updateData(nextData);
    setPendingProfileId(preview.profile.id);
    setPreview(null);
    setNotice(copy.profileApplied(preview.profile.label, preview.repository.name));
  };

  const cloneRepositoryAction = async (options: CloneOptions) => {
    const cloned = await cloneGithubRepository(options);
    const profile = cloned.data.profiles.find((item) => item.id === options.profileId);
    updateData(cloned.data);
    setCloneOpen(false);
    setSelectedRepositoryId(cloned.repository.id);
    setPendingProfileId(options.profileId);
    setNotice(copy.repositoryCloned(cloned.repository.name, profile?.label ?? "Profile"));
  };

  const removeSelected = async () => {
    if (!selectedRepository || removeConfirmingId !== selectedRepository.id) return;
    try {
      const nextData = await removeRepository(selectedRepository.id);
      updateData(nextData);
      const nextSelected = nextData.repositories[0] ?? null;
      setSelectedRepositoryId(null);
      setPendingProfileId(nextSelected?.profileId ?? nextData.profiles[0]?.id ?? "");
      setNotice(copy.repositoryRemoved);
      setRemoveConfirmingId(null);
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    }
  };

  if (loadingError) {
    return <main className="fatal-state"><AlertIcon /><h1>{copy.startFailed}</h1><p>{loadingError}</p><button className="button button--primary" onClick={() => window.location.reload()}>{copy.tryAgain}</button></main>;
  }

  if (!result || !data || !environment) {
    return <main className="loading-state"><span className="brand-mark"><BranchIcon /></span><p>{copy.inspectingEnvironment}</p></main>;
  }

  const selectedStatus = selectedRepository ? statuses[selectedRepository.id] : undefined;
  const missingTools = [!environment.git.available && "git", !environment.gh.available && "gh", !environment.ssh.available && "SSH"].filter(Boolean).join(" / ");
  const visibleNotice = errorNotice || (missingTools && !environmentDismissed ? shellCopy[locale].envMissing(missingTools) : null);

  return (
    <Shell page={page} locale={locale} repositoryCount={data.repositories.length} notice={visibleNotice} noticeLink={!errorNotice && (!environment.gh.available || !environment.ssh.available) ? "profiles" : null} onDismiss={() => { if (errorNotice) setErrorNotice(null); else setEnvironmentDismissed(true); }} onNavigate={(nextPage) => { setPage(nextPage); setSelectedRepositoryId(null); setEditingProfile(null); setPreview(null); setNotice(null); }} onLocaleChange={changeLocale}>
      {notice && <p className="ui-result" role="status">{notice}</p>}
      {page === "profiles" ? <div className="ui-page">
        <div className="ui-page-heading"><h1>{shellCopy[locale].profiles}</h1><div className="ui-heading-spacer" /><button className="ui-button ui-button--primary" type="button" onClick={() => setEditingProfile({ profile: emptyProfile(), creating: true })}><PlusIcon />{shellCopy[locale].addProfile}</button></div>
        {!editingProfile && <div className="ui-profile-list">{data.profiles.map((profile) => <article className="ui-card" key={profile.id}>
          <div className="ui-profile"><ProfileDot profile={profile} /><strong>{profile.label}</strong><small>{profile.githubUsername ? `@${profile.githubUsername}` : copy.githubNotLinked}</small></div>
          <span className="ui-muted">{!profileIsComplete(profile) ? copy.needsSetup : data.repositories.some((repo) => repo.profileId === profile.id && statuses[repo.id]?.github?.authenticated && statuses[repo.id]?.github?.username?.toLowerCase() === profile.githubUsername?.toLowerCase()) ? copy.githubLinked : profile.githubUsername ? shellCopy[locale].notConnected : copy.gitOnly}</span>
          <span className="ui-muted">{shellCopy[locale].assignedCount(data.repositories.filter((repo) => repo.profileId === profile.id).length)}</span>
          <button className="ui-button" type="button" onClick={() => setEditingProfile({ profile, creating: false })}>{shellCopy[locale].editProfile}</button>
        </article>)}</div>}
        {!data.profiles.length && !editingProfile && <p>{shellCopy[locale].noProfiles}</p>}
        {editingProfile && <ProfileEditor initial={editingProfile.profile} creating={editingProfile.creating} ghAvailable={environment.gh.available} locale={locale} onClose={() => setEditingProfile(null)} onSave={saveProfileAction} onAutoApprove={updateProfileAutoApprove} inline />}
      </div> : selectedRepository ? <RepositoryDetail key={selectedRepository.id} repository={selectedRepository} profiles={data.profiles} status={selectedStatus} pendingProfileId={pendingProfileId} preview={preview} locale={locale} busy={busy} removing={removeConfirmingId === selectedRepository.id}
        onBack={() => { setSelectedRepositoryId(null); setPreview(null); setNotice(null); }} onPendingProfile={setPendingProfileId} onReview={reviewAssignment} onCancelReview={() => setPreview(null)} onApply={applyProfileAction} onAutoApprove={updateAutoApprove}
        onStartRemove={() => setRemoveConfirmingId(selectedRepository.id)} onCancelRemove={() => setRemoveConfirmingId(null)} onRemove={removeSelected}
        onData={updateData} onFinished={refreshStatuses} />
        : <RepositoryList repositories={data.repositories} profiles={data.profiles} statuses={statuses} locale={locale} busy={busy} onOpen={selectRepository} onAdd={addRepo} onClone={() => setCloneOpen(true)} onRefresh={refreshStatuses} />}

      {cloneOpen && <CloneDialog profiles={data.profiles} locale={locale} onClose={() => setCloneOpen(false)} onClone={cloneRepositoryAction} />}
    </Shell>
  );
}

export default App;
