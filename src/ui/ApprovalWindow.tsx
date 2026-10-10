import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { approvalCopy, type Locale } from "../i18n";
import "./ui.css";

interface ApprovalRequest {
  id: string;
  tool: string;
  message: string;
  client: string | null;
  expiresAt: string;
  kind?: "assignment" | null;
  assignment?: { repositoryName: string; repositoryPath: string; profiles: { id: string; label: string; applyDefaults: boolean }[]; profileId: string | null; applyDefaults: boolean } | null;
}

const demoRequest: ApprovalRequest = {
  id: "demo", tool: "push",
  message: "Confirm GitContext operation: push. Profile: Work. GitHub user: example-user. Repository: example/repository. Branch: feature/demo. Commit: abc123. Target: current branch. Approve only if these details are correct.",
  client: "AI client", expiresAt: new Date(Date.now() + 120_000).toISOString(),
};

const assignmentDemo: ApprovalRequest = {
  id: "demo-assignment", tool: "apply_profile", message: "Assign a Profile to this repository.",
  client: "AI client", expiresAt: new Date(Date.now() + 120_000).toISOString(), kind: "assignment",
  assignment: { repositoryName: "sample", repositoryPath: "C:\\Projects\\sample", profiles: [
    { id: "personal", label: "Personal (@example-user)", applyDefaults: true },
    { id: "work", label: "Work (@example-work)", applyDefaults: false },
  ], profileId: "personal", applyDefaults: true },
};

export function ApprovalWindow() {
  const demo = location.search.includes("approval-demo");
  const [locale, setLocale] = useState<Locale>(navigator.language.startsWith("ja") ? "ja" : "en");
  const copy = approvalCopy[locale];
  const [request, setRequest] = useState<ApprovalRequest | null>(demo ? location.search.includes("approval-demo=assignment") ? assignmentDemo : demoRequest : null);
  const [profileId, setProfileId] = useState("");
  const [applyDefaults, setApplyDefaults] = useState(false);
  const [ready, setReady] = useState(false);
  const [seconds, setSeconds] = useState(120);
  const [error, setError] = useState<string | null>(null);
  const rejectRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (demo) return;
    void invoke<string | null>("approval_locale").then((saved) => { if (saved === "ja" || saved === "en") setLocale(saved); });
    let active = true;
    const refresh = async () => {
      const next = await invoke<ApprovalRequest | null>("current_approval");
      if (active) { setRequest(next); setReady(false); rejectRef.current?.focus(); }
    };
    void refresh().catch((reason) => setError(String(reason)));
    const subscription = listen("approval-request", () => void refresh().catch((reason) => setError(String(reason))));
    return () => { active = false; void subscription.then((unlisten) => unlisten()); };
  }, [demo]);

  useEffect(() => {
    if (!request) return;
    setReady(false);
    setProfileId(request.assignment?.profileId ?? "");
    setApplyDefaults(request.assignment?.applyDefaults ?? false);
    rejectRef.current?.focus();
    const enable = window.setTimeout(() => setReady(true), 1000);
    const tick = window.setInterval(() => {
      const remaining = Math.max(0, Math.ceil((Date.parse(request.expiresAt) - Date.now()) / 1000));
      setSeconds(remaining);
      if (remaining === 0 && demo) setRequest(null);
    }, 200);
    return () => { window.clearTimeout(enable); window.clearInterval(tick); };
  }, [request?.id]);

  useEffect(() => { if (ready) rejectRef.current?.focus(); }, [ready]);

  const respond = async (approved: boolean) => {
    if (!request || !ready) return;
    if (demo) { setRequest(null); return; }
    try { await invoke("answer_approval", { id: request.id, approved, profileId: request.kind === "assignment" ? profileId || null : null, applyDefaults: request.kind === "assignment" ? applyDefaults : null }); setRequest(null); }
    catch (reason) { setError(String(reason)); }
  };

  return <main className="ui-approval-page">
    <section className="ui-card ui-approval-card" aria-labelledby="approval-title">
      <h1 id="approval-title">{request?.kind === "assignment" ? copy.assignmentTitle : copy.title}</h1>
      {request ? <>
        <p className="ui-muted">{copy.client}: {request.client || copy.unknownClient}</p>
        {request.kind === "assignment" && request.assignment ? <>
          <p className="ui-approval-operation">{copy.repository}: <strong>{request.assignment.repositoryName}</strong></p>
          <p className="ui-approval-message">{request.assignment.repositoryPath}</p>
          <label className="ui-field">{copy.profile}
            <select value={profileId} onChange={(event) => {
              const selected = event.target.value;
              setProfileId(selected);
              setApplyDefaults(request.assignment?.profiles.find((profile) => profile.id === selected)?.applyDefaults ?? false);
            }}>
              <option value="">{copy.chooseProfile}</option>
              {request.assignment.profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.label}</option>)}
            </select>
          </label>
          <label className="ui-assignment-defaults"><input type="checkbox" checked={applyDefaults} onChange={(event) => setApplyDefaults(event.target.checked)} /> {copy.applyDefaults}</label>
        </> : <><p className="ui-approval-operation">{copy.operation}: <strong>{request.tool}</strong></p><p className="ui-approval-message">{request.message}</p></>}
        <p className="ui-muted" role="timer">{copy.remaining}: {seconds} {copy.seconds}</p>
        <div className="ui-confirm-actions">
          <button ref={rejectRef} className="ui-button ui-button--danger" type="button" disabled={!ready} onClick={() => void respond(false)}>{copy.reject}</button>
          <button className="ui-button ui-button--primary" type="button" disabled={!ready || seconds === 0 || (request.kind === "assignment" && !profileId)} onClick={() => void respond(true)}>{copy.approve}</button>
        </div>
      </> : <p className="ui-muted">{copy.waiting}</p>}
      {error && <p className="ui-settings-error" role="alert">{error}</p>}
    </section>
  </main>;
}
