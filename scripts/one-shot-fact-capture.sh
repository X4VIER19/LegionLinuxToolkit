#!/usr/bin/env bash
# PREPARATION ONLY: run manually only after separate approval of four WQA7 queries.
set -euo pipefail
[[ $# == 2 && $1 == --confirm && $2 == I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE ]] || {
    printf 'Exact --confirm I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE required.\n' >&2
    exit 2
}
readonly module_name=legion_lenovo_fan_wmi
readonly endpoint=/dev/legion-fact-read-experimental
readonly kernel_release=7.2.11-200.nobara.fc44.x86_64
readonly kernel_build=/usr/src/kernels/7.2.11-200.nobara.fc44.x86_64
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
[[ $(uname -r) == "$kernel_release" ]]
[[ $(<"$kernel_build/include/config/kernel.release") == "$kernel_release" ]]
[[ ! -d /sys/module/$module_name && ! -e $endpoint ]]
readonly dmi=/sys/devices/virtual/dmi/id
[[ $(<"$dmi/sys_vendor") == LENOVO && $(<"$dmi/product_name") == 83EX ]]
[[ $(<"$dmi/product_version") == 'Legion Slim 5 16ARP9' ]]
[[ $(<"$dmi/bios_version") == NUCN15WW ]]
shopt -s nullglob
devices=(/sys/bus/wmi/devices/87FB2A6D-D802-48E7-9208-4576C5F5C8D8-*)
[[ ${#devices[@]} == 1 ]]
[[ $(<"${devices[0]}/instance_count") == 15 ]]
[[ $(<"${devices[0]}/object_id") == A7 ]]

build_dir="$(mktemp -d /tmp/legion-fact-build-XXXXXX)"
capture="$(mktemp /tmp/legion-fact-capture-XXXXXX.bin)"
load_attempted=0
cleanup() {
    local result=$?
    trap - EXIT INT TERM
    if [[ $load_attempted == 1 && -d /sys/module/$module_name ]]; then
        if ! sudo rmmod "$module_name"; then
            printf 'ERROR: module remains loaded; never force removal. Build retained: %s\nCapture retained: %s\n' "$build_dir" "$capture" >&2
            exit 70
        fi
    fi
    if [[ -d /sys/module/$module_name || -e $endpoint ]]; then
        printf 'ERROR: unexpected remaining resources; capture/build retained.\n' >&2
        exit 71
    fi
    # Only the exact private mktemp build created above; capture is never deleted.
    rm -rf -- "$build_dir"
    printf 'Capture retained (including errors): %s\n' "$capture" >&2
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cp -a "$repo_root/kernel/experimental-lenovo-fan-wmi/." "$build_dir/"
env -u KCFLAGS -u KCPPFLAGS make -C "$kernel_build" M="$build_dir" \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI=m \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ=y \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ= \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ= \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING= modules
[[ $(modinfo -F name "$build_dir/$module_name.ko") == "$module_name" ]]
vermagic="$(modinfo -F vermagic "$build_dir/$module_name.ko")"
[[ ${vermagic%% *} == "$kernel_release" ]]
cd -- "$repo_root"
/home/x4vier19/.cargo/bin/cargo build --locked --offline -p legion-linux \
    --features experimental-fact-read --bin legion-linux-fact
sudo -v
load_attempted=1
sudo insmod "$build_dir/$module_name.ko" \
    fact_confirmation=I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE
[[ -d /sys/module/$module_name && -c $endpoint ]]
# One process, one read syscall, four fixed WQA7 queries at most. No retries.
# Redirection is by this unprivileged shell; stdout contains binary data only.
sudo "$repo_root/target/debug/legion-linux-fact" capture \
    --confirm I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE > "$capture"
# EXIT immediately unloads, including when the command above reports failure.
