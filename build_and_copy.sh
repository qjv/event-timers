#!/usr/bin/env bash
# Build Event Timers for GW2 (Nexus) and copy to your addons folder.
set -euo pipefail

# Source rustup/cargo if present (required for x86_64-pc-windows-msvc on Arch)
if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck source=/dev/null
  source "${HOME}/.cargo/env"
fi

# Override if your addons path differs (Linux Steam default below)
: "${GW2_ADDONS_DIR:=${HOME}/.local/share/Steam/steamapps/common/Guild Wars 2/addons}"

echo "========================================"
echo "Building Event Timers for GW2 (Nexus)"
echo "========================================"

cargo xwin build --release --target x86_64-pc-windows-msvc

DLL="target/x86_64-pc-windows-msvc/release/event_timers.dll"
mkdir -p "${GW2_ADDONS_DIR}/event_timers"
cp "${DLL}" "${GW2_ADDONS_DIR}/"
cp event_tracks.json "${GW2_ADDONS_DIR}/event_timers/"

echo "Installed:"
echo "  ${GW2_ADDONS_DIR}/event_timers.dll"
echo "  ${GW2_ADDONS_DIR}/event_timers/event_tracks.json"
echo "Build and copy complete."
