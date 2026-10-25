# Ensayo integrado de supervisión — ninguna escritura

83EX/NUCN15WW, Nobara. FACT=FNT0 queda como evidencia previa; no se captura
de nuevo. NVIDIA `temperature.gpu=34` fue comunicado por el propietario como
lectura real: confirma la fuente, no una temperatura actual ni una autorización.

## Adquisición e independencia

El binario separado `legion-cli-fan-trial-monitor` reutiliza los lectores
confinados hwmon, DMI, PlatformProfileQuery, el lector FanFullSpeed y el preflight.
No tiene transporte de escritura, setters, carga/descarga de módulos, cambio de
perfil ni apagado automático. MethodId 6 y su constante false no se modifican.
La feature `experimental-fan-trial-monitor` está desactivada por defecto.

Muestras a 1 Hz: 60 s completos de baseline y 120 s después de un marcador
**NO-WRITE**, 181 muestras en total. Las ventanas usan reloj monotónico;
la duración de pared incluye adquisición/journal. No hay servicio ni sondeo
indefinido. Las fases posteriores prueban los cortes POST del protocolo,
pero no significan que SFAN haya ocurrido. No es una validación de escritura.

Fuentes fijas, sin rutas de entrada configurables:

- Identidad: los cuatro atributos DMI ya aprobados, leídos una vez; comprobación
  exacta 83EX/NUCN15WW y registro existente. Identidad observada, no verificada.
- CPU: descubrimiento dinámico hwmon `k10temp`, única etiqueta `Tctl`, m°C /1000.
- Dos RPM: único hwmon `yogafan`, enlace device/driver/module identificado como
  yogafan_83ex, canales fan1_input/fan2_input. Se comprueba la procedencia antes
  de abrir inputs y después; cambia el enlace/dispositivo -> error. No se lee
  acpi_fan ni se acepta yogafan original como sustituto. Canales sin mapeo CPU/GPU.
  Es identificación por DMI/metadatos, no atestación criptográfica del módulo.
- AC: ACAD/type=Mains y online=1. Ausencia/formato distinto/error bloquean.
- Perfil: proveedor existente con choices -> active -> choices en cada muestra;
  exige custom válido, no una observación histórica. GZ44=3 es correspondencia
  DSDT, no lectura directa del EC. No promete atomicidad ni detectar cambios ABA.
- GPU: ejecutable instalado fijo `/usr/bin/nvidia-smi`, sin shell ni PATH, entorno
  limpiado, LC_ALL=C, argumentos exactos:
  `--query-gpu=pci.bus_id,name,temperature.gpu --format=csv,noheader,nounits`.
  Solo una RTX 4060 Laptop GPU, PCI ID estable, temperatura entera 5..110 °C;
  N/A, múltiples GPU, tipo/modelo incorrecto, error o salida >512 bytes bloquean.
  Timeout del proceso de 1 s, sin cache de último valor ni reintentos.
- FanFullSpeed: una sola consulta al endpoint existente al terminar el baseline,
  MethodId 17, instancia 0, entrada 00 00 02 04. Debe devolver disabled=0.
  El módulo de lectura debe estar previamente cargado y confirmado por separado;
  nunca se carga desde el monitor ni se repite una consulta consumida.

Una adquisición completa debe caber en la cadencia de 1 s; el watchdog externo
al worker falla a los 3 s, sin catch-up/reinicio de muestras. Conservadoramente
la antigüedad máxima de todos los datos es la duración del lote. NVIDIA entrega
una respuesta recién solicitada, pero no el timestamp físico del sensor: no se
demuestra ausencia de cache del driver. Ningún valor histórico sustituye errores.

FanFullSpeed se marca como actual solo en la muestra de frontera. Durante los
120 s posteriores no se reutiliza como estado actual: no hay más consultas
disponibles en ese endpoint por carga. El supervisor continúa evaluando sensores,
AC y perfil; el preflight estricto solo se evalúa con FullSpeed fresco en frontera.
Esto demuestra una comprobación puntual, no ausencia garantizada de cambios
posteriores del firmware o de otros programas.

Si una llamada de kernel bloquea los sensores ACPI, el watchdog marca la falta
de respuesta, no conserva los últimos números como actuales. Puede quedar un
worker bloqueado: salir/matar el monitor no cancela ese código de kernel.
No se promete supervisión física infalible ni se fuerza descarga de módulos.

## Cortes y registros

Se reutilizan los validadores del ensayo: CPU<=55 °C, GPU<=50 °C, ambos RPM>=1200
en baseline; rango total de CPU/GPU durante esos 60 s <=3 °C (estabilidad
conservadora propuesta). Después del marcador: anomalía CPU>=70 °C, GPU>=65 °C,
subida>=10 °C, RPM<1000 o caída>50%. Error de lectura/procedencia/AC/perfil,
fallo de journal o cadencia interrumpen el ensayo, sin reintentar.
Son cortes operativos propuestos, no límites oficiales ni prueba de fallo físico.

