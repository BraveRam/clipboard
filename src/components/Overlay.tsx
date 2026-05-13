import { useCallback, useRef, useState } from "react";
import { api } from "../lib/api";
import { useClipboardEntries } from "../hooks/useClipboardEntries";
import { useKeyboardNav } from "../hooks/useKeyboardNav";
import { useOverlayLifecycle } from "../hooks/useOverlayLifecycle";
import { SearchBar } from "./SearchBar";
import { EntryList } from "./EntryList";
import { HintFooter } from "./HintFooter";

export function Overlay() {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const { filtered } = useClipboardEntries(query);

  const ordered = [...filtered.pinned, ...filtered.recent];
  const count = ordered.length;

  const onOpened = useCallback(() => {
    setQuery("");
    requestAnimationFrame(() => inputRef.current?.focus());
  }, []);

  useOverlayLifecycle({ onOpened });

  const choose = useCallback(
    async (index: number) => {
      const target = ordered[index];
      if (!target) return;
      try {
        await api.paste(target.entry.id);
      } catch (e) {
        console.error("paste", e);
      }
      await api.hide();
    },
    [ordered],
  );

  const pin = useCallback(
    async (index: number) => {
      const target = ordered[index];
      if (!target) return;
      await api.togglePin(target.entry.id);
    },
    [ordered],
  );

  const remove = useCallback(
    async (index: number) => {
      const target = ordered[index];
      if (!target) return;
      await api.delete(target.entry.id);
    },
    [ordered],
  );

  const close = useCallback(() => {
    api.hide();
  }, []);

  const { index, setIndex } = useKeyboardNav({
    count,
    onChoose: choose,
    onPin: pin,
    onDelete: remove,
    onClose: close,
  });

  return (
    <div className="overlay">
      <SearchBar
        ref={inputRef}
        value={query}
        onChange={(v) => {
          setQuery(v);
          setIndex(0);
        }}
        count={count}
      />
      <EntryList
        pinned={filtered.pinned}
        recent={filtered.recent}
        selectedIndex={index}
        onSelect={setIndex}
        onChoose={choose}
        query={query}
      />
      <HintFooter />
    </div>
  );
}
