// SPDX-License-Identifier: GPL-2.0-only
/*
 * Deliberately disabled Lenovo Legion fan WMI skeleton.
 *
 * This file is not registered as a wmi_driver. Its inert probe and remove
 * callbacks exist only to validate the kernel API shape and never evaluate
 * firmware. Its init and exit functions manage only the experimental
 * read-only miscdevice.
 *
 * It records evidence specific to Lenovo Legion Slim 5 16ARP9 (83EX), BIOS
 * NUCN15WW. It is not evidence of compatibility with another model or BIOS.
 */

#include <linux/acpi.h>
#include <linux/errno.h>
#include <linux/fs.h>
#include <linux/miscdevice.h>
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

/*
 * Binding metadata for a future reviewed activation. Do not add
 * MODULE_DEVICE_TABLE(): the experimental module must not acquire a WMI
 * modalias or automatic loading path.
 */
static const struct wmi_device_id legion_fan_wmi_id_table[] = {
	{ .guid_string = LENOVO_FAN_WMI_GUID },
	{}
};

/* A registered driver still fails closed without firmware access. */
static int legion_fan_wmi_probe_disabled(struct wmi_device *wdev,
					 const void *context)
{
	(void)wdev;
	(void)context;

	return -EOPNOTSUPP;
}

static void legion_fan_wmi_remove_disabled(struct wmi_device *wdev)
{
	(void)wdev;
}

static struct wmi_driver legion_fan_wmi_driver = {
	.driver = {
		.name = "legion-fan-wmi-experimental",
	},
	.id_table = legion_fan_wmi_id_table,
	.probe = legion_fan_wmi_probe_disabled,
	.remove = legion_fan_wmi_remove_disabled,
};

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

/* Proposed binary endpoint. Reads fail closed without firmware access. */
static ssize_t legion_fan_endpoint_read_disabled(struct file *file,
					 char __user *buffer, size_t count,
					 loff_t *offset)
{
	(void)file;
	(void)buffer;
	(void)count;
	(void)offset;

	return -EOPNOTSUPP;
}

static const struct file_operations legion_fan_endpoint_fops = {
	.owner = THIS_MODULE,
	.read = legion_fan_endpoint_read_disabled,
	.llseek = noop_llseek,
};

static struct miscdevice legion_fan_endpoint = {
	.minor = MISC_DYNAMIC_MINOR,
	.name = "legion-fan-read-experimental",
	.fops = &legion_fan_endpoint_fops,
};

/* Registers the endpoint first and rolls it back if WMI registration fails. */
static int __init legion_fan_endpoint_init(void)
{
	int error;

	error = misc_register(&legion_fan_endpoint);
	if (error)
		return error;

	error = wmi_driver_register(&legion_fan_wmi_driver);
	if (error)
		misc_deregister(&legion_fan_endpoint);

	return error;
}

static void __exit legion_fan_endpoint_exit(void)
{
	wmi_driver_unregister(&legion_fan_wmi_driver);
	misc_deregister(&legion_fan_endpoint);
}

module_init(legion_fan_endpoint_init);
module_exit(legion_fan_endpoint_exit);

MODULE_DESCRIPTION("Disabled LegionLinuxToolkit Lenovo fan WMI reader skeleton");
MODULE_LICENSE("GPL");
