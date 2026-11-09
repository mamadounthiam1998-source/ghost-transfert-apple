import { useEffect, useState } from "react";
import { api, HistoryEntry } from "../api";
import { fmtDate } from "../format";
import { PageHeader, useApp } from "../ui";

export default function History() {
  const [h, setH] = useState<HistoryEntry[]>([]);
  const { confirm } = useApp();
  const load = () => api.history().then(setH).catch(() => {});
  useEffect(() => { void load(); }, []);
  return (
    <div className="page">
      <PageHeader title="Historique" sub="Journal local des opérations. Les identifiants d'appareil, e-mails et noms de profil en sont retirés avant l'enregistrement." />
      <div className="card">
        <div className="row" style={{ justifyContent: "space-between" }}><h2>{h.length} opération(s)</h2>
          <button className="btn danger" disabled={!h.length} onClick={async () => { if (await confirm("Effacer l'historique ?", "Cette action est définitive.", true)) { await api.historyClear(); void load(); } }}>Effacer</button></div>
        {h.length === 0 ? <p className="muted">Aucune opération.</p> : (
          <table><thead><tr><th>Date</th><th>Type</th><th>Résultat</th><th>Détail</th></tr></thead><tbody>
            {h.map((e) => (<tr key={e.id}><td className="small muted">{fmtDate(e.finished)}</td><td><b>{e.kind}</b> {e.simulated && <span className="pill sim">SIM</span>}</td>
              <td><span className={`pill ${e.status === "success" ? "ok" : e.status === "failed" ? "err" : "warn"}`}>{e.status}</span></td><td className="small">{e.summary}</td></tr>))}
          </tbody></table>)}
      </div>
    </div>
  );
}
