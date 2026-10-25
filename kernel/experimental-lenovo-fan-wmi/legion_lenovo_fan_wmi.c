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
#include <linux/dmi.h>
#include <linux/fs.h>
#include <linux/kconfig.h>
#include <linux/miscdevice.h>
#include <linux/mutex.h>
#include <linux/module.h>
#include <linux/slab.h>
#include <linux/types.h>
#include <linux/uaccess.h>
#include <linux/unaligned.h>
#include <linux/wmi.h>

#include "legion_fan_read_contract.h"
#include "control_policy.h"

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

/* Protected passive binding state; it is set and cleared without WMI calls. */
static DEFINE_MUTEX(legion_fan_wmi_lock);
static struct wmi_device *legion_fan_wmi_device;

/*
 * The WMI core calls this only after the exact GUID in id_table matches. This
 * establishes passive ownership of that device; it does not query firmware.
 */
static int legion_fan_wmi_probe(struct wmi_device *wdev, const void *context)
{
	(void)context;

	mutex_lock(&legion_fan_wmi_lock);
	if (legion_fan_wmi_device && legion_fan_wmi_device != wdev) {
		mutex_unlock(&legion_fan_wmi_lock);
		return -EBUSY;
	}
	legion_fan_wmi_device = wdev;
	mutex_unlock(&legion_fan_wmi_lock);

	return 0;
}

static void legion_fan_wmi_remove(struct wmi_device *wdev)
{
	mutex_lock(&legion_fan_wmi_lock);
	if (legion_fan_wmi_device == wdev)
		legion_fan_wmi_device = NULL;
	mutex_unlock(&legion_fan_wmi_lock);
}

static struct wmi_driver legion_fan_wmi_driver = {
	.driver = {
		.name = "legion-fan-wmi-experimental",
	},
	.id_table = legion_fan_wmi_id_table,
	.probe = legion_fan_wmi_probe,
	.remove = legion_fan_wmi_remove,
};

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING)
/* No setter, module parameter, endpoint or caller exists for this barrier.
 * Enabling it requires a new reviewed code change, not a load-time argument.
 */
static const bool legion_control_execution_authorized = false;
static atomic_t legion_control_attempted = ATOMIC_INIT(0);

/* Metadata must eventually come from independently validated observation;
 * caller assertions alone must never authorize execution. Currently uncalled.
 */
static int __maybe_unused legion_fan_default_control_staged(
 const u8 *payload, size_t size, int gz44, int full_speed)
{
 struct acpi_buffer input = { .length = 64, .pointer = (void *)payload };
 struct acpi_buffer output = { .length = ACPI_ALLOCATE_BUFFER };
 union acpi_object *object;
 acpi_status status;
 int error = 0;

 if (!legion_control_execution_authorized)
  return -EOPNOTSUPP;
 error = legion_control_policy(payload, size,
  dmi_match(DMI_SYS_VENDOR, "LENOVO") &&
  dmi_match(DMI_PRODUCT_NAME, "83EX") &&
  dmi_match(DMI_PRODUCT_VERSION, "Legion Slim 5 16ARP9") &&
  dmi_match(DMI_BIOS_VERSION, "NUCN15WW"), gz44, full_speed);
 if (error)
  return error;
 if (atomic_cmpxchg(&legion_control_attempted, 0, 1))
  return -EALREADY;
 mutex_lock(&legion_fan_wmi_lock);
 if (!legion_fan_wmi_device) {
  error = -ENODEV;
  goto unlock_control;
 }
 status = wmidev_evaluate_method(legion_fan_wmi_device,
  LENOVO_FAN_WMI_INSTANCE, 6, &input, &output);
 if (ACPI_FAILURE(status)) {
  error = -EIO;
  goto free_control;
 }
 object = output.pointer;
 /* WMAB returns Integer(0), including on a skipped SFAN. This validates
  * only the response shape and is NOT proof of successful application.
  */
 if (!object || object->type != ACPI_TYPE_INTEGER || object->integer.value)
  error = -EPROTO;
free_control:
 kfree(output.pointer);
unlock_control:
 mutex_unlock(&legion_fan_wmi_lock);
 return error;
}
#endif

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ)
static void legion_fan_response_init(struct legion_fan_read_response_v1 *response)
{
	memset(response, 0, sizeof(*response));
	response->version = cpu_to_le16(LEGION_FAN_READ_CONTRACT_VERSION);
	response->header_size = cpu_to_le16(LEGION_FAN_READ_HEADER_BYTES);
	response->status = cpu_to_le32(LEGION_FAN_READ_UNSUPPORTED);
}

