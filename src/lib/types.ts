export type EntryKind = "text" | "image";

export interface Entry {
  id: number;
  kind: EntryKind;
  text: string | null;
  imagePath: string | null;
  thumbB64: string | null;
  width: number | null;
  height: number | null;
  sizeBytes: number;
  contentHash: string;
  pinned: boolean;
  createdAt: number;
  lastUsedAt: number;
}
