// Color-grade settings shared with the Rust grade engine. Field names mirror
// grade::GradeSettings (serde), so GradeSettings objects are passed straight
// to the grade_preview / grade_analyze commands.

export interface GradeSettings {
  /** Invert a negative film to a positive. */
  invert: boolean;
  /** 0 = Color, 1 = B&W. */
  mode: number;
  /** Base (film mask) density per channel, subtracted in the density domain. */
  base_density: [number, number, number];
  /** Per-channel density exposure offset (Printer Lights live here too). */
  exposure: [number, number, number];
  /** Per-channel density minimum (white-point reference in density). */
  d_min: [number, number, number];
  /** Per-channel density maximum. */
  d_max: [number, number, number];
  /** Gamma (inverse power applied to normalized density). */
  gamma: number;
  /** Highlight tone push (post-gamma), [-1, 1]. */
  highlights: number;
  /** Shadow tone pull (post-gamma), [-1, 1]. */
  shadows: number;
  /** Saturation in [-1, 1] (0 = neutral). */
  saturation: number;
  /** Color temperature in [-1, 1] (positive = warmer). */
  temperature: number;
  /** Tint (green-magenta) in [-1, 1]. */
  tint: number;
}

/** Fresh default settings, mirroring the Rust `Default` impl. Returns a new
 * object (and new tuple arrays) every call so no two consumers share state. */
export function defaultGradeSettings(): GradeSettings {
  return {
    invert: false,
    mode: 0,
    base_density: [0, 0, 0],
    exposure: [0, 0, 0],
    d_min: [0, 0, 0],
    d_max: [2, 2, 2],
    gamma: 1,
    highlights: 0,
    shadows: 0,
    saturation: 0,
    temperature: 0,
    tint: 0,
  };
}

export function clamp(value: number, lo: number, hi: number): number {
  return Math.min(Math.max(value, lo), hi);
}

// Proxy settings are frontend-only for now: the backend model download (LaMa)
// does not consume a proxy yet, so the URL is persisted to localStorage and
// read by the frontend before a download is started. See the note in
// GradePanel's footer.
export const PROXY_STORAGE_KEY = "unduster.proxy.url";

export function loadProxyUrl(): string {
  try {
    return localStorage.getItem(PROXY_STORAGE_KEY) ?? "";
  } catch {
    // storage unavailable (private mode, etc.): proxy is best-effort
    return "";
  }
}

export function saveProxyUrl(url: string): void {
  try {
    if (url) {
      localStorage.setItem(PROXY_STORAGE_KEY, url);
    } else {
      localStorage.removeItem(PROXY_STORAGE_KEY);
    }
  } catch {
    // storage unavailable: nothing to persist
  }
}
