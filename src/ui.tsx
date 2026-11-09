import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { motion } from "framer-motion";
import { api, AppStatus, DeviceInfo, errText, onProgress, Progress, Settings } from "./api";
import { fmtBytes, fmtEta, fmtPercent, fmtSpeed } from "./format";

// ---------------------------------------------------------------- contexte
export type PageId =
  | "home" | "connect" | "clone" | "smart" | "analysis" | "backups" | "restore" | "history" | "security" | "diagnostics" | "settings";

interface Ctx {
  status: AppStatus | null;
  settings: Settings | null;
  refreshStatus: () => Promise<void>;
  updateSettings: (s: Settings) => Promise<void>;
  devices: DeviceInfo[];
  devicesError: string | null;
  refreshDevices: () => Promise<void>;
  source: string | null; dest: string | null;
  setSource: (u: string | null) => void; setDest: (u: string | null) => void;
  go: (p: PageId) => void;
  progress: Progress | null; resetProgress: () => void;
  confirm: (title: string, body: ReactNode, danger?: boolean) => Promise<boolean>;
  toast: (m: string, kind?: "ok" | "err") => void;
}
export const AppCtx = createContext<Ctx>(null as unknown as Ctx);
export const useApp = () => useContext(AppCtx);

export function useProvider(go: (p: PageId) => void): { ctx: Ctx; overlay: ReactNode } {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [devicesError, setDevicesError] = useState<string | null>(null);
  const [source, setSource] = useState<string | null>(null);
  const [dest, setDest] = useState<string | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [modal, setModal] = useState<{ title: string; body: ReactNode; danger: boolean; resolve: (v: boolean) => void } | null>(null);
  const [toastMsg, setToast] = useState<{ m: string; kind: "ok" | "err" } | null>(null);

  const refreshStatus = useCallback(async () => {
    try { setStatus(await api.status()); setSettings(await api.getSettings()); } catch { /* hors Tauri */ }
  }, []);
  const refreshDevices = useCallback(async () => {
    try {
      const d = await api.devices();
      setDevices(d); setDevicesError(null);
      // Un appareil qui n'est plus détecté ne doit jamais rester affiché comme connecté.
      setSource((s) => (s && d.some((x) => x.udid === s) ? s : null));
      setDest((s) => (s && d.some((x) => x.udid === s) ? s : null));
    } catch (e) { setDevices([]); setDevicesError(errText(e)); setSource(null); setDest(null); }
  }, []);
  const updateSettings = useCallback(async (s: Settings) => { await api.saveSettings(s); await refreshStatus(); }, [refreshStatus]);

  useEffect(() => { void refreshStatus(); }, [refreshStatus]);
  useEffect(() => { const u = onProgress((p) => setProgress(p)); return () => { void u.then((f) => f()); }; }, []);
  useEffect(() => {
    if (!settings) return;
    document.documentElement.dataset.theme = settings.theme;
    document.body.classList.toggle("reduce-motion", settings.reduceMotion);
  }, [settings]);

  const confirm = useCallback((title: string, body: ReactNode, danger = false) =>
    new Promise<boolean>((resolve) => setModal({ title, body, danger, resolve })), []);
  const toast = useCallback((m: string, kind: "ok" | "err" = "ok") => { setToast({ m, kind }); setTimeout(() => setToast(null), 5000); }, []);

  const ctx: Ctx = useMemo(() => ({
    status, settings, refreshStatus, updateSettings, devices, devicesError, refreshDevices, source, dest, setSource, setDest, go,
    progress, resetProgress: () => setProgress(null), confirm, toast,
  }), [status, settings, refreshStatus, updateSettings, devices, devicesError, refreshDevices, source, dest, go, progress, confirm, toast]);

  const overlay = (
    <>
      {modal && (
        <div className="modal-bg" role="dialog" aria-modal="true">
          <motion.div className="card modal" initial={{ opacity: 0, scale: 0.94, y: 12 }} animate={{ opacity: 1, scale: 1, y: 0 }}>
            <h2>{modal.title}</h2>
            <div className="muted" style={{ lineHeight: 1.55, marginBottom: 18 }}>{modal.body}</div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn" autoFocus onClick={() => { modal.resolve(false); setModal(null); }}>Annuler</button>
              <button className={`btn ${modal.danger ? "danger" : "primary"}`} onClick={() => { modal.resolve(true); setModal(null); }}>Confirmer</button>
            </div>
          </motion.div>
        </div>
      )}
      {toastMsg && <div className={`toast alert ${toastMsg.kind}`}>{toastMsg.m}</div>}
    </>
  );
  return { ctx, overlay };
}

// ---------------------------------------------------------------- hook d'opération
export function useOp<T>() {
  const { resetProgress } = useApp();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<T | null>(null);
  const run = useCallback(async (fn: () => Promise<T>) => {
    setBusy(true); setError(null); setResult(null); resetProgress();
    try { const r = await fn(); setResult(r); return r; } catch (e) { setError(errText(e)); return null; } finally { setBusy(false); }
  }, [resetProgress]);
  return { busy, error, result, run, setResult, setError };
}

