// Every icon the UI uses, in one place: Solar icons in the soft, rounded "bold duotone" style, compiled into
// components at build time (vite.config.ts). Swapping the icon set means editing only this file.
import AppWindowIcon from "~icons/solar/window-frame-bold-duotone";
import ArchiveIcon from "~icons/solar/archive-bold-duotone";
import ArrowDownIcon from "~icons/solar/arrow-down-bold-duotone";
import ArrowRightIcon from "~icons/solar/arrow-right-bold-duotone";
import BoxIcon from "~icons/solar/box-bold-duotone";
import BroomIcon from "~icons/solar/broom-bold-duotone";
import BugIcon from "~icons/solar/bug-bold-duotone";
import CalendarIcon from "~icons/solar/calendar-bold-duotone";
import CaretDownIcon from "~icons/solar/alt-arrow-down-bold-duotone";
import CaretRightIcon from "~icons/solar/alt-arrow-right-bold-duotone";
import CheckCircleIcon from "~icons/solar/check-circle-bold-duotone";
import CheckIcon from "~icons/solar/check-bold-duotone";
import CheckSquareIcon from "~icons/solar/check-square-bold-duotone";
import ClipboardIcon from "~icons/solar/clipboard-list-bold-duotone";
import CloseCircleIcon from "~icons/solar/close-circle-bold-duotone";
import CloseIcon from "~icons/solar/close-bold-duotone";
import CodeIcon from "~icons/solar/code-square-bold-duotone";
import CpuIcon from "~icons/solar/cpu-bold-duotone";
import DangerIcon from "~icons/solar/danger-triangle-bold-duotone";
import DocumentTextIcon from "~icons/solar/document-text-bold-duotone";
import DownloadIcon from "~icons/solar/download-minimalistic-bold-duotone";
import FileIcon from "~icons/solar/file-bold-duotone";
import FileTextIcon from "~icons/solar/file-text-bold-duotone";
import FolderErrorIcon from "~icons/solar/folder-error-bold-duotone";
import FolderIcon from "~icons/solar/folder-bold-duotone";
import FolderOpenIcon from "~icons/solar/folder-open-bold-duotone";
import GalleryIcon from "~icons/solar/gallery-bold-duotone";
import GlobalIcon from "~icons/solar/global-bold-duotone";
import HammerIcon from "~icons/solar/sledgehammer-bold-duotone";
import HardDriveIcon from "~icons/solar/ssd-round-bold-duotone";
import HistoryIcon from "~icons/solar/history-bold-duotone";
import HomeIcon from "~icons/solar/home-smile-bold-duotone";
import InfoIcon from "~icons/solar/info-circle-bold-duotone";
import LayersIcon from "~icons/solar/layers-bold-duotone";
import ListCrossIcon from "~icons/solar/list-cross-minimalistic-bold-duotone";
import MagicStickIcon from "~icons/solar/magic-stick-3-bold-duotone";
import MagnifierIcon from "~icons/solar/magnifer-bold-duotone";
import PlayIcon from "~icons/solar/play-bold-duotone";
import ProgrammingIcon from "~icons/solar/programming-bold-duotone";
import RefreshIcon from "~icons/solar/refresh-bold-duotone";
import RestartIcon from "~icons/solar/restart-bold-duotone";
import ShieldCheckIcon from "~icons/solar/shield-check-bold-duotone";
import ShieldIcon from "~icons/solar/shield-bold-duotone";
import ShieldWarningIcon from "~icons/solar/shield-warning-bold-duotone";
import ShopIcon from "~icons/solar/shop-bold-duotone";
import SortAlphaIcon from "~icons/solar/sort-by-alphabet-bold-duotone";
import SortSizeIcon from "~icons/solar/sort-from-top-to-bottom-bold-duotone";
import SquareIcon from "~icons/solar/stop-linear";
import StopIcon from "~icons/solar/stop-bold-duotone";
import TrashIcon from "~icons/solar/trash-bin-trash-bold-duotone";
import WidgetIcon from "~icons/solar/widget-4-bold-duotone";
import type { IconComponent } from "./types";

export {
  AppWindowIcon as AppWindow,
  ArrowDownIcon as ArrowDown,
  ArrowRightIcon as ArrowRight,
  BoxIcon as Box,
  BroomIcon as Broom,
  CalendarIcon as Calendar,
  CaretDownIcon as CaretDown,
  CaretRightIcon as CaretRight,
  CheckCircleIcon as CheckCircle,
  CheckIcon as Check,
  CheckSquareIcon as CheckSquare,
  ClipboardIcon as Clipboard,
  ProgrammingIcon as Code,
  DownloadIcon as Download,
  FileIcon as File,
  FolderIcon as Folder,
  FolderOpenIcon as FolderOpen,
  HardDriveIcon as HardDrive,
  HomeIcon as House,
  InfoIcon as Info,
  ListCrossIcon as ListRemove,
  MagnifierIcon as Search,
  PlayIcon as Play,
  RefreshIcon as Rescan,
  RestartIcon as Undo,
  ShieldCheckIcon as ShieldCheck,
  ShieldIcon as Shield,
  ShieldWarningIcon as ShieldWarning,
  SortSizeIcon as SortBySize,
  SortAlphaIcon as SortByName,
  LayersIcon as SortByGroup,
  MagicStickIcon as Sparkle,
  SquareIcon as Square,
  WidgetIcon as SpaceUsage,
  StopIcon as Stop,
  ShopIcon as Store,
  TrashIcon as Trash,
  DangerIcon as Warning,
  CloseCircleIcon as XCircle,
  CloseIcon as X,
};

/** Sidebar and table icon for each scanner category (keys are Rust's `Category` variant names). */
const CATEGORY_ICONS: Record<string, IconComponent> = {
  RecycleBin: TrashIcon,
  UserTemp: HistoryIcon,
  SystemTemp: HistoryIcon,
  WindowsUpdate: DownloadIcon,
  CrashDumps: BugIcon,
  WindowsLogs: DocumentTextIcon,
  OldWindows: ArchiveIcon,
  BrowserCache: GlobalIcon,
  AppCache: AppWindowIcon,
  ThumbnailCache: GalleryIcon,
  GpuCache: CpuIcon,
  DevCache: BoxIcon,
  NodeModules: CodeIcon,
  BuildArtifacts: HammerIcon,
  PythonEnv: ProgrammingIcon,
  LogFiles: FileTextIcon,
  LargeFiles: FileIcon,
  Custom: FolderIcon,
  AppLeftovers: FolderErrorIcon,
  AppUpdates: RefreshIcon,
  AppData: RestartIcon,
};

export function categoryIcon(category: string): IconComponent {
  return CATEGORY_ICONS[category] ?? FolderIcon;
}
