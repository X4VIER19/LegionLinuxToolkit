# Fuentes de investigación y licencias

## Evidencia directa del equipo

La información de modelo, BIOS, rutas sysfs, atributos detectados, perfiles,
mensaje del kernel, dispositivo WMI y resultados de Windows procede de las
observaciones proporcionadas para este repositorio. El equipo tiene Windows 11
con LLT funcional y Nobara Linux en discos independientes; las observaciones de
Windows son evidencia de referencia para 83EX/`NUCN15WW`, no validación de una
operación WMI en Linux.

## Documentación consultada

- Kernel Linux, [Lenovo WMI Gamezone](https://www.kernel.org/doc/html/v6.17/wmi/devices/lenovo-wmi-gamezone.html): perfiles, modo custom y eventos.
- Kernel Linux, [Lenovo WMI Other Mode](https://www.kernel.org/doc/html/latest/wmi/devices/lenovo-wmi-other.html): firmware attributes, bloques de capacidad y hwmon.
- Kernel Linux, [interfaz sysfs de hwmon](https://docs.kernel.org/hwmon/sysfs-interface.html), [interfaz térmica](https://docs.kernel.org/driver-api/thermal/sysfs-api.html), [power_supply](https://docs.kernel.org/power/power_supply_class.html) y [platform_profile](https://docs.kernel.org/userspace-api/sysfs-platform_profile.html): candidatos de telemetría genéricos y sus unidades; no prueban su presencia ni mapeo en 83EX.
- [LenovoLegionLinux](https://github.com/johnfanv2/LenovoLegionLinux): documentación y referencia de diseño Linux/matriz de capacidades; no se ha copiado código ni validado su implementación en 83EX.
- [FEATURES_AND_TESTING de LenovoLegionLinux](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/doc/FEATURES_AND_TESTING.md): referencia sobre semánticas parciales de curva y unidades; no prueba compatibilidad con 83EX.
- [Lenovo Legion Toolkit](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit): documentación de producto Windows y referencia del comportamiento de modo custom; no se ha copiado código ni validado una implementación para este equipo.

## Código fuente externo: estado de inspección

En la investigación pasiva de Fase 4A se inspeccionaron de forma dirigida la
API, guía y fuentes públicas del kernel `core.c`, `wmi-gamezone.c` y
`wmi-other.c` (SPDX `GPL-2.0-or-later`), además de fragmentos de
`kernel_module/legion-laptop.c` de LenovoLegionLinux (SPDX
`GPL-2.0-or-later`). Esta inspección está documentada en
[el informe WMI](wmi-passive-research.md) y en el
[análisis del contrato `Fan_Get_Table`](fan-get-table-contract-research.md);
no es una auditoría completa ni una prueba de compatibilidad con el 83EX.

La licencia global de LenovoLegionLinux no se declara validada aquí: solo se ha
verificado el SPDX del archivo concreto inspeccionado. No se ha copiado código
de LenovoLegionLinux, Lenovo Legion Toolkit ni del kernel. Cualquier
reutilización futura exige revisar licencia, atribución y compatibilidad de cada
archivo con MIT OR Apache-2.0.

## Hipótesis pendientes de verificar

- Que `Fan_Get_Table` use en Linux los mismos argumentos, formato de buffer y
  semántica que en Windows.
- Que una tabla de ventilador recuperada represente una curva aplicable en el
  BIOS `NUCN15WW`.
- La causa de las lecturas 0 RPM y si existe una fuente de telemetría válida.
- Que una ruta o backend usado por otro modelo Legion sea apropiado para 83EX.
- Que un resultado de LLT o WMI en Windows describa la misma estructura o el
  mismo efecto en Linux.
- Qué rutas de telemetría, controlador, etiquetas y unidades están presentes en
  el kernel Nobara exacto; se documentan como hipótesis en
  [la investigación de telemetría Linux](linux-telemetry-research.md).

## Reglas de reutilización

Se revisará la licencia de cada archivo, dependencia o fragmento antes de
integrarlo. La investigación, los nombres de interfaces y las observaciones
empíricas no implican permiso para copiar una implementación. Mantener avisos
de copyright y atribución cuando corresponda.
