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
  commitRepository,
  connectGithubProfile,
  createBranch,
  createPullRequest,
  inspectGithubProfile,
  inspectRepositoryStatuses,
  listenForGithubAuthPrompt,
  listGithubRepositories,
  listPullRequests,
  mergePullRequest,
  openGithubAuthPage,
  previewAssignment,
  previewCommit,
  previewPullRequest,
  previewPush,
  previewRepositorySync,
  publishRepository,
  pullRepository,
  pushRepository,
  removeRepository,
  saveProfile,
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
import type { AppData, ApplyPreview, AutoApprove, BootstrapResult, CloneOptions, CommitPreview, GhProfileStatus, GithubAuthPrompt, GithubRepository, ManagedPullRequest, MergePullRequestResult, MergeStrategy, Profile, PublishOptions, PullRequestManagement, PullRequestPreview, PullRequestResult, PushPreview, RepositoryRecord, RepositoryVisibility, SyncPreview } from "./types";
import type { RepositoryStatus } from "./types";
import { initials, profileIsComplete } from "./types";
import { localizeRuntimeMessage, shellCopy, uiCopy, type Locale } from "./i18n";
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

function PublishDialog({
  repository,
  profile,
  locale,
  onClose,
  onPublish,
}: {
  repository: RepositoryRecord;
  profile: Profile;
  locale: Locale;
  onClose: () => void;
  onPublish: (options: PublishOptions) => Promise<void>;
}) {
  const copy = uiCopy[locale];
  const [name, setName] = useState(repository.name);
  const [description, setDescription] = useState("");
  const [visibility, setVisibility] = useState<RepositoryVisibility>("private");
  const [publishing, setPublishing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const owner = profile.githubUsername ?? "";

  const publish = async (event: FormEvent) => {
    event.preventDefault();
    setPublishing(true);
    setError(null);
    try {
      await onPublish({ repositoryId: repository.id, profileId: profile.id, name, visibility, description });
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setPublishing(false);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <form className="modal publish-modal" onSubmit={publish} onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">GitHub</p>
            <h2>{copy.publishRepository}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.publishLead}</p>

        <div className="publish-destination">
          <ProfileAvatar profile={profile} />
          <div><span>{copy.publishDestination}</span><strong>@{owner} / {name || "…"}</strong></div>
        </div>

        <div className="profile-form-grid publish-form-grid">
          <label className="field">
            <span>{copy.githubRepositoryName}</span>
            <input required maxLength={100} pattern="[A-Za-z0-9._-]+" value={name} onChange={(event) => setName(event.currentTarget.value)} />
          </label>
          <label className="field">
            <span>{copy.visibility}</span>
            <select value={visibility} onChange={(event) => setVisibility(event.currentTarget.value as RepositoryVisibility)}>
              <option value="private">{copy.privateRepository}</option>
              <option value="public">{copy.publicRepository}</option>
            </select>
          </label>
          <label className="field field--wide">
            <span>{copy.description} <small>{copy.optional}</small></span>
            <input maxLength={350} value={description} onChange={(event) => setDescription(event.currentTarget.value)} placeholder={copy.descriptionPlaceholder} />
          </label>
        </div>

        <div className={`publish-scope ${visibility === "public" ? "is-public" : ""}`}>
          <ShieldIcon />
          <div><strong>{visibility === "private" ? copy.privatePublishTitle : copy.publicPublishTitle}</strong><span>{visibility === "private" ? copy.privatePublishLead : copy.publicPublishLead}</span></div>
        </div>

        <ol className="publish-steps">
          <li>{copy.createGithubRepository}</li>
          <li>{copy.addOriginRemote}</li>
          <li>{copy.pushCurrentBranch(repository.branch || copy.noBranch)}</li>
        </ol>

        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose} disabled={publishing}>{copy.cancel}</button>
          <button className="button button--primary" type="submit" disabled={publishing || !owner}>{publishing ? copy.publishing : copy.createAndPush}</button>
        </div>
      </form>
    </div>
  );
}

