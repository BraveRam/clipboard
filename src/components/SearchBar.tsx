import { forwardRef } from "react";

interface Props {
  value: string;
  onChange: (next: string) => void;
  count: number;
}

export const SearchBar = forwardRef<HTMLInputElement, Props>(
  function SearchBar({ value, onChange, count }, ref) {
    return (
      <div className="searchbar">
        <span className="searchbar__glyph" aria-hidden>⌕</span>
        <input
          ref={ref}
          className="searchbar__input"
          type="text"
          value={value}
          autoFocus
          spellCheck={false}
          placeholder="Search clipboard…"
          onChange={(e) => onChange(e.currentTarget.value)}
        />
        <span className="searchbar__count">
          {count} {count === 1 ? "item" : "items"}
        </span>
      </div>
    );
  },
);
