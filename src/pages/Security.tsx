import { useEffect, useState } from "react";
import { api, Check as C } from "../api";
import { Check, PageHeader } from "../ui";

const FEATURES = [
  ["Chiffrement AES-256-GCM", "Archives exportées chiffrées par blocs de 1 Mio, authentifiées (altération et troncature détectées)."],
  ["Dérivation Argon2id", "64 Mio de mémoire, 3 itérations ; les paramètres sont lus dans l'en-tête et bornés."],
  ["Intégrité SHA-256", "Chaque fichier transféré et chaque sauvegarde est contrôlé par empreinte."],
  ["Traversées de répertoires", "Tous les chemins venant d'une archive ou d'un plan sont validés avant écriture."],
  ["Pas de shell", "Les outils Apple sont lancés argument par argument ; les identifiants d'appareil sont validés."],
  ["Local uniquement", "Aucune télémétrie, aucun envoi réseau. La politique de sécurité de la fenêtre interdit les connexions externes."],
  ["Journaux anonymisés", "UDID, e-mails, numéros et noms de profil sont caviardés avant écriture."],
  ["Effacement des temporaires", "Écrasement puis suppression. Sur SSD/APFS, l'effacement physique n'est pas garanti."],
];

export default function Security() {
  const [r, setR] = useState<C[]>([]);
  const run = () => api.selftest().then(setR).catch(() => {});
  useEffect(() => { void run(); }, []);
  return (
    <div className="page">
      <PageHeader title="Ghost Security" sub="Ce qui protège vos données, et un auto-test que vous pouvez relancer à tout moment." />
      <div className="card">
        <div className="row" style={{ justifyContent: "space-between" }}><h2>Auto-test de sécurité</h2><button className="btn" onClick={run}>Relancer</button></div>
        {r.map((c) => <div key={c.id} className="row" style={{ padding: "6px 0" }}><Check ok={c.status === "ok"}>{c.status === "ok" ? "OK" : "ÉCHEC"}</Check><b>{c.name}</b><span className="muted small">{c.detail}</span></div>)}
      </div>
      <div className="grid g2" style={{ marginTop: 16 }}>{FEATURES.map(([t, d]) => <div key={t} className="card"><b>{t}</b><p className="muted small">{d}</p></div>)}</div>
    </div>
  );
}
