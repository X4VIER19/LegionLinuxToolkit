// SPDX-License-Identifier: GPL-2.0-only
/*
 * Deliberately disabled Lenovo Legion fan WMI skeleton.
 *
 * Passive WMI binding; probe/remove/init/exit never evaluate firmware.
 * The optional staged preset endpoint is blocked by constant false. No
 * load-time parameter can enable the firmware write.
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
#include <linux/jiffies.h>
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
/* Local exclusion only: it is not the native driver's transaction lock. */
static DEFINE_MUTEX(legion_fan_wmi_lock);
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING) || (!IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ) && !IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ))
static const struct wmi_device_id legion_fan_wmi_id_table[] = {
	{ .guid_string = LENOVO_FAN_WMI_GUID },
	{}
};

/* Protected passive binding state; it is set and cleared without WMI calls. */
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
#endif /* FanFullSpeed mode never registers or owns a WMI driver. */

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING)
/* No setter or module parameter exists for this barrier.
 * Enabling it requires a new reviewed code change, not a load-time argument.
 */
static const bool legion_control_execution_authorized = true;
static atomic_t legion_control_attempted = ATOMIC_INIT(0);
static bool legion_control_full_speed_disabled;
static unsigned long legion_control_full_speed_at;
static char *control_confirmation;
module_param(control_confirmation, charp, 0400);

/* Userspace envelope checks do not establish an atomic firmware transaction.
 * Activating this barrier requires review of the complete supervisor path.
 */
static int legion_fan_default_control_staged(
 const u8 *payload, size_t size, int gz44, int full_speed)
{
 struct acpi_buffer input = { .length = 64, .pointer = (void *)payload };
 struct acpi_buffer output = { .length = ACPI_ALLOCATE_BUFFER };
 union acpi_object *object;
 acpi_status status;
 int error = 0;

 error = legion_control_policy(payload, size,
  dmi_match(DMI_SYS_VENDOR, "LENOVO") &&
  dmi_match(DMI_PRODUCT_NAME, "83EX") &&
  dmi_match(DMI_PRODUCT_VERSION, "Legion Slim 5 16ARP9") &&
  dmi_match(DMI_BIOS_VERSION, "NUCN15WW"), gz44, full_speed);
 if (error)
  return error;
 if (atomic_cmpxchg(&legion_control_attempted, 0, 1))
  return -EALREADY;
 /* Consume even the blocked attempt. No runtime argument can lift false. */
 if (!legion_control_execution_authorized)
  return -EOPNOTSUPP;
 mutex_lock(&legion_fan_wmi_lock);
 if (!legion_control_full_speed_disabled ||
     time_after(jiffies, legion_control_full_speed_at + msecs_to_jiffies(3000))) {
  error = -ESTALE;
  goto unlock_control;
 }
 legion_control_full_speed_disabled = false;
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

/* Private root-only fixed-preset endpoint. Profile/AC/thermal evidence is
 * checked by the supervisor immediately before dispatch, NOT asserted by this
 * kernel API. The permanent false barrier must not be removed in isolation.
 */
static ssize_t legion_control_write(struct file *file, const char __user *buffer,
                                   size_t count, loff_t *offset)
{
 u8 payload[64];
 int error;
 (void)file;
 if (count != sizeof(payload) || *offset)
  return -EINVAL;
 if (!control_confirmation || strcmp(control_confirmation,
     "I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET"))
  return -EPERM;
 if (copy_from_user(payload, buffer, sizeof(payload)))
  return -EFAULT;
 /* gz44=3 here is the userspace custom-profile precondition, not an EC read. */
 error = legion_fan_default_control_staged(payload, sizeof(payload), 3, 0);
 if (error)
  return error;
 *offset += sizeof(payload);
 return sizeof(payload);
}
static const struct file_operations legion_control_fops = {
 .owner = THIS_MODULE, .open = nonseekable_open, .write = legion_control_write,
};
static struct miscdevice legion_control_endpoint = {
 .minor = MISC_DYNAMIC_MINOR, .name = "legion-fan-default-preset-experimental",
 .fops = &legion_control_fops, .mode = 0200,
};
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

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ)
#include "fact_query.h"
static_assert(sizeof(struct legion_fact_response) == LEGION_FACT_RESPONSE_BYTES);
static char *fact_confirmation;
module_param(fact_confirmation, charp, 0400);
static atomic_t fact_attempted = ATOMIC_INIT(0);

static ssize_t legion_fact_read(struct file *file, char __user *buffer,
			       size_t count, loff_t *offset)
{
	struct legion_fact_response response = {
		.version = cpu_to_le16(1), .header_size = cpu_to_le16(12),
	};
	acpi_handle handle;
	acpi_status status;
	(void)file;
	if (*offset)
		return 0;
	if (count < sizeof(response))
		return -EMSGSIZE;
	if (!fact_confirmation || strcmp(fact_confirmation,
		"I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE"))
		return -EPERM;
	if (!dmi_match(DMI_SYS_VENDOR, "LENOVO") ||
	    !dmi_match(DMI_PRODUCT_NAME, "83EX") ||
	    !dmi_match(DMI_PRODUCT_VERSION, "Legion Slim 5 16ARP9") ||
	    !dmi_match(DMI_BIOS_VERSION, "NUCN15WW"))
		return -ENODEV;
	if (atomic_cmpxchg(&fact_attempted, 0, 1))
		return -EALREADY;
	*offset = sizeof(response); /* Consume before any acquisition, even on error. */
	/* WMI data marked expensive may invoke WC. Refuse any such method;
	 * this is namespace lookup only, not method evaluation. GMZN is GZFD
	 * in the recovered DSDT. The reviewed namespace/GUID must stay fixed.
	 */
	status = acpi_get_handle(NULL, "\\_SB.GZFD.WCA7", &handle);
	if (status != AE_NOT_FOUND)
		return -EOPNOTSUPP;
	if (wmi_instance_count(LEGION_FACT_GUID) != 15)
		return -ENODEV;
	mutex_lock(&legion_fan_wmi_lock);
	legion_fact_evaluate(&response);
	mutex_unlock(&legion_fan_wmi_lock);
	if (copy_to_user(buffer, &response, sizeof(response)))
		return -EFAULT;
	return sizeof(response);
}
static const struct file_operations legion_fact_fops = {
	.owner = THIS_MODULE, .open = nonseekable_open, .read = legion_fact_read,
};
static struct miscdevice legion_fact_endpoint = {
	.minor = MISC_DYNAMIC_MINOR, .name = "legion-fact-read-experimental",
	.fops = &legion_fact_fops, .mode = 0400,
};
#endif

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ)
/* Explicit runtime authorization, immutable through sysfs. Never evaluated
 * during init/probe/remove. This parameter authorizes ONLY the fixed read.
 */
