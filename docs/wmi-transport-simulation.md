# Transporte WMI: contrato simulado y validación futura

## Alcance de la Fase 3

Esta fase define un contrato Rust en memoria entre una solicitud WMI y una
respuesta opaca. `legion-core` contiene solamente tipos y el trait
`WmiTransport`; la única implementación es `SimulatedWmiTransport`, ubicada en
`legion-fixtures`. No hay transporte Linux, acceso a sysfs, ACPI, EC, WMI,
dispositivo, módulo del kernel ni mecanismo de respaldo al host.

El comando `legion-cli wmi` selecciona exclusivamente escenarios embebidos por
nombre. Su salida siempre usa `source: "simulated"` y
`transport: "simulated-wmi"`; no representa el resultado de una llamada al
firmware.

## Contrato deliberadamente limitado

`WmiRequest` conserva una operación lógica, el acceso `ReadOnly`, argumentos
de ocho bits y metadatos de método. No existe un tipo de solicitud de escritura
en esta fase. En consecuencia, `Fan_Set_Table` no es una operación
representable ni ejecutable por el contrato o las fixtures.

`WmiResponse` contiene una secuencia opaca de bytes y un tamaño declarado. Solo
se valida que ese tamaño coincida con la longitud de los bytes. La aplicación
no descodifica cabeceras, campos, tablas o valores de RPM: hacerlo requeriría
conocer y validar el ABI binario.

Los escenarios disponibles son:

| Escenario | Resultado simulado |
| --- | --- |
| `valid` | Bytes sintéticos opacos con longitud consistente. |
| `unavailable` | Método no disponible, sin acceso alternativo al host. |
| `inconsistent-buffer` | Tamaño declarado distinto de la longitud real. |
| `unsupported` | Operación rechazada por la fixture. |

Las rutas como `/sys`, `/proc` o `/dev` no son escenarios válidos y no se
abren. Las pruebas se ejecutan sin `sudo` y no pueden crear un transporte real.

## Evidencia, hipótesis y desconocidos

| Elemento | Estado | Base |
| --- | --- | --- |
| Nombre `Fan_Get_Table` | Observado en Windows | Documentación de la observación previa. |
| Argumentos `FanID=1`, `SensorID=4` como `UInt8` | Observados en Windows | Prueba histórica documentada. |
| Resultado lógico de tablas de diez valores | Observado en Windows | No implica el formato de un buffer Linux. |
| GUID, identificador numérico de método y ABI de solicitud/respuesta Linux | Desconocidos | No se modelan como valores reales. |
| Bytes de la respuesta `valid` | Hipótesis sintética | Sirven solo para probar longitud, procedencia y serialización. |
| Lectura funcional de WMI desde espacio de usuario en Nobara | Desconocida | Los archivos sysfs observados no son una vía de invocación demostrada. |

La procedencia de cada solicitud o respuesta incluye plataforma observada,
origen documental, BIOS de referencia cuando está disponible y nivel de
conocimiento. Su campo `artifact` solo admite `static-simulation` en esta fase;
por tanto, el contrato no puede etiquetar una fixture como respuesta de
hardware real. Una observación de Windows no se convierte en validación ni
soporte de Linux.

## Protocolo para una validación futura de `Fan_Get_Table`

No se ejecuta ninguna parte de este protocolo en la Fase 3. `Fan_Get_Table` es
una invocación activa de firmware incluso si parece una lectura y exige una
autorización independiente y explícita antes de cada ejecución.

### Preparación común

1. Identificar el equipo exacto, BIOS, DMI cuando haya sido obtenido
   pasivamente y el entorno operativo.
2. Registrar la hipótesis concreta, nombre lógico del método, argumentos,
   tamaño máximo aceptado, forma prevista del resultado y criterios de fallo.
3. Preparar el comando exacto, revisar que no cambie perfil, curva o estado, y
   explicar al propietario su propósito y riesgo.
4. Obtener aprobación para ese comando concreto. La aprobación para desarrollo
   o para esta simulación no autoriza una invocación real.
5. Establecer un registro de fecha, identidad, BIOS, herramienta, argumentos,
   resultado bruto, longitud, error y observación posterior.

### Referencia Windows

La observación histórica de `Fan_Get_Table(FanID=1, SensorID=4)` ya está
documentada; no debe repetirse salvo que una hipótesis concreta lo justifique.
Si se autoriza una comparación adicional, se empleará una consulta de lectura
mediante PowerShell/WMI o una herramienta de diagnóstico, con el perfil visible
registrado y sin cambiarlo desde scripts. Cualquier cambio temporal mediante
LLT requiere aprobación separada. `Fan_Set_Table` sigue prohibido.

### Posible validación Linux

Primero debe demostrarse, mediante investigación pasiva, una interfaz de
espacio de usuario y su ABI para el kernel concreto. La presencia de un GUID,
un dispositivo sysfs o `wmidev_evaluate_method()` no satisface ese requisito.
Solo después se podrá proponer un comando manual de un único método, parámetros
fijados, límite de búfer y sin escritura. Antes de ejecutarlo se presentarán el
comando exacto, sus parámetros, riesgos y plan de interrupción/recuperación
para una nueva autorización.

No se extrapolará una respuesta de Windows a Linux, ni se habilitará ningún
control de ventilador a partir de esta fase.
