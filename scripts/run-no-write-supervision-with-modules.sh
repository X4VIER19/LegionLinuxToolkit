#!/usr/bin/env bash
# Preparation only: run manually AFTER authorization for the module lifecycle
# and all 181 monitor samples. Default is no-write. Staged preset mode is
# separately constant-false gated; no module installation or retries.
# Recovery is best effort; SIGKILL, kernel hangs and power loss bypass traps.
set -euo pipefail
staged=0
if [[ ${1-} == I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET && $# == 1 ]]; then
    # Not a runtime override. Requires a separately approved activation diff.
    readonly STAGED_EXECUTION_AUTHORIZED=0
    (( STAGED_EXECUTION_AUTHORIZED == 1 )) || {
        echo 'BLOCKED: staged preset execution not authorized; no modules touched.' >&2; exit 2;
    }
    staged=1
elif [[ ${1-} != I-AUTHORIZE-TEMPORARY-MODULES-AND-NO-WRITE-SUPERVISION || $# != 1 ]]; then
    echo 'Requires separate authorization and the exact confirmation argument.' >&2; exit 2;
fi
[[ $EUID == 0 ]] || { echo 'Run manually with sudo after authorization.' >&2; exit 2; }
readonly repo=/home/x4vier19/Documentos/GitHub/LegionLinuxToolkit
readonly quirk=/tmp/legion-yogafan-83ex-build/yogafan_83ex.ko
helper=/tmp/legion-full-speed-approved-build-vXMsFP/legion_lenovo_fan_wmi.ko
helper_hash=9595c09f69e84bb472e4a33ee4deb54a42a92b5e76bdfc7a09714df8aa8bc6da
if (( staged )); then
    helper="$repo/target/fan-default-preset-staged/legion_lenovo_fan_wmi.ko"
    helper_hash=365727b56e4f8539cf05993599c3f6413c3720e13805fd7f49d9035b5769face
fi
readonly helper helper_hash
readonly monitor="$repo/target/debug/legion-cli-fan-trial-monitor"
readonly driver=/sys/bus/platform/drivers/yogafan
readonly native=/sys/bus/wmi/devices/DC2A8805-3A8C-41BA-A6F7-092E0089CD3B-25/driver
die() { echo "ABORT: $*" >&2; exit 1; }
loaded() { [[ -d /sys/module/$1 ]]; }
owner() { readlink -f "$driver/module"; }
bindings() {
    local p
    for p in "$driver"/*; do
        [[ -e $p && -e /sys/bus/platform/devices/${p##*/} ]] || continue
        printf '%s\n' "${p##*/}"
    done
}
[[ $(uname -r) == 7.2.11-200.nobara.fc44.x86_64 ]] || die 'Kernel mismatch.'
for spec in sys_vendor:LENOVO product_name:83EX 'product_version:Legion Slim 5 16ARP9' bios_version:NUCN15WW; do
    [[ $(<"/sys/devices/virtual/dmi/id/${spec%%:*}") == "${spec#*:}" ]] || die 'DMI mismatch.'
done
for file in "$quirk" "$helper" "$monitor"; do
    [[ -f $file && ! -L $file ]] || die "Missing/non-regular artifact: $file"
done
sha256sum --check --strict <<< "a67312115691a71c0934a24185b629b38eeed2f15fb2ca27d4db7d3e71e2faa5  $quirk
$helper_hash  $helper
a6777cfb8ca21f1a59494c1bfc598ebe50aaa317909fd457490a45b36bdb563e  $monitor"
[[ -x $monitor ]] || die 'Monitor not executable.'
[[ $(modinfo -F name "$quirk") == yogafan_83ex ]] || die 'Quirk name mismatch.'
[[ $(modinfo -F name "$helper") == legion_lenovo_fan_wmi ]] || die 'Helper name mismatch.'
for file in "$quirk" "$helper"; do
    [[ $(modinfo -F vermagic "$file") == '7.2.11-200.nobara.fc44.x86_64 SMP preempt mod_unload '* ]] || die 'Vermagic mismatch.'
done
loaded yogafan || die 'Original yogafan must already be loaded.'
! loaded yogafan_83ex || die 'Quirk already loaded.'
! loaded legion_lenovo_fan_wmi || die 'Helper already loaded.'
[[ ! -e /dev/legion-fan-full-speed-read-experimental && ! -e /dev/legion-fan-read-experimental && ! -e /dev/legion-fan-default-preset-experimental ]] || die 'Endpoint collision.'
[[ $(owner) == /sys/module/yogafan ]] || die 'Unexpected original driver owner.'
[[ $(</sys/module/yogafan/refcnt) == 0 ]] || die 'Original module in use.'
original=$(modinfo -n yogafan)
[[ -f $original ]] || die 'Original restoration artifact missing.'
mapfile -t initial_devices < <(bindings)
[[ ${#initial_devices[@]} == 1 ]] || die 'Expected one original platform binding.'
[[ $(readlink -f "$native") == /sys/bus/wmi/drivers/lenovo_wmi_other ]] || die 'Native WMI binding unexpected.'
if (( staged )); then
    [[ -d /sys/bus/wmi/devices/92549549-4BDE-4F06-AC04-CE8BF898DBAA-22 && ! -e /sys/bus/wmi/devices/92549549-4BDE-4F06-AC04-CE8BF898DBAA-22/driver ]] || die 'Fan GUID absent or already owned; no forced binding.'
fi
printf 'Original: %s\nDevice: %s\n' "$original" "${initial_devices[0]}"

replacement_started=0
helper_attempted=0
cleanup() {
    local status=$? failed=0
    trap - EXIT INT TERM HUP
    set +e
    if (( helper_attempted )) && loaded legion_lenovo_fan_wmi; then
        rmmod legion_lenovo_fan_wmi || failed=1
        if loaded legion_lenovo_fan_wmi; then
            echo 'RECOVERY ERROR: helper remains loaded; no forced unload.' >&2
            failed=1
        fi
    fi
    if (( replacement_started )); then
        if loaded yogafan_83ex; then
            rmmod yogafan_83ex || failed=1
        fi
        if loaded yogafan_83ex; then
            echo 'RECOVERY ERROR: quirk still loaded; original NOT reloaded. Never force.' >&2
            failed=1
        else
            if ! loaded yogafan; then
                if [[ $(modinfo -n yogafan) == "$original" && -f $original ]]; then
                    modprobe yogafan || failed=1
                else
                    echo 'RECOVERY ERROR: original module path changed.' >&2
                    failed=1
                fi
            fi
            if ! loaded yogafan || [[ $(owner) != /sys/module/yogafan || ! -e $driver/${initial_devices[0]} ]]; then
                echo 'RECOVERY ERROR: original binding not restored.' >&2
                failed=1
            fi
        fi
    fi
    if [[ $(readlink -f "$native") != /sys/bus/wmi/drivers/lenovo_wmi_other ]]; then
        echo 'RECOVERY ERROR: native WMI binding changed; do not force/unbind.' >&2
        failed=1
    fi
    if (( helper_attempted )) && [[ -e /dev/legion-fan-full-speed-read-experimental || -e /dev/legion-fan-read-experimental || -e /dev/legion-fan-default-preset-experimental ]]; then
        echo 'RECOVERY ERROR: experimental endpoint remains.' >&2
        failed=1
    fi
    (( failed == 0 )) || status=70
    printf 'Cleanup status: %s. Logs preserved in %s/trial-logs/\n' "$status" "$repo" >&2
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
# Flags precede attempts so traps also detect partial-success command failures.
replacement_started=1
rmmod yogafan
! loaded yogafan || die 'Original still loaded.'
insmod "$quirk"
[[ $(owner) == /sys/module/yogafan_83ex && -e $driver/${initial_devices[0]} ]] || die 'Quirk binding failed.'
helper_attempted=1
helper_parameters=(full_speed_confirmation=I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ)
if (( staged )); then
    helper_parameters+=(control_confirmation=I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET)
fi
insmod "$helper" "${helper_parameters[@]}"
[[ -c /dev/legion-fan-full-speed-read-experimental && -r /dev/legion-fan-full-speed-read-experimental ]] || die 'FullSpeed endpoint unavailable.'
[[ $(readlink -f "$native") == /sys/bus/wmi/drivers/lenovo_wmi_other ]] || die 'Native binding changed.'
channels=0
for hw in /sys/class/hwmon/hwmon*; do
    [[ -r $hw/name ]] || continue
    [[ $(<"$hw/name") == yogafan ]] || continue
    [[ $(readlink -f "$hw/device/driver/module") == /sys/module/yogafan_83ex ]] || continue
    [[ -r $hw/fan1_input && -r $hw/fan2_input ]] || die 'RPM channels unavailable.'
    channels=$((channels + 1))
    printf 'RPM inputs ready (not read): %s/fan{1,2}_input\n' "$hw"
done
[[ $channels == 1 ]] || die 'Missing/ambiguous quirk hwmon.'
if (( staged )); then
    [[ -c /dev/legion-fan-default-preset-experimental && -w /dev/legion-fan-default-preset-experimental ]] || die 'Preset endpoint unavailable.'
    [[ $(readlink -f /sys/bus/wmi/devices/92549549-4BDE-4F06-AC04-CE8BF898DBAA-22/driver) == /sys/bus/wmi/drivers/legion-fan-wmi-experimental ]] || die 'Fan WMI binding missing.'
    "$monitor" --staged-default-trial --confirm I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET --attest-idle
    exit
fi
# No cat/dd of RPM or FullSpeed here. Keep BOTH modules loaded until this
# foreground process exits (181 samples, or the first error), then trap cleanup.
"$monitor" --live-no-write-rehearsal \
    --confirm I-AUTHORIZE-READ-ONLY-180S-83EX-SUPERVISION --attest-idle
