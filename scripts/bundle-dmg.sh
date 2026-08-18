#!/usr/bin/env bash
#
# bundle-dmg.sh — 一键构建并打包 TheUnduster 的 macOS DMG（本地使用，无 CI）。
#
# 用法:
#   scripts/bundle-dmg.sh                 # 完整构建（vite 前端 + Rust release + .app + .dmg）
#   scripts/bundle-dmg.sh --bundles app   # 只打 .app，不打 dmg（调试用，快一些）
#   scripts/bundle-dmg.sh --no-verify     # 跳过 DMG 挂载验证
#
# 产物:
#   dist/dmg/TheUnduster_<version>_aarch64.dmg
#   （.app 在 app/src-tauri/target/release/bundle/macos/）
#
# 依赖:
#   - npm / node（app/mise.toml 固定 Node 22）
#   - Rust stable（rustup 安装，通常位于 ~/.cargo/bin）
#   - hdiutil（macOS 自带，DMG 打包与验证用）
#
# 说明:
#   - DMG 未签名（本地使用）。macOS 首次打开时可能提示"无法验证开发者"，
#     右键 -> 打开 即可绕过。
#   - LaMa 模型（~207MB）不在 DMG 内，首次运行由应用自动下载（需网络）。

set -euo pipefail

# ---------------------------------------------------------------------------
# 0. 定位项目与工具链
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
APP_DIR="$ROOT_DIR/app"
DIST_DIR="$ROOT_DIR/dist/dmg"

# Rust 由 rustup 管理，可能不在默认 PATH（本项目 mise.toml 未接管 rust）：
if ! command -v cargo >/dev/null 2>&1; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "错误: 找不到 cargo。请先安装 Rust：https://rustup.rs" >&2
  exit 1
fi
if ! command -v npm >/dev/null 2>&1; then
  echo "错误: 找不到 npm。请先安装 Node 22（mise 或 https://nodejs.org）" >&2
  exit 1
fi

# 解析参数
BUNDLES="dmg"          # 默认只打 dmg（其中已包含 .app）
VERIFY_DMG=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundles) BUNDLES="${2:?--bundles 需要一个参数（app|dmg|app,dmg）}"; shift 2 ;;
    --bundles=*) BUNDLES="${1#*=}"; shift ;;
    --no-verify) VERIFY_DMG=0; shift ;;
    -h|--help) grep -E '^#' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "未知参数: $1（见脚本头部注释）" >&2; exit 1 ;;
  esac
done

cd "$APP_DIR"
echo "==> 项目: $ROOT_DIR"
echo "==> 工具: cargo $(cargo --version | awk '{print $2}'), node $(node --version), npm $(npm --version)"

# 清理上次打包可能残留的中间镜像。Tauri 内置的 bundle_dmg.sh 偶尔会因
# 前一次失败的 hdiutil 调用留下一个未清理的读写临时镜像
# （target/release/bundle/macos/rw.*.dmg），干扰下一次打包。打包前先清掉。
echo "==> 清理上次打包残留 (rw.*.dmg) ..."
rm -f "$ROOT_DIR"/target/release/bundle/macos/rw.*.dmg

# ---------------------------------------------------------------------------
# 1. 构建前端（tauri build 的 beforeBuildCommand 也会跑 npm run build，
#    这里提前跑一次，让编译错误更早暴露、输出更清晰）
# ---------------------------------------------------------------------------
echo
echo "==> [1/3] 构建前端 (vite) ..."
npm run build

# ---------------------------------------------------------------------------
# 2. Rust release 构建 + 打包
# ---------------------------------------------------------------------------
echo
echo "==> [2/3] 构建 release 并打包 (tauri build --bundles $BUNDLES) ..."
# 透传 UNDUSTER_* 环境变量给应用（如 UNDUSTER_PIXEL_BUDGET_GB），
# 其余按 tauri 默认。--no-bundle 之外不传 --features，保持仓库默认 profile。
npm run tauri build -- --bundles "$BUNDLES"

# 本仓库是 Cargo workspace，release 产物落在 workspace 根目录的共享
# target/（不是 app/src-tauri/target/）。
BUNDLE_BASE="$ROOT_DIR/target/release/bundle"

# ---------------------------------------------------------------------------
# 3. 收集产物
# ---------------------------------------------------------------------------
echo
echo "==> [3/3] 收集产物 ..."
mkdir -p "$DIST_DIR"

if [[ "$BUNDLES" == *dmg* ]]; then
  DMG_SRC="$BUNDLE_BASE/dmg"
  if ! compgen -G "$DMG_SRC/*.dmg" >/dev/null; then
    echo "错误: 未在 $DMG_SRC 找到 .dmg 产物" >&2
    exit 1
  fi
  # 复制 DMG 到产物目录，并给文件名加上品牌 "ikFilm+"（保持 productName
  # 不变，避免包名/应用名受特殊字符影响）。
  for dmg in "$DMG_SRC"/*.dmg; do
    base="$(basename "$dmg")"
    newname="${base/TheUnduster/TheUnduster ikFilm+}"
    cp "$dmg" "$DIST_DIR/$newname"
  done
  DMG_FILE="$(ls -t "$DIST_DIR"/*.dmg | head -1)"
  echo "DMG:  $DMG_FILE"
fi
if [[ "$BUNDLES" == *app* ]]; then
  APP_SRC="$BUNDLE_BASE/macos"
  if [[ -d "$APP_SRC" ]]; then
    cp -R "$APP_SRC"/*.app "$DIST_DIR/" 2>/dev/null || true
    APP_FILE="$(ls -dt "$DIST_DIR"/*.app | head -1)"
    echo "APP:  $APP_FILE"
  fi
fi

# ---------------------------------------------------------------------------
# 4. 验证 DMG（可选）
# ---------------------------------------------------------------------------
if [[ "$VERIFY_DMG" -eq 1 && -n "${DMG_FILE:-}" ]]; then
  echo
  echo "==> 验证 DMG 完整性 (hdiutil verify) ..."
  hdiutil verify "$DMG_FILE"
fi

echo
echo "==> 完成。产物目录: $DIST_DIR"
if [[ -n "${DMG_FILE:-}" ]]; then
  echo "    大小: $(du -h "$DMG_FILE" | cut -f1)"
  echo "    SHA-256: $(shasum -a 256 "$DMG_FILE" | awk '{print $1}')"
fi
