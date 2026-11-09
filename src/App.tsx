import { useCallback, useState } from "react";
import { AnimatePresence, MotionConfig, motion } from "framer-motion";
import { AppCtx, PageId, Particles, useProvider } from "./ui";
import Transfer from "./pages/Transfer";
import Diagnostics from "./pages/Diagnostics";
import Settings from "./pages/Settings";

export default function App() {
  const [page, setPage] = useState<PageId>("transfer");
  const go = useCallback((p: PageId) => setPage(p), []);
  const { ctx, overlay } = useProvider(go);
  const Current = { transfer: Transfer, diagnostics: Diagnostics, settings: Settings }[page];
  return (
    <AppCtx.Provider value={ctx}>
      <MotionConfig reducedMotion={ctx.settings?.reduceMotion ? "always" : "user"}>
        <div className="bg" /><Particles />
        <div className="shell">
          <header className="topbar">
            <div className="brand"><img src="/logo.svg" alt="" /><div><b>GHOST TRANSFERT</b><span>APPLE</span></div></div>
            <div className="row">
              {ctx.status?.simulation && <span className="pill sim">MODE SIMULATION</span>}
              {page !== "transfer" ? <button className="btn" onClick={() => setPage("transfer")}>← Retour</button> : (
                <>
                  <button className="btn" onClick={() => setPage("diagnostics")}>Diagnostic</button>
                  <button className="btn" onClick={() => setPage("settings")}>Paramètres</button>
                </>
              )}
            </div>
          </header>
          <main>
            <AnimatePresence mode="wait">
              <motion.div key={page} initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={{ duration: 0.2 }}>
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
