/* SPDX-License-Identifier: GPL-2.0-only */
/* Shared query body for the kernel and isolated userspace mock tests.
 * Required types/APIs are supplied by Linux or the standalone test shim.
 */
#ifndef LEGION_FULL_SPEED_QUERY_H
#define LEGION_FULL_SPEED_QUERY_H
struct legion_full_speed_response {
	__le16 version;
	__le16 header_size;
	__le32 status;
	__le32 payload_size;
	__le64 value;
} __packed;

static inline void legion_full_speed_evaluate(struct legion_full_speed_response *response)
{
	const u8 request[4] = { 0, 0, 2, 4 };
	struct acpi_buffer input = { .length = sizeof(request), .pointer = (void *)request };
	struct acpi_buffer output = { .length = ACPI_ALLOCATE_BUFFER };
	union acpi_object *object;
	acpi_status status;
	response->payload_size = cpu_to_le32(0);
	response->value = cpu_to_le64(0);
	/* Deprecated GUID API is intentionally restricted to this experiment.
	 * It resolves the existing WMI device without claiming its driver binding.
	 * No alternative GUID/method/argument or fallback is accepted.
	 */
	status = wmi_evaluate_method("DC2A8805-3A8C-41BA-A6F7-092E0089CD3B",
				    0, 17, &input, &output);
	if (ACPI_FAILURE(status)) {
		response->status = cpu_to_le32(LEGION_FAN_READ_TRANSPORT_ERROR);
		goto free_output;
	}
	object = output.pointer;
	if (!object || object->type != ACPI_TYPE_INTEGER) {
		response->status = cpu_to_le32(LEGION_FAN_READ_ACPI_RESPONSE_ERROR);
		goto free_output;
	}
	response->value = cpu_to_le64(object->integer.value);
	response->payload_size = cpu_to_le32(8);
	response->status = cpu_to_le32(LEGION_FAN_READ_OK);
free_output:
	kfree(output.pointer);
}
#endif
