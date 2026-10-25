/* SPDX-License-Identifier: GPL-2.0-only */
/* Pure policy shared with a standalone, hardware-free C test. */
#ifndef LEGION_FAN_CONTROL_POLICY_H
#define LEGION_FAN_CONTROL_POLICY_H

/* Zero means a staged default-curve request passes policy, not authorization. */
static inline int legion_control_policy(const unsigned char *payload,
		unsigned long size, int identity_matches, int gz44, int full_speed)
{
	unsigned long i;
	if (!identity_matches)
		return -ENODEV;
	if (gz44 != 3 || full_speed != 0)
		return -EPERM;
	if (!payload || size != 64 || payload[0] != 1)
		return -EINVAL;
	for (i = 1; i < 64; i++) {
		unsigned char expected = (i >= 6 && i < 26 && !(i & 1)) ?
			(i - 6) / 2 + 1 : 0;
		if (payload[i] != expected)
			return -EINVAL;
	}
	return 0;
}
#endif