function PushDialog({
  preview,
  locale,
  onClose,
  onPush,
}: {
  preview: PushPreview;
  locale: Locale;
  onClose: () => void;
  onPush: () => Promise<void>;
}) {
  const copy = uiCopy[locale];
  const [pushing, setPushing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const push = async () => {
    setPushing(true);
    setError(null);
    try {
      await onPush();
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setPushing(false);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <section className="modal push-modal" role="dialog" aria-modal="true" aria-labelledby="push-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">GitHub</p>
            <h2 id="push-title">{copy.pushDialogTitle}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.pushDialogLead}</p>

        <div className="publish-destination">
          <ProfileAvatar profile={preview.profile} />
          <div><span>{copy.pushProfile}</span><strong>{preview.profile.label}{preview.profile.githubUsername ? ` · @${preview.profile.githubUsername}` : ""}</strong></div>
        </div>

        <dl className="push-details">
          <div><dt>{copy.pushRemote}</dt><dd title={preview.remoteUrl}>{preview.remoteUrl}</dd></div>
          <div><dt>{copy.pushBranch}</dt><dd>{preview.branch}</dd></div>
          <div><dt>{copy.pushUpstream}</dt><dd>{preview.upstream || copy.noUpstream}</dd></div>
        </dl>

        <div className="push-safety-note">
          <ShieldIcon />
          <div><strong>{copy.pushCommittedOnlyTitle}</strong><span>{copy.pushCommittedOnlyLead}</span><small>{copy.normalPushOnly}</small></div>
        </div>
        {preview.hasUncommittedChanges && <div className="warning-note"><AlertIcon />{copy.uncommittedChangesPresent}</div>}
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions">
          <button className="button button--ghost" type="button" onClick={onClose} disabled={pushing}>{copy.cancel}</button>
          <button className="button button--primary" type="button" onClick={push} disabled={pushing}>{pushing ? copy.pushingBranch : copy.confirmPush}</button>
        </div>
      </section>
    </div>
  );
}

function SyncDialog({
  preview,
  locale,
  onClose,
  onRefresh,
  onPull,
  onPush,
}: {
  preview: SyncPreview;
  locale: Locale;
  onClose: () => void;
  onRefresh: () => Promise<SyncPreview>;
  onPull: () => Promise<SyncPreview>;
  onPush: () => Promise<SyncPreview>;
}) {
  const copy = uiCopy[locale];
  const [action, setAction] = useState<"fetch" | "pull" | "push" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const running = action !== null;
  const diverged = preview.ahead > 0 && preview.behind > 0;
  const dirty = preview.changes.length > 0;
  const canPull = preview.behind > 0 && preview.ahead === 0 && !dirty && Boolean(preview.remoteBranch);
  const canPush = !diverged && (preview.ahead > 0 || !preview.remoteBranch);
  const upToDate = Boolean(preview.remoteBranch) && preview.ahead === 0 && preview.behind === 0;

  const run = async (nextAction: "fetch" | "pull" | "push") => {
    setAction(nextAction);
    setError(null);
    try {
      if (nextAction === "fetch") await onRefresh();
      if (nextAction === "pull") await onPull();
      if (nextAction === "push") await onPush();
      setAction(null);
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setAction(null);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <section className="modal sync-modal" role="dialog" aria-modal="true" aria-labelledby="sync-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div><p className="eyebrow">Git</p><h2 id="sync-title">{copy.syncDialogTitle}</h2></div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.syncDialogLead}</p>

        <div className="publish-destination">
          <ProfileAvatar profile={preview.profile} />
          <div><span>{copy.syncProfile}</span><strong>{preview.profile.label}{preview.profile.githubUsername ? ` · @${preview.profile.githubUsername}` : ""}</strong></div>
        </div>

        <dl className="push-details">
          <div><dt>{copy.repository}</dt><dd>{preview.repository.name}</dd></div>
          <div><dt>{copy.syncBranch}</dt><dd>{preview.branch}</dd></div>
          <div><dt>{copy.syncTracking}</dt><dd>{preview.upstream || preview.remoteBranch || copy.noRemoteBranch}</dd></div>
          <div><dt>{copy.lastFetched}</dt><dd>{new Date(preview.fetchedAt).toLocaleString(locale === "ja" ? "ja-JP" : "en-US")}</dd></div>
        </dl>

        <div className="sync-counts">
          <div className={preview.ahead > 0 ? "has-count" : ""}><strong>{preview.ahead}</strong><span>{copy.aheadCommits}</span></div>
          <div className={preview.behind > 0 ? "has-count" : ""}><strong>{preview.behind}</strong><span>{copy.behindCommits}</span></div>
        </div>

        {upToDate && <div className="sync-ready"><CheckIcon /><div><strong>{copy.repositoryUpToDate}</strong><span>{copy.repositoryUpToDateLead}</span></div></div>}
        {!preview.remoteBranch && <div className="warning-note"><AlertIcon />{copy.remoteBranchMissing}</div>}
        {diverged && <div className="warning-note"><AlertIcon /><div><strong>{copy.branchesDiverged}</strong><br />{copy.branchesDivergedLead}</div></div>}
        {dirty && <div className="warning-note"><AlertIcon />{copy.pullBlockedByChanges(preview.changes.length)}</div>}
        {error && <div className="inline-error"><AlertIcon />{error}</div>}

        <div className="push-safety-note">
          <ShieldIcon />
          <div><strong>{copy.fastForwardOnly}</strong><span>{copy.fastForwardOnlyLead}</span></div>
        </div>

        <div className="modal-actions sync-actions">
          <button className="button button--ghost" type="button" onClick={onClose} disabled={running}>{copy.close}</button>
          <button className="button button--ghost" type="button" onClick={() => run("fetch")} disabled={running}>{action === "fetch" ? copy.fetching : copy.fetchAgain}</button>
          <button className="button button--ghost" type="button" onClick={() => run("pull")} disabled={!canPull || running}>{action === "pull" ? copy.pulling : copy.pullChanges}</button>
          <button className="button button--primary" type="button" onClick={() => run("push")} disabled={!canPush || running}>{action === "push" ? copy.pushingBranch : copy.pushChanges}</button>
        </div>
      </section>
    </div>
  );
}

function CommitDialog({
  preview,
  locale,
  onClose,
  onCommit,
}: {
  preview: CommitPreview;
  locale: Locale;
  onClose: () => void;
  onCommit: (message: string, pushAfterCommit: boolean) => Promise<void>;
}) {
  const copy = uiCopy[locale];
  const [message, setMessage] = useState("");
  const [action, setAction] = useState<"commit" | "push" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const hasChanges = preview.changes.length > 0;
  const messageIsValid = message.trim().length > 0 && message.trim().length <= 200 && !/[\r\n]/.test(message);

  const commit = async (pushAfterCommit: boolean) => {
    setAction(pushAfterCommit ? "push" : "commit");
    setError(null);
    try {
      await onCommit(message, pushAfterCommit);
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setAction(null);
    }
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={onClose}>
      <section className="modal commit-modal" role="dialog" aria-modal="true" aria-labelledby="commit-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">Git</p>
            <h2 id="commit-title">{copy.commitDialogTitle}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.commitDialogLead}</p>

        <div className="publish-destination">
          <ProfileAvatar profile={preview.profile} />
          <div><span>{copy.commitProfile}</span><strong>{preview.profile.label} · {preview.profile.gitName} &lt;{preview.profile.gitEmail}&gt;</strong></div>
        </div>

        <dl className="push-details">
          <div><dt>{copy.commitBranch}</dt><dd>{preview.branch}</dd></div>
          <div><dt>{copy.repository}</dt><dd>{preview.repository.name}</dd></div>
        </dl>

        {hasChanges ? (
          <>
            <p className="commit-change-heading">{copy.changesToCommit(preview.changes.length)}</p>
            <div className="commit-change-list" aria-label={copy.changesToCommit(preview.changes.length)}>
              {preview.changes.map((change, index) => (
                <div className="commit-change-row" key={`${change.status}-${change.path}-${index}`}>
                  <code>{change.status || "M"}</code><span title={change.path}>{change.path}</span>
                </div>
              ))}
            </div>
          </>
        ) : <div className="warning-note"><AlertIcon />{copy.noChangesToCommit}</div>}

        <div className="push-safety-note">
          <ShieldIcon />
          <div><strong>{copy.allChangesIncludedTitle}</strong><span>{copy.allChangesIncludedLead}</span></div>
        </div>

        <label className="field commit-message-field">
          <span>{copy.commitMessage}</span>
          <input value={message} maxLength={200} placeholder={copy.commitMessagePlaceholder} onChange={(event) => setMessage(event.target.value)} disabled={Boolean(action)} autoFocus />
          <small>{message.trim().length}/200</small>
        </label>

        {!preview.pushRemoteUrl && preview.pushUnavailableReason && (
          <div className="warning-note"><AlertIcon /><div><strong>{copy.pushAfterCommitUnavailable}</strong><br />{localizeRuntimeMessage(preview.pushUnavailableReason, locale)}</div></div>
        )}
        {error && <div className="inline-error"><AlertIcon />{error}</div>}
        <div className="modal-actions commit-actions">
          <button className="button button--ghost" type="button" onClick={onClose} disabled={Boolean(action)}>{copy.cancel}</button>
          <button className="button button--ghost" type="button" onClick={() => commit(false)} disabled={!hasChanges || !messageIsValid || Boolean(action)}>{action === "commit" ? copy.committing : copy.commitOnly}</button>
          <button className="button button--primary" type="button" onClick={() => commit(true)} disabled={!hasChanges || !messageIsValid || !preview.pushRemoteUrl || Boolean(action)}>{action === "push" ? copy.committingAndPushing : copy.commitAndPush}</button>
        </div>
      </section>
    </div>
  );
}

type PullRequestStage = "branch" | "commit" | "push" | "pr";

function PullRequestDialog({
  preview,
  locale,
  onClose,
  onRefresh,
  onCreate,
}: {
  preview: PullRequestPreview;
  locale: Locale;
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onCreate: (
    input: { branchName: string; commitMessage: string; title: string; body: string; draft: boolean },
    onProgress: (stage: PullRequestStage) => void,
  ) => Promise<PullRequestResult>;
}) {
  const copy = uiCopy[locale];
  const [branchName, setBranchName] = useState("");
  const [commitMessage, setCommitMessage] = useState("");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [draft, setDraft] = useState(false);
  const [stage, setStage] = useState<PullRequestStage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PullRequestResult | null>(null);
  const [copied, setCopied] = useState(false);
  const hasChanges = preview.changes.length > 0;
  const hasProposal = hasChanges || preview.commitsAhead > 0;
  const branchIsValid = !preview.requiresNewBranch || (branchName.trim().length > 0 && branchName.trim().length <= 200);
  const commitMessageIsValid = !hasChanges || (commitMessage.trim().length > 0 && commitMessage.trim().length <= 200 && !/[\r\n]/.test(commitMessage));
  const titleIsValid = title.trim().length > 0 && title.trim().length <= 256 && !/[\r\n]/.test(title);
  const workingBranch = preview.requiresNewBranch ? branchName.trim() || "—" : preview.currentBranch;
  const running = stage !== null;
  const stages: Array<{ id: PullRequestStage; label: string; shown: boolean }> = [
    { id: "branch", label: copy.prStepBranch, shown: preview.requiresNewBranch },
    { id: "commit", label: copy.prStepCommit, shown: hasChanges },
    { id: "push", label: copy.prStepPush, shown: preview.requiresNewBranch || hasChanges || !preview.branchPushed },
    { id: "pr", label: copy.prStepCreate, shown: true },
  ];
  const visibleStages = stages.filter((item) => item.shown);

  const submit = async () => {
    setError(null);
    setResult(null);
    try {
      const created = await onCreate({ branchName, commitMessage, title, body, draft }, setStage);
      setResult(created);
      setStage(null);
    } catch (cause) {
      setError(messageFrom(cause, locale));
      setStage(null);
      await onRefresh().catch(() => undefined);
    }
  };

  const copyUrl = async () => {
    if (!result) return;
    await navigator.clipboard.writeText(result.url);
    setCopied(true);
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={running ? undefined : onClose}>
      <section className="modal pull-request-modal" role="dialog" aria-modal="true" aria-labelledby="pull-request-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <p className="eyebrow">GitHub</p>
            <h2 id="pull-request-title">{copy.pullRequestDialogTitle}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} disabled={running} aria-label={copy.close}><CloseIcon /></button>
        </div>

        {result ? (
          <div className="pull-request-result">
            <span className="result-icon"><CheckIcon /></span>
            <p className="eyebrow">{copy.pullRequestReady}</p>
            <h3>{result.existing ? copy.pullRequestAlreadyExists(result.number) : copy.pullRequestCreated(result.number)}</h3>
            <p>{result.branch} → {result.baseBranch}</p>
            <code>{result.url}</code>
            <div className="modal-actions">
              <button className="button button--ghost" type="button" onClick={copyUrl}>{copied ? copy.copied : copy.copyPullRequestUrl}</button>
              <button className="button button--primary" type="button" onClick={onClose}>{copy.close}</button>
            </div>
          </div>
        ) : (
          <>
            <p className="modal-lead">{copy.pullRequestDialogLead}</p>
            <div className="publish-destination">
              <ProfileAvatar profile={preview.profile} />
              <div><span>{copy.pullRequestRepository}</span><strong>{preview.repositoryNameWithOwner} · {preview.profile.label}</strong></div>
            </div>

            <dl className="push-details pull-request-route">
              <div><dt>{copy.pullRequestBase}</dt><dd>{preview.baseBranch}</dd></div>
              <div><dt>{copy.pullRequestHead}</dt><dd>{workingBranch}</dd></div>
            </dl>

            {preview.requiresNewBranch ? (
              <>
                <div className="privacy-note"><BranchIcon /><span>{copy.branchWillBeCreated(preview.baseBranch)}</span></div>
                <label className="field pull-request-field">
                  <span>{copy.newBranchName}</span>
                  <input value={branchName} maxLength={200} placeholder={copy.branchNamePlaceholder} onChange={(event) => setBranchName(event.target.value)} disabled={running} autoFocus />
                </label>
              </>
            ) : <div className="privacy-note"><BranchIcon /><span>{copy.existingWorkingBranch} {copy.commitsAhead(preview.commitsAhead)}</span></div>}

            {hasChanges && (
              <>
                <p className="commit-change-heading">{copy.changesToCommit(preview.changes.length)}</p>
                <div className="commit-change-list" aria-label={copy.changesToCommit(preview.changes.length)}>
                  {preview.changes.map((change, index) => (
                    <div className="commit-change-row" key={`${change.status}-${change.path}-${index}`}>
                      <code>{change.status || "M"}</code><span title={change.path}>{change.path}</span>
                    </div>
                  ))}
                </div>
                <label className="field pull-request-field">
                  <span>{copy.commitMessage}</span>
                  <input value={commitMessage} maxLength={200} placeholder={copy.commitMessagePlaceholder} onChange={(event) => setCommitMessage(event.target.value)} disabled={running} />
                </label>
              </>
            )}

            {!hasProposal && <div className="warning-note"><AlertIcon />{copy.noPullRequestChanges}</div>}
            {preview.existingPullRequest && <div className="warning-note"><AlertIcon />{copy.existingPullRequestFound(preview.existingPullRequest.number)}</div>}

            <div className="pull-request-form-grid">
              <label className="field">
                <span>{copy.pullRequestTitle}</span>
                <input value={title} maxLength={256} placeholder={copy.pullRequestTitlePlaceholder} onChange={(event) => setTitle(event.target.value)} disabled={running} />
              </label>
              <label className="field">
                <span>{copy.pullRequestBody}</span>
                <textarea value={body} maxLength={65536} rows={4} placeholder={copy.pullRequestBodyPlaceholder} onChange={(event) => setBody(event.target.value)} disabled={running} />
              </label>
              <label className="draft-option"><input type="checkbox" checked={draft} onChange={(event) => setDraft(event.target.checked)} disabled={running} />{copy.createAsDraft}</label>
            </div>

            <ol className="pull-request-steps">
              {visibleStages.map((item) => <li className={stage === item.id ? "active" : ""} key={item.id}>{item.label}</li>)}
            </ol>
            {error && <div className="inline-error"><AlertIcon />{error}</div>}
            <div className="modal-actions">
              <button className="button button--ghost" type="button" onClick={onClose} disabled={running}>{copy.cancel}</button>
              <button className="button button--primary" type="button" onClick={submit} disabled={!hasProposal || !branchIsValid || !commitMessageIsValid || !titleIsValid || running}>
                {running ? copy.creatingPullRequest : preview.requiresNewBranch ? copy.createPullRequestAction : hasChanges ? copy.continuePullRequestAction : copy.openPullRequestAction}
              </button>
            </div>
          </>
        )}
      </section>
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

const pullRequestBlockReason = (pullRequest: ManagedPullRequest, copy: (typeof uiCopy)[Locale]) => {
  if (pullRequest.isDraft) return copy.draftCannotMerge;
  if (pullRequest.reviewDecision === "CHANGES_REQUESTED") return copy.changesRequestedCannotMerge;
  if (pullRequest.checks.some((check) => check.bucket === "fail" || check.bucket === "cancel")) return copy.ciFailedCannotMerge;
  if (pullRequest.checks.some((check) => check.bucket === "pending")) return copy.ciPendingCannotMerge;
  if (pullRequest.mergeable === "CONFLICTING") return copy.conflictCannotMerge;
  if (pullRequest.mergeable !== "MERGEABLE" || pullRequest.mergeStateStatus !== "CLEAN") return copy.policyCannotMerge;
  return null;
};

function PullRequestManagementDialog({
  management,
  locale,
  onClose,
  onRefresh,
  onMerge,
}: {
  management: PullRequestManagement;
  locale: Locale;
  onClose: () => void;
  onRefresh: () => Promise<PullRequestManagement>;
  onMerge: (pullRequest: ManagedPullRequest, strategy: MergeStrategy) => Promise<MergePullRequestResult>;
}) {
  const copy = uiCopy[locale];
  const [selectedNumber, setSelectedNumber] = useState<number | null>(management.pullRequests[0]?.number ?? null);
  const [strategy, setStrategy] = useState<MergeStrategy>("squash");
  const [confirmed, setConfirmed] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [merging, setMerging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<MergePullRequestResult | null>(null);

  const selected = management.pullRequests.find((item) => item.number === selectedNumber) ?? management.pullRequests[0] ?? null;
  const blockedBy = selected ? pullRequestBlockReason(selected, copy) : null;

  useEffect(() => {
    if (selectedNumber !== null && !management.pullRequests.some((item) => item.number === selectedNumber)) {
      setSelectedNumber(management.pullRequests[0]?.number ?? null);
      setConfirmed(false);
    }
  }, [management.pullRequests, selectedNumber]);

  const refresh = async () => {
    setRefreshing(true);
    setError(null);
    try {
      await onRefresh();
    } catch (nextError) {
      setError(messageFrom(nextError, locale));
    } finally {
      setRefreshing(false);
    }
  };

  const merge = async () => {
    if (!selected || blockedBy || !confirmed) return;
    setMerging(true);
    setError(null);
    try {
      setResult(await onMerge(selected, strategy));
    } catch (nextError) {
      setError(messageFrom(nextError, locale));
      setConfirmed(false);
    } finally {
      setMerging(false);
    }
  };

  const checkLabel = (bucket: ManagedPullRequest["checks"][number]["bucket"]) => ({
    pass: copy.checkPassed,
    pending: copy.checkPending,
    fail: copy.checkFailed,
    skipping: copy.checkSkipped,
    cancel: copy.checkCancelled,
  })[bucket];

  return (
    <div className="modal-layer" onMouseDown={onClose}>
      <section className="modal pr-management-modal" role="dialog" aria-modal="true" aria-labelledby="pr-management-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div><p className="eyebrow">GitHub</p><h2 id="pr-management-title">{copy.prManagementTitle}</h2></div>
          <button className="icon-button" onClick={onClose} aria-label={copy.close}><CloseIcon /></button>
        </div>
        <p className="modal-lead">{copy.prManagementLead}</p>

        {result ? (
          <div className="pull-request-result">
            <span className="result-icon"><CheckIcon /></span>
            <h3>{copy.mergedPullRequest(result.number)}</h3>
            <p>{result.title}</p>
            <code>{result.url}</code>
            <div className="modal-actions">
              <button className="button" type="button" onClick={onClose}>{copy.close}</button>
              <a className="button button--primary" href={result.url} target="_blank" rel="noreferrer">{copy.viewOnGithub}</a>
            </div>
          </div>
        ) : (
          <>
            <div className="pr-management-toolbar">
              <div><strong>{management.repositoryNameWithOwner}</strong><span>{copy.openPullRequests(management.pullRequests.length)}</span></div>
              <button className="button button--ghost" type="button" disabled={refreshing || merging} onClick={refresh}>{refreshing ? copy.refreshingPullRequests : copy.refreshPullRequests}</button>
            </div>

            {!management.pullRequests.length ? (
              <div className="pr-empty"><CheckIcon /><strong>{copy.noOpenPullRequests}</strong></div>
            ) : (
              <div className="pr-management-layout">
                <div className="pr-list">
                  {management.pullRequests.map((pullRequest) => {
                    const failed = pullRequest.checks.filter((check) => check.bucket === "fail" || check.bucket === "cancel").length;
                    const pending = pullRequest.checks.filter((check) => check.bucket === "pending").length;
                    const passed = pullRequest.checks.filter((check) => check.bucket === "pass" || check.bucket === "skipping").length;
                    return (
                      <button
                        type="button"
                        className={`pr-list-item ${selected?.number === pullRequest.number ? "selected" : ""}`}
                        aria-pressed={selected?.number === pullRequest.number}
                        key={pullRequest.number}
                        onClick={() => { setSelectedNumber(pullRequest.number); setConfirmed(false); setError(null); }}
                      >
                        <span className="pr-list-number">#{pullRequest.number}{pullRequest.isDraft && <em>{copy.draftBadge}</em>}</span>
                        <strong>{pullRequest.title}</strong>
                        <small>{pullRequest.headBranch} → {pullRequest.baseBranch}</small>
                        <span className={`pr-ci-summary ${failed ? "failed" : pending ? "pending" : "passed"}`}>
                          {failed ? copy.ciFailed(failed) : pending ? copy.ciPending(pending) : pullRequest.checks.length ? copy.ciPassed(passed) : copy.ciNotConfigured}
                        </span>
                      </button>
                    );
                  })}
                </div>

                {selected ? (
                  <div className="pr-detail">
                    <div className="pr-detail-heading">
                      <div><span>{copy.pullRequestNumber(selected.number)}</span><h3>{selected.title}</h3></div>
                      <a href={selected.url} target="_blank" rel="noreferrer">{copy.viewOnGithub}</a>
                    </div>
                    <p className="pr-route"><code>{selected.headBranch}</code><span>→</span><code>{selected.baseBranch}</code></p>
                    <div className="pr-detail-meta">
                      <span>{selected.author ? copy.pullRequestAuthor(selected.author) : "GitHub"}</span>
                      <span>{selected.reviewDecision === "APPROVED" ? copy.reviewApproved : selected.reviewDecision === "CHANGES_REQUESTED" ? copy.reviewChangesRequested : copy.reviewNotRequired}</span>
                    </div>

                    <h4>{copy.checksHeading}</h4>
                    {selected.checks.length ? (
                      <div className="pr-checks">
                        {selected.checks.map((check, index) => (
                          <div className={`pr-check is-${check.bucket}`} key={`${check.name}-${index}`}>
                            <span>{check.bucket === "pass" || check.bucket === "skipping" ? <CheckIcon /> : <AlertIcon />}</span>
                            <div><strong>{check.name}</strong><small>{check.workflow || check.state}</small></div>
                            {check.link ? <a href={check.link} target="_blank" rel="noreferrer">{checkLabel(check.bucket)}</a> : <em>{checkLabel(check.bucket)}</em>}
                          </div>
                        ))}
                      </div>
                    ) : <div className="pr-no-checks">{copy.ciNotConfigured}</div>}

                    <div className={`merge-assessment ${blockedBy ? "blocked" : "ready"}`}>
                      {blockedBy ? <AlertIcon /> : <CheckIcon />}
                      <div><strong>{blockedBy || copy.readyToMerge}</strong>{!blockedBy && <span>{copy.readyToMergeLead}</span>}</div>
                    </div>

                    <div className="pr-merge-controls">
                      <label className="field"><span>{copy.mergeStrategy}</span><select value={strategy} onChange={(event) => { setStrategy(event.currentTarget.value as MergeStrategy); setConfirmed(false); }}><option value="squash">{copy.strategySquash}</option><option value="merge">{copy.strategyMerge}</option><option value="rebase">{copy.strategyRebase}</option></select></label>
                      <label className="draft-option merge-confirm"><input type="checkbox" checked={confirmed} disabled={Boolean(blockedBy) || merging} onChange={(event) => setConfirmed(event.currentTarget.checked)} />{copy.mergeConfirmation(selected.number)}</label>
                      <button className="button button--danger button--wide" type="button" disabled={Boolean(blockedBy) || !confirmed || merging} onClick={merge}>{merging ? copy.mergingPullRequest : copy.mergePullRequest}</button>
                    </div>
                  </div>
                ) : <div className="pr-empty">{copy.selectPullRequest}</div>}
              </div>
            )}
            {error && <div className="inline-error"><AlertIcon />{error}</div>}
          </>
        )}
      </section>
    </div>
  );
}

function App({ locale = "en" }: { locale?: Locale }) {
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
  const [publishTarget, setPublishTarget] = useState<{ repository: RepositoryRecord; profile: Profile } | null>(null);
  const [pushPreview, setPushPreview] = useState<PushPreview | null>(null);
  const [syncPreview, setSyncPreview] = useState<SyncPreview | null>(null);
  const [commitPreview, setCommitPreview] = useState<CommitPreview | null>(null);
  const [pullRequestPreview, setPullRequestPreview] = useState<PullRequestPreview | null>(null);
  const [pullRequestManagement, setPullRequestManagement] = useState<PullRequestManagement | null>(null);
  const [busy, setBusy] = useState(false);
  const [removeConfirmingId, setRemoveConfirmingId] = useState<string | null>(null);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = copy.documentTitle;
  }, [copy.documentTitle, locale]);

  useEffect(() => {
    bootstrap()
      .then((value) => {
        setResult(value);
        setSelectedRepositoryId(null);
        setPendingProfileId(value.data.repositories[0]?.profileId ?? value.data.profiles[0]?.id ?? "");
      })
      .catch((error) => setLoadingError(messageFrom(error, locale)));
  }, [locale]);

  const data = result?.data;
  const environment = result?.environment;
  const selectedRepository = data?.repositories.find((repo) => repo.id === selectedRepositoryId) ?? null;
  const assignedProfile = data?.profiles.find((profile) => profile.id === selectedRepository?.profileId) ?? null;
  const pendingProfile = data?.profiles.find((profile) => profile.id === pendingProfileId) ?? null;

  const refreshStatuses = () => {
    inspectRepositoryStatuses()
      .then((items) => setStatuses(Object.fromEntries(items.map((item) => [item.repositoryId, item]))))
      .catch((error) => setErrorNotice(`${messageFrom(error, locale)} ${shellCopy[locale].refresh}`));
  };

  useEffect(() => {
    if (data) refreshStatuses();
  }, [data, locale]);

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

  const publishRepositoryAction = async (options: PublishOptions) => {
    const published = await publishRepository(options);
    updateData(published.data);
    setPublishTarget(null);
    setNotice(copy.repositoryPublished(published.repositoryUrl));
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

  const reviewPush = async () => {
    if (!selectedRepository || !assignedProfile) return;
    setNotice(null);
    setErrorNotice(null);
    setBusy(true);
    try {
      setPushPreview(await previewPush(selectedRepository.id, assignedProfile.id));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const pushRepositoryAction = async () => {
    if (!pushPreview) return;
    const result = await pushRepository(pushPreview.repository.id, pushPreview.profile.id);
    setPushPreview(null);
    setNotice(copy.pushCompleted(result.branch));
  };

  const storeSyncPreview = (nextPreview: SyncPreview) => {
    setSyncPreview(nextPreview);
    setResult((current) => current ? {
      ...current,
      data: {
        ...current.data,
        repositories: current.data.repositories.map((repository) => repository.id === nextPreview.repository.id
          ? { ...repository, branch: nextPreview.branch, remoteUrl: nextPreview.remoteUrl }
          : repository),
      },
    } : current);
    return nextPreview;
  };

  const reviewSync = async () => {
    if (!selectedRepository || !assignedProfile) return;
    setNotice(null);
    setErrorNotice(null);
    setBusy(true);
    try {
      storeSyncPreview(await previewRepositorySync(selectedRepository.id, assignedProfile.id));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const refreshSync = async () => {
    if (!syncPreview) throw new Error(copy.somethingWentWrong);
    return storeSyncPreview(await previewRepositorySync(syncPreview.repository.id, syncPreview.profile.id));
  };

  const pullSync = async () => {
    if (!syncPreview) throw new Error(copy.somethingWentWrong);
    const refreshed = storeSyncPreview(await pullRepository(syncPreview.repository.id, syncPreview.profile.id));
    setNotice(copy.pullCompleted(refreshed.branch));
    return refreshed;
  };

  const pushSync = async () => {
    if (!syncPreview) throw new Error(copy.somethingWentWrong);
    const result = await pushRepository(syncPreview.repository.id, syncPreview.profile.id);
    const refreshed = storeSyncPreview(await previewRepositorySync(syncPreview.repository.id, syncPreview.profile.id));
    setNotice(copy.pushCompleted(result.branch));
    return refreshed;
  };

  const reviewCommit = async () => {
    if (!selectedRepository || !assignedProfile) return;
    setNotice(null);
    setErrorNotice(null);
    setBusy(true);
    try {
      setCommitPreview(await previewCommit(selectedRepository.id, assignedProfile.id));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const commitRepositoryAction = async (message: string, pushAfterCommit: boolean) => {
    if (!commitPreview) return;
    const result = await commitRepository(commitPreview.repository.id, commitPreview.profile.id, message);
    if (pushAfterCommit) {
      try {
        await pushRepository(commitPreview.repository.id, commitPreview.profile.id);
      } catch (error) {
        throw new Error(copy.commitSucceededPushFailed(result.commitId, messageFrom(error, locale)));
      }
      setCommitPreview(null);
      setNotice(copy.commitAndPushCompleted(result.commitId, result.branch));
      return;
    }
    setCommitPreview(null);
    setNotice(copy.commitCompleted(result.commitId, result.branch));
  };

  const reviewPullRequest = async () => {
    if (!selectedRepository || !assignedProfile) return;
    setNotice(null);
    setErrorNotice(null);
    setBusy(true);
    try {
      setPullRequestPreview(await previewPullRequest(selectedRepository.id, assignedProfile.id));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const refreshPullRequest = async () => {
    if (!pullRequestPreview) return;
    setPullRequestPreview(await previewPullRequest(pullRequestPreview.repository.id, pullRequestPreview.profile.id));
  };

  const createPullRequestAction = async (
    input: { branchName: string; commitMessage: string; title: string; body: string; draft: boolean },
    onProgress: (stage: PullRequestStage) => void,
  ) => {
    if (!pullRequestPreview) throw new Error(copy.somethingWentWrong);
    const repositoryId = pullRequestPreview.repository.id;
    const profileId = pullRequestPreview.profile.id;
    if (pullRequestPreview.requiresNewBranch) {
      onProgress("branch");
      const created = await createBranch(repositoryId, profileId, input.branchName);
      updateData(created.data);
    }
    if (pullRequestPreview.changes.length > 0) {
      onProgress("commit");
      await commitRepository(repositoryId, profileId, input.commitMessage);
    }
    if (pullRequestPreview.requiresNewBranch || pullRequestPreview.changes.length > 0 || !pullRequestPreview.branchPushed) {
      onProgress("push");
      await pushRepository(repositoryId, profileId);
    }
    onProgress("pr");
    const created = await createPullRequest({
      repositoryId,
      profileId,
      baseBranch: pullRequestPreview.baseBranch,
      title: input.title,
      body: input.body,
      draft: input.draft,
    });
    setNotice(created.existing ? copy.pullRequestAlreadyExists(created.number) : copy.pullRequestCreated(created.number));
    return created;
  };

  const reviewPullRequests = async () => {
    if (!selectedRepository || !assignedProfile) return;
    setNotice(null);
    setErrorNotice(null);
    setBusy(true);
    try {
      setPullRequestManagement(await listPullRequests(selectedRepository.id, assignedProfile.id));
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    } finally {
      setBusy(false);
    }
  };

  const refreshPullRequests = async () => {
    if (!pullRequestManagement) throw new Error(copy.somethingWentWrong);
    const refreshed = await listPullRequests(pullRequestManagement.repository.id, pullRequestManagement.profile.id);
    setPullRequestManagement(refreshed);
    return refreshed;
  };

  const mergePullRequestAction = async (pullRequest: ManagedPullRequest, strategy: MergeStrategy) => {
    if (!pullRequestManagement) throw new Error(copy.somethingWentWrong);
    const merged = await mergePullRequest({
      repositoryId: pullRequestManagement.repository.id,
      profileId: pullRequestManagement.profile.id,
      number: pullRequest.number,
      strategy,
      expectedHeadOid: pullRequest.headOid,
    });
    setNotice(copy.mergedPullRequest(merged.number));
    return merged;
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
    <Shell page={page} locale={locale} repositoryCount={data.repositories.length} notice={visibleNotice} noticeLink={!errorNotice && (!environment.gh.available || !environment.ssh.available) ? "profiles" : null} onDismiss={() => { if (errorNotice) setErrorNotice(null); else setEnvironmentDismissed(true); }} onNavigate={(nextPage) => { setPage(nextPage); setSelectedRepositoryId(null); setEditingProfile(null); setPreview(null); setNotice(null); }}>
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
      </div> : selectedRepository ? <RepositoryDetail repository={selectedRepository} profiles={data.profiles} status={selectedStatus} pendingProfileId={pendingProfileId} preview={preview} locale={locale} busy={busy} removing={removeConfirmingId === selectedRepository.id}
        onBack={() => { setSelectedRepositoryId(null); setPreview(null); setNotice(null); }} onPendingProfile={setPendingProfileId} onReview={reviewAssignment} onCancelReview={() => setPreview(null)} onApply={applyProfileAction} onAutoApprove={updateAutoApprove}
        onStartRemove={() => setRemoveConfirmingId(selectedRepository.id)} onCancelRemove={() => setRemoveConfirmingId(null)} onRemove={removeSelected}
        onCommit={reviewCommit} onPush={reviewPush} onSync={reviewSync} onPullRequest={reviewPullRequest} onManagePullRequests={reviewPullRequests}
        onPublish={() => assignedProfile && setPublishTarget({ repository: selectedRepository, profile: assignedProfile })} />
        : <RepositoryList repositories={data.repositories} profiles={data.profiles} statuses={statuses} locale={locale} busy={busy} onOpen={selectRepository} onAdd={addRepo} onClone={() => setCloneOpen(true)} onRefresh={refreshStatuses} />}

      {cloneOpen && <CloneDialog profiles={data.profiles} locale={locale} onClose={() => setCloneOpen(false)} onClone={cloneRepositoryAction} />}
      {publishTarget && <PublishDialog repository={publishTarget.repository} profile={publishTarget.profile} locale={locale} onClose={() => setPublishTarget(null)} onPublish={publishRepositoryAction} />}
      {pushPreview && <PushDialog preview={pushPreview} locale={locale} onClose={() => setPushPreview(null)} onPush={pushRepositoryAction} />}
      {syncPreview && <SyncDialog preview={syncPreview} locale={locale} onClose={() => setSyncPreview(null)} onRefresh={refreshSync} onPull={pullSync} onPush={pushSync} />}
      {commitPreview && <CommitDialog preview={commitPreview} locale={locale} onClose={() => setCommitPreview(null)} onCommit={commitRepositoryAction} />}
      {pullRequestPreview && <PullRequestDialog preview={pullRequestPreview} locale={locale} onClose={() => setPullRequestPreview(null)} onRefresh={refreshPullRequest} onCreate={createPullRequestAction} />}
      {pullRequestManagement && <PullRequestManagementDialog management={pullRequestManagement} locale={locale} onClose={() => setPullRequestManagement(null)} onRefresh={refreshPullRequests} onMerge={mergePullRequestAction} />}
    </Shell>
  );
}

export default App;
