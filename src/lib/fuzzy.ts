export interface FuzzyMatch {
  score: number;
  indices: number[];
}

/**
 * Tiny subsequence fuzzy matcher. Returns null when the needle does not
 * fit into the haystack. Otherwise returns a score (higher = better)
 * and the matched character indices in the original haystack.
 *
 * Scoring favors: contiguous runs, matches at word boundaries, matches
 * near the start, and lower-case matches.
 */
export function fuzzyMatch(haystack: string, needle: string): FuzzyMatch | null {
  if (!needle) return { score: 0, indices: [] };
  const h = haystack.toLowerCase();
  const n = needle.toLowerCase();

  const indices: number[] = [];
  let hi = 0;
  let runStart = -1;
  let lastMatch = -2;
  let score = 0;

  for (let ni = 0; ni < n.length; ni++) {
    const ch = n[ni];
    let found = -1;
    for (; hi < h.length; hi++) {
      if (h[hi] === ch) {
        found = hi;
        break;
      }
    }
    if (found === -1) return null;
    indices.push(found);

    if (found === lastMatch + 1) {
      // contiguous run
      score += 6;
    } else {
      runStart = found;
      score += 1;
    }
    if (found === 0) score += 8;
    else {
      const prev = haystack[found - 1];
      if (prev === " " || prev === "/" || prev === "_" || prev === "-" || prev === ".") {
        score += 4;
      }
    }
    // small bias toward earlier matches
    score -= Math.floor(found / 12);
    lastMatch = found;
    hi = found + 1;
  }

  // Penalty for unmatched gap between first and last match
  if (indices.length >= 2) {
    const span = indices[indices.length - 1] - indices[0];
    score -= Math.floor(span / 8);
  }
  // Boost for short haystacks (more relevant)
  score += Math.max(0, 20 - haystack.length / 6);
  // Suppress unused warning while documenting intent.
  void runStart;
  return { score, indices };
}

export function highlightMatch(text: string, indices: number[]): Array<{
  text: string;
  match: boolean;
}> {
  if (indices.length === 0) return [{ text, match: false }];
  const out: Array<{ text: string; match: boolean }> = [];
  let cursor = 0;
  const set = new Set(indices);
  let buf = "";
  let bufMatch = false;
  for (let i = 0; i < text.length; i++) {
    const isMatch = set.has(i);
    if (i === 0) {
      buf = text[i];
      bufMatch = isMatch;
      continue;
    }
    if (isMatch === bufMatch) {
      buf += text[i];
    } else {
      out.push({ text: buf, match: bufMatch });
      buf = text[i];
      bufMatch = isMatch;
    }
  }
  if (buf) out.push({ text: buf, match: bufMatch });
  void cursor;
  return out;
}
