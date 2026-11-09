import { useEffect, useState } from "react";
import { api, HistoryEntry } from "../api";
import { fmtDate } from "../format";
import { Check, Flow, Phone, useApp } from "../ui";

export default function Home() {
  const { go, status, devices, source, dest, refreshDevices } = useApp();
  const [hist, setHist] = useState<HistoryEntry[]>([]);
  const [nBackups, setN] = useState(0);
  useEffect(() => { void refreshDevices(); api.history().then((h) => setHist(h.slice(0, 5))).catch(() => {}); api.backupList().then((b) => setN(b.length)).catch(() => {}); }, [refreshDevices]);
  const linked = !!source && !!dest;
  return (
    <div className="page">
      <div className="hero-wrap">
        <div className="logo-title">GHOST TRANSFERT APPLE</div>
        <div className="slogan">Votre univers Apple. Transféré en toute sécurité.</div>
        <div className="stage">
          <Phone label="SOURCE" on={!!source} name={devices.find((d) => d.udid === source)?.name} />
          <Flow active={linked} />
          <Phone label="DESTINATION" on={!!dest} tilt={1} name={devices.find((d) => d.udid === dest)?.name} />
        </div>
        <button className="btn primary hero" onClick={() => go("connect")}>CONNECTER MES APPAREILS</button>
        <p className="muted small" style={{ marginTop: 14 }}>
          {linked ? "Deux appareils réellement détectés. Le flux lumineux est actif." : "Le flux lumineux ne s'active qu'avec deux appareils réellement connectés."}
        </p>
      </div>
      <div className="grid g4" style={{ marginTop: 26 }}>
        <div className="card"><small className="muted">MODE</small><h2>{status?.simulation ? "Simulation" : "Appareils réels"}</h2>
          {status?.simulation ? <Check ok="warn">appareils fictifs</Check> : <Check ok={!!status?.toolsOk}>{status?.toolsOk ? "outils prêts" : "outils manquants"}</Check>}</div>
        <div className="card"><small className="muted">APPAREILS DÉTECTÉS</small><h2>{devices.length}</h2><span className="muted small">sur ce moment, via USB</span></div>
        <div className="card"><small className="muted">SAUVEGARDES</small><h2>{nBackups}</h2><span className="muted small">stockées localement</span></div>
        <div className="card"><small className="muted">SÉCURITÉ</small><h2>AES-256-GCM</h2><span className="muted small">Argon2id · SHA-256</span></div>
      </div>
      <div className="card" style={{ marginTop: 16 }}>
        <h2>Dernières opérations</h2>
        {hist.length === 0 ? <p className="muted">Aucune opération pour l'instant.</p> : (
          <table><tbody>{hist.map((h) => (
            <tr key={h.id}><td style={{ width: 150 }} className="muted small">{fmtDate(h.finished)}</td><td style={{ width: 90 }}><b>{h.kind}</b></td>
              <td><span className={`pill ${h.status === "success" ? "ok" : h.status === "failed" ? "err" : "warn"}`}>{h.status}</span> {h.simulated && <span className="pill sim">SIM</span>} <span className="muted small">{h.summary}</span></td></tr>
          ))}</tbody></table>
        )}
      </div>
    </div>
  );
}
