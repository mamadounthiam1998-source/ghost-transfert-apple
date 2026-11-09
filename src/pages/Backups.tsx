import { useEffect, useState } from "react";
import { api, BackupInfo, errText, pickFile, pickSave, VerifyReport } from "../api";
import { fmtBytes, fmtDate } from "../format";
import { Check, PageHeader, ProgressPanel, useApp, useOp } from "../ui";

export default function Backups() {
  const { devices, source, confirm, toast, go } = useApp();
  const [list, setList] = useState<BackupInfo[]>([]);
  const [pw, setPw] = useState("");
  const [exportPw, setExportPw] = useState("");
  const [verif, setVerif] = useState<Record<string, VerifyReport>>({});
  const op = useOp<BackupInfo>();
  const other = useOp<unknown>();
  const reload = () => api.backupList().then(setList).catch(() => {});
  useEffect(() => { void reload(); }, []);
  const dev = devices.find((d) => d.udid === source);
  const create = async () => { if (!dev) return; const r = await op.run(() => api.backupCreate(dev.udid, pw || null)); if (r) { toast("Sauvegarde terminée"); void reload(); } };
  const key = (b: BackupInfo) => b.hid + b.version;
  return (
    <div className="page">
      <PageHeader title="Sauvegardes" sub="Sauvegarde officielle Apple, stockée localement. Chaque sauvegarde reçoit un manifeste SHA-256 ; les suivantes sont incrémentales. Les mots de passe ne sont jamais enregistrés." />
      <div className="card">
        <h2>Nouvelle sauvegarde</h2>
        {!dev ? <div className="alert warn">Aucun appareil source sélectionné. <button className="btn" onClick={() => go("connect")}>Connecter</button></div> : (
          <div className="row" style={{ alignItems: "flex-end" }}>
            <div style={{ flex: 1, minWidth: 240 }}><label className="small muted">Appareil</label><div><b>{dev.name}</b> {dev.simulated && <span className="pill sim">SIMULATION</span>}</div></div>
            <div style={{ flex: 1, minWidth: 240 }}><label className="small muted">Mot de passe de chiffrement (facultatif)</label><input type="password" autoComplete="off" value={pw} onChange={(e) => setPw(e.target.value)} /></div>
            <button className="btn primary" disabled={op.busy} onClick={create}>SAUVEGARDER</button>
            {op.busy && <button className="btn danger" onClick={() => api.cancel()}>Annuler</button>}
          </div>)}
      </div>
      <div style={{ marginTop: 16 }}><ProgressPanel busy={op.busy || other.busy} /></div>
      {(op.error || other.error) && <div className="alert err" style={{ marginTop: 16 }}>{op.error ?? other.error}</div>}
      <div className="card" style={{ marginTop: 16 }}>
        <div className="row" style={{ justifyContent: "space-between" }}><h2>Historique des versions</h2>
          <button className="btn" onClick={async () => {
            const f = await pickFile("Archive chiffrée .ghost"); if (!f) return;
            const p = window.prompt("Mot de passe de l'archive :"); if (!p) return;
            await other.run(async () => { try { return await api.backupImport(f, p, false); } catch (e) { const m = errText(e); if (m.includes("existe déjà") && await confirm("Écraser la sauvegarde existante ?", "Une sauvegarde identique existe déjà.", true)) return api.backupImport(f, p, true); throw e; } });
            void reload();
          }}>Importer une archive chiffrée</button></div>
        {list.length === 0 ? <p className="muted">Aucune sauvegarde.</p> : (
          <table><thead><tr><th>Appareil</th><th>Date</th><th>Contenu</th><th>État</th><th /></tr></thead><tbody>
            {list.map((b) => { const v = verif[key(b)]; return (
              <tr key={key(b)}>
                <td><b>{b.deviceName}</b><div className="muted small">{b.osVersion && `iOS ${b.osVersion}`} {b.simulated && <span className="pill sim">SIM</span>}</div></td>
                <td className="small">{fmtDate(b.created)}</td>
                <td className="small">{b.files} fichiers · {fmtBytes(b.bytes)}<div className="muted">{b.newOrChanged} nouveaux/modifiés</div></td>
                <td>{b.encrypted && <span className="pill ok">chiffrée</span>} {v && <Check ok={v.intact}>{v.intact ? "intègre" : `${v.missing.length + v.modified.length} anomalie(s)`}</Check>}</td>
                <td><div className="row" style={{ justifyContent: "flex-end" }}>
                  <button className="btn" disabled={op.busy || other.busy} onClick={async () => { const r = await other.run(() => api.backupVerify(b.hid, b.version)); if (r) setVerif((x) => ({ ...x, [key(b)]: r as VerifyReport })); }}>Vérifier</button>
                  <button className="btn" disabled={op.busy || other.busy} onClick={async () => {
                    const pwd = exportPw || window.prompt("Mot de passe de l'export (8 caractères minimum) :"); if (!pwd) return; setExportPw("");
                    const out = await pickSave("Exporter la sauvegarde", `${b.deviceName.replace(/\W+/g, "_")}.ghost`); if (!out) return;
                    const r = await other.run(() => api.backupExport(b.hid, out, pwd)); if (r !== null) toast("Export chiffré créé");
                  }}>Exporter</button>
                  <button className="btn danger" disabled={op.busy || other.busy} onClick={async () => {
                    if (!(await confirm("Supprimer définitivement ?", <>La sauvegarde de « {b.deviceName} » et toutes ses versions seront écrasées puis effacées. L'effacement physique n'est pas garanti sur SSD.</>, true))) return;
                    await other.run(() => api.backupDelete(b.hid)); void reload();
                  }}>Supprimer</button></div></td>
              </tr>); })}
          </tbody></table>)}
      </div>
    </div>
  );
}
