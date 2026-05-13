import { useEffect, useRef } from "react";
import type { FilteredEntry } from "../hooks/useClipboardEntries";
import { EntryRow } from "./EntryRow";

interface Props {
  pinned: FilteredEntry[];
  recent: FilteredEntry[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onChoose: (index: number) => void;
  query: string;
}

export function EntryList({
  pinned,
  recent,
  selectedIndex,
  onSelect,
  onChoose,
  query,
}: Props) {
  const total = pinned.length + recent.length;
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const node = container.querySelector<HTMLElement>(
      `[data-index="${selectedIndex}"]`,
    );
    if (node) {
      node.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  if (total === 0) {
    return (
      <div className="list" ref={containerRef}>
        <div className="list__empty">
          <div className="list__empty-glyph" aria-hidden>
            {query ? "⚲" : "✂"}
          </div>
          <div>
            {query
              ? "No matches"
              : "Your clipboard is quiet. Copy anything to begin."}
          </div>
        </div>
      </div>
    );
  }

  let i = 0;
  return (
    <div className="list" ref={containerRef} role="listbox">
      {pinned.length > 0 && (
        <>
          <div className="section__label">
            <span>Pinned</span>
            <span>{pinned.length}</span>
          </div>
          {pinned.map((p) => {
            const index = i++;
            return (
              <div key={p.entry.id} data-index={index}>
                <EntryRow
                  entry={p.entry}
                  matchIndices={p.matchIndices}
                  selected={index === selectedIndex}
                  onMouseEnter={() => onSelect(index)}
                  onClick={() => onChoose(index)}
                />
              </div>
            );
          })}
        </>
      )}
      {recent.length > 0 && (
        <>
          <div className="section__label">
            <span>Recent</span>
            <span>
              {recent.length}/{50}
            </span>
          </div>
          {recent.map((p) => {
            const index = i++;
            return (
              <div key={p.entry.id} data-index={index}>
                <EntryRow
                  entry={p.entry}
                  matchIndices={p.matchIndices}
                  selected={index === selectedIndex}
                  onMouseEnter={() => onSelect(index)}
                  onClick={() => onChoose(index)}
                />
              </div>
            );
          })}
        </>
      )}
    </div>
  );
}
