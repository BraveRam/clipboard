import { useCallback, useEffect, useState } from "react";

interface Options {
  count: number;
  onChoose: (index: number) => void;
  onPin: (index: number) => void;
  onDelete: (index: number) => void;
  onClose: () => void;
}

export function useKeyboardNav({
  count,
  onChoose,
  onPin,
  onDelete,
  onClose,
}: Options) {
  const [index, setIndex] = useState(0);

  // Clamp selection when list size shrinks.
  useEffect(() => {
    if (index >= count) setIndex(Math.max(0, count - 1));
  }, [count, index]);

  const reset = useCallback(() => setIndex(0), []);

  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setIndex((i) => (count === 0 ? 0 : (i + 1) % count));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setIndex((i) => (count === 0 ? 0 : (i - 1 + count) % count));
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (count > 0) onChoose(index);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key.toLowerCase() === "p" && e.ctrlKey) {
        e.preventDefault();
        if (count > 0) onPin(index);
      } else if (
        (e.key === "Backspace" || e.key === "Delete") &&
        e.ctrlKey
      ) {
        e.preventDefault();
        if (count > 0) onDelete(index);
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [count, index, onChoose, onPin, onDelete, onClose]);

  return { index, setIndex, reset };
}
