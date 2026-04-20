#!/usr/bin/env bash
set -euo pipefail

# --- Colors ---
RESET="\033[0m"
BOLD="\033[1m"
MAGENTA="\033[35m"
CYAN="\033[36m"
GREEN="\033[32m"
RED="\033[31m"

# --- Spinner setup ---
SPINNER=(⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏)
SPIN_IDX=0
TOTAL=6
CURRENT=0

draw() {
    local msg="$1"
    printf "\r${MAGENTA}${BOLD}%s${RESET} ${CYAN}${BOLD}%2d/%d${RESET} ${GREEN}%s${RESET}" \
        "${SPINNER[$SPIN_IDX]}" "$CURRENT" "$TOTAL" "$msg"
}

tick() {
    SPIN_IDX=$(( (SPIN_IDX + 1) % ${#SPINNER[@]} ))
}

run_step() {
    local msg="$1"
    shift

    CURRENT=$((CURRENT + 1))

    local tmp_err
    tmp_err=$(mktemp)

    (
        while true; do
            draw "$msg"
            tick
            sleep 0.08
        done
    ) &
    local spinner_pid=$!

    if ! "$@" > /dev/null 2>"$tmp_err"; then
        kill "$spinner_pid" 2>/dev/null || true
        wait "$spinner_pid" 2>/dev/null || true

        printf "\n${RED}[✗] Failed:${RESET} %s\n" "$msg"
        cat "$tmp_err"
        rm -f "$tmp_err"
        exit 1
    fi

    kill "$spinner_pid" 2>/dev/null || true
    wait "$spinner_pid" 2>/dev/null || true

    rm -f "$tmp_err"
    draw "$msg"
}

# --- Sanity check ---
if ! command -v pacman >/dev/null; then
    echo -e "${RED}This script only supports Arch Linux (btw).${RESET}"
    exit 1
fi

# --- Steps ---
run_step "Installing packages" sudo pacman -Syu --needed --noconfirm \
    base-devel clang lld nasm python qemu-full edk2-ovmf mtools dosfstools rustup

if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

run_step "Setting Rust stable" rustup default stable
run_step "Adding target (none)" rustup target add x86_64-unknown-none
run_step "Adding target (uefi)" rustup target add x86_64-unknown-uefi

run_step "Verifying toolchain" bash -c '
    command -v clang &&
    command -v nasm &&
    command -v ld &&
    command -v objcopy &&
    command -v qemu-system-x86_64 &&
    command -v mkfs.fat &&
    command -v mcopy &&
    command -v python3
'

printf "\n${GREEN}[✓] Environment ready.${RESET}\n"
