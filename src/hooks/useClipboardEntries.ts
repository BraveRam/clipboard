import { useCallback, useEffect, useMemo, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import { fuzzyMatch } from "../lib/fuzzy";
import type { Entry } from "../lib/types";

export interface FilteredEntry {
  entry: Entry;
  matchIndices: number[];
  score: number;
}

export function useClipboardEntries(query: string) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const list = await api.list();
      setEntries(list);
    } catch (e) {
      console.error("list entries", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const unlisteners: UnlistenFn[] = [];
    (async () => {
      unlisteners.push(
        await listen("clipboard:entry-captured", () => refresh()),
      );
      unlisteners.push(
        await listen("clipboard:entries-changed", () => refresh()),
      );
    })();
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [refresh]);

  const filtered = useMemo<{
    pinned: FilteredEntry[];
    recent: FilteredEntry[];
  }>(() => {
    const pinned: FilteredEntry[] = [];
    const recent: FilteredEntry[] = [];
    const q = query.trim();

    for (const entry of entries) {
      const searchTarget = entry.text ?? `image ${entry.width ?? ""}x${entry.height ?? ""}`;
      let matchIndices: number[] = [];
      let score = 0;
      if (q.length > 0) {
        const m = fuzzyMatch(searchTarget, q);
        if (!m) continue;
        matchIndices = m.indices;
        score = m.score;
      } else {
        score = entry.lastUsedAt;
      }
      const wrapped: FilteredEntry = { entry, matchIndices, score };
      if (entry.pinned) pinned.push(wrapped);
      else recent.push(wrapped);
    }

    if (q.length > 0) {
      pinned.sort((a, b) => b.score - a.score);
      recent.sort((a, b) => b.score - a.score);
    } else {
      pinned.sort((a, b) => b.entry.lastUsedAt - a.entry.lastUsedAt);
      recent.sort((a, b) => b.entry.lastUsedAt - a.entry.lastUsedAt);
    }
    return { pinned, recent };
  }, [entries, query]);

  return { entries, filtered, loading, refresh };
}
