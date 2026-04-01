#!/usr/bin/env bash
set -euo pipefail
umask 022

BINARY_NAME="super-lazygit"
OWNER="quangdang46"
REPO="super_lazygit_rust"
DEST="${DEST:-$HOME/.local/bin}"
VERSION="${VERSION:-}"
QUIET=0
EASY=0
VERIFY=0
FROM_SOURCE=0
UNINSTALL=0
MAX_RETRIES=3
DOWNLOAD_TIMEOUT=120
LOCK_DIR="/tmp/${BINARY_NAME}-install.lock.d"
TMP=""

log_info() { [ "$QUIET" -eq 1 ] && return 0; echo "[${BINARY_NAME}] $*" >&2; }
log_warn() { echo "[${BINARY_NAME}] WARN: $*" >&2; }
log_success() { [ "$QUIET" -eq 1 ] && return 0; echo "✓ $*" >&2; }
die() { echo "ERROR: $*" >&2; exit 1; }

usage() {
    cat <<'EOF'
Install super-lazygit from GitHub releases.

Usage: install.sh [options]

Options:
  --dest PATH         Install into PATH
  --dest=PATH         Install into PATH
  --version VERSION   Install a specific release tag
  --version=VERSION   Install a specific release tag
  --system            Install into /usr/local/bin
  --easy-mode         Append DEST to shell rc files if needed
  --verify            Run --version after install
  --from-source       Build from source instead of downloading assets
  --quiet, -q         Reduce output
  --uninstall         Remove installed binary
  -h, --help          Show this help
EOF
    exit 0
}

cleanup() {
    rm -rf "$TMP" "$LOCK_DIR" 2>/dev/null || true
}
trap cleanup EXIT

acquire_lock() {
    if mkdir "$LOCK_DIR" 2>/dev/null; then
        echo $$ > "$LOCK_DIR/pid"
        return 0
    fi
    die "Another install is running. If stuck: rm -rf $LOCK_DIR"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dest) DEST="$2"; shift 2 ;;
        --dest=*) DEST="${1#*=}"; shift ;;
        --version) VERSION="$2"; shift 2 ;;
        --version=*) VERSION="${1#*=}"; shift ;;
        --system) DEST="/usr/local/bin"; shift ;;
        --easy-mode) EASY=1; shift ;;
        --verify) VERIFY=1; shift ;;
        --from-source) FROM_SOURCE=1; shift ;;
        --quiet|-q) QUIET=1; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help) usage ;;
        *) shift ;;
    esac
done

remove_installer_path_lines() {
    local rc="$1"
    [ -f "$rc" ] || return 0

    local tmp_file
    tmp_file=$(mktemp "${TMPDIR:-/tmp}/${BINARY_NAME}-rc.XXXXXX")
    grep -vF "# ${BINARY_NAME} installer" "$rc" > "$tmp_file" || true
    cat "$tmp_file" > "$rc"
    rm -f "$tmp_file"
}

if [ "$UNINSTALL" -eq 1 ]; then
    rm -f "$DEST/$BINARY_NAME"
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
        remove_installer_path_lines "$rc"
    done
    log_success "Uninstalled $BINARY_NAME"
    exit 0
fi

detect_platform() {
    local os arch
    case "$(uname -s)" in
        Linux*) os="linux" ;;
        Darwin*) os="macos" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *) die "Unsupported OS: $(uname -s)" ;;
    esac
    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) die "Unsupported arch: $(uname -m)" ;;
    esac
    echo "${os}-${arch}"
}

