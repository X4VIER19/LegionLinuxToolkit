# This Makefile is a standalone kernel-tree fragment. It is deliberately not
# included by the repository root or by any installed kernel build tree.
obj-$(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI) += legion_lenovo_fan_wmi.o

ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ),)
ifeq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ),y)
ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ)$(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ)$(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING),)
$(error FACT capture must not enable other firmware features)
endif
ccflags-y += -DCONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ=1
else
$(error CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ must be unset or exactly y)
endif
endif

ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ),)
ifeq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ),y)
ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ),)
$(error FanFullSpeed experiment must not enable other fan firmware features)
endif
ccflags-y += -DCONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ=1
else
$(error CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ must be unset or exactly y)
endif
endif

ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING),)
ifeq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING),y)
ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ),y)
$(error Control staging requires the same module's FanFullSpeed reader)
endif
ccflags-y += -DCONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING=1
else
$(error CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING must be unset or exactly y)
endif
endif

# This standalone fragment is not included in the kernel's Kconfig tree, so a
# command-line CONFIG value selects obj-m but does not create a C preprocessor
# macro. Make that bridge explicit and fail closed: only the literal `y` may
# compile the firmware-call path; unset keeps it disabled and every other
# value is rejected instead of being interpreted permissively.
ifneq ($(strip $(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ)),)
ifeq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ),y)
ccflags-y += -DCONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ=1
else
$(error CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_REAL_READ must be unset or exactly y)
endif
endif
