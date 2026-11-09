import { api } from "../api";
import { useApp } from "../ui";

export default function Settings() {
  const { settings, updateSettings, confirm, refreshStatus, refreshDevices } = useApp();
  if (!settings) return <div className="page"><p className="muted">Chargement…</p></div>;
  const toggleSim = async (on: boolean) => {
    if (on && !(await confirm("Activer le mode simulation ?", "Des iPhone FICTIFS seront affichés pour tester le logiciel. Rien ne sera écrit sur un vrai téléphone.", false))) return;
    await api.setSimulation(on); await refreshStatus(); await refreshDevices();
  };
  return (
    <div className="page" style={{ maxWidth: 560 }}>
      <h1>Paramètres</h1>
      <div className="card col">
        <div className="row"><b style={{ flex: 1 }}>Thème</b>
          <button className={`chip ${settings.theme === "dark" ? "on" : ""}`} onClick={() => updateSettings({ ...settings, theme: "dark" })}>Sombre</button>
          <button className={`chip ${settings.theme === "light" ? "on" : ""}`} onClick={() => updateSettings({ ...settings, theme: "light" })}>Clair</button></div>
        <label className="check"><input type="checkbox" checked={settings.reduceMotion} onChange={(e) => updateSettings({ ...settings, reduceMotion: e.target.checked })} /> Réduire les animations</label>
        <label className="check"><input type="checkbox" checked={settings.simulation} onChange={(e) => toggleSim(e.target.checked)} /> Mode simulation (iPhone fictifs, pour tester)</label>
      </div>
    </div>
  );
}
