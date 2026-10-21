# Fase 5A: telemetría y perfiles simulados

## Alcance

La Fase 5A añade modelos de dominio y datos **estáticos simulados** para que el
CLI y una futura GUI puedan representar telemetría sin descubrir, leer ni
modificar hardware. No contiene adaptadores Linux, rutas del host, sysfs, WMI,
ACPI, EC ni operaciones de perfiles, límites o ventiladores.

Todas las respuestas del CLI incluyen la envoltura JSON `source: "simulated"`
y el artefacto `static-simulation`. Los números de los fixtures son ejemplos de
representación: no son mediciones ni límites del equipo.

## Modelo de dominio

`legion-core` expone una `TelemetrySnapshot` inmutable con identidad,
`EnergyProfiles`, temperaturas, frecuencias, potencia y ventiladores. Cada
`TelemetryValue<T>` conserva:

- Una unidad explícita: `°C`, `MHz`, `W` o `RPM`.
- Una clasificación: medición instantánea, límite configurado o valor nominal.
- Un estado: disponible, no disponible, no validado o error de lectura.
- Procedencia, artefacto y nivel de confianza.

`PowerReadingKind` distingue además consumo instantáneo y límite configurado;
la validación rechaza que su tipo contradiga la clasificación del valor. Así un
límite de potencia no puede presentarse como consumo. Las validaciones también
rechazan una unidad incompatible con el tipo de telemetría y estados imposibles,
por ejemplo un error de lectura que contenga a la vez un valor numérico.

`FanTelemetry` mantiene separados el valor RPM reportado, la validación y la
interpretación. Un `0 RPM` no validado se conserva como `Some(0)` y se marca
como no concluyente; nunca se transforma en la afirmación de que el ventilador
esté detenido.

## Capas y extensibilidad

`legion-fixtures` construye los únicos escenarios disponibles en esta fase. La
CLI solo acepta sus nombres fijos y rechaza rutas como `/sys`, `/proc` y `/dev`;
no existe un respaldo que consulte el host. `legion-core` no conoce Linux ni
los adaptadores: una GUI futura puede consumir la misma `TelemetrySnapshot`.

La adición futura de una fuente real requiere una fase independiente, una
interfaz de adaptador revisada y la validación de la combinación completa de
modelo, DMI, BIOS, interfaz, operación y kernel. Esta arquitectura no autoriza
esa fuente ni cambia las restricciones de `AGENTS.md`.

## Escenarios de fixture

| Escenario | Propósito |
| --- | --- |
| `valid` | Valores sintéticos con unidades y clasificaciones explícitas. |
| `absent` | Dato de potencia sin valor y estado no disponible. |
| `unvalidated` | Dato de temperatura sin valor, marcado no validado. |
| `read-error` | Dato de potencia con error de lectura y sin valor. |
| `inconsistent` | Unidad deliberadamente incompatible; el modelo lo rechaza. |

Los fixtures incluyen una lectura de ventilador de `0 RPM` no validada y no
concluyente para asegurar que las capas de presentación no deduzcan una parada.

## CLI simulada

```text
legion-cli telemetry --format text
legion-cli telemetry --scenario absent --format json
```

Los comandos operan únicamente sobre escenarios embebidos. La salida de texto
declara que es una simulación y señala los límites configurados como tales.