// ---------------------------------------------------------------- visuels
export function Particles() {
  const ref = useRef<HTMLCanvasElement>(null);
  const { settings } = useApp();
  useEffect(() => {
    const c = ref.current; if (!c) return;
    const reduce = settings?.reduceMotion || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) return;
    const ctx = c.getContext("2d")!;
    let w = 0, h = 0, raf = 0;
    const resize = () => { w = c.width = window.innerWidth; h = c.height = window.innerHeight; };
    resize(); window.addEventListener("resize", resize);
    const ps = Array.from({ length: 55 }, () => ({ x: Math.random() * w, y: Math.random() * h, r: Math.random() * 1.6 + 0.4, vx: (Math.random() - 0.5) * 0.18, vy: -Math.random() * 0.25 - 0.05, a: Math.random() * 0.5 + 0.2 }));
    const tick = () => {
      if (!document.hidden) {
        ctx.clearRect(0, 0, w, h);
        for (const p of ps) {
          p.x += p.vx; p.y += p.vy;
          if (p.y < -5) { p.y = h + 5; p.x = Math.random() * w; }
          ctx.beginPath(); ctx.arc(p.x, p.y, p.r, 0, 7); ctx.fillStyle = `rgba(103,232,249,${p.a})`; ctx.shadowColor = "#22d3ee"; ctx.shadowBlur = 8; ctx.fill();
        }
      }
      raf = requestAnimationFrame(tick);
    };
    tick();
    return () => { cancelAnimationFrame(raf); window.removeEventListener("resize", resize); };
  }, [settings?.reduceMotion]);
  return <canvas ref={ref} className="particles" aria-hidden="true" />;
}

export function Phone({ label, on, name, tilt = 0 }: { label: string; on?: boolean; name?: string; tilt?: number }) {
  const id = `g${label.replace(/\W/g, "")}`;
  return (
    <div className={`phone ${tilt ? "b" : ""} ${on ? "on" : ""}`}>
      <svg viewBox="0 0 160 320" role="img" aria-label={name ?? label}>
        <defs>
          <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor={on ? "#67e8f9" : "#3a4a6b"} /><stop offset="1" stopColor={on ? "#6366f1" : "#1a2438"} />
          </linearGradient>
          <linearGradient id={`${id}s`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={on ? "#0e2a4d" : "#0b1322"} /><stop offset="1" stopColor={on ? "#0a1830" : "#070d19"} />
          </linearGradient>
        </defs>
        <rect x="6" y="6" width="148" height="308" rx="30" fill={`url(#${id})`} />
        <rect x="11" y="11" width="138" height="298" rx="26" fill={`url(#${id}s)`} />
        <rect x="58" y="20" width="44" height="12" rx="6" fill="#02050b" />
        {on && <ellipse cx="80" cy="170" rx="46" ry="60" fill="#22d3ee" opacity="0.13" />}
        <path d="M80 130c-18 0-30 13-30 29v44c0 3 3 4 5 2l6-6 6 6c1 1 3 1 4 0l7-7 7 7c1 1 3 1 4 0l6-6 6 6c2 2 5 1 5-2v-44c0-16-12-29-30-29z" fill={on ? "#67e8f9" : "#2b3a57"} opacity={on ? 0.9 : 0.6} />
        <circle cx="70" cy="160" r="4" fill="#050a14" /><circle cx="90" cy="160" r="4" fill="#050a14" />
      </svg>
      <div className="cap">{label}</div>
    </div>
  );
}

export function Flow({ active }: { active: boolean }) {
  const d = "M0 40 C 110 -6, 290 86, 400 40";
  return (
    <svg className="flow" viewBox="0 0 400 80" preserveAspectRatio="none" aria-hidden="true" style={{ height: 80 }}>
      <defs><linearGradient id="flowg" x1="0" x2="1"><stop offset="0" stopColor="#67e8f9" /><stop offset="1" stopColor="#6366f1" /></linearGradient></defs>
      <path className="base" d={d} fill="none" strokeWidth="3" strokeLinecap="round" />
      {active && <path className="live" d={d} fill="none" strokeWidth="3" strokeLinecap="round" />}
      {active && [0, 0.8, 1.6].map((b) => (
        <circle key={b} r="5" fill="#67e8f9" style={{ filter: "drop-shadow(0 0 6px #22d3ee)" }}>
          <animateMotion dur="2.4s" begin={`${b}s`} repeatCount="indefinite" path={d} />
        </circle>
      ))}
    </svg>
  );
}

export function Ring({ percent, label }: { percent: number; label?: string }) {
  const r = 82, c = 2 * Math.PI * r;
  return (
    <div className="ring" role="progressbar" aria-valuenow={Math.round(percent)} aria-valuemin={0} aria-valuemax={100}>
      <svg viewBox="0 0 190 190">
        <defs><linearGradient id="ringg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#67e8f9" /><stop offset="1" stopColor="#6366f1" /></linearGradient></defs>
        <circle cx="95" cy="95" r={r} fill="none" stroke="rgba(120,170,255,.14)" strokeWidth="10" />
        <motion.circle cx="95" cy="95" r={r} fill="none" stroke="url(#ringg)" strokeWidth="10" strokeLinecap="round"
          strokeDasharray={c} animate={{ strokeDashoffset: c * (1 - Math.min(100, Math.max(0, percent)) / 100) }} transition={{ duration: 0.35 }}
          style={{ filter: "drop-shadow(0 0 8px rgba(34,211,238,.6))" }} />
      </svg>
      <div className="t"><b>{fmtPercent(percent)}</b><span>{label ?? "PROGRESSION"}</span></div>
    </div>
  );
}

/** Affiche exclusivement les valeurs fournies par le moteur : aucune progression inventee. */
export function ProgressPanel({ onCancel, busy }: { onCancel?: () => void; busy: boolean }) {
  const { progress: p } = useApp();
  if (!busy && !p) return null;
  const q = p ?? { phase: "Démarrage…", percent: 0, filesDone: 0, filesTotal: 0, bytesDone: 0, bytesTotal: 0, speedBps: 0, etaSecs: null, errors: 0, verified: 0, current: null, message: null, simulated: false };
  return (
    <motion.div className="card" initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }}>
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 14 }}>
        <div className="row"><span className="spinner" /> <b>{q.phase}</b>{q.simulated && <span className="pill sim">SIMULATION</span>}</div>
        <span className="pill ok"><span className="dot" /> Local · chiffré · aucun envoi réseau</span>
      </div>
      <div className="ring-wrap">
        <Ring percent={q.percent} />
        <div className="stats">
          <Stat k="Fichiers traités" v={q.filesTotal ? `${q.filesDone} / ${q.filesTotal}` : "—"} />
          <Stat k="Fichiers restants" v={q.filesTotal ? String(Math.max(0, q.filesTotal - q.filesDone)) : "—"} />
          <Stat k="Taille transférée" v={q.bytesTotal ? `${fmtBytes(q.bytesDone)} / ${fmtBytes(q.bytesTotal)}` : fmtBytes(q.bytesDone || null)} />
          <Stat k="Vitesse réelle" v={fmtSpeed(q.speedBps)} />
          <Stat k="Temps restant" v={fmtEta(q.etaSecs)} />
          <Stat k="Erreurs" v={String(q.errors)} />
          <Stat k="Vérifiés (SHA-256)" v={String(q.verified)} />
        </div>
      </div>
      {(q.current || q.message) && <p className="muted small mono" style={{ marginTop: 14, wordBreak: "break-all" }}>{q.message ?? q.current}</p>}
      {onCancel && <div style={{ marginTop: 10 }}><button className="btn danger" onClick={onCancel}>Annuler l'opération</button></div>}
    </motion.div>
  );
}