Registros JSONL persistentes en `trial-logs/` del repositorio (excluido de Git),
directorios 0700, archivos nuevos 0600 sin sobreescritura/enlaces. Cada evento
se sincroniza a disco antes de continuar. Guarda origen, hora, reloj monotónico,
bytes de CPU/RPM/NVIDIA/AC, metadatos de módulo/DMI, perfil original/normalizado,
errores y preflight de frontera. Muestra CPU/GPU/RPM/estado por terminal.
No se publican estos logs en commits. SIGKILL o fallo de alimentación pueden
dejar un último registro incompleto: sin evento final completed no hay ensayo
completo. Ni siquiera un log completo es RealReadAuthorization ni token de escritura.

La ausencia de carga intensiva es una atestación explícita del operador, no
una medición automática de carga. No correr juegos/compilaciones/actualizaciones,
no tocar Fn+Q ni sustituir módulos mientras se supervise. El monitor no puede
parar ni revertir SFAN, ni recuperar un kernel bloqueado en un read. El watchdog
puede registrar falta de respuesta, pero no cancelar de forma segura el código
del kernel. Ante anomalía real seguir el protocolo térmico/apagado ya revisado,
no forzar descarga ni enviar curvas de rescate.

## Ensayo preparado y permisos pendientes

Simulación ejecutable ahora, sin hardware ni tiempos de espera reales:

```bash
/home/x4vier19/.cargo/bin/cargo run --locked --offline -p legion-cli \
  --features experimental-fan-control-preflight \
  --bin legion-cli-fan-trial-monitor -- --simulate
```

Para compilar la variante real sin ejecutarla:

```bash
/home/x4vier19/.cargo/bin/cargo build --locked --offline -p legion-cli \
  --features experimental-fan-trial-monitor --bin legion-cli-fan-trial-monitor
```

Comando de ensayo REAL SIN ESCRITURA para una autorización expresa posterior:

```bash
sudo /home/x4vier19/Documentos/GitHub/LegionLinuxToolkit/target/debug/legion-cli-fan-trial-monitor \
  --live-no-write-rehearsal \
  --confirm I-AUTHORIZE-READ-ONLY-180S-83EX-SUPERVISION --attest-idle
```

No ejecutado automáticamente en esta preparación. Requiere primero:

La comprobación pasiva realizada al preparar este cambio encontró yogafan
original presente, yogafan_83ex ausente y el módulo/endpoint FanFullSpeed
ausentes. `/usr/bin/nvidia-smi` existe como ejecutable regular instalado.
No se ejecutó el ensayo real ni se cargó nada para satisfacer esos requisitos.

1. yogafan_83ex ya cargado y vinculado con ambos canales. Si está el original,
   el programa aborta. Su sustitución/restauración debe autorizarse/realizarse
   fuera de este ensayo; no descargarlo ni cambiarlo durante las ventanas.
2. Módulo FullSpeed preparado **solo lectura**, parámetro de confirmación
   correspondiente y guardia aún sin consumir. No activar variantes de método
   5/6/FACT. lenovo_wmi_other queda vinculado al GUID nativo, sin unbind.
3. Personalizado seleccionado manualmente con autorización y AC conectada;
   NVIDIA ya cargado y herramienta instalada. El monitor no corrige nada.
4. Permiso para las consultas periódicas: 181 CPU, 181 por canal RPM, 181 NVIDIA,
   AC/perfil por muestra y una FanFullSpeed en frontera; metadata/DMI al inicio.
   Sensores hwmon/perfil pueden ejecutar firmware a través del kernel: no se
   presentan como lecturas pasivas. No nuevas capturas FACT ni MethodId 6.

La salida exitosa acredita SOLO ese ensayo de adquisición/supervisión. Para
autorizar después Fan_Set_Table faltarán: aprobar cortes/contingencia; revisar
el diff mínimo del endpoint de preset aún bloqueado; conectar una comprobación
reciente al intento real (un log anterior no basta); consumir una guardia de
escritura de un intento y acordar cómo mantener el supervisor independiente
durante la llamada. No quitar false sin ese diff. Fn+Q/Windows/reinicio no
garantizan rollback; restauración parcial sigue sin demostrarse.

Compatibilidad práctica pendiente del diff de escritura: el módulo de solo
FullSpeed y el de control staged comparten nombre y sus opciones actuales son
mutuamente excluyentes. La activación futura debe integrar la comprobación
reciente y el endpoint restringido en el mismo helper revisado, no intentar
reemplazar módulos entre un log de readiness y la escritura. Este ensayo no
modifica el kernel ni autoriza esa integración. yogafan_83ex debe permanecer
sin sustitución durante todo el ensayo y cualquier futura llamada.
