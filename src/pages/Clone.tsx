import { useState } from "react";
import { api, CloneReport } from "../api";
import { fmtBytes } from "../format";
import { Check, PageHeader, ProgressPanel, SuccessMark, useApp, useOp } from "../ui";

const MODES = [
  { id: "quick", t: "MODE RAPIDE", d: "Sauvegarde officielle de la source, puis restauration des réglages sur la destination.", sys: false, set: true },
  { id: "full", t: "MODE COMPLET", d: "Sauvegarde et restauration complètes (système et réglages) avec vérification des empreintes.", sys: true, set: true },
  { id: "custom", t: "MODE PERSONNALISÉ", d: "Vous choisissez ce que la restauration doit inclure.", sys: true, set: true },
] as const;

export default function Clone() {
  const { devices, source, dest, go, confirm, status } = useApp();
  const [mode, setMode] = useState<string>("full");
  const [sys, setSys] = useState(true);
  const [set, setSet] = useState(true);
  const [pw, setPw] = useState("");
  const [ack, setAck] = useState(false);
  const op = useOp<CloneReport>();
  const s = devices.find((d) => d.udid === source), t = devices.find((d) => d.udid === dest);
  if (!s || !t) return (
    <div className="page"><PageHeader title="Ghost Clone" sub="Migration logique la plus fidèle permise par Apple." />
      <div className="alert warn">Sélectionnez d'abord un appareil source et un appareil destination. <button className="btn" onClick={() => go("connect")}>Connecter mes appareils</button></div></div>
  );
  const start = async () => {
    const ok = await confirm("Remplacer les données de la destination ?",
      <>Toutes les données de <b>{t.name}</b> seront remplacées par celles de <b>{s.name}</b>. Une sauvegarde de la source est d'abord créée et vérifiée. Cette opération ne peut pas être annulée une fois la restauration commencée.</>, true);
    if (!ok) return;
    await op.run(() => api.clone({ sourceUdid: s.udid, destUdid: t.udid, mode, system: sys, settings: set, password: pw || null, confirmErase: true }));
  };
  const r = op.result;
  return (
    <div className="page">
      <PageHeader title="Ghost Clone" sub="Sauvegarde et restauration officielles Apple, contrôlées par empreintes SHA-256. Ce n'est pas un clonage bit à bit : Apple ne l'autorise pas aux logiciels tiers." />
      {status?.simulation && <div className="alert warn" style={{ marginBottom: 14 }}>Mode simulation : les appareils sont fictifs, rien n'est écrit sur un vrai téléphone.</div>}
      <div className="grid g3">
        {MODES.map((m) => (
          <div key={m.id} className={`card hover ${mode === m.id ? "sel" : ""}`} onClick={() => { setMode(m.id); if (m.id !== "custom") { setSys(m.sys); setSet(m.set); } }} role="button" tabIndex={0}>
            <b>{m.t}</b><p className="muted small">{m.d}</p>
          </div>
        ))}
      </div>
      {mode === "custom" && (
        <div className="card" style={{ marginTop: 16 }}>
          <label className="check"><input type="checkbox" checked={sys} onChange={(e) => setSys(e.target.checked)} /> Restaurer les données système</label>
          <label className="check" style={{ marginTop: 10 }}><input type="checkbox" checked={set} onChange={(e) => setSet(e.target.checked)} /> Restaurer les réglages</label>
          <p className="muted small">La restauration Apple est « tout ou rien » par type de données : le choix par fichier se fait dans Transfert intelligent.</p>
        </div>
      )}
      <div className="card" style={{ marginTop: 16 }}>
        <h2>Aperçu avant opération</h2>
        <ol className="muted" style={{ lineHeight: 1.8 }}>
          <li>Sauvegarde de <b>{s.name}</b> — environ {fmtBytes(s.usedBytes)} ; ensuite seulement les changements.</li>
          <li>Calcul et contrôle des empreintes SHA-256 de la sauvegarde.</li>
          <li>Restauration sur <b>{t.name}</b> (espace libre : {fmtBytes(t.freeBytes)}) — les données de la destination seront <b>remplacées</b>.</li>
          <li>Rapport détaillé et entrée dans l'historique.</li>
        </ol>
        <div className="grid g2" style={{ marginTop: 10 }}>
          <div><label className="small muted">Mot de passe de chiffrement de la sauvegarde (facultatif, conservé nulle part)</label>
            <input type="password" value={pw} onChange={(e) => setPw(e.target.value)} autoComplete="off" /></div>
        </div>
        <label className="check" style={{ marginTop: 16 }}><input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
          Je comprends que les données de « {t.name} » seront remplacées.</label>
        <div className="row" style={{ marginTop: 16 }}>
          <button className="btn primary" disabled={!ack || op.busy} onClick={start}>LANCER GHOST CLONE</button>
          {op.busy && <button className="btn danger" onClick={() => api.cancel()}>Annuler</button>}
        </div>
      </div>
      <div style={{ marginTop: 16 }}><ProgressPanel busy={op.busy} /></div>
      {op.error && <div className="alert err" style={{ marginTop: 16 }}>{op.error}</div>}
      {r && (
        <div className="card" style={{ marginTop: 16 }}>
          <div className="row" style={{ gap: 20 }}><SuccessMark ok={r.success && r.integrity.intact} />
            <div><h2>{r.success && r.integrity.intact ? "Transfert terminé et vérifié" : "Terminé avec des réserves"}</h2>
              <div className="row"><Check ok={r.integrity.intact}>{r.integrity.ok}/{r.integrity.checked} fichiers vérifiés</Check><Check ok={r.restored}>restauration lancée</Check>{r.simulated && <span className="pill sim">SIMULATION</span>}</div></div></div>
          <ul className="muted">{r.notes.map((n) => <li key={n}>{n}</li>)}</ul>
          <button className="btn" onClick={() => go("history")}>Ouvrir l'historique</button>
        </div>
      )}
    </div>
  );
}