resolve_version() {
    [ -n "$VERSION" ] && return 0
    VERSION=$(curl -fsSL \
        --connect-timeout 10 --max-time 30 \
        -H "Accept: application/vnd.github+json" \
        "https://api.github.com/repos/${OWNER}/${REPO}/releases/latest" \
        2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/') || true
    if [ -z "$VERSION" ]; then
        VERSION=$(curl -fsSL -o /dev/null -w '%{url_effective}' \
            "https://github.com/${OWNER}/${REPO}/releases/latest" \
            2>/dev/null | sed -E 's|.*/tag/||') || true
    fi
    [[ "$VERSION" =~ ^v[0-9] ]] || die "Could not resolve version"
}

download_file() {
    local url="$1" dest="$2"
    local partial="${dest}.part"
    local attempt=0
    while [ $attempt -lt $MAX_RETRIES ]; do
        attempt=$((attempt + 1))
        curl -fL \
            --connect-timeout 30 \
            --max-time "$DOWNLOAD_TIMEOUT" \
            --retry 2 \
            $( [ -s "$partial" ] && echo "--continue-at -" ) \
            $( [ "$QUIET" -eq 0 ] && [ -t 2 ] && echo "--progress-bar" || echo "-sS" ) \
            -o "$partial" "$url" && mv -f "$partial" "$dest" && return 0
        [ $attempt -lt $MAX_RETRIES ] && { log_warn "Retrying in 3s..."; sleep 3; }
    done
    return 1
}

checksum_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

maybe_add_path() {
    case ":$PATH:" in
        *":$DEST:"*) return 0 ;;
    esac
    if [ "$EASY" -eq 1 ]; then
        for rc in "$HOME/.zshrc" "$HOME/.bashrc"; do
            [ -f "$rc" ] && [ -w "$rc" ] || continue
            grep -qF "$DEST" "$rc" && continue
            printf '\nexport PATH="%s:$PATH"  # %s installer\n' "$DEST" "$BINARY_NAME" >> "$rc"
        done
        log_warn "PATH updated. Restart your shell or run: export PATH=\"$DEST:\$PATH\""
    fi
}

install_binary_atomic() {
    local src="$1" dest="$2"
    local tmp_dest="${dest}.tmp.$$"
    install -m 0755 "$src" "$tmp_dest"
    mv -f "$tmp_dest" "$dest" || { rm -f "$tmp_dest"; die "Failed to install binary"; }
}

install_from_source() {
    log_info "Building from source"
    cargo build --release --locked -p super-lazygit-app --bin "$BINARY_NAME"
    mkdir -p "$DEST"
    install_binary_atomic "target/release/$BINARY_NAME" "$DEST/$BINARY_NAME"
}

install_from_release() {
    local platform archive url binary_path
    platform="$(detect_platform)"
    archive="${BINARY_NAME}-${VERSION}-${platform}.tar.gz"
    case "$platform" in
        windows-*) archive="${BINARY_NAME}-${VERSION}-${platform}.zip" ;;
    esac
    url="https://github.com/${OWNER}/${REPO}/releases/download/${VERSION}/${archive}"
    TMP="$(mktemp -d)"
    download_file "$url" "$TMP/$archive" || die "Failed to download $url"

    if download_file "${url}.sha256" "$TMP/checksum.sha256" 2>/dev/null; then
        expected=$(awk '{print $1}' "$TMP/checksum.sha256")
        actual=$(checksum_file "$TMP/$archive")
        [ "$expected" = "$actual" ] || die "Checksum mismatch"
    fi

    mkdir -p "$TMP/unpack"
    case "$archive" in
        *.tar.gz) tar -xzf "$TMP/$archive" -C "$TMP/unpack" ;;
        *.zip) unzip -q "$TMP/$archive" -d "$TMP/unpack" ;;
        *) die "Unsupported archive format: $archive" ;;
    esac

    binary_path=$(find "$TMP/unpack" -type f \( -name "$BINARY_NAME" -o -name "$BINARY_NAME.exe" \) | head -n 1)
    [ -n "$binary_path" ] || die "Installed archive did not contain $BINARY_NAME"
    mkdir -p "$DEST"
    install_binary_atomic "$binary_path" "$DEST/$BINARY_NAME"
}

main() {
    acquire_lock
    resolve_version
    if [ "$FROM_SOURCE" -eq 1 ]; then
        install_from_source
    else
        install_from_release
    fi
    maybe_add_path
    if [ "$VERIFY" -eq 1 ]; then
        "$DEST/$BINARY_NAME" --version >/dev/null
    fi
    log_success "Installed $BINARY_NAME ${VERSION} to $DEST"
}

main "$@"
