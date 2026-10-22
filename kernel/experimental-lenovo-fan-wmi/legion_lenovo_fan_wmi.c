// SPDX-License-Identifier: GPL-2.0-only
/*
 * Deliberately disabled Lenovo Legion fan WMI skeleton.
 *
 * This file is not registered as a wmi_driver and has no module init, probe,
 * remove, shutdown, or firmware-evaluation path.  Its disabled helper always
 * returns -EOPNOTSUPP before any WMI method could be evaluated.
 *
 * It records evidence specific to Lenovo Legion Slim 5 16ARP9 (83EX), BIOS
 * NUCN15WW. It is not evidence of compatibility with another model or BIOS.
 */

#include <linux/acpi.h>
#include <linux/errno.h>
#include <linux/module.h>
#include <linux/types.h>
#include <linux/wmi.h>

#include "legion_fan_read_contract.h"

#define LENOVO_FAN_WMI_GUID "92549549-4BDE-4F06-AC04-CE8BF898DBAA"
#define LENOVO_FAN_WMI_INSTANCE 0
#define LENOVO_FAN_GET_TABLE_METHOD 5
#define LENOVO_FAN_GET_TABLE_INPUT_BYTES 2
#define LENOVO_FAN_GET_TABLE_FAT2_BYTES LEGION_FAN_READ_FAT2_BYTES
#define LENOVO_FAN_GET_TABLE_POINTS 10

/* Confirmed AML input order: [FanID, SensorID]. */
struct legion_fan_get_table_request {
	u8 fan_id;
	u8 sensor_id;
} __packed;

/*
 * Confirmed FAT2 byte layout.  These are opaque little-endian words here:
 * this driver does not assign physical meanings to their values.
 */
struct legion_fan_get_table_fat2 {
	__le32 fan_table_size;
	__le32 fan_table[LENOVO_FAN_GET_TABLE_POINTS];
	__le32 sensor_table_size;
	__le32 sensor_table[LENOVO_FAN_GET_TABLE_POINTS];
} __packed;

static_assert(sizeof(struct legion_fan_get_table_request) ==
	      LENOVO_FAN_GET_TABLE_INPUT_BYTES);
static_assert(sizeof(struct legion_fan_get_table_fat2) ==
	      LENOVO_FAN_GET_TABLE_FAT2_BYTES);

/* Metadata only: this does not bind or register a WMI driver. */
static const struct wmi_device_id legion_fan_wmi_id_table[] = {
	{ .guid_string = LENOVO_FAN_WMI_GUID },
	{}
};
MODULE_DEVICE_TABLE(wmi, legion_fan_wmi_id_table);

/*
 * Deliberately unavailable. A future reviewed implementation must add its own
 * explicit activation barrier before it can call any kernel WMI API.  This
 * skeleton never evaluates method 5.
 */
static int __maybe_unused
legion_fan_wmi_read_once_disabled(struct wmi_device *wdev,
				  const struct legion_fan_get_table_request *request,
				  struct acpi_buffer *output)
{
	if (!wdev || !request || !output)
		return -EINVAL;

	return -EOPNOTSUPP;
}

MODULE_DESCRIPTION("Disabled LegionLinuxToolkit Lenovo fan WMI reader skeleton");
MODULE_LICENSE("GPL");
