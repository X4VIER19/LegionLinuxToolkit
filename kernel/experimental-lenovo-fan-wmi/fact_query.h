/* SPDX-License-Identifier: GPL-2.0-only */
/* Fixed WQA7 batch. Linux APIs or an offline test shim supply the symbols. */
#ifndef LEGION_FACT_QUERY_H
#define LEGION_FACT_QUERY_H
#define LEGION_FACT_GUID "87FB2A6D-D802-48E7-9208-4576C5F5C8D8"
#define LEGION_FACT_BYTES 72
#define LEGION_FACT_RESPONSE_BYTES 316
struct legion_fact_record {
	__le16 instance;
	__le16 reserved;
	u8 bytes[LEGION_FACT_BYTES];
} __packed;
struct legion_fact_response {
	__le16 version;
	__le16 header_size;
	__le32 status;
	__le32 payload_size; /* Completed records, even on error; never partial success. */
	struct legion_fact_record records[4];
} __packed;

static inline void legion_fact_evaluate(struct legion_fact_response *response)
{
	static const u8 instances[] = { 6, 9, 10, 11 };
	unsigned int i;
	response->status = cpu_to_le32(LEGION_FAN_READ_TRANSPORT_ERROR);
	response->payload_size = cpu_to_le32(0);
	for (i = 0; i < 4; i++) {
		struct acpi_buffer output = { .length = ACPI_ALLOCATE_BUFFER };
		union acpi_object *object;
		acpi_status status;
		bool valid = false;

		status = wmi_query_block(LEGION_FACT_GUID, instances[i], &output);
		if (ACPI_FAILURE(status))
			goto release;
		response->status = cpu_to_le32(LEGION_FAN_READ_ACPI_RESPONSE_ERROR);
		object = output.pointer;
		if (!object || object->type != ACPI_TYPE_BUFFER)
			goto release;
		response->status = cpu_to_le32(LEGION_FAN_READ_ABI_ERROR);
		if (object->buffer.length != LEGION_FACT_BYTES || !object->buffer.pointer)
			goto release;
		if (get_unaligned_le32(object->buffer.pointer + 4) != 10 ||
		    get_unaligned_le32(object->buffer.pointer + 32) != 10)
			goto release;
		response->records[i].instance = cpu_to_le16(instances[i]);
		memcpy(response->records[i].bytes, object->buffer.pointer, LEGION_FACT_BYTES);
		response->payload_size = cpu_to_le32((i + 1) * sizeof(response->records[0]));
		valid = true;
release:
		kfree(output.pointer);
		if (!valid)
			return; /* No next instance after a failure, no retry. */
		response->status = cpu_to_le32(LEGION_FAN_READ_TRANSPORT_ERROR);
	}
	response->status = cpu_to_le32(LEGION_FAN_READ_OK);
}
#endif
