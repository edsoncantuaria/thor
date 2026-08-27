#!/usr/bin/env bash
# Interactive menu to build Thor installers for Linux, macOS, or Windows.
#
# Tauri bundles are NOT cross-compiled from a single machine: a Linux box can
# only produce the Linux .deb/.rpm/.AppImage, a Windows box the MSI/NSIS
# installer (and needs vcvars64), and macOS the .dmg/.app (per-arch). This
# menu detects the current OS, offers a native build when the target matches,
# and otherwise offers to dispatch the "Release" GitHub Actions workflow
# (release.yml), which builds all four bundles on their own runners.
#
# Usage: ./scripts/build-menu.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION=$(node -pe "require('./package.json').version" 2>/dev/null || echo "unknown")

detect_os() {
  case "$(uname -s)" in
    Linux*) echo "linux" ;;
    Darwin*) echo "macos" ;;
    MINGW* | MSYS* | CYGWIN*) echo "windows" ;;
    *) echo "unknown" ;;
  esac
}

CURRENT_OS="$(detect_os)"

print_bundle_paths() {
  local search_dir="$1"
  local found=0
  while IFS= read -r -d '' path; do
    echo "  -> $path"
    found=1
  done < <(find "$search_dir" -type f \( -iname '*.AppImage' -o -iname '*.deb' -o -iname '*.rpm' -o -iname '*.dmg' -o -iname '*.app' -o -iname '*.msi' -o -iname '*.exe' \) -print0 2>/dev/null)
  [[ "$found" -eq 0 ]] && echo "  (no bundle files found under $search_dir)"
}

build_linux() {
  if [[ "$CURRENT_OS" != "linux" ]]; then
    echo "This machine is '$CURRENT_OS', not Linux — a native Linux bundle can't be produced here."
    echo "Use the 'Dispatch CI release' option instead."
    return 1
  fi
  echo "Building Linux bundle (deb/rpm/AppImage)..."
  npm run build && npm run tauri build
  echo "Done. Artifacts:"
  print_bundle_paths "$ROOT/src-tauri/target/release/bundle"
}

build_macos() {
  if [[ "$CURRENT_OS" != "macos" ]]; then
    echo "This machine is '$CURRENT_OS', not macOS — a native macOS bundle can't be produced here."
    echo "Use the 'Dispatch CI release' option instead."
    return 1
  fi
  echo "Which architecture?"
  select arch in "Apple Silicon (aarch64)" "Intel (x86_64)" "Both" "Cancel"; do
    case "$REPLY" in
      1) TARGETS=("aarch64-apple-darwin") ; break ;;
      2) TARGETS=("x86_64-apple-darwin") ; break ;;
      3) TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin") ; break ;;
      4) return 1 ;;
      *) echo "Invalid option." ;;
    esac
  done
  npm run build
  for target in "${TARGETS[@]}"; do
    echo "Building macOS bundle for $target..."
    npm run tauri build -- --target "$target"
  done
  echo "Done. Artifacts:"
  for target in "${TARGETS[@]}"; do
    print_bundle_paths "$ROOT/src-tauri/target/$target/release/bundle"
  done
}

build_windows() {
  if [[ "$CURRENT_OS" != "windows" ]]; then
    echo "This machine is '$CURRENT_OS', not Windows — a native Windows installer can't be produced here."
    echo "Use the 'Dispatch CI release' option instead."
    return 1
  fi
  local vcvars="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Auxiliary\\Build\\vcvars64.bat"
  echo "Building Windows bundle (MSI/NSIS) via vcvars64..."
  cmd /c "\"$vcvars\" >NUL && npm run build && npm run tauri build"
  echo "Done. Artifacts:"
  print_bundle_paths "$ROOT/src-tauri/target/release/bundle"
}

dispatch_ci() {
  echo "Dispatching the 'Release' GitHub Actions workflow (builds Windows, Linux, and both macOS"
  echo "targets on their own runners, then publishes a GitHub Release)."
  npm run release:publish
}

print_menu() {
  echo ""
  echo "==================================================="
  echo " Thor build menu — v$VERSION (detected OS: $CURRENT_OS)"
  echo "==================================================="
  echo " 1) Build for Linux   (deb/rpm/AppImage)"
  echo " 2) Build for macOS   (dmg/app, per-arch)"
  echo " 3) Build for Windows (msi/nsis)"
  echo " 4) Dispatch CI release (all platforms, via GitHub Actions)"
  echo " 5) Exit"
  echo "==================================================="
}

while true; do
  print_menu
  read -rp "Choose an option [1-5]: " choice
  case "$choice" in
    1) build_linux ;;
    2) build_macos ;;
    3) build_windows ;;
    4) dispatch_ci ;;
    5) echo "Bye."; exit 0 ;;
    *) echo "Invalid option." ;;
  esac
done
