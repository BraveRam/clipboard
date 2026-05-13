import { invoke } from "@tauri-apps/api/core";
import type { Entry } from "./types";

export const api = {
  list: () => invoke<Entry[]>("entries_list"),
  paste: (id: number) => invoke<boolean>("entry_paste", { id }),
  togglePin: (id: number) => invoke<boolean>("entry_pin_toggle", { id }),
  delete: (id: number) => invoke<void>("entry_delete", { id }),
  clearUnpinned: () => invoke<void>("entries_clear_unpinned"),
  hide: () => invoke<void>("overlay_hide"),
  show: () => invoke<void>("overlay_show"),
};
