/* SPDX-License-Identifier: GPL-2.0-only */
/* Standalone userspace simulation; this is not part of the kernel module. */
#include <assert.h>
#include <errno.h>
#include "control_policy.h"

int main(void)
{
	unsigned char payload[64] = { 1 };
	unsigned int i;
	for (i = 0; i < 10; i++)
		payload[6 + i * 2] = i + 1;
	assert(legion_control_policy(payload, 64, 1, 3, 0) == 0);
	assert(legion_control_policy(payload, 64, 0, 3, 0) == -ENODEV);
	assert(legion_control_policy(payload, 64, 1, 7, 0) == -EPERM);
	assert(legion_control_policy(payload, 64, 1, -1, 0) == -EPERM);
	assert(legion_control_policy(payload, 64, 1, 3, 1) == -EPERM);
	assert(legion_control_policy(payload, 64, 1, 3, -1) == -EPERM);
	assert(legion_control_policy(payload, 63, 1, 3, 0) == -EINVAL);
	assert(legion_control_policy(0, 64, 1, 3, 0) == -EINVAL);
	for (i = 0; i < 64; i++) {
		payload[i] ^= 1;
		assert(legion_control_policy(payload, 64, 1, 3, 0) == -EINVAL);
		payload[i] ^= 1;
	}
	return 0;
}
