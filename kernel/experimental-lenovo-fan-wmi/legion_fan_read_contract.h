/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * Versioned, read-only contract proposed for a future kernel/userspace bridge.
 *
 * This header does not create an endpoint. It is a layout specification only;
 * the experimental module remains unregistered and returns -EOPNOTSUPP.
 */

#ifndef _LEGION_FAN_READ_CONTRACT_H
#define _LEGION_FAN_READ_CONTRACT_H

#include <linux/types.h>

#define LEGION_FAN_READ_CONTRACT_VERSION 1
#define LEGION_FAN_READ_HEADER_BYTES 12
#define LEGION_FAN_READ_FAT2_BYTES 88
#define LEGION_FAN_READ_RESPONSE_BYTES \
	(LEGION_FAN_READ_HEADER_BYTES + LEGION_FAN_READ_FAT2_BYTES)

enum legion_fan_read_status {
	LEGION_FAN_READ_OK = 0,
	LEGION_FAN_READ_UNSUPPORTED = 1,
	LEGION_FAN_READ_TRANSPORT_ERROR = 2,
	LEGION_FAN_READ_ACPI_RESPONSE_ERROR = 3,
	LEGION_FAN_READ_ABI_ERROR = 4,
};

/*
 * Payload contents remain opaque at this boundary. Version one reserves it
 * for the 88 bytes of FAT2 only after a successful read.
 */
struct legion_fan_read_response_v1 {
	__u16 version;
	__u16 header_size;
	__u32 status;
	__u32 payload_size;
	__u8 payload[LEGION_FAN_READ_FAT2_BYTES];
} __packed;

static_assert(sizeof(struct legion_fan_read_response_v1) ==
	      LEGION_FAN_READ_RESPONSE_BYTES);

#endif /* _LEGION_FAN_READ_CONTRACT_H */
