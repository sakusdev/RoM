#!/usr/bin/env bash
set -euo pipefail

REPOSITORY="sakusdev/RoM"
RELEASE_TAG="${ROM_VERSION:-@TAG@}"
INSTALL_DIR="${ROM_INSTALL_DIR:-${HOME}/.local/bin}"

log() {
  printf '[RoM] %s\n' "$*"
}

fail() {
  printf '[RoM] error: %s\n' "$*" >&2
  exit 1
}

[[ "$(uname -s)" == 'Linux' ]] || fail 'Pixel Terminal installer requires Linux.'

case "$(uname -m)" in
  aarch64|arm64)
    PLATFORM="linux-aarch64"
    ;;
  x86_64|amd64)
    PLATFORM="linux-x86_64"
    ;;
  *)
    fail "Unsupported Linux architecture: $(uname -m)."
    ;;
esac

command -v curl >/dev/null 2>&1 || fail 'curl is required. Install it with: sudo apt install curl'
command -v sha256sum >/dev/null 2>&1 || fail 'sha256sum is required. Install coreutils first.'

if [[ "$RELEASE_TAG" == '@TAG@' ]]; then
  log 'Resolving the newest published RoM release'
  release_json="$(
    curl --fail --location --silent --show-error \
      -H 'Accept: application/vnd.github+json' \
      "https://api.github.com/repos/${REPOSITORY}/releases?per_page=20"
  )"
  tag_lines="$(printf '%s' "$release_json" | grep -oE '"tag_name"[[:space:]]*:[[:space:]]*"[^"]+"')"
  first_tag_line="${tag_lines%%$'\n'*}"
  RELEASE_TAG="$(cut -d '"' -f 4 <<<"$first_tag_line")"
fi

[[ -n "$RELEASE_TAG" ]] || fail 'Could not determine a release tag.'

BASE_URL="https://github.com/${REPOSITORY}/releases/download/${RELEASE_TAG}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
mkdir -p "$INSTALL_DIR"

install_asset() {
  local asset="$1"
  local destination="$2"

  log "Downloading ${asset} from ${RELEASE_TAG}"
  curl --fail --location --retry 3 --show-error \
    "${BASE_URL}/${asset}" \
    --output "${TMP_DIR}/${asset}"
  curl --fail --location --retry 3 --show-error \
    "${BASE_URL}/${asset}.sha256" \
    --output "${TMP_DIR}/${asset}.sha256"

  (
    cd "$TMP_DIR"
    sha256sum --check "${asset}.sha256"
  )

  install -m 0755 "${TMP_DIR}/${asset}" "${INSTALL_DIR}/${destination}"
}

install_asset "rom-server-${PLATFORM}" 'rom-server'
install_asset "rom-bootstrap-${PLATFORM}" 'rom-bootstrap'

log "Installed RoM ${RELEASE_TAG} into ${INSTALL_DIR}"
if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
  log "Add this line to ~/.bashrc: export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
log 'Run: rom-bootstrap --help'
log 'Run: rom-server --help'
