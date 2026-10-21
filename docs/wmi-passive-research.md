# Fase 4A: investigación pasiva de WMI

**Alcance:** informe documental preparado sin cargar módulos, leer interfaces
del equipo, invocar WMI/ACPI/EC ni modificar perfiles. Se refiere al objetivo
Lenovo Legion Slim 5 16ARP9 (`83EX`), BIOS `NUCN15WW`, y al kernel informado
`7.2.0-202.nobara.fc44.x86_64`. La fuente del kernel consultada es la rama
principal pública y puede diferir de la versión empaquetada por Nobara.

## Clasificación de evidencia

| Tipo | Elementos |
| --- | --- |
| Observación previa del equipo | GUID `92549549-4BDE-4F06-AC04-CE8BF898DBAA`, tipo `method`, `object_id=AB`, una instancia, sin driver asociado; `Fan_Get_Table(FanID=1, SensorID=4)` funcionó en Windows. |
| Documentación/código público inspeccionado | API WMI del kernel, guía de desarrollo, `core.c`, `wmi-gamezone.c`, `wmi-other.c`, y fragmentos concretos de `legion-laptop.c` de LenovoLegionLinux. |
| Conclusión de este informe | La vía técnicamente candidata es un controlador de kernel WMI por dispositivo; no hay una interfaz de espacio de usuario validada para invocar el método en este equipo. |
| Desconocido en 83EX/NUCN15WW | Binary MOF disponible, ID de método, formato de entrada/salida, tipo de objeto devuelto, tamaño mínimo, semántica de la tabla y RPM real mediante `LENOVO_FAN_METHOD`. |

Nada observado en Windows ni en otro modelo se clasifica aquí como validación
Linux.

## Arquitectura WMI del kernel

El núcleo ACPI-WMI crea un `struct wmi_device` por bloque WMI y enlaza un
`struct wmi_driver` mediante su tabla de GUID. Esta interfaz basada en bus es la
recomendada para controladores nuevos; la interfaz antigua que busca por GUID
está obsoleta porque no maneja correctamente GUID duplicados.

Una llamada de método en un futuro controlador usaría conceptualmente
`wmidev_invoke_method(wdev, instance, method_id, in, out, min_size)`. Requiere
un `wmi_device` ya enlazado, número de instancia, ID de método y buffers cuya
forma viene definida por el Binary MOF del dispositivo. El kernel exige que el
propio controlador valide y parsee los bytes; una longitud mínima o una
alineación de ocho bytes no valida el protocolo ni los valores.

`wmidev_evaluate_method()` también existe, pero está documentada como obsoleta.
Su implementación construye una llamada ACPI con instancia, ID y buffer. Es
una **invocación activa**, incluso si el método se denomina `Get`; no se llama
en esta fase ni se considera una API de espacio de usuario.

Para el dispositivo observado, el núcleo forma el nombre ACPI de los bloques de
método como `WM` + `object_id`. Por tanto, `object_id=AB` permite inferir
`WMAB` como nombre interno candidato. Esta inferencia describe el enrutamiento
del núcleo; no prueba que el método acepte `Fan_Get_Table`, su ID o sus buffers.

### Consecuencia para un controlador futuro

La detección podría limitarse a guardar el `wmi_device` y metadatos enumerados.
No debe consultar soporte, perfil, tabla ni RPM desde `probe()`, `init()`,
`remove()` o `shutdown()`. La guía pública del kernel permite que otros
controladores llamen WMI en `probe()`; eso no satisface las reglas de seguridad
de este repositorio y no se reutilizará como patrón.

## Controladores Lenovo ya disponibles

### `lenovo_wmi_gamezone`

El código y la documentación públicos asocian Gamezone al GUID
`887B54E3-DDDC-4B2C-8B88-68A26A8835D0`. Gestiona los perfiles
`low-power`, `balanced`, `performance`, `max-power` y `custom`, y coordina los
cambios mediante notificaciones. Es coherente con los perfiles ya observados
en el 83EX.

