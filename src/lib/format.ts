export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

export function formatRelative(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 30_000) return "just now";
  if (diff < 60_000) return `${Math.floor(diff / 1000)}s ago`;
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
}

export function previewText(text: string): {
  preview: string;
  mono: boolean;
} {
  const oneLine = text.replace(/\s+/g, " ").trim();
  const looksLikeCode =
    /^[$#>]|=>|\{|\}|\(|\)|;|=== |---|^\s*\$ |^[A-Z_]+=/.test(text) ||
    /\.(rs|ts|tsx|js|jsx|py|go|rb|sh|sql|yaml|yml|toml|md)\b/.test(text);
  return { preview: oneLine, mono: looksLikeCode };
}
