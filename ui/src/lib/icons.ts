import {
  AppWindow,
  Archive,
  Box,
  Bug,
  Code,
  Cpu,
  Download,
  File,
  FileText,
  Folder,
  FolderX,
  Globe,
  Hammer,
  History,
  Image,
  Package,
  ScrollText,
  Trash2,
} from "@lucide/svelte";
import type { IconComponent } from "./types";

/** Sidebar and table icon for each scanner category (keys are Rust's `Category` variant names). */
const CATEGORY_ICONS: Record<string, IconComponent> = {
  RecycleBin: Trash2,
  UserTemp: History,
  SystemTemp: History,
  WindowsUpdate: Download,
  CrashDumps: Bug,
  WindowsLogs: ScrollText,
  OldWindows: Archive,
  BrowserCache: Globe,
  AppCache: AppWindow,
  ThumbnailCache: Image,
  GpuCache: Cpu,
  DevCache: Package,
  NodeModules: Box,
  BuildArtifacts: Hammer,
  PythonEnv: Code,
  LogFiles: FileText,
  LargeFiles: File,
  Custom: Folder,
  AppLeftovers: FolderX,
};

export function categoryIcon(category: string): IconComponent {
  return CATEGORY_ICONS[category] ?? Folder;
}
