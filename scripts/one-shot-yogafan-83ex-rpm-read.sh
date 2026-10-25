#!/usr/bin/env bash
# One manually authorized, temporary yogafan 83EX RPM validation.
#
# This script unloads only the currently loaded `yogafan` module, loads the
# supplied experimental module once, reads each discovered fan input once, and
# attempts restoration on every normal shell exit. It never writes a hwmon
# control, does not refer to acpi_fan, and must not be run without separate
# authorization for this exact module lifecycle and the two ACPI reads.
#
# Recovery is best effort only. A shell trap cannot recover from a kernel hang,
# SIGKILL, power loss, lost privileges, or an unsuccessful module reload.

set -euo pipefail

readonly EXPECTED_KERNEL="7.2.11-200.nobara.fc44.x86_64"
readonly EXPECTED_VENDOR="LENOVO"
readonly EXPECTED_PRODUCT="83EX"
readonly EXPECTED_MODEL="Legion Slim 5 16ARP9"
readonly EXPECTED_BIOS="NUCN15WW"
readonly ORIGINAL_MODULE="yogafan"
readonly EXPERIMENTAL_MODULE="yogafan_83ex"
readonly CONFIRMATION="I-UNDERSTAND-TEMPORARY-YOGAFAN-83EX-READ"
readonly DMI_ROOT="/sys/devices/virtual/dmi/id"
readonly PLATFORM_DRIVER_ROOT="/sys/bus/platform/drivers/yogafan"

experimental_path=""
log_file=""
original_module_path=""
original_was_loaded=0
original_unloaded=0
experimental_load_attempted=0

usage() {
    cat <<'EOF'
Usage:
  sudo one-shot-yogafan-83ex-rpm-read.sh \
    --module /absolute/path/yogafan_83ex.ko \
    --confirm I-UNDERSTAND-TEMPORARY-YOGAFAN-83EX-READ

The supplied module must be the isolated 83EX build. No other module name,
kernel release, DMI identity, pre-existing binding, or driver state is accepted.
EOF
}

log() {
    printf '%s\n' "$*" | tee -a "$log_file"
}

die() {
    log "ABORT: $*"
    exit 1
}

module_loaded() {
    test -d "/sys/module/$1"
}

read_dmi() {
    local attribute=$1
    local value
    IFS= read -r value < "${DMI_ROOT}/${attribute}" || true
    printf '%s' "$value"
}

driver_device_names() {
    local candidate

    test -d "$PLATFORM_DRIVER_ROOT" || return 1
    for candidate in "$PLATFORM_DRIVER_ROOT"/*; do
        test -e "$candidate" || continue
        if test -e "/sys/bus/platform/devices/$(basename "$candidate")"; then
            basename "$candidate"
        fi
    done
}

driver_device_count() {
    local name
    local count=0

    while IFS= read -r name; do
        test -n "$name" || continue
        count=$((count + 1))
    done < <(driver_device_names)
    printf '%s' "$count"
}

require_single_yogafan_binding() {
    local count
    count="$(driver_device_count)" || return 1
    test "$count" = "1"
}

restore_original() {
    local restore_failed=0

    if module_loaded "$EXPERIMENTAL_MODULE"; then
        log "Cleanup: attempting to unload ${EXPERIMENTAL_MODULE}."
        if ! rmmod "$EXPERIMENTAL_MODULE"; then
            log "ERROR: ${EXPERIMENTAL_MODULE} remains loaded; do not use rmmod -f."
            restore_failed=1
        fi
    fi

    if test "$original_unloaded" = "1" && ! module_loaded "$ORIGINAL_MODULE"; then
        log "Cleanup: attempting to restore ${ORIGINAL_MODULE} from ${original_module_path}."
        if ! modprobe "$ORIGINAL_MODULE"; then
            log "ERROR: unable to restore ${ORIGINAL_MODULE}."
            restore_failed=1
        fi
    fi

    if test "$original_was_loaded" = "1"; then
        if ! module_loaded "$ORIGINAL_MODULE" || ! require_single_yogafan_binding; then
            log "ERROR: original yogafan is not restored and singly bound."
            restore_failed=1
        fi
    fi

    return "$restore_failed"
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM HUP
    set +e

    if ! restore_original; then
        log "Manual recovery, if the experimental module is still loaded:"
        log "  sudo rmmod ${EXPERIMENTAL_MODULE}"
        log "  sudo modprobe ${ORIGINAL_MODULE}"
        log "  test -d /sys/module/${ORIGINAL_MODULE}"
        log "  test -L /sys/bus/platform/drivers/yogafan/*/driver"
        status=70
    fi

    log "Log retained: ${log_file}"
    exit "$status"
}

