import { useEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  addRepository,
  applyAssignment,
  bootstrap,
  chooseCloneDestinationDirectory,
  chooseRepositoryDirectory,
  cloneGithubRepository,
  dismissAiIntegrationNotice,
  inspectRepositoryStatuses,
  inspectGithubProfile,
  listGithubRepositories,
  listAiClients,
  previewAssignment,
  removeRepository,
  reportRepositoryStatuses,
  saveProfile,
  setLocale,
  setProfileAutoApprove,
  setRepositoryAutoApprove,
} from "./backend";
import {
  AlertIcon,
  BranchIcon,
  CloseIcon,
  SearchIcon,
} from "./Icons";
import type { AiInventory, AppData, ApplyPreview, AutoApprove, BootstrapResult, CloneOptions, GhProfileStatus, GithubRepository, Profile } from "./types";
import type { RepositoryStatus } from "./types";
import { initials, profileIsComplete } from "./types";
import { localizeRuntimeMessage, shellCopy, uiCopy, type Locale } from "./i18n";
import { resolveLocale } from "./locale";
import { Shell, type ShellPage } from "./ui/Shell";
import { RepositoryList } from "./ui/RepositoryList";
import { RepositoryDetail } from "./ui/RepositoryDetail";
import { shouldRefreshOnFocus } from "./ui/status";
import { ProfileEditor } from "./ui/ProfileEditor";
import { ProfileList } from "./ui/ProfileList";
import { FirstRunGuide } from "./ui/FirstRunGuide";
import { SettingsPage } from "./ui/SettingsPage";
import { HistoryPage } from "./ui/HistoryPage";
import { AiIntegrationPage } from "./ui/AiIntegrationPage";
import { shouldShowAiNotice } from "./ui/aiNotice";
import { repositoryProfileStatus } from "./ui/profileStatus";
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
  const [aiInventory, setAiInventory] = useState<AiInventory | null>(null);
  const [statuses, setStatuses] = useState<Record<string, RepositoryStatus>>({});
  const [profileStatuses, setProfileStatuses] = useState<Record<string, GhProfileStatus>>({});
  const [profileRefreshing, setProfileRefreshing] = useState(false);
  const checkedProfiles = useRef(new Set<string>());
  const statusRefresh = useRef({ at: 0, running: false });
  const [statusRefreshing, setStatusRefreshing] = useState(false);
  const [statusesRefreshedAt, setStatusesRefreshedAt] = useState<Date | null>(null);
  const [page, setPage] = useState<ShellPage>("repositories");
  const [attentionFilterRequest, setAttentionFilterRequest] = useState(0);
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
    if (!isTauri()) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void listen("open-attention-repositories", () => {
      if (!active) return;
      setSelectedRepositoryId(null);
      setEditingProfile(null);
      setPage("repositories");
      setAttentionFilterRequest((value) => value + 1);
    }).then((stop) => { if (active) unlisten = stop; else stop(); });
    return () => { active = false; unlisten?.(); };
  }, []);

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
  const aiNoticeDismissed = data?.settings.aiIntegrationNoticeDismissed ?? false;

  useEffect(() => {
    if (!data?.profiles.length || aiNoticeDismissed) return;
    let active = true;
    void listAiClients()
      .then((inventory) => { if (active) setAiInventory(inventory); })
      .catch(() => { if (active) setAiInventory(null); });
    return () => { active = false; };
  }, [Boolean(data?.profiles.length), aiNoticeDismissed]);

  const dismissAiNotice = async () => {
    try {
      const settings = await dismissAiIntegrationNotice();
      setResult((current) => current ? { ...current, data: { ...current.data, settings } } : current);
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
    }
  };

  const navigate = (nextPage: ShellPage) => {
    if (nextPage === "ai" && !aiNoticeDismissed) void dismissAiNotice();
    if (nextPage === "profiles" && page !== "profiles") checkedProfiles.current.clear();
    setPage(nextPage);
    setSelectedRepositoryId(null);
    setEditingProfile(null);
    setPreview(null);
    setNotice(null);
  };
  const selectedRepository = data?.repositories.find((repo) => repo.id === selectedRepositoryId) ?? null;
  const pendingProfile = data?.profiles.find((profile) => profile.id === pendingProfileId) ?? null;

  const refreshProfileStatuses = (force = false) => {
    if (!data) return;
    const profiles = data.profiles.filter((profile) => force || (!checkedProfiles.current.has(profile.id) && !repositoryProfileStatus(profile.id, statuses, data.repositories)));
    if (!profiles.length) return;
    profiles.forEach((profile) => checkedProfiles.current.add(profile.id));
    setProfileRefreshing(true);
    void Promise.all(profiles.map(async (profile) => {
      try {
        const status = await inspectGithubProfile(profile.id, profile.ghConfigDir);
        setProfileStatuses((current) => ({ ...current, [profile.id]: status }));
      } catch {
        setProfileStatuses((current) => ({ ...current, [profile.id]: { available: false, authenticated: false } }));
      }
    })).finally(() => setProfileRefreshing(false));
  };

  useEffect(() => {
    if (page === "profiles" || (data && data.repositories.length === 0)) refreshProfileStatuses();
    // Each profile is checked once while this page is open; a manual refresh can check again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page, data, statuses]);

  const currentProfileStatuses = data ? Object.fromEntries(data.profiles.flatMap((profile) => {
    const status = profileStatuses[profile.id] ?? repositoryProfileStatus(profile.id, statuses, data.repositories);
    return status ? [[profile.id, status]] : [];
  })) as Record<string, GhProfileStatus> : {};

  const refreshStatuses = () => {
    if (statusRefresh.current.running) return;
    statusRefresh.current = { at: Date.now(), running: true };
    setStatusRefreshing(true);
    inspectRepositoryStatuses()
      .then((items) => {
        setStatuses(Object.fromEntries(items.map((item) => [item.repositoryId, item])));
        setStatusesRefreshedAt(new Date());
        void reportRepositoryStatuses(items).catch((error) => console.error("Could not update tray status", error));
      })
      .catch((error) => setErrorNotice(`${messageFrom(error, locale)} ${shellCopy[locale].refresh}`))
      .finally(() => {
        statusRefresh.current.running = false;
        setStatusRefreshing(false);
      });
  };

  useEffect(() => {
    if (data) refreshStatuses();
  }, [data]);

  // Commits or pushes made outside GitContext show up when the user comes back to the window.
  useEffect(() => {
    if (!data) return;
    const onReturn = () => {
      if (document.visibilityState !== "visible") return;
      const { at, running } = statusRefresh.current;
      if (shouldRefreshOnFocus(at, Date.now(), running)) refreshStatuses();
    };
    window.addEventListener("focus", onReturn);
    document.addEventListener("visibilitychange", onReturn);
    return () => {
      window.removeEventListener("focus", onReturn);
      document.removeEventListener("visibilitychange", onReturn);
    };
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
      if (!path) return false;
      setBusy(true);
      const repository = await addRepository(path);
      if (data) updateData({ ...data, repositories: [...data.repositories.filter((repo) => repo.id !== repository.id), repository] });
      setSelectedRepositoryId(repository.id);
      setPendingProfileId(data?.profiles[0]?.id ?? "");
      setNotice(copy.repositoryAdded(repository.name));
      return true;
    } catch (error) {
      setErrorNotice(messageFrom(error, locale));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const saveProfileAction = async (profile: Profile) => {
    const nextData = await saveProfile(profile);
    updateData(nextData);
    checkedProfiles.current.add(profile.id);
    setProfileStatuses((current) => ({ ...current, [profile.id]: { available: true, authenticated: false } }));
    if (profile.ghConfigDir) {
      void inspectGithubProfile(profile.id, profile.ghConfigDir)
        .then((status) => setProfileStatuses((current) => ({ ...current, [profile.id]: status })))
        .catch(() => undefined);
    }
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
  const showAiNotice = shouldShowAiNotice(data.profiles.length, aiNoticeDismissed, aiInventory);
  const visibleNotice = errorNotice || (missingTools && !environmentDismissed ? shellCopy[locale].envMissing(missingTools) : null) || (showAiNotice ? shellCopy[locale].aiIntegrationAvailable : null);
  const aiNoticeVisible = !errorNotice && (!missingTools || environmentDismissed) && showAiNotice;
  const openProfileEditor = (profile: Profile) => setEditingProfile({ profile, creating: false });
  const createProfile = () => setEditingProfile({ profile: emptyProfile(), creating: true });
  const profileList = <ProfileList profiles={data.profiles} repositories={data.repositories} statuses={currentProfileStatuses} locale={locale} refreshing={profileRefreshing} onRefresh={() => refreshProfileStatuses(true)} onCreate={createProfile} onOpen={openProfileEditor} />;
  const firstRunGuide = <FirstRunGuide profiles={data.profiles} repositories={data.repositories} statuses={currentProfileStatuses} locale={locale}
    onCreate={() => { setPage("profiles"); createProfile(); }}
    onConnect={() => { setPage("profiles"); const profile = data.profiles.find((item) => item.ghConfigDir) ?? data.profiles[0]; if (profile) openProfileEditor(profile); }}
    onAdd={() => { void addRepo().then((added) => { if (added) setPage("repositories"); }); }}
    onAi={() => navigate("ai")} />;

  return (
    <Shell page={page} locale={locale} repositoryCount={data.repositories.length} notice={visibleNotice} noticeLink={aiNoticeVisible ? "ai" : !errorNotice && missingTools ? "settings" : null} noticeLinkLabel={aiNoticeVisible ? shellCopy[locale].openAiIntegration : undefined} onDismiss={() => { if (errorNotice) setErrorNotice(null); else if (aiNoticeVisible) void dismissAiNotice(); else setEnvironmentDismissed(true); }} onNavigate={navigate}>
      {notice && <p className="ui-result" role="status">{notice}</p>}
      {page === "settings" ? <SettingsPage locale={locale} result={result} onLocaleChange={changeLocale} onEnvironment={(nextEnvironment) => setResult((current) => current ? { ...current, environment: nextEnvironment } : current)} onSettingsChange={(settings) => setResult((current) => current ? { ...current, data: { ...current.data, settings } } : current)} onRestored={(nextResult) => { setResult(nextResult); setSelectedRepositoryId(null); setStatuses({}); setProfileStatuses({}); checkedProfiles.current.clear(); updateLocale(previewLocale ?? resolveLocale(nextResult.data.settings?.locale, navigator.language)); }} />
        : page === "history" ? <HistoryPage data={data} locale={locale} />
        : page === "ai" ? <AiIntegrationPage locale={locale} />
        : page === "profiles" ? editingProfile ? <ProfileEditor key={editingProfile.profile.id} initial={editingProfile.profile} creating={editingProfile.creating} ghAvailable={environment.gh.available} locale={locale} initialStatus={currentProfileStatuses[editingProfile.profile.id]} onStatus={(id, status) => setProfileStatuses((current) => ({ ...current, [id]: status }))} onClose={() => setEditingProfile(null)} onSave={saveProfileAction} onAutoApprove={updateProfileAutoApprove} /> : data.repositories.length === 0 ? <div className="ui-page">{firstRunGuide}{data.profiles.length > 0 && profileList}</div> : profileList
        : selectedRepository ? <RepositoryDetail key={selectedRepository.id} repository={selectedRepository} profiles={data.profiles} status={selectedStatus} pendingProfileId={pendingProfileId} preview={preview} locale={locale} busy={busy} removing={removeConfirmingId === selectedRepository.id}
        onBack={() => { setSelectedRepositoryId(null); setPreview(null); setNotice(null); }} onPendingProfile={setPendingProfileId} onReview={reviewAssignment} onCancelReview={() => setPreview(null)} onApply={applyProfileAction} onAutoApprove={updateAutoApprove}
        onStartRemove={() => setRemoveConfirmingId(selectedRepository.id)} onCancelRemove={() => setRemoveConfirmingId(null)} onRemove={removeSelected}
        onData={updateData} onFinished={refreshStatuses} />
        : data.repositories.length === 0 ? firstRunGuide
        : <RepositoryList key={attentionFilterRequest} initialFilter={attentionFilterRequest > 0 ? "action" : "all"} repositories={data.repositories} profiles={data.profiles} statuses={statuses} locale={locale} busy={busy} refreshing={statusRefreshing} refreshedAt={statusesRefreshedAt} onOpen={selectRepository} onAdd={addRepo} onClone={() => setCloneOpen(true)} onRefresh={refreshStatuses} />}

      {cloneOpen && <CloneDialog profiles={data.profiles} locale={locale} onClose={() => setCloneOpen(false)} onClone={cloneRepositoryAction} />}
    </Shell>
  );
}

export default App;
