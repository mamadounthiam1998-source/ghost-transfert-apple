import { useEffect, useState } from "react";
import { api, BackupInfo } from "../api";
import { fmtDate } from "../format";
import { Check, PageHeader, ProgressPanel, useApp, useOp } from "../ui";

export default function Restore() {
  const { devices, dest, confirm, go } = useApp();
  const [list, setList] = useState<BackupInfo[]>([]);
  const [sel, setSel] = useState<string>("");
  const [sys, setSys] = useState(false); const [set, setSet] = useState(true); const [pw, setPw] = useState(""); const [ack, setAck] = useState(false);
  const op = useOp<boolean>();
  useEffect(() => { api.backupList().then((l) => { setList(l); if (l[0]) setSel(l[0].hid + "|" + l[0].version); }).catch(() => {}); }, []);
  const t = devices.find((d) => d.udid === dest);
  const b = list.find((x) => x.hid + "|" + x.version === sel);
  const go1 = async () => {
    if (!t || !b) return;
    if (!(await confirm("Restaurer sur cet appareil ?", <>Les données de <b>{t.name}</b> seront remplacées par la sauvegarde de <b>{b.deviceName}</b> du {fmtDate(b.created)}.</>, true))) return;
    await op.run(async () => { await api.restore({ udid: t.udid, hid: b.hid, system: sys, settings: set, reboot: true, password: pw || null, confirmErase: true }); return true; });
  };
  return (
    <div className="page">
      <PageHeader title="Restaurations" sub="Restaure une sauvegarde locale sur l'appareil de destination via le mécanisme officiel Apple. La restauration utilise toujours le dernier état stocké de la sauvegarde ; les versions antérieures servent à l'historique d'intégrité." />
      {!t ? <div className="alert warn">Aucun appareil destination sélectionné. <button className="btn" onClick={() => go("connect")}>Connecter</button></div> : (
        <div className="card">
          <div className="row" style={{ marginBottom: 12 }}><b>Destination :</b> {t.name} {t.simulated && <span className="pill sim">SIMULATION</span>}<Check ok={t.trusted}>{t.trusted ? "approuvé" : "non approuvé"}</Check></div>
          {list.length === 0 ? <p className="muted">Aucune sauvegarde disponible.</p> : (
            <>
              <label className="small muted">Sauvegarde</label>
              <select value={sel} onChange={(e) => setSel(e.target.value)}>{list.map((x) => <option key={x.hid + x.version} value={x.hid + "|" + x.version}>{x.deviceName} — {fmtDate(x.created)}{x.encrypted ? " (chiffrée)" : ""}</option>)}</select>
              <div className="row" style={{ margin: "14px 0" }}>
                <label className="check"><input type="checkbox" checked={sys} onChange={(e) => setSys(e.target.checked)} /> Données système</label>
                <label className="check"><input type="checkbox" checked={set} onChange={(e) => setSet(e.target.checked)} /> Réglages</label>
              </div>
              {b?.encrypted && <div><label className="small muted">Mot de passe de la sauvegarde</label><input type="password" autoComplete="off" value={pw} onChange={(e) => setPw(e.target.value)} /></div>}
              <label className="check" style={{ marginTop: 14 }}><input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} /> Je comprends que les données de l'appareil de destination seront remplacées.</label>
              <div className="row" style={{ marginTop: 14 }}><button className="btn primary" disabled={!ack || op.busy || !b} onClick={go1}>RESTAURER</button>{op.busy && <button className="btn danger" onClick={() => api.cancel()}>Annuler</button>}</div>
            </>)}
        </div>)}
      <div style={{ marginTop: 16 }}><ProgressPanel busy={op.busy} /></div>
      {op.error && <div className="alert err" style={{ marginTop: 16 }}>{op.error}</div>}
      {op.result === true && <div className="alert ok" style={{ marginTop: 16 }}>Restauration terminée côté logiciel. Laissez l'appareil redémarrer, puis contrôlez son contenu : Apple ne permet pas de le relire pour vérification.</div>}
    </div>
  );
}
