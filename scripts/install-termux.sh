#!/data/data/com.termux/files/usr/bin/bash
set -euo pipefail

REPOSITORY="sakusdev/RoM"
RELEASE_TAG="${ROM_VERSION:-@TAG@}"
INSTALL_DIR="${ROM_INSTALL_DIR:-${PREFIX:-/data/data/com.termux/files/usr}/bin}"

log() {
  printf '[RoM] %s\n' "$*"
}

fail() {
  printf '[RoM] error: %s\n' "$*" >&2
  exit 1
}

command -v pkg >/dev/null 2>&1 || fail 'This installer must be run inside Termux.'

case "$(uname -m)" in
  aarch64|arm64)
    PLATFORM="android-aarch64"
    ;;
  *)
    fail "Unsupported Termux architecture: $(uname -m). RoM currently publishes Android AArch64 binaries."
    ;;
esac

log 'Installing download and checksum tools through pkg'
pkg install -y curl coreutils ca-certificates >/dev/null

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

install_asset "ferrum-server-${PLATFORM}" 'ferrum-server'
install_asset "rom-bootstrap-${PLATFORM}" 'rom-bootstrap'

log "Installed RoM ${RELEASE_TAG} into ${INSTALL_DIR}"
log 'Run: rom-bootstrap --help'
log 'Run: ferrum-server --help'
