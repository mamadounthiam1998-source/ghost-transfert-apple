import { useState } from "react";
import { api, Category, CATEGORY_LABEL, ConflictPolicy, errText, pickFolder, PlanSummary, TransferReport } from "../api";
import { fmtBytes } from "../format";
import { Check, PageHeader, ProgressPanel, SuccessMark, useApp, useOp } from "../ui";

const CATS = Object.keys(CATEGORY_LABEL) as Category[];

export default function Smart() {
  const { confirm, go } = useApp();
  const [src, setSrc] = useState(""); const [dst, setDst] = useState("");
  const [cats, setCats] = useState<Category[]>([]);
  const [dedupe, setDedupe] = useState(true); const [verify, setVerify] = useState(true);
  const [conflict, setConflict] = useState<ConflictPolicy>("skip");
  const [plan, setPlan] = useState<PlanSummary | null>(null);
  const [perr, setPerr] = useState<string | null>(null);
  const op = useOp<TransferReport>();
  const sel = cats.length ? cats : null;
  const preview = async () => { setPerr(null); setPlan(null); try { setPlan(await api.smartPlan(src, dst, sel)); } catch (e) { setPerr(errText(e)); } };
  const start = async () => {
    let confirmOverwrite = false;
    if (conflict === "overwrite") {
      if (!(await confirm("Écraser les fichiers existants ?", "Les fichiers différents déjà présents à la destination seront remplacés.", true))) return;
      confirmOverwrite = true;
    }
    await op.run(() => api.smartTransfer({ source: src, dest: dst, categories: sel, dedupe, conflict, confirmOverwrite, verify }));
  };
  const r = op.result;
  const full = r && r.success;
  return (
    <div className="page">
      <PageHeader title="Transfert intelligent" sub="Copie vérifiée d'un dossier vers un autre (export de photos, dossier monté, sauvegarde extraite). Chaque fichier est copié de façon atomique, comparé par SHA-256, et le transfert peut être repris." />
      <div className="card">
        <div className="grid g2">
          <div><label className="small muted">Dossier source</label><div className="row"><input type="text" value={src} onChange={(e) => setSrc(e.target.value)} /><button className="btn" onClick={async () => { const p = await pickFolder("Dossier source"); if (p) setSrc(p); }}>Parcourir</button></div></div>
          <div><label className="small muted">Dossier destination</label><div className="row"><input type="text" value={dst} onChange={(e) => setDst(e.target.value)} /><button className="btn" onClick={async () => { const p = await pickFolder("Dossier destination"); if (p) setDst(p); }}>Parcourir</button></div></div>
        </div>
        <h2 style={{ marginTop: 18 }}>Catégories <span className="muted small">(aucune = tout)</span></h2>
        <div className="row">{CATS.map((c) => <button key={c} className={`chip ${cats.includes(c) ? "on" : ""}`} onClick={() => setCats((x) => x.includes(c) ? x.filter((y) => y !== c) : [...x, c])}>{CATEGORY_LABEL[c]}</button>)}</div>
        <div className="grid g3" style={{ marginTop: 18 }}>
          <label className="check"><input type="checkbox" checked={dedupe} onChange={(e) => setDedupe(e.target.checked)} /> Éviter les doublons (contenu identique)</label>
          <label className="check"><input type="checkbox" checked={verify} onChange={(e) => setVerify(e.target.checked)} /> Vérifier chaque fichier (SHA-256)</label>
          <div><label className="small muted">En cas de conflit</label>
            <select value={conflict} onChange={(e) => setConflict(e.target.value as ConflictPolicy)}>
              <option value="skip">Ne pas toucher (signalé)</option><option value="rename">Garder les deux (renommer)</option><option value="overwrite">Écraser (confirmation)</option></select></div>
        </div>
        <div className="row" style={{ marginTop: 18 }}>
          <button className="btn" disabled={!src || !dst || op.busy} onClick={preview}>Prévisualiser</button>
          <button className="btn primary" disabled={!src || !dst || op.busy || (plan !== null && !plan.enoughSpace)} onClick={start}>LANCER LE TRANSFERT</button>
          {op.busy && <button className="btn danger" onClick={() => api.cancel()}>Annuler</button>}
        </div>
        {perr && <div className="alert err" style={{ marginTop: 12 }}>{perr}</div>}
        {plan && <div className={`alert ${plan.enoughSpace ? "info" : "err"}`} style={{ marginTop: 12 }}>
          {plan.files} fichiers · {fmtBytes(plan.bytes)} à copier · {fmtBytes(plan.freeBytes)} libres à la destination
          {!plan.enoughSpace && " — espace insuffisant, le transfert est bloqué."}</div>}
      </div>
      <div style={{ marginTop: 16 }}><ProgressPanel busy={op.busy} /></div>
      {op.error && <div className="alert err" style={{ marginTop: 16 }}>{op.error}</div>}
      {r && (
        <div className="card" style={{ marginTop: 16 }}>
          <div className="row" style={{ gap: 20 }}><SuccessMark ok={!!full} />
            <div><h2>{full ? "Transfert terminé et vérifié" : "Transfert terminé avec des éléments non traités"}</h2>
              <div className="row"><Check ok={r.errors.length === 0 && r.verifiedFailed === 0}>{r.errors.length} erreur(s)</Check><Check ok={r.notTransferred.length === 0 ? true : "warn"}>{r.notTransferred.length} non transféré(s)</Check></div></div></div>
          <div className="grid g4" style={{ marginTop: 14 }}>
            {([["Copiés", r.copied], ["Déjà présents", r.alreadyPresent], ["Doublons ignorés", r.duplicatesSkipped], ["Vérifiés OK", r.verifiedOk]] as const).map(([k, v]) => (
              <div key={k} className="stat"><small>{k}</small><b>{v}</b></div>))}
          </div>
          {[...r.errors.map((i) => ({ ...i, e: true })), ...r.notTransferred.map((i) => ({ ...i, e: false }))].slice(0, 30).map((i) => (
            <p key={i.rel + i.reason} className={`small ${i.e ? "" : "muted"}`} style={{ color: i.e ? "var(--err)" : undefined }}>{i.rel} — {i.reason}</p>))}
          <button className="btn" onClick={() => go("history")}>Ouvrir l'historique</button>
        </div>
      )}
    </div>
  );
}
