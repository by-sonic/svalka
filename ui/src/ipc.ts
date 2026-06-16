import { invoke } from "@tauri-apps/api/core";

// The "Поддержать" button opens this in the default browser via open_url.
export const BOOSTY_URL = "https://boosty.to/bysonicx";

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
  /** real_size came from the registry estimate, not a real folder walk */
  estimated: boolean;
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
export interface SkippedItem {
  path: string;
  reason: string;
}
export interface QuarantineOutcome {
  ok: boolean;
  message: string;
  freed: number;
  moved: number;
  skipped: SkippedItem[];
}
export interface UsageNode {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
  protected: boolean;
  children: UsageNode[];
}
export interface LargeFile {
  path: string;
  size: number;
  protected: boolean;
}
export interface UsageReport {
  root: string;
  total: number;
  free: number;
  volume_total: number;
  tree: UsageNode;
  largest: LargeFile[];
}

export const api = {
  scan: (categories: CategoryId[]) => invoke<Category[]>("scan", { categories }),
  getPrograms: () => invoke<Program[]>("get_programs"),
  quarantineItems: (source: string, volumeRoot: string, paths: string[]) =>
    invoke<QuarantineOutcome>("quarantine_items", {
      source,
      volumeRoot,
      paths,
    }),
  listQuarantine: () => invoke<QuarantineEntry[]>("list_quarantine"),
  restoreEntry: (id: string) => invoke<void>("restore_entry", { id }),
  deleteEntry: (id: string) => invoke<void>("delete_entry", { id }),
  clearQuarantine: () => invoke<number>("clear_quarantine"),
  uninstallPrograms: (programs: Program[]) =>
    invoke<UninstallResult[]>("uninstall_programs", { programs }),
  findLeftovers: (program: Program) =>
    invoke<Leftover[]>("find_leftovers_cmd", { program }),
  analyzeDisk: (root: string) => invoke<UsageReport>("analyze_disk", { root }),
  listDrives: () => invoke<string[]>("list_drives"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
};
