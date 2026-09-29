import { useCallback, useEffect, useRef, useState } from "react";
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

  const focusInput = useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    el.focus();
  }, []);


  const onFocus = useCallback(() => {
    focusInput();
  }, [focusInput]);

  useEffect(() => {
    focusInput();
  }, [focusInput]);

  const choose = useCallback(
    async (index: number) => {
      const target = ordered[index];
      if (!target) return;
      try {
        if (await api.paste(target.entry.id)) await api.hide();
      } catch (e) {
        console.error("copy", e);
      }
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

  const onOpened = useCallback(() => {
    setQuery("");
    setIndex(0);
    focusInput();
  }, [focusInput, setIndex]);
  useOverlayLifecycle({ onOpened, onFocus });

  // Global keydown fallback: if any typing happens while the search input is not active,
  // immediately focus the input and route the character/backspace into the query.
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      const el = inputRef.current;
      if (!el) return;
      if (document.activeElement === el) return;

      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (
        e.key === "Tab" ||
        e.key === "Escape" ||
        e.key === "ArrowUp" ||
        e.key === "ArrowDown" ||
        e.key === "ArrowLeft" ||
        e.key === "ArrowRight" ||
        e.key === "Enter"
      ) {
        return;
      }

      if (e.key.length === 1) {
        e.preventDefault();
        el.focus();
        setQuery((prev) => prev + e.key);
        setIndex(0);
      } else if (e.key === "Backspace") {
        e.preventDefault();
        el.focus();
        setQuery((prev) => prev.slice(0, -1));
        setIndex(0);
      }
    };

    window.addEventListener("keydown", handleGlobalKeyDown);
    return () => window.removeEventListener("keydown", handleGlobalKeyDown);
  }, [setIndex]);

  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target !== inputRef.current) {
          e.preventDefault();
          focusInput();
        }
      }}
    >
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
