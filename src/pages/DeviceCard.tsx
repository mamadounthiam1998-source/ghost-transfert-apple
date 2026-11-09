import { useCallback, useEffect, useRef, useState } from "react";
import { api, Battery, DeviceDetails, DeviceInfo, errText } from "../api";
import { fmtBytes } from "../format";
import { Phone } from "../ui";

function BatteryChip({ b }: { b: Battery }) {
  if (b.percent == null) return null;
  const level = b.percent;
  const color = level <= 20 && !b.charging ? "var(--err)" : level <= 40 && !b.charging ? "var(--warn)" : "var(--ok)";
  return (
    <span className="batt" style={{ color }} title={b.charging ? "En charge" : "Sur batterie"}>
      <span className="batt-body"><i style={{ width: `${level}%`, background: color }} /></span>
      <b>{level} %</b>{b.charging ? " ⚡" : ""}
    </span>
  );
}

export default function DeviceCard({ role, d, busy, onPair }: { role: string; d: DeviceInfo | undefined; busy: boolean; onPair: (d: DeviceInfo) => void }) {
  const [details, setDetails] = useState<DeviceDetails | null>(null);
  const [showDetails, setShowDetails] = useState(false);
  const [showSensitive, setShowSensitive] = useState(false);
  const [screenOn, setScreenOn] = useState(false);
  const [screen, setScreen] = useState<string | null>(null);
  const [screenErr, setScreenErr] = useState<string | null>(null);
  const udid = d?.udid;
  const trusted = !!d?.trusted;
  const stopped = useRef(false);

  // Détails + batterie : au branchement, puis toutes les 20 s hors transfert.
  const loadDetails = useCallback(async () => {
    if (!udid || !trusted) { setDetails(null); return; }
    try { setDetails(await api.details(udid)); } catch { /* conserve les derniers détails */ }
  }, [udid, trusted]);
  useEffect(() => { setDetails(null); setScreen(null); setScreenErr(null); setScreenOn(false); void loadDetails(); }, [udid, trusted, loadDetails]);
  useEffect(() => {
    if (busy) return;
    const t = setInterval(() => void loadDetails(), 20000);
    return () => clearInterval(t);
  }, [busy, loadDetails]);

  // Écran réel : une capture à la fois (pas de chevauchement), pause pendant un transfert.
  useEffect(() => {
    stopped.current = false;
    if (!screenOn || !udid || !trusted) return;
    let timer: ReturnType<typeof setTimeout>;
    const tick = async () => {
      if (stopped.current) return;
      if (!busy) {
        try { const s = await api.screenshot(udid); if (!stopped.current) { setScreen(`data:${s.mime};base64,${s.data}`); setScreenErr(null); } }
        catch (e) { if (!stopped.current) { setScreenErr(errText(e)); setScreenOn(false); setScreen(null); } return; }
      }
      timer = setTimeout(tick, 1500);
    };
    void tick();
    return () => { stopped.current = true; clearTimeout(timer); };
  }, [screenOn, udid, trusted, busy]);

  return (
    <div className={`card dev ${d ? "sel" : ""}`}>
      <div className="dev-top">
        <Phone label={role} on={!!d} name={d?.name} screen={screen} />
        {d ? (
          <div className="dev-info">
            <b>{d.name}</b>
            <span className="muted small">{d.model}{d.osVersion ? ` · iOS ${d.osVersion}` : ""}</span>
            {details?.battery && <BatteryChip b={details.battery} />}
            <span className="muted small">Capacité {fmtBytes(d.capacityBytes)} · libre {fmtBytes(d.freeBytes)}</span>
            {d.simulated && <span className="pill sim">SIMULATION</span>}
            {!d.trusted ? (
              <>
                <span className="pill err"><span className="dot" />Non approuvé</span>
                <button className="btn primary" onClick={() => onPair(d)}>Se fier à cet ordinateur</button>
              </>
            ) : (
              <div className="row" style={{ gap: 8 }}>
                <button className="btn small-btn" onClick={() => setShowDetails((v) => !v)}>{showDetails ? "Masquer les détails" : "Tous les détails"}</button>
                <button className={`btn small-btn ${screenOn ? "primary" : ""}`} onClick={() => { setScreenErr(null); setScreenOn((v) => !v); if (screenOn) setScreen(null); }}>
                  {screenOn ? "Masquer l'écran" : "Afficher l'écran"}</button>
              </div>
            )}
          </div>
        ) : (
          <div className="dev-info"><b className="muted">En attente…</b><span className="muted small">Branchez et déverrouillez un iPhone</span></div>
        )}
      </div>
      {screenErr && <div className="alert warn small" style={{ marginTop: 12 }}>{screenErr}</div>}
      {showDetails && details && (
        <div className="details">
          {details.battery && (
            <div className="dgroup"><h3>Batterie</h3>
              <dl className="kv">
                {details.battery.percent != null && (<><dt>Niveau</dt><dd>{details.battery.percent} %</dd></>)}
                {details.battery.charging != null && (<><dt>État</dt><dd>{details.battery.full ? "Chargée" : details.battery.charging ? "En charge" : details.battery.plugged ? "Branchée, ne charge pas" : "Sur batterie"}</dd></>)}
                {details.battery.cycleCount != null && (<><dt>Cycles de charge</dt><dd>{details.battery.cycleCount}</dd></>)}
                {details.battery.healthPercent != null && (<><dt>Capacité maximale (estimée)</dt><dd>{details.battery.healthPercent} %</dd></>)}
              </dl></div>
          )}
          {details.groups.map((g) => (
            <div key={g.title} className="dgroup"><h3>{g.title}</h3>
              {g.sensitive && !showSensitive ? (
                <button className="btn small-btn" onClick={() => setShowSensitive(true)}>Afficher (données personnelles)</button>
              ) : (
                <dl className="kv">{g.items.map((i) => (<><dt key={i.label + "k"}>{i.label}</dt><dd key={i.label + "v"} className="mono">{i.value}</dd></>))}</dl>
              )}
            </div>
          ))}
        </div>
      )}
      {showDetails && !details && d?.trusted && <p className="muted small" style={{ marginTop: 12 }}>Chargement des détails…</p>}
    </div>
  );
}
