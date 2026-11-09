import { useState } from "react";
import { api, errText, pickFile, pickFolder } from "../api";
import { PageHeader, useApp } from "../ui";

export default function Settings() {
  const { settings, updateSettings, status, confirm, toast, refreshStatus, refreshDevices } = useApp();
  const [upd, setUpd] = useState({ path: "", sha: "", sig: "", key: "" });
  if (!settings) return <div className="page"><PageHeader title="Paramètres" sub="" /><p className="muted">Chargement…</p></div>;
  const toggleSim = async (on: boolean) => {
    if (on && !(await confirm("Activer le mode simulation ?", "Des appareils FICTIFS seront affichés. Rien ne sera écrit sur un vrai téléphone. Toutes les opérations resteront marquées « SIMULATION ».", false))) return;
    await api.setSimulation(on); await refreshStatus(); await refreshDevices();
  };
  return (
    <div className="page">
      <PageHeader title="Paramètres" sub="Apparence, stockage, mode simulation et vérification des mises à jour." />
      <div className="grid g2">
        <div className="card"><h2>Apparence</h2>
          <div className="row"><button className={`chip ${settings.theme === "dark" ? "on" : ""}`} onClick={() => updateSettings({ ...settings, theme: "dark" })}>Sombre</button>
            <button className={`chip ${settings.theme === "light" ? "on" : ""}`} onClick={() => updateSettings({ ...settings, theme: "light" })}>Clair</button></div>
          <label className="check" style={{ marginTop: 14 }}><input type="checkbox" checked={settings.reduceMotion} onChange={(e) => updateSettings({ ...settings, reduceMotion: e.target.checked })} /> Réduire les animations (le réglage du système est aussi respecté)</label></div>
        <div className="card"><h2>Sauvegardes</h2><p className="muted small mono" style={{ wordBreak: "break-all" }}>{status?.backupsDir}</p>
          <button className="btn" onClick={async () => { const p = await pickFolder("Dossier des sauvegardes"); if (p) await updateSettings({ ...settings, backupsDir: p }); }}>Changer le dossier</button></div>
        <div className="card"><h2>Mode simulation</h2>
          <p className="muted small">Pour tester l'interface et les moteurs sans appareil physique. Jamais activé automatiquement.</p>
          <label className="check"><input type="checkbox" checked={settings.simulation} onChange={(e) => toggleSim(e.target.checked)} /> Utiliser des appareils fictifs</label></div>
        <div className="card"><h2>Mises à jour</h2>
          <p className="muted small">Vérification d'un paquet : empreinte SHA-256 + signature Ed25519. Un paquet non vérifié n'est jamais installé.</p>
          <div className="col">
            <div className="row"><input type="text" placeholder="Fichier du paquet" value={upd.path} onChange={(e) => setUpd({ ...upd, path: e.target.value })} /><button className="btn" onClick={async () => { const f = await pickFile("Paquet de mise à jour"); if (f) setUpd({ ...upd, path: f }); }}>…</button></div>
            <input type="text" placeholder="SHA-256 annoncé" value={upd.sha} onChange={(e) => setUpd({ ...upd, sha: e.target.value })} />
            <input type="text" placeholder="Signature (base64)" value={upd.sig} onChange={(e) => setUpd({ ...upd, sig: e.target.value })} />
            <input type="text" placeholder="Clé publique de l'éditeur (base64)" value={upd.key} onChange={(e) => setUpd({ ...upd, key: e.target.value })} />
            <button className="btn" onClick={async () => { try { await api.verifyUpdate(upd.path, upd.sha, upd.sig, upd.key); toast("Paquet authentique et intact"); } catch (e) { toast(errText(e), "err"); } }}>Vérifier le paquet</button>
          </div></div>
      </div>
    </div>
  );
}