El código publicado consulta el modo térmico durante `probe()` y usa métodos
WMI para leer o establecer perfiles. Es una implementación del kernel existente,
no una autorización para añadir llamadas automáticas a LegionLinuxToolkit. El
modo `custom` habilita ajustes de PPT y curva según la documentación del kernel,
pero cambiar a ese modo sigue fuera del alcance actual.

### `lenovo_wmi_other`

Other Mode corresponde al GUID `DC2A8805-3A8C-41BA-A6F7-092E0089CD3B` y publica
los firmware attributes de límites observados. Su fuente pública actual también
puede registrar HWMON `fanX_input`, `fanX_target`, límites y divisor. Para ello
consulta Capability Data y solo expone canales que anuncian soporte y tienen
restricciones RPM válidas.

El mensaje observado en el 83EX, `fan reporting/tuning is unsupported on this
device`, se emite cuando el controlador no encuentra ningún canal válido para
registrar HWMON tras esas comprobaciones. Esto explica el significado del
mensaje en esa versión de fuente; no identifica por sí solo qué bloque o valor
faltó en el kernel instalado, ni prueba que el ventilador no gire.

El código actual ofrece parámetros que relajan comprobaciones o exponen más
canales. No son una alternativa aceptable: omiten validaciones y su propia
documentación advierte que un objetivo demasiado bajo puede detener un
ventilador. Forzarlos está prohibido por `AGENTS.md`.

## `LENOVO_FAN_METHOD` y `Fan_Get_Table`

| Campo | Estado para 83EX/NUCN15WW |
| --- | --- |
| GUID `92549549-4BDE-4F06-AC04-CE8BF898DBAA` | Observado en Linux y Windows. |
| Tipo `method`, una instancia, `object_id=AB` | Observado en Linux. |
| Nombre `Fan_Get_Table`; `FanID=1`, `SensorID=4` | Observado funcionando en Windows. |
| ID de método `5` | Hipótesis procedente de documentación/código de terceros, no comprobada en Linux ni asumida por el proyecto. |
| ABI de entrada | Desconocido: que dos parámetros sean `UInt8` en Windows no determina el empaquetado WMI/ACPI Linux. |
| ABI, tipo y longitud de salida | Desconocidos. Las tablas Windows no son un buffer Linux capturado. |
| Semántica de valores 1–10 y de las tablas | Desconocida para Linux; no demuestra puntos activos, RPM ni curvas escribibles. |

LenovoLegionLinux contiene una definición específica de `Fan_Get_Table` con ID
5 y usa la interfaz antigua `wmi_evaluate_method()` en su módulo. Es evidencia
de una implementación externa para modelos concretos, no un contrato para el
83EX. En particular, su propio código contiene configuraciones y comentarios
por modelo/BIOS que muestran diferencias de longitud, unidades y tablas entre
equipos que comparten el nombre del método o el GUID.

El Binary MOF es la fuente pública indicada por el kernel para conocer ID y
layout. El driver `wmi-bmof` lo recupera mediante una consulta de bloque WMI y
lo expone como `bmof`; usarlo requeriría verificar disponibilidad, posiblemente
cargar un módulo y consultar firmware. Por ello no se hizo ni se propone en
esta fase.

## Alternativas para RPM y curvas

| Vía | Valor actual | Estado y límite |
| --- | --- | --- |
| HWMON de `lenovo_wmi_other` | `fanX_input` sería RPM si el driver lo expone | Mejor vía estándar, pero no se expuso en el 83EX según el mensaje observado. No forzar capacidades. |
| `acpi_fan` / `yogafan` | Reportaron 0 RPM | No concluyente; cero no prueba un ventilador detenido ni valida la telemetría. |
| `Fan_GetCurrentFanSpeed` de `LENOVO_FAN_METHOD` | Referenciado por LenovoLegionLinux | ID, ABI y presencia no validados para 83EX; es una invocación activa prohibida sin autorización posterior. |
| `LENOVO_FAN_TABLE_DATA` | Puede describir niveles/calibración en algunos modelos | No es una prueba de RPM instantáneo ni de compatibilidad del 83EX; consultarlo es una operación WMI. |
| EC, puertos de E/S o ACPI directos | LenovoLegionLinux los usa para algunos modelos | Prohibidos aquí: son específicos por BIOS/EC, pueden cambiar tras BIOS y no son un respaldo seguro. |