test "$(id -u)" = "0" || {
    printf 'Run this script through sudo after explicit authorization.\n' >&2
    exit 64
}

if test "$#" != "4" || test "$1" != "--module" || test "$3" != "--confirm"; then
    usage >&2
    exit 64
fi
experimental_path=$2
test "$4" = "$CONFIRMATION" || {
    usage >&2
    exit 64
}

test "${experimental_path#/}" != "$experimental_path" || {
    printf 'Module path must be absolute.\n' >&2
    exit 64
}
test -f "$experimental_path" && ! test -L "$experimental_path" || {
    printf 'Experimental module must be a regular, non-symlink file.\n' >&2
    exit 64
}
test "$(basename "$experimental_path")" = "${EXPERIMENTAL_MODULE}.ko" || {
    printf 'Experimental module filename must be %s.ko.\n' "$EXPERIMENTAL_MODULE" >&2
    exit 64
}

log_file="$(mktemp /tmp/legion-yogafan-83ex-rpm-read-XXXXXX.log)"
trap cleanup EXIT INT TERM HUP
log "Starting authorized one-shot yogafan 83EX validation."

test "$(uname -r)" = "$EXPECTED_KERNEL" || die "unexpected kernel release"
test "$(read_dmi sys_vendor)" = "$EXPECTED_VENDOR" || die "unexpected DMI vendor"
test "$(read_dmi product_name)" = "$EXPECTED_PRODUCT" || die "unexpected DMI product"
test "$(read_dmi product_version)" = "$EXPECTED_MODEL" || die "unexpected DMI model"
test "$(read_dmi bios_version)" = "$EXPECTED_BIOS" || die "unexpected DMI BIOS"

test "$(modinfo -F name "$experimental_path")" = "$EXPERIMENTAL_MODULE" || die "unexpected experimental module name"
test "$(modinfo -F vermagic "$experimental_path" | awk '{print $1}')" = "$EXPECTED_KERNEL" || die "experimental vermagic mismatch"
test "$(modinfo -n "$ORIGINAL_MODULE")" != "(builtin)" || die "original yogafan is built in"
original_module_path="$(modinfo -n "$ORIGINAL_MODULE")"
test -f "$original_module_path" || die "original yogafan module file is unavailable"

module_loaded "$ORIGINAL_MODULE" || die "original yogafan is not loaded"
original_was_loaded=1
! module_loaded "$EXPERIMENTAL_MODULE" || die "experimental module is already loaded"
require_single_yogafan_binding || die "original yogafan binding is absent or ambiguous"
test "$(cat "/sys/module/${ORIGINAL_MODULE}/refcnt")" = "0" || die "original yogafan has references"
log "Initial module: ${ORIGINAL_MODULE} from ${original_module_path}."
log "Initial binding: $(driver_device_names)."

if ! rmmod "$ORIGINAL_MODULE"; then
    die "could not unload original yogafan"
fi
original_unloaded=1
! module_loaded "$ORIGINAL_MODULE" || die "original yogafan remains loaded after rmmod"

experimental_load_attempted=1
if ! insmod "$experimental_path"; then
    die "could not load experimental yogafan module"
fi
module_loaded "$EXPERIMENTAL_MODULE" || die "experimental module did not remain loaded"
require_single_yogafan_binding || die "experimental yogafan binding is absent or ambiguous"
log "Experimental binding: $(driver_device_names)."

hwmon_paths=()
for hwmon in /sys/class/hwmon/hwmon*; do
    test -d "$hwmon" || continue
    test "$(cat "$hwmon/name")" = "yogafan" || continue
    test -r "$hwmon/fan1_input" && test -r "$hwmon/fan2_input" || continue
    hwmon_paths+=("$hwmon")
done
test "${#hwmon_paths[@]}" = "1" || die "expected exactly one two-channel yogafan hwmon device"
log "Experimental hwmon path: ${hwmon_paths[0]}."

# Each redirection invokes one hwmon read. Do not add retries, loops, or
# preliminary reads here: these are the only two authorized ACPI evaluations.
IFS= read -r fan1_rpm < "${hwmon_paths[0]}/fan1_input" || die "fan1_input read failed"
IFS= read -r fan2_rpm < "${hwmon_paths[0]}/fan2_input" || die "fan2_input read failed"
log "fan1_input reported: ${fan1_rpm}"
log "fan2_input reported: ${fan2_rpm}"

# The EXIT trap performs the only experimental unload and original restoration.
