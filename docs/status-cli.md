# Estado térmico puntual en Nobara — 83EX/NUCN15WW

```bash
cargo run --locked --offline -p legion-cli --bin legion-cli -- status
cargo run --locked --offline -p legion-cli --bin legion-cli -- status --format json
```

Es una consulta REAL y explícita, no una simulación; no se ejecutó durante el
desarrollo. No necesita features experimentales ni sudo por defecto. No eleva
privilegios: permisos insuficientes se reportan como errores, sin reintentos.

Reutiliza los cuatro atributos DMI permitidos para comprobar 83EX/NUCN15WW,
el lector confinado de hwmon (k10temp:Tctl y fanN_input de acpi_fan/yogafan),
ACAD/type y online, y PlatformProfileQuery (choices→active→choices, sin
atomicidad). Cada ejecución adquiere una sola instantánea, sin intervalos ni
procesos residentes; CPU/AC/cada fan se consultan una vez, sin repetir fallos.
Los campos fallidos no se reemplazan por cero ni por observaciones anteriores.

GPU usa el mismo lector NVIDIA extraído del monitor: una ejecución fija de
/usr/bin/nvidia-smi, entorno limpiado, temperatura.gpu/PCI/modelo, validación
estricta y espera acotada a 1 s. Requiere driver NVIDIA y /dev/nvidiactl ya
presentes para evitar carga de módulos automática. La consulta puede despertar
la GPU; no se conoce la edad física/caché del sensor. Las lecturas sysfs pueden
activar firmware a través del kernel: no se describen como metadatos pasivos.

Las fuentes y estados se conservan en JSON. yogafan original/no identificado
permanece no validado y su 0 no prueba detención física. acpi_fan informa sus
entradas disponibles, sin garantizar que representen ambos ventiladores. Un
quirk ya cargado se identifica mediante DMI y metadatos del módulo existentes;
status nunca lo carga ni sustituye controladores. Sin asignación CPU/GPU de RPM.

El comando no abre endpoints experimentales, no consulta FanFullSpeed, no
invoca WMI de control, no escribe registros, perfiles ni curvas. La feature
Linux `linux-status` selecciona solo los lectores, no control-plan ni setters.
Las tres barreras de escritura continúan en 0/false. Si un driver se bloquea
en una lectura del kernel, no se promete cancelación ni rescate automático.
