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
- [LenovoLegionLinux](https://github.com/johnfanv2/LenovoLegionLinux): documentación y referencia de diseño Linux/matriz de capacidades; no se ha copiado código ni validado su implementación en 83EX.
- [FEATURES_AND_TESTING de LenovoLegionLinux](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/doc/FEATURES_AND_TESTING.md): referencia sobre semánticas parciales de curva y unidades; no prueba compatibilidad con 83EX.
- [Lenovo Legion Toolkit](https://github.com/LenovoLegionToolkit-Team/LenovoLegionToolkit): documentación de producto Windows y referencia del comportamiento de modo custom; no se ha copiado código ni validado una implementación para este equipo.

## Código fuente externo: estado de inspección

No se ha realizado una auditoría o revisión completa de código fuente externo
para LegionLinuxToolkit. El archivo del kernel
[`wmi-other.c`](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/lenovo/wmi-other.c)
se ha identificado como referencia disponible (GPL-2.0-or-later), pero no se
presenta como implementación inspeccionada ni compatible con 83EX. Tampoco se
ha copiado código de LenovoLegionLinux o Lenovo Legion Toolkit.

## Hipótesis pendientes de verificar

- Que `Fan_Get_Table` use en Linux los mismos argumentos, formato de buffer y
  semántica que en Windows.
- Que una tabla de ventilador recuperada represente una curva aplicable en el
  BIOS `NUCN15WW`.
- La causa de las lecturas 0 RPM y si existe una fuente de telemetría válida.
- Que una ruta o backend usado por otro modelo Legion sea apropiado para 83EX.
- Que un resultado de LLT o WMI en Windows describa la misma estructura o el
  mismo efecto en Linux.

## Reglas de reutilización

Se revisará la licencia de cada archivo, dependencia o fragmento antes de
integrarlo. La investigación, los nombres de interfaces y las observaciones
empíricas no implican permiso para copiar una implementación. Mantener avisos
de copyright y atribución cuando corresponda.
