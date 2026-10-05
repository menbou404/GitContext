import { useEffect, useState } from "react";
import { applyAiClient, listAiClients, planAiClient, verifyAiClient } from "../backend";
import { aiCopy, type Locale } from "../i18n";
import type { AiAction, AiClient, AiClientInfo, AiInventory, AiPlan, AiTier } from "../types";
import { ConfirmPanel } from "./ConfirmPanel";

const names: Record<AiClient, string> = { claude_code: "Claude Code", codex: "Codex (CLI / Desktop)", claude_desktop: "Claude Desktop" };
export const requiresAiConsent = (client: AiClient, tier: AiTier) => tier === "remote" && client !== "claude_code";
export const canReviewAiPlan = (client: AiClient, tier: AiTier, understood: boolean) => !requiresAiConsent(client, tier) || understood;

export function AiIntegrationPage({ locale }: { locale: Locale }) {
  const copy = aiCopy[locale];
  const [inventory, setInventory] = useState<AiInventory | null>(null);
  const [selected, setSelected] = useState<{ client: AiClient; action: AiAction } | null>(null);
  const [tier, setTier] = useState<AiTier>("read");
  const [trust, setTrust] = useState(false);
  const [understood, setUnderstood] = useState(false);
  const [plan, setPlan] = useState<AiPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);

  const refresh = async () => {
    try { setInventory(await listAiClients()); setError(""); }
    catch (cause) { setError(String(cause)); }
  };
  useEffect(() => { void refresh(); }, []);

  const start = async (info: AiClientInfo, action: AiAction) => {
    setSelected({ client: info.client, action }); setTier(action === "connect" ? "read" : info.tier);
    setTrust(action === "connect" ? false : info.trust); setUnderstood(false); setPlan(null); setMessage(""); setError("");
    if (action === "disconnect") {
      try { setPlan(await planAiClient(info.client, action, info.tier, false)); }
      catch (cause) { setError(String(cause)); }
    }
  };
  const selectTier = (next: AiTier) => { setTier(next); setTrust(false); setUnderstood(false); setPlan(null); };
  const review = async () => {
    if (!selected) return;
    setBusy(true); setError("");
    try { setPlan(await planAiClient(selected.client, selected.action, tier, trust)); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  const apply = async () => {
    if (!plan) return;
    setBusy(true); setError(""); setMessage("");
    try {
      await applyAiClient(plan);
      await refresh();
      setSelected(null); setPlan(null);
      if (plan.action !== "disconnect") {
        setMessage(copy.verifying);
        try { setMessage(copy.toolCount(await verifyAiClient(plan.client))); }
        catch (cause) { setMessage(""); setError(`${copy.verifyFailed}: ${String(cause)}`); }
      } else { setMessage(copy.actions.disconnect); }
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  const check = async (client: AiClient) => {
    setBusy(true); setMessage(copy.verifying); setError("");
    try { setMessage(copy.toolCount(await verifyAiClient(client))); }
    catch (cause) { setMessage(""); setError(String(cause)); }
    finally { setBusy(false); }
  };
  const copyCommand = async (command: string) => {
    try { await navigator.clipboard.writeText(command); setCopied(true); }
    catch { setCopied(false); }
  };
  return <div className="ui-page ui-ai-page">
    <div className="ui-page-heading"><div><h1>{copy.title}</h1><p className="ui-muted">{copy.lead}</p></div><button className="ui-button" type="button" onClick={() => void refresh()}>{copy.refresh}</button></div>
    {inventory && <>
      <div className="ui-ai-server ui-card"><span className="ui-muted">{copy.server}</span><strong>gitcontext-mcp</strong><span className="ui-ai-badge">{inventory.server.development ? copy.development : copy.release}</span><code>{inventory.server.path}</code><span className="ui-muted">{inventory.server.version}</span></div>
      {!inventory.server.built && <p className="ui-ai-caution" role="status">{copy.unbuilt}</p>}
      <section className="ui-ai-clients ui-card" aria-label={copy.clients}><h2>{copy.clients}</h2>
        {inventory.clients.map((info) => {
          const open = selected?.client === info.client;
          const needsConsent = requiresAiConsent(info.client, tier);
          const codeTrust = tier === "remote" && info.client === "claude_code";
          const canReview = canReviewAiPlan(info.client, tier, understood);
          return <div className="ui-ai-client" key={info.client}>
            <div className="ui-ai-row">
              <div><strong>{names[info.client]}</strong><code>{info.configPath}</code></div>
              <div><span className={`ui-ai-status ui-ai-status--${info.state}`}>{copy.states[info.state]}</span><small>{copy.confirmation[info.confirmation]}</small>{info.state === "connected" && <button className="ui-ai-verify" type="button" onClick={() => void check(info.client)} disabled={busy}>{copy.verify}</button>}</div>
              <div>{info.state === "repair" ? <span className="ui-muted">{copy.missing}</span> : info.state === "connected" ? <><span className="ui-muted">{copy.scope} </span>{copy.tiers[info.tier]}</> : <span className="ui-muted">{copy.scope} —</span>}{info.trust && <small className="ui-ai-danger">{copy.clientApproval}</small>}<small>{copy.name} <code>{info.registrationName}</code></small></div>
              <div className="ui-ai-actions">
                {info.state === "connected" ? <><button className="ui-button" type="button" onClick={() => void start(info, "change")}>{copy.actions.change}</button><button className="ui-button ui-ai-danger" type="button" onClick={() => void start(info, "disconnect")}>{copy.actions.disconnect}</button></>
                  : info.state === "not_found" ? null : <button className="ui-button" type="button" onClick={() => void start(info, info.state === "repair" ? "repair" : "connect")} disabled={!inventory.server.built}>{copy.actions[info.state === "repair" ? "repair" : "connect"]}</button>}
              </div>
            </div>
            {open && <div className="ui-ai-editor">
              {selected.action !== "disconnect" && <fieldset><legend>{copy.scope}</legend>{(["read", "local", "remote"] as const).map((option) => <label className="ui-ai-option" key={option}><input type="radio" name={`ai-tier-${info.client}`} checked={tier === option} onChange={() => selectTier(option)} /><span><strong>{copy.tiers[option]}</strong>{option === "read" && <span className="ui-muted">（{copy.recommended}）</span>}<small>{copy.tierHelp[option]}</small>{option === "remote" && <small className="ui-ai-danger">{copy.remoteNote}</small>}</span></label>)}</fieldset>}
              {needsConsent && <div className="ui-ai-warning"><p>{copy.warning}</p><label><input type="checkbox" checked={understood} onChange={(event) => { setUnderstood(event.target.checked); setTrust(event.target.checked); setPlan(null); }} />{copy.understand}</label></div>}
              {codeTrust && <div className="ui-ai-warning"><p>{copy.codeTab}</p><label><input type="checkbox" checked={trust} onChange={(event) => { setTrust(event.target.checked); setPlan(null); }} />{copy.trust}</label></div>}
              {!plan && selected.action !== "disconnect" && <div className="ui-ai-editor-actions"><button className="ui-button" type="button" onClick={() => setSelected(null)}>{copy.cancel}</button><button className="ui-button ui-button--primary" type="button" onClick={() => void review()} disabled={busy || !canReview}>{copy.review}</button></div>}
              {plan && <ConfirmPanel locale={locale} rows={[{ label: copy.config, value: <code>{plan.configPath}</code> }, { label: copy.name, value: plan.registrationName }, { label: copy.support, value: copy.confirmation[info.confirmation] }]}>
                <div className="ui-ai-diff"><strong>{plan.commandLine ? copy.command : copy.changes}</strong>{plan.commandLine ? <><pre tabIndex={0}>{plan.commandLine}</pre>{plan.manual && <p className="ui-ai-caution">{copy.manual}</p>}</> : <><div><span>{copy.before}</span><pre>{plan.before || "—"}</pre></div><div><span>{copy.after}</span><pre>{plan.after || "—"}</pre></div></>}<p className="ui-muted">{copy.backup}</p></div>
                <button className="ui-button" type="button" onClick={() => { setPlan(null); setSelected(null); }}>{copy.cancel}</button>
                {plan.manual ? <button className="ui-button ui-button--primary" type="button" onClick={() => void copyCommand(plan.commandLine || "")}>{copied ? copy.copied : copy.copy}</button> : <button className="ui-button ui-button--primary" type="button" onClick={() => void apply()} disabled={busy}>{selected.action === "disconnect" ? copy.remove : copy.apply}</button>}
              </ConfirmPanel>}
            </div>}
          </div>;
        })}
      </section>
      <details className="ui-ai-manual"><summary>{copy.manualLink}</summary><p>{copy.manualGuide}</p><code>{inventory.server.path}</code><code>--max-tier read</code></details>
    </>}
    {message && <p className="ui-result" role="status">{message}</p>}
    {error && <p className="ui-error" role="alert">{copy.error}: {error}</p>}
  </div>;
}
