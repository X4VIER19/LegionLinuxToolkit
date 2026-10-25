/* SPDX-License-Identifier: GPL-2.0-only */
/* Offline mock of the exact shared evaluator: never linked to ACPI or WMI. */
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>
typedef uint8_t u8;
#define __packed __attribute__((packed))
#define static_assert _Static_assert
#include "legion_fan_read_contract.h"
#define ACPI_ALLOCATE_BUFFER ((size_t)-1)
#define ACPI_FAILURE(s) ((s) != 0)
#define ACPI_TYPE_BUFFER 3
#define cpu_to_le16(v) (v)
#define cpu_to_le32(v) (v)
typedef int acpi_status;
struct acpi_buffer { size_t length; void *pointer; };
union acpi_object {
	int type;
	struct { int type; unsigned int length; unsigned char *pointer; } buffer;
};
static union acpi_object object;
static unsigned char payload[72];
static int calls, frees, scenario, fail_at;
static unsigned int get_unaligned_le32(const unsigned char *p)
{
	return (unsigned int)p[0] | ((unsigned int)p[1] << 8) |
		((unsigned int)p[2] << 16) | ((unsigned int)p[3] << 24);
}
static acpi_status wmi_query_block(const char *guid, u8 instance, struct acpi_buffer *output)
{
	static const u8 expected[] = {6,9,10,11};
	assert(!strcmp(guid, "87FB2A6D-D802-48E7-9208-4576C5F5C8D8"));
	assert(instance == expected[calls]);
	assert(output->length == ACPI_ALLOCATE_BUFFER && !output->pointer);
	memset(payload, 0, sizeof(payload));
	payload[4] = payload[32] = 10;
	object.buffer.type = ACPI_TYPE_BUFFER;
	object.buffer.length = 72;
	object.buffer.pointer = payload;
	output->pointer = &object;
	if (calls++ != fail_at)
		return 0;
	switch (scenario) {
	case 1: return 1;
	case 2: object.type = 1; break;
	case 3: object.buffer.length = 71; break;
	case 4: payload[4] = 9; break;
	case 5: payload[32] = 11; break;
	case 6: object.buffer.pointer = NULL; break;
	case 7: output->pointer = NULL; break;
	}
	return 0;
}
static void kfree(void *p) { if (p) { assert(p == &object); frees++; } }
#include "fact_query.h"
int main(void)
{
	assert(sizeof(struct legion_fact_response) == 316);
	for (scenario = 0; scenario < 8; scenario++) {
		for (fail_at = 0; fail_at < 4; fail_at++) {
			struct legion_fact_response response = {0};
			calls = frees = 0;
			legion_fact_evaluate(&response);
			if (!scenario) {
				assert(calls == 4 && frees == 4);
				assert(response.status == 0 && response.payload_size == 304);
				assert(response.records[0].instance == 6);
				assert(response.records[3].instance == 11);
			} else {
				assert(calls == fail_at + 1);
				assert(frees == calls - (scenario == 7));
				assert(response.status == (unsigned int)(scenario == 1 ? 2 :
					(scenario == 2 || scenario == 7 ? 3 : 4)));
				assert(response.payload_size == (unsigned int)fail_at * 76);
			}
		}
	}
	return 0;
}
