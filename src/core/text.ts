// Text layout shared by the canvas, thumbnails and PDF export.

export const LINE_HEIGHT = 1.35;
export const FONT_FAMILY = '-apple-system, BlinkMacSystemFont, "Helvetica Neue", Helvetica, Arial, sans-serif';

let measureCtx: CanvasRenderingContext2D | null = null;
const cache = new Map<string, number>();

export function textWidth(s: string, size: number): number {
  const key = size + "|" + s;
  const hit = cache.get(key);
  if (hit !== undefined) return hit;
  if (!measureCtx) measureCtx = document.createElement("canvas").getContext("2d");
  measureCtx!.font = `${size}px ${FONT_FAMILY}`;
  const w = measureCtx!.measureText(s).width;
  if (cache.size > 5000) cache.clear();
  cache.set(key, w);
  return w;
}

/** Word-wraps to max width. Newlines always break; long words break by characters. */
export function wrap(text: string, size: number, maxW: number): string[] {
  const lines: string[] = [];
  for (const para of text.split("\n")) {
    let line = "";
    for (const word of para.match(/\S+\s*|\s+/g) ?? []) {
      const cand = line + word;
      if (!line || textWidth(cand.trimEnd(), size) <= maxW) line = cand;
      else {
        lines.push(line.trimEnd());
        line = word.trimStart();
      }
      while (textWidth(line.trimEnd(), size) > maxW && line.length > 1) {
        let cut = 1;
        while (cut < line.length && textWidth(line.slice(0, cut + 1), size) <= maxW) cut++;
        lines.push(line.slice(0, cut));
        line = line.slice(cut);
      }
    }
    lines.push(line.trimEnd());
  }
  return lines;
}

export const textHeight = (text: string, size: number, w: number) => Math.max(1, wrap(text, size, w).length) * size * LINE_HEIGHT;