static void legion_fan_response_set_status(
	struct legion_fan_read_response_v1 *response,
	enum legion_fan_read_status status)
{
	response->status = cpu_to_le32(status);
	response->payload_size = cpu_to_le32(0);
}

/*
 * This code is compiled only by an explicit, non-default Kconfig option. It
 * is reachable solely from the read callback below, never from probe/init/
 * remove/exit. The returned FAT2 words remain opaque at this layer.
 */
static void legion_fan_get_table_once(
	struct legion_fan_read_response_v1 *response)
{
	const struct legion_fan_get_table_request request = {
		.fan_id = 1,
		.sensor_id = 4,
	};
	struct acpi_buffer input = {
		.length = sizeof(request),
		.pointer = (void *)&request,
	};
	struct acpi_buffer output = {
		.length = ACPI_ALLOCATE_BUFFER,
		.pointer = NULL,
	};
	union acpi_object *object;
	acpi_status status;

	mutex_lock(&legion_fan_wmi_lock);
	if (!legion_fan_wmi_device) {
		legion_fan_response_set_status(response, LEGION_FAN_READ_UNSUPPORTED);
		goto unlock;
	}

	status = wmidev_evaluate_method(legion_fan_wmi_device,
					LENOVO_FAN_WMI_INSTANCE,
					LENOVO_FAN_GET_TABLE_METHOD,
					&input, &output);
	if (ACPI_FAILURE(status)) {
		legion_fan_response_set_status(response,
					       LEGION_FAN_READ_TRANSPORT_ERROR);
		goto free_output;
	}

	object = output.pointer;
	if (!object || object->type != ACPI_TYPE_BUFFER) {
		legion_fan_response_set_status(response,
					       LEGION_FAN_READ_ACPI_RESPONSE_ERROR);
		goto free_output;
	}
	if (object->buffer.length != LENOVO_FAN_GET_TABLE_FAT2_BYTES) {
		legion_fan_response_set_status(response, LEGION_FAN_READ_ABI_ERROR);
		goto free_output;
	}
	if (get_unaligned_le32(object->buffer.pointer) !=
		LENOVO_FAN_GET_TABLE_POINTS ||
	    get_unaligned_le32(object->buffer.pointer + 44) !=
		LENOVO_FAN_GET_TABLE_POINTS) {
		legion_fan_response_set_status(response, LEGION_FAN_READ_ABI_ERROR);
		goto free_output;
	}

	memcpy(response->payload, object->buffer.pointer,
	       LENOVO_FAN_GET_TABLE_FAT2_BYTES);
	response->payload_size = cpu_to_le32(LENOVO_FAN_GET_TABLE_FAT2_BYTES);
	response->status = cpu_to_le32(LEGION_FAN_READ_OK);

free_output:
	kfree(output.pointer);
unlock:
	mutex_unlock(&legion_fan_wmi_lock);
}
#endif

/*
 * The only possible firmware-call entry point. The feature is disabled by
 * default; without it, every read fails closed with -EOPNOTSUPP.
 */
static ssize_t legion_fan_endpoint_read_disabled(struct file *file,
					 char __user *buffer, size_t count,
					 loff_t *offset)
{
	(void)file;

#if !IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ)
	(void)buffer;
	(void)count;
	(void)offset;
	return -EOPNOTSUPP;
#else
	struct legion_fan_read_response_v1 response;

	if (*offset)
		return 0;
	if (count < sizeof(response))
		return -EMSGSIZE;

	legion_fan_response_init(&response);
	legion_fan_get_table_once(&response);
	if (copy_to_user(buffer, &response, sizeof(response)))
		return -EFAULT;

	*offset = sizeof(response);
	return sizeof(response);
#endif
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
