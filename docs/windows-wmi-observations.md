# Observaciones Windows y WMI de ventiladores

## Entorno de referencia

El equipo objetivo dispone de Windows 11 y Nobara Linux en discos
independientes. Windows tiene Lenovo Legion Toolkit (LLT) instalado y operativo:
permite gestionar perfiles de energía y cambiar curvas personalizadas cuando el
usuario selecciona el perfil Personalizado. LLT y Windows son una fuente de
observaciones comparativas del firmware, no una especificación de una API Linux
ni evidencia de compatibilidad de un método WMI en Linux.

## Datos recuperados en Windows

- Clase: `LENOVO_FAN_METHOD`
- GUID: `92549549-4BDE-4F06-AC04-CE8BF898DBAA`
- Instancia: `ACPI\\PNP0C14\\GMZN_0`
- `Fan_Get_Table`: funcional en Windows.
- `Fan_Set_Table`: existe, pero no se ha probado y queda prohibido en esta fase.

## Esquema confirmado del 83EX

Los metadatos de `LENOVO_FAN_METHOD` obtenidos directamente en Windows para el
83EX con BIOS `NUCN15WW` confirman el siguiente esquema WMI/CIM:

- `Fan_Get_Table`: `WmiMethodId=5`; `FanID: UInt8` (`ID(0)`) y
  `SensorID: UInt8` (`ID(1)`) como entradas. El orden lógico de las salidas
  es `FanTableSize: UInt32` (`ID(2)`), `FanTable: UInt32Array` (`ID(3)`,
  `WmiSizeIs("FanTableSize")`), `SensorTableSize: UInt32` (`ID(4)`) y
  `SensorTable: UInt32Array` (`ID(5)`, `WmiSizeIs("SensorTableSize")`).
- `Fan_Set_Table`: `WmiMethodId=6`; entrada `FanTable: UInt8Array` (`ID(0)`).

Esta evidencia confirma nombres, tipos CIM, direcciones, IDs lógicos y los IDs WMI para el entorno
Windows documentado. No confirma el empaquetado de los argumentos en ACPI,
la codificación ni longitud de una respuesta Linux, ni autoriza llamadas a
ninguno de los métodos. `Fan_Set_Table` sigue prohibido y se registra solo
como metadato.

Con `FanID=1` y `SensorID=4`, `Fan_Get_Table` devolvió dos tablas de diez
valores, en ambos arreglos con índices de 1 a 10. También se recuperaron 15
tablas mediante `LENOVO_FAN_TABLE_DATA`, con valores nominales de hasta 5000
RPM. El mismo resultado de índices 1 a 10 se observó en los modos Personalizado
y Equilibrado. Es una observación de lectura; no demuestra que la tabla activa,
su semántica o su formato sean idénticos entre perfiles o utilizables en Linux.

## Presencia en Linux

Linux detecta el dispositivo WMI:

```text
/sys/bus/wmi/devices/92549549-4BDE-4F06-AC04-CE8BF898DBAA-22
```

Propiedades observadas: `instance_count=1`, `object_id=AB`, `expensive=0`,
`DEVTYPE=method` y ningún controlador asociado. La API
`wmidev_evaluate_method()` está disponible en el kernel instalado.

## Interpretación y límites

La presencia de un dispositivo y la disponibilidad de una API del kernel solo
demuestran una posible vía de investigación. No prueban que los parámetros,
buffers, semántica de tabla ni efectos sean idénticos a Windows. En particular,
no se debe inferir que una tabla leída sea aplicable, ni que `Fan_Set_Table`
sea seguro para este BIOS.

Por ello, el primer prototipo no llamará automáticamente `Fan_Get_Table`.
Cualquier consulta WMI futura tendrá activación manual, argumentos fijados y
auditables, límite de tamaño de respuesta, y requerirá autorización previa para
ejecutarse sobre el equipo.

## Protocolo para comparaciones futuras

Las comparaciones deben priorizar consultas de solo lectura con PowerShell, WMI
o herramientas de diagnóstico. Se registrarán versión de BIOS, perfil visible,
método, argumentos, resultado bruto y herramienta usada. No se usarán scripts
para modificar EC, firmware, WMI, perfiles o curvas. Si fuera indispensable
cambiar temporalmente el perfil desde LLT para observar una diferencia, se
explicará el objetivo antes y se requerirá autorización explícita.

`Fan_Set_Table` no se invocará. Una respuesta correcta de Windows solo guía una
hipótesis que deberá verificarse por separado y de forma segura para Linux.
