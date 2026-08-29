import { useEffect, useMemo, useState, type CSSProperties, type FormEvent } from "react";
import {
  addRepository,
  applyAssignment,
  bootstrap,
  chooseGhConfigDirectory,
  chooseRepositoryDirectory,
  chooseSshKey,
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
import type { AppData, ApplyPreview, BootstrapResult, Profile } from "./types";
import { compactPath, initials, profileIsComplete } from "./types";
import "./App.css";

const accents = ["#d8a33f", "#56a7d9", "#d97866", "#8e78d4", "#5ca989"];

const emptyProfile = (): Profile => ({
  id: "",
  label: "",
  accent: accents[0],
  gitName: "",
  gitEmail: "",
  githubUsername: "",
  sshKeyPath: "",
  ghConfigDir: "",
});

const messageFrom = (error: unknown) => {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : "Something went wrong.";
};

const formatAppliedAt = (value?: string | null) => {
  if (!value) return "Not applied";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Applied";
  return new Intl.DateTimeFormat(undefined, {
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
  onClose,
  onSave,
}: {
  initial: Profile;
  onClose: () => void;
  onSave: (profile: Profile) => Promise<void>;
}) {
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const update = (key: keyof Profile, value: string) =>
    setDraft((current) => ({ ...current, [key]: value }));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await onSave(draft);
      onClose();
    } catch (cause) {
      setError(messageFrom(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <form className="modal profile-modal" onSubmit={submit} onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">Repository identity</p>
            <h2>{draft.id ? "Edit profile" : "Create profile"}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close">
            <CloseIcon />
          </button>
        </div>

        <div className="profile-form-grid">
          <label className="field field--wide">
            <span>Profile name</span>
            <input required value={draft.label} onChange={(event) => update("label", event.currentTarget.value)} placeholder="Personal" />
          </label>

          <fieldset className="accent-field field--wide">
            <legend>Color</legend>
            <div className="accent-options">
              {accents.map((accent) => (
                <button
                  key={accent}
                  type="button"
                  className={draft.accent === accent ? "selected" : ""}
                  style={{ background: accent }}
                  onClick={() => update("accent", accent)}
                  aria-label={`Use color ${accent}`}
                >
                  {draft.accent === accent && <CheckIcon />}
                </button>
              ))}
            </div>
          </fieldset>

          <label className="field">
            <span>Git author name</span>
            <input required value={draft.gitName} onChange={(event) => update("gitName", event.currentTarget.value)} placeholder="Your Name" />
          </label>
          <label className="field">
            <span>Git author email</span>
            <input required type="email" value={draft.gitEmail} onChange={(event) => update("gitEmail", event.currentTarget.value)} placeholder="you@example.com" />
          </label>
          <label className="field field--wide">
            <span>GitHub username <small>optional</small></span>
            <div className="input-prefix"><span>@</span><input value={draft.githubUsername ?? ""} onChange={(event) => update("githubUsername", event.currentTarget.value)} placeholder="username" /></div>
          </label>
          <label className="field field--wide">
            <span>Existing SSH private key <small>reference only</small></span>
            <div className="path-input">
              <input value={draft.sshKeyPath ?? ""} onChange={(event) => update("sshKeyPath", event.currentTarget.value)} placeholder="C:\\Users\\you\\.ssh\\id_ed25519_personal" />
              <button type="button" onClick={async () => { const path = await chooseSshKey(); if (path) update("sshKeyPath", path); }}>Browse</button>
            </div>
          </label>
          <label className="field field--wide">
            <span>Existing gh config directory <small>optional</small></span>
            <div className="path-input">
              <input value={draft.ghConfigDir ?? ""} onChange={(event) => update("ghConfigDir", event.currentTarget.value)} placeholder="C:\\Users\\you\\.config\\gh-personal" />
              <button type="button" onClick={async () => { const path = await chooseGhConfigDirectory(); if (path) update("ghConfigDir", path); }}>Browse</button>
            </div>
          </label>
        </div>

        <div className="privacy-note">
          <ShieldIcon />
          <span>Only paths and public identity settings are saved. Key contents and GitHub tokens never enter GitContext.</span>
        </div>
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose}>Cancel</button>
          <button className="button button--primary" disabled={saving} type="submit">{saving ? "Saving…" : "Save profile"}</button>
        </div>
      </form>
    </div>
  );
}

function ApplyDialog({
  preview,
  onClose,
  onApply,
}: {
  preview: ApplyPreview;
  onClose: () => void;
  onApply: () => Promise<void>;
}) {
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const apply = async () => {
    setApplying(true);
    setError(null);
    try {
      await onApply();
    } catch (cause) {
      setError(messageFrom(cause));
      setApplying(false);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <section className="modal apply-modal" role="dialog" aria-modal="true" aria-labelledby="apply-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">Review changes</p>
            <h2 id="apply-title">Apply {preview.profile.label} to {preview.repository.name}?</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close"><CloseIcon /></button>
        </div>
        <p className="modal-lead">These changes are limited to this repository. Global Git settings are left untouched.</p>

        <div className="change-list">
          {preview.changes.map((change) => (
            <div className="change-row" key={change.key}>
              <code>{change.key}</code>
              <div className="change-values">
                <span>{change.currentValue || "Not set"}</span>
                <ChevronIcon />
                <strong>{change.nextValue}</strong>
              </div>
            </div>
          ))}
        </div>
        {preview.warnings.map((warning) => <div className="warning-note" key={warning}><AlertIcon />{warning}</div>)}
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose}>Cancel</button>
          <button className="button button--primary" type="button" onClick={apply} disabled={applying}>{applying ? "Applying…" : "Apply safely"}</button>
        </div>
      </section>
    </div>
  );
}

function App() {
  const [result, setResult] = useState<BootstrapResult | null>(null);
  const [loadingError, setLoadingError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [profileFilter, setProfileFilter] = useState<string | null>(null);
  const [selectedRepositoryId, setSelectedRepositoryId] = useState<string | null>(null);
  const [pendingProfileId, setPendingProfileId] = useState("");
  const [editingProfile, setEditingProfile] = useState<Profile | null>(null);
  const [preview, setPreview] = useState<ApplyPreview | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    bootstrap()
      .then((value) => {
        setResult(value);
        setSelectedRepositoryId(value.data.repositories[0]?.id ?? null);
        setPendingProfileId(value.data.repositories[0]?.profileId ?? value.data.profiles[0]?.id ?? "");
      })
      .catch((error) => setLoadingError(messageFrom(error)));
  }, []);

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
      const path = await chooseRepositoryDirectory();
      if (!path) return;
      setBusy(true);
      const repository = await addRepository(path);
      if (data) updateData({ ...data, repositories: [...data.repositories.filter((repo) => repo.id !== repository.id), repository] });
      setProfileFilter(null);
      setSelectedRepositoryId(repository.id);
      setPendingProfileId(data?.profiles[0]?.id ?? "");
      setNotice(`${repository.name} was added. Choose a profile to finish setup.`);
    } catch (error) {
      setNotice(messageFrom(error));
    } finally {
      setBusy(false);
    }
  };

  const saveProfileAction = async (profile: Profile) => {
    const nextData = await saveProfile(profile);
    updateData(nextData);
    setNotice(`${profile.label} profile was saved.`);
  };

  const reviewAssignment = async () => {
    if (!selectedRepository || !pendingProfile) return;
    setNotice(null);
    setBusy(true);
    try {
      setPreview(await previewAssignment(selectedRepository.id, pendingProfile.id));
    } catch (error) {
      setNotice(messageFrom(error));
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
    setNotice(`${preview.profile.label} is now applied to ${preview.repository.name}.`);
  };

  const removeSelected = async () => {
    if (!selectedRepository || !window.confirm(`Remove ${selectedRepository.name} from GitContext? Git settings will not be deleted.`)) return;
    try {
      const nextData = await removeRepository(selectedRepository.id);
      updateData(nextData);
      const nextSelected = nextData.repositories[0] ?? null;
      setSelectedRepositoryId(nextSelected?.id ?? null);
      setPendingProfileId(nextSelected?.profileId ?? nextData.profiles[0]?.id ?? "");
      setNotice("Repository removed from GitContext. Its Git config was left unchanged.");
    } catch (error) {
      setNotice(messageFrom(error));
    }
  };

  if (loadingError) {
    return <main className="fatal-state"><AlertIcon /><h1>GitContext could not start</h1><p>{loadingError}</p><button className="button button--primary" onClick={() => window.location.reload()}>Try again</button></main>;
  }

  if (!result || !data || !environment) {
    return <main className="loading-state"><span className="brand-mark"><BranchIcon /></span><p>Inspecting your Git environment…</p></main>;
  }

  const assignedCount = data.repositories.filter((repo) => repo.profileId).length;
  const readyProfileCount = data.profiles.filter(profileIsComplete).length;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark"><BranchIcon /></span>
          <div><strong>GitContext</strong><small>Identity manager</small></div>
        </div>

        <nav className="side-nav" aria-label="Main navigation">
          <button className="active"><FolderIcon />Repositories<span>{data.repositories.length}</span></button>
          <button><ShieldIcon />Safety log<span className="soon">Soon</span></button>
        </nav>

        <div className="profiles-heading">
          <span>Profiles</span>
          <button className="icon-button icon-button--dark" onClick={() => setEditingProfile(emptyProfile())} aria-label="Add profile"><PlusIcon /></button>
        </div>
        <div className="profile-list">
          {data.profiles.map((profile) => {
            const complete = profileIsComplete(profile);
            const count = data.repositories.filter((repo) => repo.profileId === profile.id).length;
            return (
              <div className={`profile-card ${profileFilter === profile.id ? "selected" : ""}`} key={profile.id}>
                <button className="profile-card-main" onClick={() => setProfileFilter(profileFilter === profile.id ? null : profile.id)}>
                  <ProfileAvatar profile={profile} />
                  <span className="profile-copy"><strong>{profile.label}</strong><small>{profile.githubUsername ? `@${profile.githubUsername}` : "GitHub not linked"}</small></span>
                  <span className="profile-count">{count}</span>
                </button>
                <div className="profile-card-foot">
                  <span className={complete ? "ready" : "needs-setup"}>{complete ? <CheckIcon /> : <AlertIcon />}{complete ? "Ready" : "Needs setup"}</span>
                  <button onClick={() => setEditingProfile(profile)} aria-label={`Edit ${profile.label}`}><EditIcon /></button>
                </div>
              </div>
            );
          })}
        </div>

        <div className="sidebar-security">
          <ShieldIcon />
          <div><strong>Secrets stay outside</strong><small>Uses your existing gh and ~/.ssh</small></div>
        </div>
      </aside>

      <div className="workspace">
        <header className="topbar">
          <div><p className="breadcrumb">Workspace / <strong>Repositories</strong></p></div>
          <div className="topbar-tools">
            <ToolBadge label="Git" available={environment.git.available} />
            <ToolBadge label="gh" available={environment.gh.available} />
            <ToolBadge label="SSH" available={environment.ssh.available} />
            <span className="topbar-divider" />
            <span className="local-badge">Local only</span>
          </div>
        </header>

        <main className="content">
          {result.demoMode && <div className="demo-banner"><span>Interactive preview</span> No real Git settings are changed in browser mode.</div>}
          {notice && <div className="notice"><CheckIcon /><span>{notice}</span><button onClick={() => setNotice(null)} aria-label="Dismiss"><CloseIcon /></button></div>}

          <section className="hero-row">
            <div>
              <p className="eyebrow">Repository identities</p>
              <h1>Every repository, the right identity.</h1>
              <p>Assign a profile once. Git author, SSH key, and GitHub CLI context stay aligned.</p>
            </div>
            <button className="button button--primary button--add" onClick={addRepo} disabled={busy}><PlusIcon />Add repository</button>
          </section>

          <section className="summary-grid" aria-label="Workspace summary">
            <article><span className="summary-icon summary-icon--green"><FolderIcon /></span><div><strong>{data.repositories.length}</strong><small>Repositories</small></div><em>{assignedCount}/{data.repositories.length || 0} assigned</em></article>
            <article><span className="summary-icon summary-icon--gold"><ShieldIcon /></span><div><strong>{readyProfileCount}</strong><small>Ready profiles</small></div><em>{data.profiles.length} total</em></article>
            <article><span className={`summary-icon ${environment.gh.available ? "summary-icon--blue" : "summary-icon--muted"}`}><TerminalIcon /></span><div><strong>{environment.gh.available ? "Ready" : "Missing"}</strong><small>GitHub CLI</small></div><em>{environment.gh.available ? "Authenticated externally" : "Optional for MVP"}</em></article>
          </section>

          <div className={`repository-layout ${selectedRepository ? "has-inspector" : ""}`}>
            <section className="repository-panel">
              <div className="panel-toolbar">
                <div className="search-box"><SearchIcon /><input value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Search repositories" /></div>
                {profileFilter && <button className="filter-chip" onClick={() => setProfileFilter(null)}>{data.profiles.find((profile) => profile.id === profileFilter)?.label}<CloseIcon /></button>}
                <span className="panel-count">{filteredRepositories.length} shown</span>
              </div>

              {filteredRepositories.length ? (
                <div className="repo-table">
                  <div className="repo-table-head"><span>Repository</span><span>Profile</span><span>Status</span><span /></div>
                  {filteredRepositories.map((repository) => {
                    const profile = data.profiles.find((item) => item.id === repository.profileId);
                    const selected = repository.id === selectedRepositoryId;
                    return (
                      <button className={`repo-row ${selected ? "selected" : ""}`} key={repository.id} onClick={() => selectRepository(repository.id)}>
                        <span className="repo-main"><span className="repo-icon"><FolderIcon /></span><span><strong>{repository.name}</strong><small title={repository.path}>{compactPath(repository.path)}</small></span></span>
                        <span>{profile ? <span className="profile-pill"><ProfileAvatar profile={profile} size="small" />{profile.label}</span> : <span className="unassigned-pill"><AlertIcon />Unassigned</span>}</span>
                        <span className={repository.profileId ? "repo-status is-configured" : "repo-status"}><span className="status-dot" />{repository.profileId ? "Configured" : "Action needed"}</span>
                        <span className="repo-chevron"><ChevronIcon /></span>
                      </button>
                    );
                  })}
                </div>
              ) : (
                <div className="empty-state"><span><FolderIcon /></span><h3>No repositories found</h3><p>{data.repositories.length ? "Try a different search or clear the profile filter." : "Add a local Git repository to assign its first identity."}</p>{!data.repositories.length && <button className="button button--primary" onClick={addRepo}><PlusIcon />Add repository</button>}</div>
              )}
            </section>

            {selectedRepository && (
              <aside className="inspector">
                <div className="inspector-head">
                  <div className="repo-icon repo-icon--large"><FolderIcon /></div>
                  <div><p className="eyebrow">Selected repository</p><h2>{selectedRepository.name}</h2></div>
                  <button className="icon-button" aria-label="Repository menu"><MoreIcon /></button>
                </div>
                <p className="inspector-path" title={selectedRepository.path}>{selectedRepository.path}</p>
                <div className="repo-meta">
                  <span><BranchIcon />{selectedRepository.branch || "No branch"}</span>
                  <span>{formatAppliedAt(selectedRepository.lastAppliedAt)}</span>
                </div>

                <div className="inspector-section">
                  <label htmlFor="profile-select">Assigned profile</label>
                  <div className="profile-select-wrap">
                    {pendingProfile && <ProfileAvatar profile={pendingProfile} size="small" />}
                    <select id="profile-select" value={pendingProfileId} onChange={(event) => setPendingProfileId(event.currentTarget.value)}>
                      <option value="" disabled>Select a profile</option>
                      {data.profiles.map((profile) => <option value={profile.id} key={profile.id}>{profile.label}</option>)}
                    </select>
                  </div>
                  {pendingProfile && !profileIsComplete(pendingProfile) && <div className="field-warning"><AlertIcon />Complete this profile before applying it.</div>}
                </div>

                <div className="identity-card">
                  <div className="identity-card-title"><span>Effective identity</span>{assignedProfile ? <span className="ready"><CheckIcon />Applied</span> : <span className="needs-setup"><AlertIcon />Not set</span>}</div>
                  <dl>
                    <div><dt>Commit as</dt><dd>{assignedProfile?.gitName || "—"}</dd></div>
                    <div><dt>Email</dt><dd>{assignedProfile?.gitEmail || "—"}</dd></div>
                    <div><dt>SSH key</dt><dd title={assignedProfile?.sshKeyPath ?? undefined}><KeyIcon />{assignedProfile?.sshKeyPath ? compactPath(assignedProfile.sshKeyPath, 28) : "Not managed"}</dd></div>
                    <div><dt>GitHub CLI</dt><dd><TerminalIcon />{assignedProfile?.ghConfigDir ? "Profile directory" : "Not managed"}</dd></div>
                  </dl>
                </div>

                <div className="scope-note"><ShieldIcon /><div><strong>Repository-local change</strong><span>Global .gitconfig and credentials will not be modified.</span></div></div>

                <button className="button button--primary button--wide" disabled={!pendingProfile || !profileIsComplete(pendingProfile) || busy} onClick={reviewAssignment}>Review & apply profile</button>
                <button className="danger-link" onClick={removeSelected}><TrashIcon />Remove from GitContext</button>
              </aside>
            )}
          </div>
        </main>
      </div>

      {editingProfile && <ProfileEditor initial={editingProfile} onClose={() => setEditingProfile(null)} onSave={saveProfileAction} />}
      {preview && <ApplyDialog preview={preview} onClose={() => setPreview(null)} onApply={applyProfileAction} />}
    </div>
  );
}

export default App;
