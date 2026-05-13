import { memo } from "react";
import type { Entry } from "../lib/types";
import { formatBytes, previewText } from "../lib/format";
import { highlightMatch } from "../lib/fuzzy";

interface Props {
  entry: Entry;
  matchIndices: number[];
  selected: boolean;
  onMouseEnter: () => void;
  onClick: () => void;
}

export const EntryRow = memo(function EntryRow({
  entry,
  matchIndices,
  selected,
  onMouseEnter,
  onClick,
}: Props) {
  const isImage = entry.kind === "image";

  return (
    <div
      className="row"
      data-selected={selected}
      data-pinned={entry.pinned}
      onMouseEnter={onMouseEnter}
      onMouseDown={(e) => {
        // Prevent the search input from losing focus on row click.
        e.preventDefault();
      }}
      onClick={onClick}
      role="option"
      aria-selected={selected}
    >
      <div className="row__kind" aria-hidden>
        {entry.pinned ? "📌" : isImage ? "🖼" : "¶"}
      </div>

      <div className="row__body">
        {isImage ? (
          <ImageRowBody entry={entry} />
        ) : (
          <TextRowBody
            text={entry.text ?? ""}
            indices={matchIndices}
            bytes={entry.sizeBytes}
          />
        )}
      </div>

      <div className="row__hint" aria-hidden>
        ↵ paste
      </div>
    </div>
  );
});

function TextRowBody({
  text,
  indices,
  bytes,
}: {
  text: string;
  indices: number[];
  bytes: number;
}) {
  const { preview, mono } = previewText(text);
  const segments = highlightMatch(preview, indices);
  return (
    <>
      <div className={`row__preview${mono ? " row__preview--mono" : ""}`}>
        {segments.map((s, i) =>
          s.match ? (
            <span key={i} className="match">
              {s.text}
            </span>
          ) : (
            <span key={i}>{s.text}</span>
          ),
        )}
      </div>
      <div className="row__meta">
        <span>{text.length} chars</span>
        <span>{formatBytes(bytes)}</span>
      </div>
    </>
  );
}

function ImageRowBody({ entry }: { entry: Entry }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      {entry.thumbB64 ? (
        <img
          className="row__thumb"
          src={`data:image/png;base64,${entry.thumbB64}`}
          alt=""
        />
      ) : null}
      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
        <div className="row__preview">
          Image · {entry.width}×{entry.height}
        </div>
        <div className="row__meta">
          <span>{formatBytes(entry.sizeBytes)}</span>
        </div>
      </div>
    </div>
  );
}
