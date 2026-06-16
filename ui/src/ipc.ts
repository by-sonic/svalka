import { invoke } from "@tauri-apps/api/core";

export type CategoryId =
  | "SystemTemp"
  | "RecycleBin"
  | "ThumbnailCache"
  | "CrashDumps"
  | "Wer"
  | "WindowsUpdate"
  | "BrowserCache"
  | "DevCache";

export interface CleanItem {
  kind: "File" | "Dir";
  path: string;
  size: number;
}
export interface Category {
  id: CategoryId;
  label: string;
  items: CleanItem[];
}
export interface Program {
  name: string;
  publisher: string | null;
  install_location: string | null;
  uninstall: string | null;
  quiet_uninstall: string | null;
  real_size: number;
  source: "Registry" | "Uwp";
}
export interface MovedFile {
  original: string;
  stored: string;
  size: number;
}
export interface RegBackup {
  hive: string;
  subkey: string;
  reg_file: string;
}
export interface QuarantineEntry {
  id: string;
  created: number;
  source: string;
  store_dir: string;
  files: MovedFile[];
  reg: RegBackup[];
}
export interface Leftover {
  kind: "File" | "Dir";
  path: string;
  size: number;
}
export interface UninstallResult {
  program: string;
  ok: boolean;
  message: string;
}

export const api = {
  scan: (categories: CategoryId[]) => invoke<Category[]>("scan", { categories }),
  getPrograms: () => invoke<Program[]>("get_programs"),
  quarantineItems: (source: string, volumeRoot: string, paths: string[]) =>
    invoke<{ ok: boolean; message: string; freed: number }>("quarantine_items", {
      source,
      volumeRoot,
      paths,
    }),
  listQuarantine: () => invoke<QuarantineEntry[]>("list_quarantine"),
  restoreEntry: (id: string) => invoke<void>("restore_entry", { id }),
  uninstallPrograms: (programs: Program[]) =>
    invoke<UninstallResult[]>("uninstall_programs", { programs }),
  findLeftovers: (program: Program) =>
    invoke<Leftover[]>("find_leftovers_cmd", { program }),
};