En consecuencia, no existe aún una fuente de RPM real validada ni una ruta
aceptable para leer curvas en Linux para este equipo.

## LenovoLegionLinux: inspección y licencia

**Código inspeccionado:** `kernel_module/legion-laptop.c`, en particular su
cabecera SPDX, wrappers WMI y referencias de `Fan_Get_Table`. La cabecera del
archivo declara `GPL-2.0-or-later`. También se consultaron su README y
`doc/FEATURES_AND_TESTING.md`.

**No inspeccionado exhaustivamente:** el repositorio completo, sus scripts,
historial, dependencias, módulos o cada archivo de licencia. La licencia global
del repositorio no queda establecida por este informe; no se reutilizará código
ni estructuras del proyecto sin revisar la licencia y atribución del archivo
concreto y la compatibilidad con la licencia dual MIT OR Apache-2.0 de este
repositorio.

Su README documenta módulo externo, EC, ACPI y modos de forzar carga. Esas vías
no se ejecutaron ni son compatibles con las restricciones de la fase actual.

## Riesgos e incógnitas que bloquean una prueba futura

1. No hay ABI de `Fan_Get_Table` validado para el 83EX/NUCN15WW.
2. No hay interfaz de usuario Linux demostrada para invocar el método; la API
   identificada está dentro del kernel y exige un controlador.
3. Un ID de método, estructura o longitud tomada de otro Legion puede devolver
   datos engañosos, fallar o producir efectos imprevistos.
4. La semántica de una tabla 1–10 puede ser índice, escala, placeholder o datos
   por modo; no permite inferir RPM ni una curva activa.
5. El soporte HWMON de Other Mode depende de bloques de capacidad que no están
   validados en el kernel exacto instalado.

No se propone experimento real en esta entrega. Antes de cualquier Fase 4B se
deberán presentar por separado el método, ID y ABI respaldados por evidencia;
argumentos y tamaño máximo; comando o módulo exacto; riesgo; registro; límites
de salida; plan de interrupción y alternativa de no ejecutar nada. Será necesaria
una autorización explícita nueva.

El análisis posterior y más específico del contrato candidato está en
[Fase 4A.1: contrato técnico de `Fan_Get_Table`](fan-get-table-contract-research.md).
Confirma que el ID `5` de fuentes externas, así como cualquier empaquetado
binario, siguen siendo hipótesis para el 83EX/`NUCN15WW`.

## Fuentes públicas consultadas

- [API WMI del kernel](https://www.kernel.org/doc/html/latest/driver-api/wmi.html)
- [Guía de desarrollo de controladores WMI](https://www.kernel.org/doc/html/latest/wmi/driver-development-guide.html)
- [Driver WMI Binary MOF](https://docs.kernel.org/wmi/devices/wmi-bmof.html)
- [Código `core.c` de ACPI-WMI](https://github.com/torvalds/linux/blob/master/drivers/platform/wmi/core.c)
- [Código `wmi-gamezone.c`](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/lenovo/wmi-gamezone.c)
- [Código `wmi-other.c`](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/lenovo/wmi-other.c)
- [LenovoLegionLinux: `legion-laptop.c`](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/kernel_module/legion-laptop.c)
- [LenovoLegionLinux: capacidades y pruebas](https://github.com/johnfanv2/LenovoLegionLinux/blob/main/doc/FEATURES_AND_TESTING.md)
