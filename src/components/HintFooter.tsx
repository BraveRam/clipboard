import { CornerDownLeft } from "lucide-react";

export function HintFooter() {
  return (
    <div className="hints" aria-hidden>
      <div className="hints__group">
        <span className="kbd">↑</span>
        <span className="kbd">↓</span>
        <span>navigate</span>
      </div>
      <div className="hints__group">
        <span className="kbd">
          <CornerDownLeft size={10} />
        </span>
        <span>copy</span>
      </div>
      <div className="hints__group">
        <span className="kbd">Ctrl</span>
        <span className="kbd">P</span>
        <span>pin</span>
      </div>
      <div className="hints__group">
        <span className="kbd">Ctrl</span>
        <span className="kbd">⌫</span>
        <span>delete</span>
      </div>
      <div className="hints__group" style={{ marginLeft: "auto" }}>
        <span className="kbd">Esc</span>
        <span>close</span>
      </div>
    </div>
  );
}
