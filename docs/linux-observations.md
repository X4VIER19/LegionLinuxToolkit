# Observaciones Linux

Estas observaciones corresponden al entorno informado: Nobara Linux, kernel
`7.2.0-202.nobara.fc44.x86_64`, Legion Slim 5 16ARP9 (83EX), BIOS `NUCN15WW`.
No constituyen una garantía para otro equipo o BIOS.

## Perfiles

El kernel usa `lenovo_wmi_gamezone`. La ruta observada es:

```text
/sys/firmware/acpi/platform_profile
```

Perfiles disponibles: `low-power`, `balanced`, `performance`, `max-power` y
`custom`. El perfil activo durante las pruebas fue `balanced`.

El driver Gamezone del kernel publica perfiles de plataforma y coordina sus
cambios con otras interfaces Lenovo. El perfil `custom` es el modo que el
firmware usa para habilitar ajustes de PPT y curva, pero esta fase no cambia a
ese perfil ni modifica dichos valores.

### Hito: primera lectura real aislada

El 2026-10-21 se realizó una única ejecución explícitamente autorizada del
binario experimental de solo lectura de LegionLinuxToolkit. Se ejecutó sin
`sudo`, con la feature no predeterminada
`approved-real-platform-profile-read`, confirmación literal requerida y salida
JSON. El binario no acepta raíces, rutas, variables de entorno, reintentos ni
opciones de escritura; `legion-cli` no intervino.

La secuencia `platform_profile_choices → platform_profile →
platform_profile_choices` completó correctamente y devolvió:

| Atributo | Texto observado |
| --- | --- |
| Perfil activo | `performance` |
| Opciones, primera y segunda lectura | `low-power balanced performance max-power custom` |

Las dos lecturas de opciones fueron idénticas. El resultado se publicó con
artefacto y origen `linux-read-only`/`LinuxReadOnly`, fuente
`legion-linux::readonly:sysfs-platform-profile` y confianza `low`.

**Validado únicamente por este hito:** en el 83EX con BIOS `NUCN15WW` y el
entorno Nobara documentado, una ejecución del proveedor experimental pudo
abrir las rutas fijas autorizadas, completar las tres lecturas dentro de los
límites configurados, validar formato UTF-8 y consistencia, y terminar sin
errores reportados. No cambió ningún perfil, ventilador, límite, firmware ni
módulo.

**Límites:** la igualdad antes/después de `platform_profile_choices` detecta
algunos cambios concurrentes, no una instantánea atómica ni un cambio A→B→A.
Esta observación no prueba que una lectura futura tendrá el mismo resultado,
ni demuestra que el controlador sea pasivo en todas las condiciones.

**No validado:** escritura o cambio de perfiles; comportamiento de potencia,
temperaturas o ventiladores; WMI, ACPI, EC, curvas, `Fan_Set_Table`; otras
BIOS, equipos, kernels o distribuciones; y el uso permanente o automático del
proveedor. La feature sigue desactivada por defecto y cada nueva lectura real
requiere autorización independiente.

## Atributos de energía

`lenovo_wmi_other` expone atributos en:

```text
/sys/class/firmware-attributes/lenovo-wmi-other-0/attributes/
```

Atributos detectados:

- `cpu_temp`, `gpu_temp`
- `ppt_cpu_cl`, `ppt_pl1_spl`, `ppt_pl2_sppt`, `ppt_pl3_fppt`
- `gpu_nv_ac_offset`, `gpu_nv_cpu_boost`, `gpu_nv_ctgp`, `gpu_nv_ppab`

Cada atributo permite consultar valor actual, valor predeterminado y límites.
Los valores `cpu_temp` y `gpu_temp` son límites térmicos configurables; no son
sensores instantáneos. Asimismo, `current_value` de un límite de potencia es el
límite configurado vigente, no una medición de consumo real. La documentación
del kernel describe que Other Mode publica atributos por perfil y que sus
bloques de capacidad contienen valor actual, predeterminado, mínimo, máximo e
incremento.

## Ventiladores

Se detectaron `acpi_fan` y `yogafan`, ambos con 0 RPM durante las pruebas.
Además, el kernel registró:

```text
lenovo_wmi_other: fan reporting/tuning is unsupported on this device
```

La causa no está determinada. Puede ser una capacidad no expuesta por el
firmware, una ruta de telemetría distinta o una lectura que no represente el
estado físico. No se realizarán escrituras ni pruebas activas para resolverlo
sin autorización.
