/* SPDX-License-Identifier: GPL-2.0-only */
/* Offline shim: no kernel, devices or actual ACPI/WMI entry points. */
#include <assert.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>
typedef uint8_t u8;
typedef uint16_t __le16;
typedef uint32_t __le32;
typedef uint64_t __le64;
typedef int acpi_status;
#define __packed __attribute__((packed))
#define ACPI_ALLOCATE_BUFFER ((size_t)-1)
#define ACPI_FAILURE(status) ((status) != 0)
#define ACPI_TYPE_INTEGER 1
#define LEGION_FAN_READ_OK 0
#define LEGION_FAN_READ_TRANSPORT_ERROR 2
#define LEGION_FAN_READ_ACPI_RESPONSE_ERROR 3
#define cpu_to_le32(value) (value)
#define cpu_to_le64(value) (value)
struct acpi_buffer { size_t length; void *pointer; };
union acpi_object {
	int type;
	struct { int type; uint64_t value; } integer;
};
static union acpi_object object;
static int fail_transport, missing, calls, freed;
static acpi_status wmi_evaluate_method(const char *guid,
	int instance, int method, struct acpi_buffer *input, struct acpi_buffer *output)
{
	const unsigned char expected[] = {0, 0, 2, 4};
	assert(!strcmp(guid, "DC2A8805-3A8C-41BA-A6F7-092E0089CD3B"));
	assert(instance == 0 && method == 17);
	assert(input->length == 4 && !memcmp(input->pointer, expected, 4));
	assert(output->length == ACPI_ALLOCATE_BUFFER && !output->pointer);
	calls++;
	output->pointer = missing ? NULL : &object;
	return fail_transport;
}
static void kfree(void *pointer) { if (pointer) { assert(pointer == &object); freed++; } }
#include "full_speed_query.h"
int main(void)
{
	int scenario;
	for (scenario = 0; scenario < 5; scenario++) {
		struct legion_full_speed_response response = {0};
		calls = freed = 0;
		fail_transport = scenario == 2;
		missing = scenario == 3;
		object.integer.type = scenario == 4 ? 99 : ACPI_TYPE_INTEGER;
		object.integer.value = scenario;
		legion_full_speed_evaluate(&response);
		assert(calls == 1 && freed == !missing);
		if (scenario < 2) {
			assert(response.status == 0 && response.payload_size == 8);
			assert(response.value == (unsigned int)scenario);
		} else {
			assert(response.status == (scenario == 2 ? 2 : 3));
			assert(response.payload_size == 0);
		}
	}
	return 0;
}
