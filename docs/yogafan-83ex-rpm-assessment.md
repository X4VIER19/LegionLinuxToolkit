# Evaluación de RPM mediante yogafan para el 83EX

## Estado de la investigación

Esta evaluación es estática. No leyó sysfs, no evaluó objetos ACPI, no leyó el
EC y no cargó ningún módulo. El módulo `yogafan` instalado para el kernel Nobara
`7.2.11-200.nobara.fc44.x86_64` se identificó como módulo interno, pero el
paquete `kernel-devel` no contiene su fuente C exacta.

Los metadatos del SRPM oficial de Nobara identifican
`kernel-7.2.11-200.nobara.fc44.src.rpm` y su payload
`cachyos-7.2.11-1.tar.gz`. La etiqueta pública correspondiente de CachyOS es
`cachyos-7.2.11-1` (`b0ce79ee33ddd93d513f77341167956e1355758e`). La fuente
se recuperó de esa revisión y `drivers/hwmon/yogafan.c` verificó SHA-256
`87da68973bbe5ed6c42c179ba7884be50f5263321605ebecf35c60989d410c99`.

El parche se aplicó únicamente a una copia temporal de ese archivo y compiló
contra las cabeceras `7.2.11-200.nobara.fc44.x86_64`. El módulo temporal
`yogafan_83ex.ko` tiene el `vermagic` coincidente y conserva la dependencia
solamente de las APIs de lectura ACPI `acpi_get_handle` y
`acpi_evaluate_integer`; no se instaló ni se cargó. Kbuild advirtió que el
`pahole` local (`0`) difiere del usado para construir el kernel (`132`) y omitió
la generación BTF por falta de `vmlinux`; no fue un error de compilación.

La inspección del binario del módulo instalado confirma que contiene las rutas
ACPI `\\_SB.PCI0.LPC0.EC0.FANS` y `\\_SB.PCI0.LPC0.EC0.FA2S`, y no contiene
`FA1S`. El código Linux público de yogafan usa configuraciones DMI y publica
cada canal disponible como `fanX_input` de hwmon, realizando una evaluación
ACPI del objeto configurado cuando se consulta el atributo. El código público
es GPL-2.0-only y no se copia en LegionLinuxToolkit.

## Evidencia de la DSDT del 83EX

La DSDT estática del equipo declara los campos `FA1S` y `FA2S` como campos de
ocho bits de la región `FANR`. Dos rutas independientes los multiplican por
`0x64` antes de devolverlos:

- `WMAE`, en su rama de consulta de ventilador, usa `TYP0=1` para `FA1S` y
  `TYP0=2` para `FA2S`.
- `_TZ.FAN5._FST` coloca `FA2S * 0x64` en el campo `Speed` del paquete ACPI
  `_FST`.

La especificación ACPI define ese campo `Speed` como RPM actuales. Por ello el
factor matemático `raw * 100` está demostrado para `FA2S`; la rama WMAE muestra
el mismo factor para `FA1S`. No se ha demostrado que el canal 1 sea CPU ni que
el canal 2 sea GPU, así que el proyecto debe conservarlos como canales sin
asignación física.

`GFAN`/`FAT2`, `CFST`, `FNT*` y `FI*` no son candidatos a RPM: contienen
índices y configuración de curva. Tampoco `fanX_target` o un valor de perfil
pueden sustituir una medida `fanX_input`.

## Quirk propuesto, no compilado ni instalado

`kernel/yogafan-83ex/0001-hwmon-yogafan-add-legion-slim-5-16arp9-83ex.patch`
es un parche GPL-2.0-only separado que se comprobó y aplicó sobre una copia
temporal de esa revisión CachyOS. No forma parte de Kbuild, no se aplica
automáticamente y no se incluye en los crates Rust MIT OR Apache-2.0. El único
cambio a evaluar es un quirk DMI exacto para el 83EX que tenga:

```text
canales: 2
rutas:   \\_SB.PCI0.LPC0.EC0.FA1S, \\_SB.PCI0.LPC0.EC0.FA2S
escala:  100
```

La coincidencia usa fabricante `LENOVO`, nombre de producto `83EX` y versión de
producto `Legion Slim 5 16ARP9`; la BIOS `NUCN15WW` queda registrada como
evidencia de validación y no como coincidencia DMI restrictiva. La entrada debe
preceder a la regla Legion genérica, no reemplazarla ni cambiar otros modelos.

El controlador público intenta resolver cada manejador ACPI configurado durante
`probe`; omite los ausentes y devuelve `-ENODEV` si ninguno queda disponible.
Durante una lectura hwmon, un fallo al evaluar el entero ACPI se propaga como
`-EIO`. Su filtro RLLag convierte valores ya escalados inferiores a 50 en cero
y suaviza muestras posteriores. No valida sentinelas de firmware: un byte
inesperado distinto de cero se escalaría y publicaría. Por ello `0` sigue siendo
un valor informado, no prueba de que el ventilador esté detenido.

## Integración futura de LegionLinuxToolkit

LegionLinuxToolkit debe consumir únicamente una interfaz hwmon ya expuesta por
el kernel y aceptar `fan1_input`/`fan2_input` como RPM medidas solo cuando el
nombre del driver, los dos canales, la identidad DMI y el quirk documentado
coincidan. El adaptador de Rust será estrictamente de lectura y conservará:

- valor original y RPM normalizadas;
- canal lógico `1` o `2`, sin etiqueta CPU/GPU;
- procedencia del driver hwmon y versión del kernel;
- estado no validado si falta cualquier parte de la coincidencia.

Una lectura `0` se conserva como número reportado; no demuestra que el
ventilador esté detenido sin una correlación posterior autorizada.

## Verificación futura, separada y autorizable

Antes de la primera lectura real se necesita: (1) revisar una estrategia segura
frente al posible `yogafan` instalado ya cargado —sin reemplazarlo ni descargarlo
por defecto—, (2) revisar que el módulo temporal no contiene rutas de escritura,
PWM ni control, y (3) autorización explícita para cargar un módulo de prueba y
leer una vez ambos `fanX_input`. La lectura de `_FST` por `acpi_fan` también
evalúa un método ACPI y requiere autorización independiente, aunque su finalidad
sea consultar RPM.

Si el quirk no puede validarse, se mantiene `yogafan` como fuente no fiable y
no se usa WMAE, WMI ni EC directo como sustituto.
