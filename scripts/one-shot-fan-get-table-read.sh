#!/usr/bin/env bash
# One manually authorized Fan_Get_Table(1,4) read for the documented 83EX.
#
# This script loads a temporary, locally built experimental module, performs
# exactly one 100-byte read from its miscdevice, and removes the module before
# it exits. That one device read may invoke WMI method 5. Do not run it without
# explicit authorization for that firmware invocation.

set -euo pipefail

readonly EXPECTED_COMMIT="86421d35f0d3bef67694ebaa4c73a7ac2cafac40"
readonly MODULE_NAME="legion_lenovo_fan_wmi"
readonly DEVICE_PATH="/dev/legion-fan-read-experimental"
readonly DRIVER_NAME="legion-fan-wmi-experimental"
readonly WMI_GUID="92549549-4BDE-4F06-AC04-CE8BF898DBAA"
readonly WMI_DEVICE="/sys/bus/wmi/devices/${WMI_GUID}-22"
readonly DMI_ROOT="/sys/devices/virtual/dmi/id"

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
build_dir=""
response_file=""
module_load_attempted=0

module_is_loaded() {
    test -d "/sys/module/${MODULE_NAME}"
}

cleanup() {
    local result=$?
    trap - EXIT INT TERM

    # Check the kernel's module list rather than only trusting insmod's exit
    # status: an interrupted or otherwise failing insmod could still leave the
    # module loaded.
    if [ "$module_load_attempted" -eq 1 ] && module_is_loaded; then
        if ! sudo rmmod "$MODULE_NAME"; then
            printf 'ERROR: %s remains loaded; do not use rmmod -f.\n' "$MODULE_NAME" >&2
            printf 'Response file retained: %s\n' "${response_file:-not-created}" >&2
            exit 70
        fi
    fi

    if module_is_loaded || [ -e "$DEVICE_PATH" ]; then
        printf 'ERROR: module resources remain present after cleanup.\n' >&2
        printf 'Response file retained: %s\n' "${response_file:-not-created}" >&2
        exit 71
    fi

    if [ -n "$build_dir" ]; then
        rm -rf -- "$build_dir"
    fi

    if [ -n "$response_file" ]; then
        printf 'Response file retained: %s\n' "$response_file"
    fi
    exit "$result"
}
trap cleanup EXIT INT TERM

read_dmi_attribute() {
    local attribute=$1
    local value
    IFS= read -r value < "${DMI_ROOT}/${attribute}" || true
    printf '%s' "$value"
}

cd -- "$repo_root"
test "$(git rev-parse HEAD)" = "$EXPECTED_COMMIT"

kernel_release="$(uname -r)"
kernel_build="/lib/modules/${kernel_release}/build"
test -d "$kernel_build"
# `make kernelrelease` may omit the distribution-specific local version.
# Kbuild records the full release used to configure these headers here.
kernel_build_release_file="${kernel_build}/include/config/kernel.release"
test -r "$kernel_build_release_file"
kernel_build_release="$(tr -d '\n' < "$kernel_build_release_file")"
test "$kernel_build_release" = "$kernel_release"

test "$(read_dmi_attribute sys_vendor)" = "LENOVO"
test "$(read_dmi_attribute product_name)" = "83EX"
test "$(read_dmi_attribute product_version)" = "Legion Slim 5 16ARP9"
test "$(read_dmi_attribute bios_version)" = "NUCN15WW"

test -d "$WMI_DEVICE"
test ! -e "${WMI_DEVICE}/driver"
test ! -d "/sys/module/${MODULE_NAME}"
test ! -e "$DEVICE_PATH"

build_dir="$(mktemp -d /tmp/legion-fan-wmi-real-read-XXXXXX)"
response_file="$(mktemp /tmp/legion-fan-fat2-response-XXXXXX.bin)"
cp -a kernel/experimental-lenovo-fan-wmi/. "$build_dir/"

make -C "$kernel_build" \
    M="$build_dir" \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI=m \
    CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ=y \
    modules

# Interactive sudo authorization occurs before the module load. The script is
# intentionally run as the regular user so the captured file remains usable by
# the offline Rust analyzer.
sudo -v
module_load_attempted=1
if ! sudo insmod "$build_dir/${MODULE_NAME}.ko"; then
    printf 'ERROR: insmod failed; checking whether the module was nevertheless loaded.\n' >&2
    exit 1
fi

test -c "$DEVICE_PATH"
test -L "${WMI_DEVICE}/driver"
test "$(basename "$(readlink -f "${WMI_DEVICE}/driver")")" = "$DRIVER_NAME"

# Exactly one kernel read request. Do not add iflag=fullblock, loops, retries,
# or another read command: a 100-byte request can invoke method 5 once.
sudo dd if="$DEVICE_PATH" bs=100 count=1 status=none > "$response_file"
test "$(stat -c %s "$response_file")" = "100"

# Normal exit invokes cleanup(), which performs the only rmmod attempt.
