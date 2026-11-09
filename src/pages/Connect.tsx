import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { api, Compatibility, DeviceInfo, errText } from "../api";
import { fmtBytes } from "../format";
import { Check, Flow, PageHeader, Phone, UsageBars, useApp } from "../ui";

function Info({ d }: { d: DeviceInfo }) {
  return (
    <motion.dl className="kv" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
      <dt>Nom</dt><dd>{d.name}</dd><dt>Modèle</dt><dd>{d.model}</dd><dt>Système</dt><dd>{d.osVersion ? `iOS/iPadOS ${d.osVersion}` : "—"}</dd>
      <dt>Capacité</dt><dd>{fmtBytes(d.capacityBytes)}</dd><dt>Utilisé</dt><dd>{fmtBytes(d.usedBytes)}</dd><dt>Disponible</dt><dd>{fmtBytes(d.freeBytes)}</dd>
      <dt>Connexion</dt><dd><Check ok={d.connected}>{d.connected ? "vérifiée (USB)" : "absente"}</Check></dd>
      <dt>Confiance</dt><dd><Check ok={d.trusted}>{d.trusted ? "approuvé" : "non approuvé"}</Check></dd>
    </motion.dl>
  );
}

export default function Connect() {
  const { devices, devicesError, refreshDevices, source, dest, setSource, setDest, status, go, toast } = useApp();
  const [compat, setCompat] = useState<Compatibility | null>(null);
  const [loading, setLoading] = useState(true);
  useEffect(() => { const run = async () => { await refreshDevices(); setLoading(false); }; void run(); const t = setInterval(run, 3000); return () => clearInterval(t); }, [refreshDevices]);
  useEffect(() => {
    setCompat(null);
    if (source && dest) api.compat(source, dest).then(setCompat).catch(() => setCompat(null));
  }, [source, dest, devices]);
  const s = devices.find((d) => d.udid === source), t = devices.find((d) => d.udid === dest);
  const Zone = ({ role, sel, setSel, other }: { role: string; sel: DeviceInfo | undefined; setSel: (u: string | null) => void; other: string | null }) => (
    <div className={`card ${sel ? "sel" : ""}`}>
      <div className="row" style={{ justifyContent: "space-between" }}><h2>APPAREIL {role}</h2>{sel?.simulated && <span className="pill sim">SIMULATION</span>}</div>
      <div className="row" style={{ alignItems: "flex-start", gap: 22 }}>
        <Phone label={role} on={!!sel} name={sel?.name} />
        <div style={{ flex: 1, minWidth: 230 }}>
          {sel ? <Info d={sel} /> : <p className="muted">Choisissez un appareil détecté ci-dessous.</p>}
          {sel?.note && <p className="muted small">{sel.note}</p>}
          {sel && !sel.trusted && (
            <button className="btn primary" style={{ marginTop: 8 }} onClick={async () => {
              try { toast(await api.pair(sel.udid)); } catch (e) { toast(errText(e), "err"); }
              await refreshDevices();
            }}>Demander l'approbation sur l'appareil</button>
          )}
        </div>
      </div>
      {sel && (<div style={{ marginTop: 16 }}><h2>Données par catégorie</h2><UsageBars device={sel} /></div>)}
      <div className="row" style={{ marginTop: 14 }}>
        {devices.filter((d) => d.udid !== other).map((d) => (
          <button key={d.udid} className={`chip ${sel?.udid === d.udid ? "on" : ""}`} onClick={() => setSel(d.udid)}>{d.name}</button>
        ))}
      </div>
    </div>
  );
  return (
    <div className="page">
      <PageHeader title="Connexion des appareils" sub="Branchez les appareils avec un câble USB-C ou Lightning. Un appareil n'apparaît que s'il est réellement détecté ; la liste se rafraîchit toutes les 3 secondes." />
      {devicesError && <div className="alert err" style={{ marginBottom: 16 }}>{devicesError}
        {!status?.toolsOk && <div style={{ marginTop: 8 }}><button className="btn" onClick={() => go("diagnostics")}>Ouvrir le diagnostic</button></div>}</div>}
      {!devicesError && !loading && devices.length === 0 && (
        <div className="alert info" style={{ marginBottom: 16 }}><b>Aucun appareil Apple détecté.</b>
          <ul>
            <li>Utilisez un câble qui transfère les données (pas un câble de charge seule), branché directement sur le PC.</li>
            <li>Déverrouillez l'iPhone, puis touchez « Se fier à cet ordinateur » et saisissez son code.</li>
            <li>Windows : l'application « Appareils Apple » (Microsoft Store) ou iTunes doit être installée.</li>
            <li>En cas de doute, ouvrez <b>Diagnostic</b> : il indique précisément ce qui manque.</li>
          </ul>
          <button className="btn" onClick={() => go("diagnostics")}>Ouvrir le diagnostic</button></div>
      )}
      <div className="grid g2"><Zone role="SOURCE" sel={s} setSel={setSource} other={dest} /><Zone role="DESTINATION" sel={t} setSel={setDest} other={source} /></div>
      <div style={{ margin: "10px auto 0", maxWidth: 520 }}><Flow active={!!compat?.ok} /></div>
      {compat && (
        <motion.div className="col" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
          {compat.blockers.length > 0 && <div className="alert err"><b>Migration impossible :</b><ul>{compat.blockers.map((b) => <li key={b}>{b}</li>)}</ul></div>}
          {compat.warnings.length > 0 && <div className="alert warn"><b>À savoir :</b><ul>{compat.warnings.map((b) => <li key={b}>{b}</li>)}</ul></div>}
          {compat.ok && (
            <div className="alert ok row" style={{ justifyContent: "space-between" }}><span>Les deux appareils sont connectés, approuvés et compatibles.</span>
              <span className="row"><button className="btn" onClick={() => go("analysis")}>Analyser</button><button className="btn primary" onClick={() => go("clone")}>Ghost Clone</button></span></div>
          )}
        </motion.div>
      )}
    </div>
  );
}
