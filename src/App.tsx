import { useCallback, useState } from "react";
import { AnimatePresence, MotionConfig, motion } from "framer-motion";
import { AppCtx, Icon, PageId, Particles, useProvider } from "./ui";
import Home from "./pages/Home";
import Connect from "./pages/Connect";
import Clone from "./pages/Clone";
import Smart from "./pages/Smart";
import Analysis from "./pages/Analysis";
import Backups from "./pages/Backups";
import Restore from "./pages/Restore";
import History from "./pages/History";
import Security from "./pages/Security";
import Diagnostics from "./pages/Diagnostics";
import Settings from "./pages/Settings";

const NAV: { id: PageId; label: string; icon: string; el: () => JSX.Element }[] = [
  { id: "home", label: "Accueil", icon: "M3 11l9-8 9 8v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z", el: Home },
  { id: "connect", label: "Connexion des appareils", icon: "M8 4v6a4 4 0 0 0 8 0V4M10 4V2m4 2V2M12 14v8", el: Connect },
  { id: "clone", label: "Ghost Clone", icon: "M9 9h11v11H9zM5 15H4V4h11v1", el: Clone },
  { id: "smart", label: "Transfert intelligent", icon: "M4 12h13M13 6l6 6-6 6", el: Smart },
  { id: "analysis", label: "Analyse", icon: "M4 20V10m6 10V4m6 16v-7m4 7H2", el: Analysis },
  { id: "backups", label: "Sauvegardes", icon: "M4 7c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3zm0 0v10c0 1.7 3.6 3 8 3s8-1.3 8-3V7", el: Backups },
  { id: "restore", label: "Restaurations", icon: "M3 12a9 9 0 1 0 3-6.7M3 4v5h5", el: Restore },
  { id: "history", label: "Historique", icon: "M12 7v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z", el: History },
  { id: "security", label: "Sécurité", icon: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z", el: Security },
  { id: "diagnostics", label: "Diagnostic", icon: "M3 12h4l3-8 4 16 3-8h4", el: Diagnostics },
  { id: "settings", label: "Paramètres", icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19 12l2-1-2-4-2 1a7 7 0 0 0-2-1l-.5-2h-4L10 7a7 7 0 0 0-2 1L6 7l-2 4 2 1a7 7 0 0 0 0 2l-2 1 2 4 2-1a7 7 0 0 0 2 1l.5 2h4L14 19a7 7 0 0 0 2-1l2 1 2-4-2-1a7 7 0 0 0 0-2z", el: Settings },
];

export default function App() {
  const [page, setPage] = useState<PageId>("home");
  const go = useCallback((p: PageId) => setPage(p), []);
  const { ctx, overlay } = useProvider(go);
  const Current = NAV.find((n) => n.id === page)!.el;
  return (
    <AppCtx.Provider value={ctx}>
      <MotionConfig reducedMotion={ctx.settings?.reduceMotion ? "always" : "user"}>
        <div className="bg" /><Particles />
        <div className="app">
          <nav className="sidebar" aria-label="Navigation principale">
            <div className="brand"><img src="/logo.svg" alt="" /><div><b>GHOST<br />TRANSFERT</b><span>APPLE</span></div></div>
            {NAV.map((n) => (
              <button key={n.id} className={`nav ${page === n.id ? "active" : ""}`} onClick={() => setPage(n.id)} aria-current={page === n.id ? "page" : undefined}>
                <Icon d={n.icon} />{n.label}
              </button>
            ))}
            <div className="grow" />
            <div className={`badge-mode ${ctx.status?.simulation ? "sim" : ""}`}>
              {ctx.status?.simulation ? "MODE SIMULATION — appareils fictifs" : ctx.status?.toolsOk ? "Outils Apple prêts" : "Outils Apple manquants (voir Diagnostic)"}
            </div>
          </nav>
          <main>
            <AnimatePresence mode="wait">
              <motion.div key={page} initial={{ opacity: 0, y: 14, filter: "blur(4px)" }} animate={{ opacity: 1, y: 0, filter: "blur(0px)" }} exit={{ opacity: 0, y: -8 }} transition={{ duration: 0.22 }}>
                <Current />
              </motion.div>
            </AnimatePresence>
          </main>
        </div>
        {overlay}
      </MotionConfig>
    </AppCtx.Provider>
  );
}
