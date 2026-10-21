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
