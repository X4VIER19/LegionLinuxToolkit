# This Makefile is a standalone kernel-tree fragment. It is deliberately not
# included by the repository root or by any installed kernel build tree.
obj-$(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI) += legion_lenovo_fan_wmi.o

ifneq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING),)
ifeq ($(CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING),y)
ccflags-y += -DCONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING=1
else
$(error CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_WMI_CONTROL_STAGING must be unset or exactly y)
endif
endif
