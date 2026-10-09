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
}

const demoRequest: ApprovalRequest = {
  id: "demo", tool: "push",
  message: "Confirm GitContext operation: push. Profile: Work. GitHub user: example-user. Repository: example/repository. Branch: feature/demo. Commit: abc123. Target: current branch. Approve only if these details are correct.",
  client: "AI client", expiresAt: new Date(Date.now() + 120_000).toISOString(),
};

export function ApprovalWindow() {
  const demo = location.search.includes("approval-demo");
  const [locale, setLocale] = useState<Locale>(navigator.language.startsWith("ja") ? "ja" : "en");
  const copy = approvalCopy[locale];
  const [request, setRequest] = useState<ApprovalRequest | null>(demo ? demoRequest : null);
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
    try { await invoke("answer_approval", { id: request.id, approved }); setRequest(null); }
    catch (reason) { setError(String(reason)); }
  };

  return <main className="ui-approval-page">
    <section className="ui-card ui-approval-card" aria-labelledby="approval-title">
      <h1 id="approval-title">{copy.title}</h1>
      {request ? <>
        <p className="ui-muted">{copy.client}: {request.client || copy.unknownClient}</p>
        <p className="ui-approval-operation">{copy.operation}: <strong>{request.tool}</strong></p>
        <p className="ui-approval-message">{request.message}</p>
        <p className="ui-muted" role="timer">{copy.remaining}: {seconds} {copy.seconds}</p>
        <div className="ui-confirm-actions">
          <button ref={rejectRef} className="ui-button ui-button--danger" type="button" disabled={!ready} onClick={() => void respond(false)}>{copy.reject}</button>
          <button className="ui-button ui-button--primary" type="button" disabled={!ready || seconds === 0} onClick={() => void respond(true)}>{copy.approve}</button>
        </div>
      </> : <p className="ui-muted">{copy.waiting}</p>}
      {error && <p className="ui-settings-error" role="alert">{error}</p>}
    </section>
  </main>;
}