/** Volume par catégorie, exactement tel que déclaré par l'appareil. */
export function UsageBars({ device }: { device: DeviceInfo }) {
  const total = device.usedBytes ?? device.usage.reduce((a, c) => a + c.bytes, 0);
  if (device.usage.length === 0) {
    return <p className="muted small">Cet appareil n'a pas fourni le détail par catégorie{device.trusted ? "." : " : il doit d'abord approuver cet ordinateur."}</p>;
  }
  return (
    <div>
      {device.usage.map((c) => (
        <div key={c.key} className="cat-row" style={{ gridTemplateColumns: "minmax(150px, 1.4fr) 2fr 90px" }}>
          <span className="small">{c.label}</span>
          <div className="bar"><motion.i initial={{ width: 0 }} animate={{ width: `${total ? Math.min(100, (c.bytes / total) * 100) : 0}%` }} /></div>
          <span className="muted small mono" style={{ textAlign: "right" }}>{fmtBytes(c.bytes)}</span>
        </div>
      ))}
      <p className="muted small" style={{ marginTop: 8 }}>Total utilisé : <b>{fmtBytes(device.usedBytes)}</b></p>
    </div>
  );
}

export const Stat = ({ k, v }: { k: string; v: string }) => (<div className="stat"><small>{k}</small><b>{v}</b></div>);

export function PageHeader({ title, sub }: { title: string; sub: string }) {
  return (<><h1>{title}</h1><p className="sub">{sub}</p></>);
}

export function Check({ ok, children }: { ok: boolean | "warn"; children: ReactNode }) {
  return <span className={`pill ${ok === true ? "ok" : ok === "warn" ? "warn" : "err"}`}><span className="dot" />{children}</span>;
}

export function SuccessMark({ ok }: { ok: boolean }) {
  return ok ? (
    <svg className="success-mark" viewBox="0 0 90 90" aria-hidden="true"><circle cx="45" cy="45" r="41" /><path d="M27 46l12 12 25-27" /></svg>
  ) : (
    <svg className="warn-mark" viewBox="0 0 90 90" aria-hidden="true"><path d="M45 12 82 76H8z" fill="none" stroke="currentColor" strokeWidth="5" strokeLinejoin="round" /><path d="M45 36v20M45 64v3" stroke="currentColor" strokeWidth="5" strokeLinecap="round" /></svg>
  );
}

export const Icon = ({ d }: { d: string }) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={d} /></svg>
);
