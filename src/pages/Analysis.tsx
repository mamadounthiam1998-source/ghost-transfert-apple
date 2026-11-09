import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { AnalysisReport, api, BackupInfo, CATEGORY_LABEL, Category, errText, pickFolder } from "../api";
import { fmtBytes } from "../format";
import { PageHeader, Stat, UsageBars, useApp, useOp } from "../ui";

export default function Analysis() {
  const { devices, source, dest, go } = useApp();
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const op = useOp<AnalysisReport>();
  useEffect(() => { api.backupList().then(setBackups).catch(() => {}); }, []);
  const r = op.result;
  const max = r ? Math.max(1, ...Object.values(r.categories).map((c) => c.bytes)) : 1;
  const s = devices.find((d) => d.udid === source), t = devices.find((d) => d.udid === dest);
  return (
    <div className="page">
      <PageHeader title="Analyse" sub="Apple ne laisse pas un logiciel tiers compter les photos ou contacts directement sur un appareil verrouillé. L'analyse détaillée porte donc sur une sauvegarde ou un dossier ; l'espace des appareils est lu en direct." />
      {s && t && (
        <div className="grid g4" style={{ marginBottom: 16 }}>
          <div className="card"><Stat k="Données à migrer" v={fmtBytes(s.usedBytes)} /></div>
          <div className="card"><Stat k="Espace libre destination" v={fmtBytes(t.freeBytes)} /></div>
          <div className="card"><Stat k="Capacité destination" v={fmtBytes(t.capacityBytes)} /></div>
          <div className="card"><Stat k="Verdict stockage" v={s.usedBytes != null && t.freeBytes != null ? (s.usedBytes <= t.freeBytes ? "Suffisant" : "Insuffisant") : "—"} /></div>
        </div>
      )}
      {[s, t].filter((d): d is NonNullable<typeof d> => !!d).map((d) => (
        <div key={d.udid} className="card" style={{ marginBottom: 16 }}>
          <h2>Contenu de « {d.name} » <span className="muted small">(lu sur l'appareil)</span></h2>
          <UsageBars device={d} />
        </div>
      ))}
      <div className="card">
        <h2>Analyse détaillée (nombre de fichiers)</h2>
        <div className="row">
          <button className="btn" disabled={op.busy} onClick={async () => { const p = await pickFolder("Dossier à analyser"); if (p) await op.run(() => api.analyzeFolder(p)); }}>Un dossier…</button>
          {backups.slice(0, 4).map((b) => <button key={b.hid + b.version} className="btn" disabled={op.busy} onClick={() => op.run(() => api.analyzeBackup(b.hid))}>Sauvegarde « {b.deviceName} »</button>)}
          {op.busy && <span className="spinner" />}
        </div>
        {backups.length === 0 && <p className="muted small">Aucune sauvegarde : créez-en une dans « Sauvegardes » pour obtenir le décompte par catégorie.</p>}
      </div>
      {op.error && <div className="alert err" style={{ marginTop: 16 }}>{errText(op.error)}</div>}
      {r && (
        <motion.div className="card" style={{ marginTop: 16 }} initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
          <div className="grid g3" style={{ marginBottom: 12 }}><Stat k="Fichiers" v={String(r.totalFiles)} /><Stat k="Taille totale" v={fmtBytes(r.totalBytes)} /><Stat k="Fichiers suspects" v={String(r.suspicious.length)} /></div>
          {Object.entries(r.categories).sort((a, b) => b[1].bytes - a[1].bytes).map(([k, c]) => (
            <div key={k} className="cat-row"><span>{CATEGORY_LABEL[k as Category] ?? k}</span>
              <div className="bar"><motion.i initial={{ width: 0 }} animate={{ width: `${(c.bytes / max) * 100}%` }} /></div>
              <span className="muted small" style={{ textAlign: "right" }}>{c.files} · {fmtBytes(c.bytes)}</span></div>))}
          {r.suspicious.length > 0 && <div className="alert warn" style={{ marginTop: 12 }}>Médias vides ou illisibles (possiblement endommagés) :<ul>{r.suspicious.slice(0, 10).map((x) => <li key={x}>{x}</li>)}</ul></div>}
          <div className="alert info" style={{ marginTop: 12 }}>Éléments nécessitant une opération Apple complémentaire : applications (à retélécharger depuis l'App Store), Apple Pay, Face ID/Touch ID, eSIM et contenus uniquement stockés dans iCloud.</div>
          <button className="btn" style={{ marginTop: 12 }} onClick={() => go("smart")}>Transférer ces données</button>
        </motion.div>
      )}
    </div>
  );
}