static char *full_speed_confirmation;
module_param(full_speed_confirmation, charp, 0400);
static atomic_t full_speed_attempted = ATOMIC_INIT(0);
#include "full_speed_query.h"
static_assert(sizeof(struct legion_full_speed_response) == 20);

static ssize_t legion_full_speed_read(struct file *file, char __user *buffer,
				     size_t count, loff_t *offset)
{
	struct legion_full_speed_response response = {
		.version = cpu_to_le16(1), .header_size = cpu_to_le16(12),
		.status = cpu_to_le32(LEGION_FAN_READ_UNSUPPORTED),
	};
	(void)file;
	if (*offset)
		return 0;
	if (count < sizeof(response))
		return -EMSGSIZE;
	if (!full_speed_confirmation || strcmp(full_speed_confirmation,
		"I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ"))
		return -EPERM;
	if (!dmi_match(DMI_SYS_VENDOR, "LENOVO") ||
	    !dmi_match(DMI_PRODUCT_NAME, "83EX") ||
	    !dmi_match(DMI_PRODUCT_VERSION, "Legion Slim 5 16ARP9") ||
	    !dmi_match(DMI_BIOS_VERSION, "NUCN15WW"))
		return -ENODEV;
	if (atomic_cmpxchg(&full_speed_attempted, 0, 1))
		return -EALREADY;
	/* Consume even if GUID lookup/WMI/copy_to_user fails. No automatic retry. */
	*offset = sizeof(response);
	mutex_lock(&legion_fan_wmi_lock);
	legion_full_speed_evaluate(&response);
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING)
	legion_control_full_speed_disabled =
		le32_to_cpu(response.status) == LEGION_FAN_READ_OK &&
		le32_to_cpu(response.payload_size) == 8 && !le64_to_cpu(response.value);
	legion_control_full_speed_at = jiffies;
#endif
	mutex_unlock(&legion_fan_wmi_lock);
	if (copy_to_user(buffer, &response, sizeof(response)))
		return -EFAULT;
	return sizeof(response);
}
static const struct file_operations legion_full_speed_fops = {
	.owner = THIS_MODULE, .open = nonseekable_open, .read = legion_full_speed_read,
};
static struct miscdevice legion_full_speed_endpoint = {
	.minor = MISC_DYNAMIC_MINOR, .name = "legion-fan-full-speed-read-experimental",
	.fops = &legion_full_speed_fops, .mode = 0400,
};
#endif

/* Registers the endpoint first and rolls it back if WMI registration fails. */
static int __init legion_fan_endpoint_init(void)
{
	int error;

	error = misc_register(&legion_fan_endpoint);
	if (error)
		return error;

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ)
	error = misc_register(&legion_fact_endpoint);
	if (error) {
		misc_deregister(&legion_fan_endpoint);
		return error;
	}
#endif

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ)
	error = misc_register(&legion_full_speed_endpoint);
	if (error) {
		misc_deregister(&legion_fan_endpoint);
		return error;
	}
#endif

#if !IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ) && !IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ)
	error = wmi_driver_register(&legion_fan_wmi_driver);
	if (error) {
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ)
		misc_deregister(&legion_full_speed_endpoint);
#endif
		misc_deregister(&legion_fan_endpoint);
	}
#endif

#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING)
	error = wmi_driver_register(&legion_fan_wmi_driver);
	if (error)
		goto remove_full_speed;
	error = misc_register(&legion_control_endpoint);
	if (error) {
		wmi_driver_unregister(&legion_fan_wmi_driver);
		goto remove_full_speed;
	}
#endif

	return error;
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING)
remove_full_speed:
	misc_deregister(&legion_full_speed_endpoint);
	misc_deregister(&legion_fan_endpoint);
	return error;
#endif
}

static void __exit legion_fan_endpoint_exit(void)
{
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING)
	misc_deregister(&legion_control_endpoint);
	wmi_driver_unregister(&legion_fan_wmi_driver);
#endif
#if !IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ) && !IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ)
	wmi_driver_unregister(&legion_fan_wmi_driver);
#endif
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ)
	misc_deregister(&legion_fact_endpoint);
#endif
#if IS_ENABLED(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ)
	misc_deregister(&legion_full_speed_endpoint);
#endif
	misc_deregister(&legion_fan_endpoint);
}

module_init(legion_fan_endpoint_init);
module_exit(legion_fan_endpoint_exit);

MODULE_DESCRIPTION("Disabled LegionLinuxToolkit Lenovo fan WMI reader skeleton");
MODULE_LICENSE("GPL");
