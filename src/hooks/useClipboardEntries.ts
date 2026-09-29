import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import { fuzzyMatch } from "../lib/fuzzy";
import { subscriptionScope } from "../lib/subscriptions";
import { normalizeWhitespace } from "../lib/format";
import type { Entry } from "../lib/types";

export interface FilteredEntry {
  entry: Entry;
  matchIndices: number[];
  score: number;
}

export function useClipboardEntries(query: string) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);

  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const refresh = useCallback(() => refreshRef.current(), []);

  useEffect(() => {
    const scope = subscriptionScope();
    let request = 0;
    const fetchEntries = async () => {
      const current = ++request;
      try {
        const list = await api.list();
        if (scope.active && current === request) setEntries(list);
      } catch (error) {
        if (scope.active) console.error("list entries", error);
      } finally {
        if (scope.active && current === request) setLoading(false);
      }
    };
    refreshRef.current = fetchEntries;
    void (async () => {
      for (const event of ["clipboard:entry-captured", "clipboard:entries-changed", "overlay:opened"]) {
        await scope.add(() => listen(event, () => {
          if (scope.active) void fetchEntries();
        }));
      }
      if (scope.active) await fetchEntries();
    })();
    return () => {
      scope.dispose();
      refreshRef.current = async () => {};
    };
  }, []);

  const filtered = useMemo<{
    pinned: FilteredEntry[];
    recent: FilteredEntry[];
  }>(() => {
    const pinned: FilteredEntry[] = [];
    const recent: FilteredEntry[] = [];
    const q = normalizeWhitespace(query);

    for (const entry of entries) {
      const searchTarget = normalizeWhitespace(entry.text ?? `image ${entry.width ?? ""}x${entry.height ?? ""}`);
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
      pinned.sort((a, b) => b.entry.lastUsedAt - a.entry.lastUsedAt || b.entry.id - a.entry.id);
      recent.sort((a, b) => b.entry.lastUsedAt - a.entry.lastUsedAt || b.entry.id - a.entry.id);
    }
    return { pinned, recent };
  }, [entries, query]);

  return { entries, filtered, loading, refresh };
}
