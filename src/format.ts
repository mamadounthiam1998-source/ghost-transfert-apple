export function fmtBytes(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "—";
  const u = ["o", "Ko", "Mo", "Go", "To"];
  let i = 0;
  let v = n;
  while (v >= 1000 && i < u.length - 1) { v /= 1000; i++; }
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${u[i]}`;
}
export const fmtSpeed = (bps: number) => (bps > 0 ? `${fmtBytes(bps)}/s` : "—");
export function fmtEta(s: number | null | undefined): string {
  if (s == null) return "—";
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.floor(s / 60)} min ${s % 60} s`;
  return `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
}
export function fmtPercent(p: number): string {
  const c = Math.max(0, Math.min(100, p));
  return `${c.toFixed(c >= 99.95 || c === 0 ? 0 : 1)} %`;
}
export function fmtDate(iso: string): string {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString("fr-FR", { dateStyle: "medium", timeStyle: "short" });
}

/** Temps restant estimé (s) à partir du temps écoulé et du pourcentage réel ; null tant que c'est trop tôt. */
export function etaFrom(elapsedMs: number, percent: number): number | null {
  if (percent < 2 || percent >= 100 || elapsedMs < 3000) return null;
  return Math.round((elapsedMs / 1000) * ((100 - percent) / percent));
}
