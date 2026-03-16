#!/usr/bin/env bash
# zenmux-adapter installer
#
# Usage:
#   curl --proto '=https' --tlsv1.2 -sSf \
#     https://raw.githubusercontent.com/aitiotekt/zenmux-adapter/main/scripts/install.sh | sh

set -euo pipefail

REPO="aitiotekt/zenmux-adapter"
BINARY="zenmux-adapter"
INSTALL_DIR="${HOME}/.local/bin"

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()    { printf "${BLUE}info${NC}:    %s\n"    "$*"; }
success() { printf "${GREEN}ok${NC}:      %s\n"    "$*"; }
warn()    { printf "${YELLOW}warning${NC}: %s\n"    "$*"; }
err()     { printf "${RED}error${NC}:   %s\n"    "$*" >&2; exit 1; }

# ── Platform detection ───────────────────────────────────────────────────────
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux"  ;;
        Darwin*) echo "macos"  ;;
        *)       err "Unsupported OS: $(uname -s)" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64"  ;;
        aarch64|arm64)  echo "aarch64" ;;
        *)              err "Unsupported architecture: $(uname -m)" ;;
    esac
}

get_target() {
    local os="$1" arch="$2"
    case "${os}-${arch}" in
        linux-x86_64)   echo "x86_64-unknown-linux-musl"  ;;
        linux-aarch64)  echo "aarch64-unknown-linux-musl" ;;
        macos-x86_64)   echo "x86_64-apple-darwin"        ;;
        macos-aarch64)  echo "aarch64-apple-darwin"        ;;
        *)              err "No pre-built binary for ${os}/${arch}" ;;
    esac
}

# ── Version resolution ───────────────────────────────────────────────────────
get_latest_version() {
    local version
    if command -v curl &>/dev/null; then
        version=$(curl -fsSL \
            "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
    elif command -v wget &>/dev/null; then
        version=$(wget -qO- \
            "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
    else
        err "curl or wget is required"
    fi

    if [[ -z "$version" ]]; then
        err "Failed to fetch the latest release version (GitHub API rate limit?)"
    fi
    echo "$version"
}

# ── Download helper ──────────────────────────────────────────────────────────
download() {
    local url="$1" dest="$2"
    if command -v curl &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$dest"
    else
        wget --https-only -qO "$dest" "$url"
    fi
}

# ── Main ─────────────────────────────────────────────────────────────────────
main() {
    printf "\n${BOLD}Installing ${BINARY}${NC}\n\n"

    info "Detecting platform..."
    local os arch target
    os=$(detect_os)
    arch=$(detect_arch)
    target=$(get_target "$os" "$arch")
    info "Platform : ${os} / ${arch}"
    info "Target   : ${target}"

    info "Fetching latest release..."
    local version
    version=$(get_latest_version)
    info "Version  : ${version}"

    local filename="${BINARY}-${version}-${target}.tar.gz"
    local url="https://github.com/${REPO}/releases/download/${version}/${filename}"

    info "Downloading ${filename} ..."
    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "${tmp_dir}"' EXIT

    download "$url" "${tmp_dir}/${filename}"

    info "Extracting..."
    tar -xzf "${tmp_dir}/${filename}" -C "${tmp_dir}"

    info "Installing to ${INSTALL_DIR}/${BINARY} ..."
    mkdir -p "${INSTALL_DIR}"
    install -m 755 "${tmp_dir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

    printf "\n"
    success "Installed ${BINARY} ${version} → ${INSTALL_DIR}/${BINARY}"

    # PATH hint
    if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
        printf "\n"
        warn "${INSTALL_DIR} is not in your \$PATH."
        warn "Add this line to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        warn "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
    fi

    printf "\n"
    info "Run \`${BINARY} --help\` to get started."
    printf "\n"
}

main "$@"
