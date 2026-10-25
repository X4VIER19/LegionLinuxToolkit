# FanFullSpeed sin apropiarse del GUID Other

## Disponibilidad verificada estáticamente

Nobara `7.2.11-200.nobara.fc44.x86_64`: include/linux/acpi.h:421 declara
wmi_evaluate_method; Module.symvers:25593 la exporta GPL desde
drivers/platform/wmi/wmi. CONFIG_ACPI_WMI=m. Nuestro módulo es GPL y su
compilación/modpost comprueba la dependencia; no se instala ni carga.

Se inspeccionó la fuente base identificada previamente por el SRPM Nobara:
[CachyOS b0ce79e core.c](https://github.com/CachyOS/linux/blob/b0ce79ee33ddd93d513f77341167956e1355758e/drivers/platform/wmi/core.c),
funciones wmi_find_device_by_guid, wmi_device_put, wmi_evaluate_method y
wmidev_evaluate_method. Se contrastó con
[stable v7.2.11](https://github.com/gregkh/linux/blob/v7.2.11/drivers/platform/wmi/core.c).
No se presume haber reconstruido todos los parches downstream del binario
Nobara. Cabeceras, symvers y modpost sí corresponden a su kernel-devel exacto.
La [documentación WMI](https://docs.kernel.org/driver-api/wmi.html) mantiene
esta interfaz como deprecated; su disponibilidad no se extrapola a otros kernels.

## Propiedad, vida útil y concurrencia

La API busca el dispositivo en el bus por GUID, toma la referencia devuelta
por bus_find_device, llama wmidev_evaluate_method y ejecuta put_device al
terminar. No exige que el llamador sea su driver vinculado ni utiliza datos
privados de lenovo_wmi_other. La dependencia de símbolo mantiene referenciado
el núcleo WMI mientras el módulo está cargado. No se guarda un wmi_device
ajeno, ni se necesita crear un segundo driver o eliminar la vinculación actual.

La búsqueda devuelve la primera coincidencia, no una selección por sufijo
`-25`. La API no resuelve por sí sola GUID duplicados: antes de la prueba
se exige una única coincidencia del GUID en la máquina. No hay fallback.
Si falta el GUID, devuelve error ACPI, sin reintentar. Hay que abortar ante
hot-removal/suspensión/transiciones; una referencia a device no garantiza por
sí sola que el firmware o todos sus objetos ACPI sigan operativos.

El mutex propio y la guardia atómica solo excluyen nuestras solicitudes.
La implementación inspeccionada no serializa con las operaciones privadas
de lenovo_wmi_other. WMAE es NotSerialized. La rama exacta FFON no escribe,
pero puede coincidir con un cambio de estado producido por otro controlador.
El dato sigue siendo puntual, no autoriza escrituras posteriores ni acredita
estabilidad del perfil/FullSpeed. No utilizar durante cambios de perfil,
configuración, suspensión o retirada de drivers. La ausencia de una transacción
global sigue siendo una limitación; no se promete exclusión entre drivers.

## Adaptación mínima

Solo la ruta FanFullSpeed cambia a wmi_evaluate_method con GUID fijo,
instancia 0, método 17, entrada 00 00 02 04. Conserva Integer ACPI, contrato
de 20 bytes, liberación kfree incluso ante error, confirmación, DMI exacto y
una tentativa por carga del módulo. Init/exit solo gestionan miscdevices en
esta modalidad: no registran ni retiran ningún wmi_driver. La ruta de lectura
de tablas y la constante false del método 6 no se habilitan ni se alteran.

Si un kernel futuro retira este símbolo o cambia su semántica, fallará la
compilación/modpost: no reemplazarlo silenciosamente. La alternativa de largo
plazo es proponer una consulta de solo lectura FanFullSpeed en lenovo_wmi_other
con su propia sincronización y ABI documentada. No se implementa aquí.

## Procedimiento futuro, sujeto a autorización separada

No se ejecutó ninguna consulta ni carga en esta tarea. Antes de cargar,
confirmar mediante metadatos un único dispositivo Other vinculado a
lenovo_wmi_other, kernel exacto y ausencia del módulo experimental. No cambiar
perfiles ni detener/desvincular el controlador nativo. Compilar el módulo en
un directorio temporal con todos sus headers y únicamente
CONFIG_LEGION_LINUX_EXPERIMENTAL_FAN_FULL_SPEED_READ=y; verificar vermagic.

Preparar CLI con:
```bash
cargo build --locked -p legion-cli --features experimental-fan-full-speed-read \
  --bin legion-cli-fan-full-speed-read
```

Tras revisar y autorizar la ruta concreta del .ko, cargar una vez con
full_speed_confirmation=I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ. Comprobar
que el vínculo driver Other sigue siendo lenovo_wmi_other. Ejecutar solo:

```bash
sudo ./target/debug/legion-cli-fan-full-speed-read \
  --confirm I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ --format json
```

Descargar el módulo experimental inmediatamente, incluso ante error de lectura:
`sudo rmmod legion_lenovo_fan_wmi`. Verificar eliminación de nodos y conservación
del enlace del driver nativo. No reintentar si falla carga/consulta. Si falla
descarga, informar y no forzarla. Una aprobación futura debe concretar .ko,
comandos de carga/comprobación/limpieza y alcance de esa única consulta.
