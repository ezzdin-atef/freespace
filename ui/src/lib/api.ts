// Typed wrappers over the Rust commands in src/commands.rs. This is the only file that knows command names.
import { invoke } from "@tauri-apps/api/core";

export interface Drive {
  root: string;
  kind: string;
  total: number;
  free: number;
}

export interface Init {
  drives: Drive[];
  is_admin: boolean;
}

export const init = () => invoke<Init>("init");
