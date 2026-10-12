import { useEffect, useState } from "react";
import { applyAiClient, listAiClients, loadState, planAiClient, setAiClientPolicy, verifyAiClient } from "../backend";
import { aiCopy, type Locale } from "../i18n";
import type { AiAction, AiClient, AiClientInfo, AiClientKey, AiClientPolicies, AiInventory, AiPlan, AiTier } from "../types";
import { ConfirmPanel } from "./ConfirmPanel";

const names: Record<AiClient, string> = { claude_code: "Claude Code", codex: "Codex (CLI / Desktop)", claude_desktop: "Claude Desktop" };
const defaultPolicies = (): AiClientPolicies => ({ claudeCode: { maxTier: "read", allowTrustClientApproval: false }, claudeCodeDesktop: { maxTier: "read", allowTrustClientApproval: false }, codex: { maxTier: "read", allowTrustClientApproval: false }, other: { maxTier: "read", allowTrustClientApproval: false } });
const keysFor = (client: AiClient): AiClientKey[] => client === "claude_code" ? ["claudeCode", "claudeCodeDesktop"] : [client === "codex" ? "codex" : "other"];
const tierRank: Record<AiTier, number> = { read: 0, local: 1, remote: 2 };
export const registrationNeedsUpdate = (info: AiClientInfo, policies: AiClientPolicies) => info.command !== null && keysFor(info.client).some((key) => tierRank[policies[key].maxTier] > tierRank[info.tier] || (policies[key].allowTrustClientApproval && !info.trust));

