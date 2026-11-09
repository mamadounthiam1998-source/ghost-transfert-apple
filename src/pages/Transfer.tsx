import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { api, CloneReport, Compatibility, DeviceInfo, errText } from "../api";
import DeviceCard from "./DeviceCard";
import { etaFrom, fmtBytes, fmtEta } from "../format";
import { Flow, Ring, SuccessMark, UsageBars, useApp, useOp } from "../ui";

export default function Transfer() {
  const { devices, devicesError, refreshDevices, source, dest, setSource, setDest, progress, confirm, toast, go, status } = useApp();
  const op = useOp<CloneReport>();
  const [compat, setCompat] = useState<Compatibility | null>(null);
  const started = useRef(0);
  const best = useRef(0);

  // Détection : toutes les 3 s, sauf pendant un transfert.
  useEffect(() => {
    void refreshDevices();
    if (op.busy) return;
    const t = setInterval(() => void refreshDevices(), 3000);
    return () => clearInterval(t);
  }, [refreshDevices, op.busy]);

  // Attribution automatique : 1er appareil = source, 2e = destination.
  useEffect(() => {
    if (op.busy) return;
    if (!source && devices[0]) setSource(devices[0].udid);
    if (source && !dest) { const o = devices.find((d) => d.udid !== source); if (o) setDest(o.udid); }
  }, [devices, source, dest, setSource, setDest, op.busy]);

  const s = devices.find((d) => d.udid === source);
  const t = devices.find((d) => d.udid === dest);
  useEffect(() => {
    setCompat(null);
    if (s && t) api.compat(s.udid, t.udid).then(setCompat).catch(() => setCompat(null));
  }, [s?.udid, t?.udid, s?.trusted, t?.trusted, devices.length]); // eslint-disable-line react-hooks/exhaustive-deps

  const pair = async (d: DeviceInfo) => {
    try { toast(await api.pair(d.udid)); } catch (e) { toast(errText(e), "err"); }
    await refreshDevices();
  };

  const start = async () => {
    if (!s || !t) return;
    const ok = await confirm("Lancer le transfert ?",
      <>Les données de <b>{t.name}</b> seront <b>remplacées</b> par celles de <b>{s.name}</b> ({fmtBytes(s.usedBytes)}). Ne débranchez aucun appareil pendant l'opération.</>, true);
    if (!ok) return;
    started.current = Date.now(); best.current = 0;
    await op.run(() => api.clone({ sourceUdid: s.udid, destUdid: t.udid, mode: "full", system: true, settings: true, password: null, confirmErase: true }));
  };

  const pct = Math.max(best.current, progress?.percent ?? 0);
  best.current = pct;
  const total = s?.usedBytes ?? 0;
  const done = op.result;
  const ready = !!compat?.ok && !op.busy;

  return (
    <div className="page">
      {status && !status.simulation && !status.toolsOk && (
        <div className="alert err" style={{ marginBottom: 14 }}>Les outils Apple sont introuvables. <button className="btn" onClick={() => go("diagnostics")}>Ouvrir le diagnostic</button></div>
      )}
      {devicesError && <div className="alert err" style={{ marginBottom: 14 }}>{devicesError} <button className="btn" onClick={() => go("diagnostics")}>Diagnostic</button></div>}

      <div className="stage2">
        <DeviceCard role="SOURCE" d={s} busy={op.busy} onPair={pair} />
        <div className="mid">
          <Flow active={op.busy || !!compat?.ok} />
          <button className="btn small-btn" disabled={op.busy || devices.length < 2} title="Inverser source et destination"
            onClick={() => { setSource(dest); setDest(source); }}>⇄ Inverser</button>
        </div>
        <DeviceCard role="DESTINATION" d={t} busy={op.busy} onPair={pair} />
      </div>

      <AnimatePresence mode="wait">
        {op.busy ? (
          <motion.div key="run" className="card big" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
            <Ring percent={pct} label="TRANSFERT" />
            <div className="run-info">
              <h2>{progress?.phase ?? "Démarrage…"}</h2>
              <div className="bar big-bar"><i style={{ width: `${pct}%` }} /></div>
              <p className="qty">≈ {fmtBytes((total * pct) / 100)} <span className="muted">sur {fmtBytes(total)}</span></p>
              <p className="muted small">Temps restant estimé : {fmtEta(etaFrom(Date.now() - started.current, pct))}</p>
              {progress?.simulated && <span className="pill sim">SIMULATION</span>}
              <div><button className="btn danger" onClick={() => api.cancel()}>Annuler</button></div>
            </div>
          </motion.div>
        ) : done ? (
          <motion.div key="done" className="card big" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
            <SuccessMark ok={done.success && done.integrity.intact} />
            <div className="run-info">
              <h2>{done.success && done.integrity.intact ? "Transfert terminé" : "Transfert terminé avec des réserves"}</h2>
              <p className="muted">{done.integrity.ok} fichiers vérifiés sur {done.integrity.checked}. Laissez l'iPhone de destination redémarrer, puis contrôlez son contenu.</p>
              <ul className="muted small">{done.notes.map((n) => <li key={n}>{n}</li>)}</ul>
              <button className="btn primary" onClick={() => { op.setResult(null); best.current = 0; }}>OK</button>
            </div>
          </motion.div>
        ) : (
          <motion.div key="idle" className="card" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
            <div className="row" style={{ justifyContent: "space-between" }}>
              <h2>Données à transférer</h2>
              {s && <b className="qty-total">{fmtBytes(s.usedBytes)}</b>}
            </div>
            {s ? <UsageBars device={s} /> : <p className="muted">Les données de l'appareil source apparaîtront ici dès qu'il sera détecté et approuvé.</p>}
            {compat && compat.blockers.length > 0 && <div className="alert err" style={{ marginTop: 12 }}><ul>{compat.blockers.map((b) => <li key={b}>{b}</li>)}</ul></div>}
            {compat && compat.ok && s && t && s.usedBytes != null && t.freeBytes != null && (
              <p className="small" style={{ marginTop: 12, color: s.usedBytes <= t.freeBytes ? "var(--ok)" : "var(--err)" }}>
                Espace libre sur la destination : {fmtBytes(t.freeBytes)}
              </p>
            )}
            {op.error && <div className="alert err" style={{ marginTop: 12 }}>{op.error}</div>}
            <div style={{ marginTop: 16, textAlign: "center" }}>
              <button className="btn primary hero" disabled={!ready} onClick={start}>TRANSFÉRER</button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
