# Plan incremental: prototipo de inventario y WMI de solo lectura

## Fase 0 — Línea base documental (completada)

Registrar el equipo objetivo, BIOS, interfaces detectadas, reglas de seguridad
y preguntas abiertas. No se carga ningún módulo ni se llama WMI.

Esta fase no autoriza un controlador propio. Antes de contemplarlo se debe
documentar por qué las interfaces existentes del kernel no cubren una capacidad
concreta del 83EX/`NUCN15WW`.

## Requisitos de entrada para la Fase 1

- Aprobación explícita de esta documentación inicial; no equivale a autorización
  para invocar firmware ni acceder a hardware real.
- Decidir lenguaje, formato de distribución y licencia del proyecto antes de
  incorporar código o dependencias.
- Definir y revisar el contrato de entrada/salida del CLI y las fixtures sysfs
  simuladas, incluyendo casos de ausencia, permisos y datos malformados.
- Mantener los adaptadores reales fuera de la configuración de prueba y de CI.
- Conservar la compatibilidad de hardware como 83EX/`NUCN15WW` únicamente; el
  DMI exacto se incorporará al registro cuando se obtenga mediante una lectura
  pasiva autorizada por la política aplicable.

## Fase 1 — Inventario pasivo de CLI

Crear un CLI sobre una fuente de datos simulada de sysfs. Debe emitir JSON
estable y una vista humana. Las pruebas automatizadas solo usan fixtures de
árboles sysfs falsos; no hay acceso a hardware, firmware ni rutas reales en CI.

**Criterio de salida:** cada campo indica su fuente, unidad y si es medición o
límite. Los fallos de lectura no se convierten en ceros. La configuración de
prueba no puede instanciar un transporte real de WMI, ACPI o EC, acepta solo
una raíz de datos simulados, rechaza `/sys`, `/proc` y `/dev`, no usa hardware
como respaldo y no se ejecuta con `sudo`.

## Fase 2 — Registro de compatibilidad

Implementar una base declarativa local por modelo, DMI, BIOS, interfaz,
operación y versión de kernel cuando corresponda. Para 83EX/NUCN15WW, marcar
perfiles y atributos como lectura observada; ventilador como desconocido. El
registro no habilita métodos WMI.

**Criterio de salida:** el programa se niega a declarar soporte de ventilador o
escritura basándose solo en GUID, familia, DMI o una tabla de otro modelo.

## Fase 3 — Abstracción de transporte WMI simulada y aislada

Definir una abstracción de transporte que permita construir y validar una
petición de método sin enviarla, usando exclusivamente datos simulados. Añadir
fixtures de respuestas conocidas de Windows y pruebas de decodificación,
longitudes y rangos de tablas. La aplicación debe seguir funcionando sin un
transporte WMI real. Las fixtures deben conservar procedencia (Windows, BIOS,
perfil y argumentos) y no convertir una observación de LLT/WMI en
compatibilidad Linux validada.

No está confirmada una interfaz WMI funcional desde espacio de usuario en
Nobara. Los archivos sysfs observados no se interpretan como una vía para
invocar métodos WMI. Esta fase no implementa ni prueba transporte WMI real, no
es un controlador del kernel y no justifica cargar módulos.

**Criterio de salida:** se valida el contrato de buffers offline; no hay
`Fan_Get_Table` ni `Fan_Set_Table` ejecutable desde detección o CI. Ningún
callback del futuro controlador (`probe()`, `init()`, `remove()` o
`shutdown()`) puede añadir llamadas WMI sin revisión y aprobación específicas.

## Fase 4 — Consulta manual de `Fan_Get_Table` (solo tras autorización)

Antes de esta fase, presentar el método, argumentos exactos, efecto esperado,
límites de respuesta y plan de reversión. Tras autorización explícita, exponer
un único comando opt-in de lectura con parámetros restrictivos y registro local.
Debe validar modelo, DMI, BIOS, interfaz, operación y kernel aplicable antes de
ejecutarse.

**Criterio de salida:** se compara la respuesta Linux con los datos de Windows
sin atribuir significado de control a los valores hasta corroborarlo. Las
comparaciones de Windows comienzan con consultas de solo lectura; un cambio
temporal desde LLT requiere autorización independiente.

## Fase 5 — Telemetría y UX

Solo cuando exista una fuente de RPM validada, presentar series temporales con
procedencia y avisos de calidad. La posibilidad de editar curvas queda fuera de
este plan y requiere un plan de seguridad independiente que incluya límites de
valor/buffer, confirmación específica, registro, supervisión térmica y un
procedimiento de interrupción y recuperación.