export function AiIntegrationPage({ locale }: { locale: Locale }) {
  const copy = aiCopy[locale];
  const [inventory, setInventory] = useState<AiInventory | null>(null);
  const [policies, setPolicies] = useState<AiClientPolicies>(defaultPolicies);
  const [pendingConsent, setPendingConsent] = useState<AiClientKey | null>(null);
  const [selected, setSelected] = useState<{ client: AiClient; action: AiAction } | null>(null);
  const [plan, setPlan] = useState<AiPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);

  const refresh = async () => {
    try { const [nextInventory, data] = await Promise.all([listAiClients(), loadState()]); setInventory(nextInventory); setPolicies(data.settings.aiClients ?? defaultPolicies()); setError(""); }
    catch (cause) { setError(String(cause)); }
  };
  useEffect(() => { void refresh(); }, []);

  const savePolicy = async (key: AiClientKey, maxTier: AiTier, allowTrustClientApproval: boolean) => {
    setBusy(true); setError("");
    try { const settings = await setAiClientPolicy(key, { maxTier, allowTrustClientApproval }); setPolicies(settings.aiClients ?? defaultPolicies()); setMessage(copy.policySaved); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };

  const start = async (info: AiClientInfo, action: AiAction) => {
    setSelected({ client: info.client, action }); setPlan(null); setMessage(""); setError(""); setBusy(true);
    const allowedTrust = action !== "disconnect" && keysFor(info.client).some((key) => policies[key].allowTrustClientApproval);
    try { setPlan(await planAiClient(info.client, action, "remote", allowedTrust)); }
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
          const updateNeeded = registrationNeedsUpdate(info, policies);
          return <div className="ui-ai-client" key={info.client}>
            <div className="ui-ai-row">
              <div><strong>{names[info.client]}</strong><code>{info.configPath}</code></div>
              <div><span className={`ui-ai-status ui-ai-status--${info.state}`}>{copy.states[info.state]}</span><small>{copy.confirmation[info.confirmation]}</small>{info.state === "connected" && <button className="ui-ai-verify" type="button" onClick={() => void check(info.client)} disabled={busy}>{copy.verify}</button>}</div>
              <div>{info.state === "repair" ? <span className="ui-muted">{copy.missing}</span> : info.state === "connected" ? <><span className="ui-muted">{copy.scope} </span>{copy.tiers[info.tier]}</> : <span className="ui-muted">{copy.scope} —</span>}{updateNeeded && <small className="ui-ai-danger">{copy.registrationUpdate}</small>}<small>{copy.name} <code>{info.registrationName}</code></small></div>
              <div className="ui-ai-actions">
                {info.state === "connected" ? <><button className="ui-button" type="button" onClick={() => void start(info, "change")}>{updateNeeded ? copy.updateRegistration : copy.actions.change}</button><button className="ui-button ui-ai-danger" type="button" onClick={() => void start(info, "disconnect")}>{copy.actions.disconnect}</button></>
                  : info.state === "not_found" ? null : <button className="ui-button" type="button" onClick={() => void start(info, info.state === "repair" ? "repair" : "connect")} disabled={!inventory.server.built}>{copy.actions[info.state === "repair" ? "repair" : "connect"]}</button>}
              </div>
            </div>
            {updateNeeded && <p className="ui-ai-caution" role="status">{copy.registrationUpdateHelp}</p>}
            {info.client === "claude_code" && <p className="ui-muted">{copy.sharedRegistration}</p>}
            {keysFor(info.client).map((key) => <div className="ui-ai-editor" key={key}>
              {info.client === "claude_code" && <strong>{key === "claudeCode" ? copy.cli : copy.codeTabLabel}</strong>}
              <label>{copy.scopeSetting} <select aria-label={`${names[info.client]} ${key === "claudeCode" ? copy.cli : key === "claudeCodeDesktop" ? copy.codeTabLabel : ""} ${copy.scopeSetting}`} value={policies[key].maxTier} disabled={busy} onChange={(event) => void savePolicy(key, event.target.value as AiTier, policies[key].allowTrustClientApproval)}>
                {(["read", "local", "remote"] as const).map((option) => <option value={option} key={option}>{copy.tiers[option]}</option>)}
              </select></label>
              {policies[key].maxTier === "remote" && <small className="ui-ai-danger">{copy.remoteNote}</small>}
              {key !== "claudeCode" && <label><input type="checkbox" checked={policies[key].allowTrustClientApproval} disabled={busy} onChange={(event) => { if (event.target.checked) setPendingConsent(key); else { setPendingConsent(null); void savePolicy(key, policies[key].maxTier, false); } }} />{copy.trustSetting}</label>}
              {pendingConsent === key && <div className="ui-ai-warning"><p>{copy.warning}</p><label><input type="checkbox" checked={false} onChange={(event) => { if (event.target.checked) { setPendingConsent(null); void savePolicy(key, policies[key].maxTier, true); } }} />{copy.understand}</label></div>}
            </div>)}
            {open && <div className="ui-ai-editor">
              {plan && <ConfirmPanel locale={locale} rows={[{ label: copy.config, value: <code>{plan.configPath}</code> }, { label: copy.name, value: plan.registrationName }, { label: copy.support, value: copy.confirmation[info.confirmation] }]}>
                <div className="ui-ai-diff"><strong>{plan.commandLine ? copy.command : copy.changes}</strong>{plan.commandLine ? <><pre tabIndex={0}>{plan.commandLine}</pre>{plan.manual && <p className="ui-ai-caution">{copy.manual}</p>}</> : <><div><span>{copy.before}</span><pre>{plan.before || "—"}</pre></div><div><span>{copy.after}</span><pre>{plan.after || "—"}</pre></div></>}<p className="ui-muted">{copy.backup}</p></div>
                <button className="ui-button" type="button" onClick={() => { setPlan(null); setSelected(null); }}>{copy.cancel}</button>
                {plan.manual ? <button className="ui-button ui-button--primary" type="button" onClick={() => void copyCommand(plan.commandLine || "")}>{copied ? copy.copied : copy.copy}</button> : <button className="ui-button ui-button--primary" type="button" onClick={() => void apply()} disabled={busy}>{selected.action === "disconnect" ? copy.remove : copy.apply}</button>}
              </ConfirmPanel>}
            </div>}
          </div>;
        })}
      </section>
      <details className="ui-ai-manual"><summary>{copy.manualLink}</summary><p>{copy.manualGuide}</p><code>{inventory.server.path}</code><code>--max-tier remote</code></details>
    </>}
    {message && <p className="ui-result" role="status">{message}</p>}
    {error && <p className="ui-error" role="alert">{copy.error}: {error}</p>}
  </div>;
}
