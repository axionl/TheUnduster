// Minimal i18n for the app UI. Defaults to English; the user can switch to
// Chinese (and back) via the toolbar, persisted in localStorage. `t(key)`
// returns the current-language string (or the key itself if missing), and
// components reference the `lang` store (via `$lang`) so a switch re-renders.
import { writable, get } from "svelte/store";

export type Lang = "en" | "zh";

type Dict = Record<string, string>;

const en: Dict = {
  appName: "TheUnduster",
  appSuffix: "| ikFilm+",
  // Workflow tabs
  tabClean: "Dust Removal",
  tabGrade: "Color Grade",
  // File / frame toolbar
  openScan: "Open scan",
  openRoll: "Open roll",
  detect: "Detect",
  detecting: "Detecting…",
  detected: "Detected",
  heal: "Heal",
  healing: "Healing…",
  export: "Export",
  exporting: "Exporting…",
  undo: "Undo",
  redo: "Redo",
  approve: "Approve",
  unapprove: "Unapprove",
  healApproved: "Heal approved",
  exportApproved: "Export approved",
  // Model / misc
  downloadModel: "Download healing model (207 MB)",
  repairModel: "Repair healing model",
  downloadRealModel: "Download real healing model (207 MB)",
  cancel: "Cancel",
  cancelling: "Cancelling",
  loadModel: "Load Model",
  modelFolder: "Model Folder",
  // Empty state
  noScanOpen: "no scan open",
  dropHint: "or drop a scan or a roll folder anywhere in this window",
  // Grade panel
  autoInvert: "Auto Invert",
  applyToClean: "Apply to Dust Removal",
  invert: "Invert",
  mode: "Mode",
  color: "Color",
  bw: "B&W",
  gamma: "Gamma",
  highlights: "Highlights",
  shadows: "Shadows",
  saturation: "Saturation",
  temperature: "Temperature",
  tint: "Tint",
  baseDensity: "Base density",
  exposure: "Exposure",
  dMin: "D-Min",
  dMax: "D-Max",
  filmStyle: "Film Style",
  none: "None",
  lutStrength: "LUT strength",
  loadCubeLut: "Load .cube LUT",
  noGradeHint: "Open a film scan to grade it (invert, density, exposure, gamma, white balance…)",
  gradePreview: "Color grade preview",
  updating: "updating…",
  noPreview: "no preview",
};

const zh: Dict = {
  appName: "TheUnduster",
  appSuffix: "| ikFilm+",
  tabClean: "除尘",
  tabGrade: "校色",
  openScan: "打开扫描",
  openRoll: "打开胶卷",
  detect: "检测",
  detecting: "检测中…",
  detected: "已检测",
  heal: "修复",
  healing: "修复中…",
  export: "导出",
  exporting: "导出中…",
  undo: "撤销",
  redo: "重做",
  approve: "通过",
  unapprove: "取消通过",
  healApproved: "修复已通过",
  exportApproved: "导出已通过",
  downloadModel: "下载修复模型（207 MB）",
  repairModel: "修复修复模型",
  downloadRealModel: "下载真实修复模型（207 MB）",
  cancel: "取消",
  cancelling: "取消中",
  loadModel: "加载模型",
  modelFolder: "模型目录",
  noScanOpen: "未打开扫描",
  dropHint: "或把扫描/胶卷文件夹拖到窗口任意位置",
  autoInvert: "自动反相",
  applyToClean: "应用到除尘",
  invert: "反相",
  mode: "模式",
  color: "彩色",
  bw: "黑白",
  gamma: "Gamma",
  highlights: "高光",
  shadows: "阴影",
  saturation: "饱和度",
  temperature: "色温",
  tint: "色调",
  baseDensity: "片基密度",
  exposure: "曝光",
  dMin: "D-Min",
  dMax: "D-Max",
  filmStyle: "胶片风格",
  none: "无",
  lutStrength: "LUT 强度",
  loadCubeLut: "载入 .cube LUT",
  noGradeHint: "打开胶片扫描后即可校色（反相、密度、曝光、Gamma、白平衡…）",
  gradePreview: "校色预览",
  updating: "更新中…",
  noPreview: "暂无预览",
};

/** Both language dictionaries, keyed by Lang. Exported so components can build
 * a reactive `$derived` translation function that Svelte's fine-grained
 * reactivity tracks (`dicts[$lang][key]`), which `get()` inside a plain
 * function would not. */
export const dicts: Record<Lang, Dict> = { en, zh };

function initialLang(): Lang {
  try {
    const l = localStorage.getItem("unduster-lang");
    return l === "zh" ? "zh" : "en";
  } catch {
    return "en";
  }
}

/** Current language store; components read it as `$lang` to re-render on switch. */
export const lang = writable<Lang>(initialLang());

export function setLang(l: Lang): void {
  lang.set(l);
  try {
    localStorage.setItem("unduster-lang", l);
  } catch {
    /* ignore */
  }
}

export function getLang(): Lang {
  return get(lang);
}

/** Translate a key to the current language. */
export function t(key: string): string {
  const d = dicts[get(lang)] ?? en;
  return d[key] ?? key;
}
