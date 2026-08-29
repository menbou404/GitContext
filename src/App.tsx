import { useEffect, useMemo, useState, type CSSProperties, type FormEvent } from "react";
import {
  addRepository,
  applyAssignment,
  bootstrap,
  chooseGhConfigDirectory,
  chooseRepositoryDirectory,
  chooseSshKey,
  connectGithubProfile,
  inspectGithubProfile,
  previewAssignment,
  removeRepository,
  saveProfile,
} from "./backend";
import {
  AlertIcon,
  BranchIcon,
  CheckIcon,
  ChevronIcon,
  CloseIcon,
  EditIcon,
  FolderIcon,
  KeyIcon,
  MoreIcon,
  PlusIcon,
  SearchIcon,
  ShieldIcon,
  TerminalIcon,
  TrashIcon,
} from "./Icons";
import type { AppData, ApplyPreview, BootstrapResult, GhProfileStatus, Profile } from "./types";
import { compactPath, initials, profileIsComplete } from "./types";
import { localizeRuntimeMessage, uiCopy, type Locale } from "./i18n";
import "./App.css";

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

const formatAppliedAt = (value: string | null | undefined, locale: Locale) => {
  const copy = uiCopy[locale];
  if (!value) return copy.notApplied;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return copy.applied;
  return new Intl.DateTimeFormat(locale === "ja" ? "ja-JP" : undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
};

function ToolBadge({ label, available }: { label: string; available: boolean }) {
  return (
    <span className={`tool-badge ${available ? "is-ready" : "is-missing"}`}>
      <span className="status-dot" />
      {label}
    </span>
  );
}

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
}: {
  initial: Profile;
  creating: boolean;
  ghAvailable: boolean;
  locale: Locale;
  onClose: () => void;
  onSave: (profile: Profile) => Promise<void>;
}) {
  const copy = uiCopy[locale];
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [linking, setLinking] = useState(false);
  const [checking, setChecking] = useState(false);
  const [ghStatus, setGhStatus] = useState<GhProfileStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

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
    setLinking(true);
    try {
      applyGhStatus(await connectGithubProfile(draft.id, draft.ghConfigDir));
    } catch (cause) {
      setError(messageFrom(cause, locale));
    } finally {
      setLinking(false);
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

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <form className="modal profile-modal" onSubmit={submit} onMouseDown={(event) => event.stopPropagation()}>
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

function ApplyDialog({
  preview,
  locale,
  onClose,
  onApply,
}: {
  preview: ApplyPreview;
  locale: Locale;
  onClose: () => void;
  onApply: () => Promise<void>;
}) {
  const copy = uiCopy[locale];
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const apply = async () => {
    setApplying(true);
    setError(null);
    try {
      await onApply();
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setApplying(false);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <section className="modal apply-modal" role="dialog" aria-modal="true" aria-labelledby="apply-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">{copy.reviewChanges}</p>
            <h2 id="apply-title">{copy.applyQuestion(preview.profile.label, preview.repository.name)}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.applyLead}</p>

        <div className="change-list">
          {preview.changes.map((change) => (
            <div className="change-row" key={change.key}>
              <code>{change.key}</code>
              <div className="change-values">
                <span>{change.currentValue || copy.notSet}</span>
                <ChevronIcon />
                <strong>{change.nextValue}</strong>
              </div>
            </div>
          ))}
        </div>
        {preview.warnings.map((warning) => <div className="warning-note" key={warning}><AlertIcon />{localizeRuntimeMessage(warning, locale)}</div>)}
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose}>{copy.cancel}</button>
          <button className="button button--primary" type="button" onClick={apply} disabled={applying}>{applying ? copy.applying : copy.applySafely}</button>
        </div>
      </section>
    </div>
  );
}

function App({ locale = "en" }: { locale?: Locale }) {
  const copy = uiCopy[locale];
  const [result, setResult] = useState<BootstrapResult | null>(null);
  const [loadingError, setLoadingError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [profileFilter, setProfileFilter] = useState<string | null>(null);
  const [selectedRepositoryId, setSelectedRepositoryId] = useState<string | null>(null);
  const [pendingProfileId, setPendingProfileId] = useState("");
  const [editingProfile, setEditingProfile] = useState<EditingProfile | null>(null);
  const [preview, setPreview] = useState<ApplyPreview | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = copy.documentTitle;
  }, [copy.documentTitle, locale]);

  useEffect(() => {
    bootstrap()
      .then((value) => {
        setResult(value);
        setSelectedRepositoryId(value.data.repositories[0]?.id ?? null);
        setPendingProfileId(value.data.repositories[0]?.profileId ?? value.data.profiles[0]?.id ?? "");
      })
      .catch((error) => setLoadingError(messageFrom(error, locale)));
  }, [locale]);

  const data = result?.data;
  const environment = result?.environment;
  const selectedRepository = data?.repositories.find((repo) => repo.id === selectedRepositoryId) ?? null;
  const assignedProfile = data?.profiles.find((profile) => profile.id === selectedRepository?.profileId) ?? null;
  const pendingProfile = data?.profiles.find((profile) => profile.id === pendingProfileId) ?? null;

  const filteredRepositories = useMemo(() => {
    if (!data) return [];
    const term = query.trim().toLowerCase();
    return data.repositories.filter((repo) => {
      const matchesProfile = !profileFilter || repo.profileId === profileFilter;
      const matchesQuery = !term || [repo.name, repo.path, repo.remoteUrl ?? ""].some((value) => value.toLowerCase().includes(term));
      return matchesProfile && matchesQuery;
    });
  }, [data, profileFilter, query]);

  const updateData = (nextData: AppData) => setResult((current) => current ? { ...current, data: nextData } : current);

  const selectRepository = (id: string) => {
    const repository = data?.repositories.find((repo) => repo.id === id);
    setSelectedRepositoryId(id);
    setPendingProfileId(repository?.profileId ?? data?.profiles[0]?.id ?? "");
  };

  const addRepo = async () => {
    setNotice(null);
    try {
      const path = await chooseRepositoryDirectory(copy.selectRepositoryDialog);
      if (!path) return;
      setBusy(true);
      const repository = await addRepository(path);
      if (data) updateData({ ...data, repositories: [...data.repositories.filter((repo) => repo.id !== repository.id), repository] });
      setProfileFilter(null);
      setSelectedRepositoryId(repository.id);
      setPendingProfileId(data?.profiles[0]?.id ?? "");
      setNotice(copy.repositoryAdded(repository.name));
    } catch (error) {
      setNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const saveProfileAction = async (profile: Profile) => {
    const nextData = await saveProfile(profile);
    updateData(nextData);
    setNotice(copy.profileSaved(profile.label));
  };

  const reviewAssignment = async () => {
    if (!selectedRepository || !pendingProfile) return;
    setNotice(null);
    setBusy(true);
    try {
      setPreview(await previewAssignment(selectedRepository.id, pendingProfile.id));
    } catch (error) {
      setNotice(messageFrom(error, locale));
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

  const removeSelected = async () => {
    if (!selectedRepository || !window.confirm(copy.removeConfirm(selectedRepository.name))) return;
    try {
      const nextData = await removeRepository(selectedRepository.id);
      updateData(nextData);
      const nextSelected = nextData.repositories[0] ?? null;
      setSelectedRepositoryId(nextSelected?.id ?? null);
      setPendingProfileId(nextSelected?.profileId ?? nextData.profiles[0]?.id ?? "");
      setNotice(copy.repositoryRemoved);
    } catch (error) {
      setNotice(messageFrom(error, locale));
    }
  };

  if (loadingError) {
    return <main className="fatal-state"><AlertIcon /><h1>{copy.startFailed}</h1><p>{loadingError}</p><button className="button button--primary" onClick={() => window.location.reload()}>{copy.tryAgain}</button></main>;
  }

  if (!result || !data || !environment) {
    return <main className="loading-state"><span className="brand-mark"><BranchIcon /></span><p>{copy.inspectingEnvironment}</p></main>;
  }

  const assignedCount = data.repositories.filter((repo) => repo.profileId).length;
  const readyProfileCount = data.profiles.filter(profileIsComplete).length;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark"><BranchIcon /></span>
          <div><strong>GitContext</strong><small>{copy.identityManager}</small></div>
        </div>

        <nav className="side-nav" aria-label={copy.mainNavigation}>
          <button className="active"><FolderIcon />{copy.repositories}<span>{data.repositories.length}</span></button>
          <button><ShieldIcon />{copy.safetyLog}<span className="soon">{copy.soon}</span></button>
        </nav>

        <div className="profiles-heading">
          <span>{copy.profiles}</span>
          <button className="icon-button icon-button--dark" onClick={() => setEditingProfile({ profile: emptyProfile(), creating: true })} aria-label={copy.addProfile}><PlusIcon /></button>
        </div>
        <div className="profile-list">
          {data.profiles.map((profile) => {
            const complete = profileIsComplete(profile);
            const count = data.repositories.filter((repo) => repo.profileId === profile.id).length;
            return (
              <div className={`profile-card ${profileFilter === profile.id ? "selected" : ""}`} key={profile.id}>
                <button className="profile-card-main" onClick={() => setProfileFilter(profileFilter === profile.id ? null : profile.id)}>
                  <ProfileAvatar profile={profile} />
                  <span className="profile-copy"><strong>{profile.label}</strong><small>{profile.githubUsername ? `@${profile.githubUsername}` : copy.githubNotLinked}</small></span>
                  <span className="profile-count">{count}</span>
                </button>
                <div className="profile-card-foot">
                  <span className={complete ? "ready" : "needs-setup"}>{complete ? <CheckIcon /> : <AlertIcon />}{complete ? (profile.githubUsername ? copy.githubLinked : copy.gitOnly) : copy.needsSetup}</span>
                  <button onClick={() => setEditingProfile({ profile, creating: false })} aria-label={copy.editProfileLabel(profile.label)}><EditIcon /></button>
                </div>
              </div>
            );
          })}
        </div>
        {!data.profiles.length && (
          <div className="profile-empty-card">
            <strong>{copy.noProfilesTitle}</strong>
            <p>{copy.noProfilesLead}</p>
            <button className="button" onClick={() => setEditingProfile({ profile: emptyProfile(), creating: true })}><PlusIcon />{copy.createFirstProfile}</button>
          </div>
        )}

        <div className="sidebar-security">
          <ShieldIcon />
          <div><strong>{copy.secretsStayOutside}</strong><small>{copy.usesExistingSecrets}</small></div>
        </div>
      </aside>

      <div className="workspace">
        <header className="topbar">
          <div><p className="breadcrumb">{copy.workspace} / <strong>{copy.repositories}</strong></p></div>
          <div className="topbar-tools">
            <ToolBadge label="Git" available={environment.git.available} />
            <ToolBadge label="gh" available={environment.gh.available} />
            <ToolBadge label="SSH" available={environment.ssh.available} />
            <span className="topbar-divider" />
            <span className="local-badge">{copy.localOnly}</span>
            <a className="language-link" href={locale === "ja" ? "./index.html" : "./ja.html"} aria-label={copy.switchLanguageLabel}>{copy.switchLanguage}</a>
          </div>
        </header>

        <main className="content">
          {result.demoMode && <div className="demo-banner"><span>{copy.demoPreview}</span> {copy.demoNotice}</div>}
          {notice && <div className="notice"><CheckIcon /><span>{notice}</span><button onClick={() => setNotice(null)} aria-label={copy.dismiss}><CloseIcon /></button></div>}

          <section className="hero-row">
            <div>
              <p className="eyebrow">{copy.repositoryIdentities}</p>
              <h1>{copy.heroTitle}</h1>
              <p>{copy.heroDescription}</p>
            </div>
            <button className="button button--primary button--add" onClick={addRepo} disabled={busy}><PlusIcon />{copy.addRepository}</button>
          </section>

          <section className="summary-grid" aria-label={copy.workspaceSummary}>
            <article><span className="summary-icon summary-icon--green"><FolderIcon /></span><div><strong>{data.repositories.length}</strong><small>{copy.repositories}</small></div><em>{copy.assigned(assignedCount, data.repositories.length || 0)}</em></article>
            <article><span className="summary-icon summary-icon--gold"><ShieldIcon /></span><div><strong>{readyProfileCount}</strong><small>{copy.readyProfiles}</small></div><em>{copy.total(data.profiles.length)}</em></article>
            <article><span className={`summary-icon ${environment.gh.available ? "summary-icon--blue" : "summary-icon--muted"}`}><TerminalIcon /></span><div><strong>{environment.gh.available ? copy.ready : copy.missing}</strong><small>GitHub CLI</small></div><em>{environment.gh.available ? copy.authenticatedExternally : copy.optionalForMvp}</em></article>
          </section>

          <div className={`repository-layout ${selectedRepository ? "has-inspector" : ""}`}>
            <section className="repository-panel">
              <div className="panel-toolbar">
                <div className="search-box"><SearchIcon /><input value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder={copy.searchRepositories} /></div>
                {profileFilter && <button className="filter-chip" onClick={() => setProfileFilter(null)}>{data.profiles.find((profile) => profile.id === profileFilter)?.label}<CloseIcon /></button>}
                <span className="panel-count">{copy.shown(filteredRepositories.length)}</span>
              </div>

              {filteredRepositories.length ? (
                <div className="repo-table">
                  <div className="repo-table-head"><span>{copy.repository}</span><span>{copy.profile}</span><span>{copy.status}</span><span /></div>
                  {filteredRepositories.map((repository) => {
                    const profile = data.profiles.find((item) => item.id === repository.profileId);
                    const selected = repository.id === selectedRepositoryId;
                    return (
                      <button className={`repo-row ${selected ? "selected" : ""}`} key={repository.id} onClick={() => selectRepository(repository.id)}>
                        <span className="repo-main"><span className="repo-icon"><FolderIcon /></span><span><strong>{repository.name}</strong><small title={repository.path}>{compactPath(repository.path)}</small></span></span>
                        <span>{profile ? <span className="profile-pill"><ProfileAvatar profile={profile} size="small" />{profile.label}</span> : <span className="unassigned-pill"><AlertIcon />{copy.unassigned}</span>}</span>
                        <span className={repository.profileId ? "repo-status is-configured" : "repo-status"}><span className="status-dot" />{repository.profileId ? copy.configured : copy.actionNeeded}</span>
                        <span className="repo-chevron"><ChevronIcon /></span>
                      </button>
                    );
                  })}
                </div>
              ) : (
                <div className="empty-state"><span><FolderIcon /></span><h3>{copy.noRepositories}</h3><p>{data.repositories.length ? copy.noRepositoriesFiltered : copy.noRepositoriesYet}</p>{!data.repositories.length && <button className="button button--primary" onClick={addRepo}><PlusIcon />{copy.addRepository}</button>}</div>
              )}
            </section>

            {selectedRepository && (
              <aside className="inspector">
                <div className="inspector-head">
                  <div className="repo-icon repo-icon--large"><FolderIcon /></div>
                  <div><p className="eyebrow">{copy.selectedRepository}</p><h2>{selectedRepository.name}</h2></div>
                  <button className="icon-button" aria-label={copy.repositoryMenu}><MoreIcon /></button>
                </div>
                <p className="inspector-path" title={selectedRepository.path}>{selectedRepository.path}</p>
                <div className="repo-meta">
                  <span><BranchIcon />{selectedRepository.branch || copy.noBranch}</span>
                  <span>{formatAppliedAt(selectedRepository.lastAppliedAt, locale)}</span>
                </div>

                <div className="inspector-section">
                  <label htmlFor="profile-select">{copy.assignedProfile}</label>
                  <div className="profile-select-wrap">
                    {pendingProfile && <ProfileAvatar profile={pendingProfile} size="small" />}
                    <select id="profile-select" value={pendingProfileId} onChange={(event) => setPendingProfileId(event.currentTarget.value)}>
                      <option value="" disabled>{copy.selectProfile}</option>
                      {data.profiles.map((profile) => <option value={profile.id} key={profile.id}>{profile.label}</option>)}
                    </select>
                  </div>
                  {pendingProfile && !profileIsComplete(pendingProfile) && <div className="field-warning"><AlertIcon />{copy.completeProfile}</div>}
                </div>

                <div className="identity-card">
                  <div className="identity-card-title"><span>{copy.effectiveIdentity}</span>{assignedProfile ? <span className="ready"><CheckIcon />{copy.applied}</span> : <span className="needs-setup"><AlertIcon />{copy.notSet}</span>}</div>
                  <dl>
                    <div><dt>{copy.commitAs}</dt><dd>{assignedProfile?.gitName || "—"}</dd></div>
                    <div><dt>{copy.email}</dt><dd>{assignedProfile?.gitEmail || "—"}</dd></div>
                    <div><dt>{copy.sshKey}</dt><dd title={assignedProfile?.sshKeyPath ?? undefined}><KeyIcon />{assignedProfile?.sshKeyPath ? compactPath(assignedProfile.sshKeyPath, 28) : copy.notManaged}</dd></div>
                    <div><dt>GitHub CLI</dt><dd><TerminalIcon />{assignedProfile?.ghConfigDir ? copy.profileDirectory : copy.notManaged}</dd></div>
                  </dl>
                </div>

                <div className="scope-note"><ShieldIcon /><div><strong>{copy.repositoryLocalChange}</strong><span>{copy.globalSettingsUntouched}</span></div></div>

                <button className="button button--primary button--wide" disabled={!pendingProfile || !profileIsComplete(pendingProfile) || busy} onClick={reviewAssignment}>{copy.reviewAndApply}</button>
                <button className="danger-link" onClick={removeSelected}><TrashIcon />{copy.removeFromGitContext}</button>
              </aside>
            )}
          </div>
        </main>
      </div>

      {editingProfile && <ProfileEditor initial={editingProfile.profile} creating={editingProfile.creating} ghAvailable={environment.gh.available} locale={locale} onClose={() => setEditingProfile(null)} onSave={saveProfileAction} />}
      {preview && <ApplyDialog preview={preview} locale={locale} onClose={() => setPreview(null)} onApply={applyProfileAction} />}
    </div>
  );
}

export default App;
