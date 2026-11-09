import { useEffect, useState } from "react";
import { api, Check as C, errText, pickSave } from "../api";
import { Check, PageHeader, useApp } from "../ui";

export default function Diagnostics() {
  const { status, toast, refreshStatus } = useApp();
  const [r, setR] = useState<C[]>([]);
  const [busy, setBusy] = useState(false);
  const run = async () => { setBusy(true); try { setR(await api.diagnostics()); await refreshStatus(); } finally { setBusy(false); } };
  useEffect(() => { void run(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);
  return (
    <div className="page">
      <PageHeader title="Diagnostic" sub="État des outils Apple, des pilotes et des services. Le rapport exporté est anonymisé." />
      <div className="card">
        <div className="row" style={{ justifyContent: "space-between" }}><h2>Composants</h2>
          <div className="row"><button className="btn" onClick={run} disabled={busy}>{busy ? "Analyse…" : "Relancer"}</button>
            <button className="btn" onClick={async () => { const p = await pickSave("Exporter le diagnostic", "diagnostic-ghost.json"); if (!p) return; try { await api.exportDiagnostics(p); toast("Rapport anonymisé exporté"); } catch (e) { toast(errText(e), "err"); } }}>Exporter le rapport</button></div></div>
        {r.map((c) => (
          <div key={c.id} style={{ padding: "10px 0", borderBottom: "1px solid var(--card-border)" }}>
            <div className="row"><Check ok={c.status === "ok" ? true : c.status === "warn" ? "warn" : false}>{c.status === "ok" ? "OK" : c.status === "warn" ? "ATTENTION" : "ERREUR"}</Check><b>{c.name}</b><span className="muted small mono">{c.detail}</span></div>
            {c.solution && <p className="small" style={{ margin: "6px 0 0 4px", color: "var(--warn)" }}>→ {c.solution}</p>}
            {c.id === "amds" && c.status !== "ok" && (
              <div style={{ margin: "8px 0 0 4px" }}>
                <button className="btn primary" onClick={async () => { try { await api.openAppleDevicesStore(); toast("Page du Microsoft Store ouverte : cliquez sur « Obtenir », puis rebranchez l'iPhone."); } catch (e) { toast(errText(e), "err"); } }}>Installer « Appareils Apple » (Microsoft Store)</button>
                <p className="muted small">Après l'installation, redémarrez si Windows le demande, puis cliquez sur « Relancer » ci-dessus.</p>
              </div>)}
          </div>))}
      </div>
      <div className="card" style={{ marginTop: 16 }}>
        <h2>Version et plateforme</h2>
        <dl className="kv"><dt>Application</dt><dd>{status?.version}</dd><dt>Système</dt><dd>{status?.platform}</dd><dt>Dossier des sauvegardes</dt><dd className="mono small">{status?.backupsDir}</dd></dl>
      </div>
    </div>
  );
}
