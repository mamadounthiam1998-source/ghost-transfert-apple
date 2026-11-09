import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

export type Category =
  | "photos" | "videos" | "audio" | "documents" | "contacts" | "calendar" | "messages" | "settings" | "other";

export const CATEGORY_LABEL: Record<Category, string> = {
  photos: "Photos", videos: "Vidéos", audio: "Audio", documents: "Documents", contacts: "Contacts",
  calendar: "Calendrier", messages: "Messages", settings: "Réglages", other: "Autres",
};

export interface UsageCategory { key: string; label: string; bytes: number }
export interface DeviceInfo {
  udid: string; name: string; model: string; productType: string; osVersion: string;
  connected: boolean; trusted: boolean;
  capacityBytes: number | null; usedBytes: number | null; freeBytes: number | null;
  usage: UsageCategory[]; simulated: boolean; operations: string[]; note: string | null;
}
export interface Compatibility { ok: boolean; warnings: string[]; blockers: string[] }
export interface AppStatus {
  simulation: boolean; toolsOk: boolean; missingTools: string[]; version: string;
  platform: string; backupsDir: string; busy: boolean;
}
export interface Settings { backupsDir: string | null; theme: "dark" | "light"; reduceMotion: boolean; simulation: boolean }
export interface Progress {
  phase: string; percent: number; filesDone: number; filesTotal: number; bytesDone: number; bytesTotal: number;
  speedBps: number; etaSecs: number | null; errors: number; verified: number;
  current: string | null; message: string | null; simulated: boolean;
}
export interface CategoryStat { files: number; bytes: number }
export interface AnalysisReport {
  root: string; totalFiles: number; totalBytes: number; categories: Record<string, CategoryStat>;
  suspicious: string[]; symlinksIgnored: number;
}
export interface BackupInfo {
  hid: string; version: string; sourceUdid: string; created: string; deviceName: string; osVersion: string;
  encrypted: boolean; simulated: boolean; files: number; bytes: number; newOrChanged: number; dataDir: string;
}
export interface VerifyReport { checked: number; ok: number; missing: string[]; modified: string[]; intact: boolean }
export interface CloneReport {
  backup: BackupInfo; integrity: VerifyReport; restored: boolean; notes: string[]; success: boolean; simulated: boolean;
}
export interface ItemIssue { rel: string; reason: string }
export interface TransferReport {
  plannedFiles: number; copied: number; alreadyPresent: number; duplicatesSkipped: number; renamed: number;
  verifiedOk: number; verifiedFailed: number; bytesCopied: number; notTransferred: ItemIssue[]; errors: ItemIssue[];
  durationMs: number; success: boolean;
}
export interface PlanSummary { files: number; bytes: number; freeBytes: number; enoughSpace: boolean }
export interface HistoryEntry {
  id: string; kind: string; started: string; finished: string;
  status: "success" | "partial" | "failed" | "cancelled"; summary: string; simulated: boolean;
}
export interface Check { id: string; name: string; status: "ok" | "warn" | "error"; detail: string; solution: string | null }
export type ConflictPolicy = "skip" | "rename" | "overwrite";

export const api = {
  status: () => invoke<AppStatus>("app_status"),
  setSimulation: (enabled: boolean) => invoke<void>("set_simulation", { enabled }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  devices: () => invoke<DeviceInfo[]>("list_devices"),
  pair: (udid: string) => invoke<string>("pair_device", { udid }),
  compat: (sourceUdid: string, destUdid: string) => invoke<Compatibility>("check_compat", { sourceUdid, destUdid }),
  analyzeFolder: (path: string) => invoke<AnalysisReport>("analyze_folder", { path }),
  analyzeBackup: (hid: string) => invoke<AnalysisReport>("analyze_backup", { hid }),
  backupCreate: (udid: string, password: string | null) => invoke<BackupInfo>("backup_create", { udid, password }),
  backupList: () => invoke<BackupInfo[]>("backup_list"),
  backupVerify: (hid: string, version: string) => invoke<VerifyReport>("backup_verify", { hid, version }),
  backupDelete: (hid: string) => invoke<void>("backup_delete", { hid, confirm: true }),
  backupExport: (hid: string, outPath: string, password: string) => invoke<void>("backup_export", { hid, outPath, password }),
  backupImport: (path: string, password: string, overwrite: boolean) => invoke<string>("backup_import", { path, password, overwrite }),
  restore: (req: { udid: string; hid: string; system: boolean; settings: boolean; reboot: boolean; password: string | null; confirmErase: boolean }) =>
    invoke<void>("restore_backup", { req }),
  clone: (req: { sourceUdid: string; destUdid: string; mode: string; system: boolean; settings: boolean; password: string | null; confirmErase: boolean }) =>
    invoke<CloneReport>("clone_start", { req }),
  smartPlan: (source: string, dest: string, categories: Category[] | null) => invoke<PlanSummary>("smart_plan", { source, dest, categories }),
  smartTransfer: (req: { source: string; dest: string; categories: Category[] | null; dedupe: boolean; conflict: ConflictPolicy; confirmOverwrite: boolean; verify: boolean }) =>
    invoke<TransferReport>("smart_transfer", { req }),
  cancel: () => invoke<void>("cancel_operation"),
  history: () => invoke<HistoryEntry[]>("history_list", { limit: 200 }),
  historyClear: () => invoke<void>("history_clear", { confirm: true }),
  diagnostics: () => invoke<Check[]>("diagnostics_run"),
  selftest: () => invoke<Check[]>("security_selftest"),
  exportDiagnostics: (outPath: string) => invoke<void>("diagnostics_export", { outPath }),
  verifyUpdate: (path: string, sha256: string, signature: string, publicKey: string) =>
    invoke<void>("verify_update", { path, sha256, signature, publicKey }),
};

export const onProgress = (cb: (p: Progress) => void) => listen<Progress>("ghost-progress", (e) => cb(e.payload));

export async function pickFolder(title: string): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === "string" ? r : null;
}
export async function pickFile(title: string): Promise<string | null> {
  const r = await open({ directory: false, multiple: false, title });
  return typeof r === "string" ? r : null;
}
export async function pickSave(title: string, name: string): Promise<string | null> {
  return (await save({ title, defaultPath: name })) ?? null;
}

export const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));
