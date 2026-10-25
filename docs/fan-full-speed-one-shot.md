# GetFeatureValue FanFullSpeed — contrato y prueba pendiente

Fuente estática: DSDT 83EX/NUCN15WW previamente recuperado. `_WDG` en 37437–37439
asocia DC2A8805-3A8C-41BA-A6F7-092E0089CD3B con object ID AE, una instancia,
flag método. `WMAE` (39483) acepta Arg1=0x11 y divide Arg2 en word TYP0,
byte FEA0 y byte DEV0. Para IDs=0x04020000 little-endian:

| Entrada | Tipo/desplazamiento | Valor |
|---|---|---|
| TYP0 | UInt16 LE, 0–1 | 0 |
| FEA0 | UInt8, 2 | 2 |
| DEV0 | UInt8, 3 | 4 |

Instancia 0, método 17, buffer exacto `00 00 02 04`. La rama DEV0=4/FEA0=2
(40334) retorna EC0.FFON, campo de un bit (13266) en ECMM SystemMemory
(10020). La respuesta Linux es un objeto ACPI Integer, **no** un buffer de
cuatro bytes. Esta rama no contiene asignaciones ni llamadas auxiliares.
No se extrapola al resto de GetFeatureValue: otras ramas inicializan estado.
Leer el registro mapeado del EC es una operación activa; su implementación
interna no se demuestra con AML. No se garantiza inocuidad por llamarse Get.

## Preparación existente reutilizada

El mismo módulo contiene el segundo GUID bajo
CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ=y, desactivado por defecto.
En esta modalidad solo intenta vincular el GUID Other; Kbuild rechaza combinar
las opciones de lectura de tablas o control. No hay autoload. Si otro driver
posee el GUID no se fuerza ni se desplaza: el endpoint responde unsupported.
Debe comprobarse ese bloqueo antes de una prueba de carga autorizada.

La activación requiere además el parámetro inmutable
`full_speed_confirmation=I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ`.
Init/probe/remove son pasivos. Una lectura explícita del nuevo miscdevice
de raíz `/dev/legion-fan-full-speed-read-experimental` (0400) comprueba DMI
exacto, consume una guardia global por carga antes del intento, mantiene el
mutex WMI y solicita exclusivamente el método 17. No acepta IDs externos.
Una apertura nueva, un fallo WMI o copy_to_user fallido no permiten repetir.

El resultado tiene 20 bytes: cabecera v1 de 12 bytes (version u16, header_size
u16, status u32, payload_size u32) y valor ACPI u64 LE. Status 0 exige size=8;
unsupported=1, error WMI=2, objeto ACPI incorrecto/ausente=3. Se conserva el
buffer completo en Rust; error/partial nunca se convierte en cero válido.
0=disabled, 1=enabled, otros enteros=unknown; los errores de contrato o
adquisición son error. Unknown/error bloquean la futura escritura. Enabled
es lectura válida, pero bloquea el control: no se desactiva automáticamente.

La CLI experimental hace una sola llamada read sobre la ruta constante,
rechaza enlaces y objetos no character-device y no reintenta. El DMI se
comprueba en kernel; el reporte deja los demás requisitos sin adquirir y
conserva la referencia Windows histórica. No inventa lecturas actuales de
perfil/temperatura/RPM. Se integra mediante el mismo preflight y serializador.
No produce RealReadAuthorization. MethodId 6 conserva su constante false;
SetFeatureValue no se implementa.

## Ejecución futura: requiere autorización separada, no realizada aquí

Primero verificar kernel/cabeceras, identidad y propietario del GUID mediante
metadatos. Si está vinculado a lenovo_wmi_other u otro driver, **abortar**:
estos comandos no deben utilizarse para desplazarlo. No usar unbind ni force.
Compilar temporalmente el módulo con únicamente FULL_SPEED_READ=y; comprobar
vermagic. Preparar la CLI sin ejecutarla:

```bash
cargo build --locked -p legion-cli --features experimental-fan-full-speed-read \
  --bin legion-cli-fan-full-speed-read
```

Solo tras aprobación del procedimiento, módulo exacto y comandos:

```bash
sudo insmod /RUTA_TEMPORAL_REVISADA/legion_lenovo_fan_wmi.ko \
  full_speed_confirmation=I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ
sudo ./target/debug/legion-cli-fan-full-speed-read \
  --confirm I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ --format json
sudo rmmod legion_lenovo_fan_wmi
```

La ruta del módulo debe concretarse en esa aprobación; no es un comando listo
para ejecutar ahora. Una sola consulta, sin reintentar. Descargar incluso ante
error de lectura; si falla la descarga informar, nunca rmmod -f. La carga no
consulta firmware por sí sola. No cambiar perfil ni ventiladores en esta prueba.
El valor Windows 0 no predice el resultado Linux. Un disabled solo acredita el
instante observado, no todos los requisitos de una escritura posterior.

Pruebas offline: códec Rust (entrada, enabled/disabled/unknown/error y longitudes),
preflight bloqueado y confirmación; shim C usa el mismo cuerpo de consulta con
WMI simulado, comprueba una llamada y liberación del objeto ante éxito, error
WMI, objeto ausente y tipo incorrecto. No carga ni ejecuta el módulo.
