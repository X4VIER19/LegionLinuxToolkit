# Primer ensayo del preset: protocolo propuesto, ejecución bloqueada

83EX / NUCN15WW / Nobara 7.2.11. Preparación únicamente: no se carga el
módulo, no se leen sensores ni se ejecuta MethodId 6 en esta tarea. AGENTS.md
no cambia. La autorización futura deberá establecer expresamente la excepción
experimental para UNA escritura y las lecturas/carga/descarga que la acompañen.

## Evidencia incorporada y pendiente concreta

El propietario confirmó captura FACT de 316 bytes, paquetes 6/9/10/11,
matching_catalogues=[0], uniquely_matches_fnt0=true y consistencia con Windows;
módulo retirado. Se registra como observación Linux histórica, no estado EC
actual ni garantía de la próxima selección SFAN. No se pide otra captura FACT.

CPU: k10temp, etiqueta Tctl, observación previa 43.5 °C. RPM: acpi_fan
funcional y yogafan_83ex con FA1S/FA2S x100 observado; dos canales sin mapeo
CPU/GPU. El yogafan original de 0 RPM no sirve de testigo. Un solo canal
acpi_fan tampoco demuestra ambos ventiladores.

**Bloqueo concreto de supervisión:** no consta una fuente de temperatura GPU
instantánea validada en Linux. No sustituirla por acpitz, NVMe ni gpu_temp de
lenovo_wmi_other (límite). No declarar lista la ejecución hasta identificar
una fuente para la RTX 4060 y comprobar sus lecturas/unidades. Candidato:
NVML/nvidia-smi del controlador ya instalado, si existe; no instalarlo ni
ejecutarlo en esta preparación. Su consulta puede despertar la GPU y alterar
el consumo. Esa comprobación necesita permiso independiente; ejemplo exacto
para revisión, NO ejecutado:

```bash
nvidia-smi --query-gpu=index,name,pci.bus_id,temperature.gpu --format=csv,noheader,nounits
```

Si falta, falla o devuelve N/A, cancelar; no inventar temperaturas ni agregar
otra interfaz WMI. La eventual supervisión debe seleccionar inequívocamente
la RTX 4060 identificada, no asumir GPU 0. Ninguna fuente nueva queda autorizada
por este documento.

## Protocolo térmico conservador propuesto

Estos números son **cortes operativos para este ensayo en reposo**, no límites
oficiales del Ryzen/NVIDIA, seguridad térmica demostrada ni reglas del EC.
No se obtuvieron del catálogo FNT/FI. Son deliberadamente restrictivos y
necesitan aprobación del propietario antes de usarse en una ejecución real.

| Condición | Antes del intento | Después de iniciar el intento |
|---|---|---|
| CPU Tctl | <=55 °C durante 60 s | Anomalía a >=70 °C |
| GPU instantánea identificada | <=50 °C durante 60 s | Anomalía a >=65 °C |
| Subida frente a última muestra previa | Sin tendencia creciente sostenida | Anomalía si cualquiera sube >=10 °C |
| Canales RPM fiables 1 y 2 | Ambos >=1200 RPM | Anomalía <1000 RPM o caída >50% |
| Frescura | Todas las muestras <=3 s | Pérdida/error/no actualización >3 s: anomalía |
| Alimentación/perfil/carga | AC estable, custom/GZ44=3, FullSpeed=0, sin carga intensiva | Cualquier cambio: detener ensayo |

El umbral RPM es una regla conservadora del experimento: un valor menor puede
ser legítimo bajo otra política y no demuestra ventilador detenido. La lectura
0 no validada del yogafan original se rechaza por procedencia, no por inferencia
de estado físico. Dos valores iguales no acreditan independencia mecánica.

Equipo en superficie rígida, ventilación libre, sin juegos, compilaciones,
benchmarks, actualizaciones ni otra carga intensiva. Guardar el trabajo antes.
No inducir carga para hacer girar ventiladores ni alcanzar un criterio de éxito.
No tocar Fn+Q, Vantage, perfiles ni curvas durante la ventana.

## Supervisión independiente y secuencia manual

1. **No escritura todavía:** acordar/aprobar sensores, cortes y contingencia;
   confirmar Windows/LLT accesible, preset/ajustes de referencia conservados.
   Identificar la fuente GPU pendiente y preparar los dos canales validados.
   La sustitución temporal de yogafan necesita su propia autorización; no la
   realiza el ensayo de escritura. Mantener acpi_fan y lenovo_wmi_other intactos.
2. Supervisión en un proceso/aplicación independiente del módulo y de la CLI
   que hará la llamada, visible en otra terminal/pantalla y con registro horario.
   Usar k10temp:Tctl y la fuente GPU efectivamente validada; RPM mediante el
   quirk identificado por módulo, enlace y DMI, no solo name=yogafan.
   Usar las herramientas de diagnóstico existentes: no se añade otro monitor.
   Si ninguna herramienta existente proporciona esos cuatro valores, se bloquea
   el intento; una salida antigua de la CLI no es supervisión continua.
3. Ventana finita propuesta: una muestra por segundo, 60 s previos y 120 s
   posteriores; no servicio, daemon ni sondeo indefinido. Esta adquisición
   repetida requiere autorización específica adicional a la única escritura.
   La implementación offline no arranca ningún temporizador ni proceso.
4. En la última muestra previa: verificar identidad/BIOS, AC, custom/GZ44=3,
   FanFullSpeed=0 reciente (no el dato Windows ni el dato Linux histórico),
   ausencia de errores y carga; mantener estable la selección del equipo.
   La consulta de modo/FullSpeed usa exclusivamente los lectores ya revisados
   y necesitará autorizar sus invocaciones activas. No repetir FACT.
5. Si cualquier condición falta, CANCELAR ANTES de entrar al transporte. No
   bajar temperaturas cambiando perfiles automáticamente, alterar límites ni
   considerar satisfechos requisitos por flags declarados.
6. Solo con autorización nueva y el diff de activación revisado: una llamada
   instancia 0 / MethodId 6 / GUID 92549549-4BDE-4F06-AC04-CE8BF898DBAA,
   payload fijo de 64 bytes [1..10], sin reintentos. Guardar bytes exactos,
   instante de intento, resultado ACPI/errores y muestras antes/después.
7. Ante anomalía POSTERIOR: terminar el ensayo, retirar cargas y solicitar
   apagado normal sin esperar a que el sistema siga calentándose. Un límite
   adicional de contingencia propuesto es CPU>=80 °C o GPU>=75 °C, o pérdida
   de supervisión mientras suben las temperaturas: no esperar al fin de los
   120 s ni a arrancar Windows. El apagado normal (menú de escritorio o
   `systemctl poweroff`) deberá estar incluido en el permiso de contingencia.
   Si el kernel no responde y la temperatura sigue subiendo, el apagado físico
   es último recurso manual con riesgo de pérdida de datos; no se promete
   reseteo del EC. No automatizar escrituras de rescate ni modificar FullSpeed.

**Cancelar antes** evita invocar SFAN. **Detectar después** no puede detener ni
revertir de forma demostrada el AML que ya empezó. Ctrl+C, cerrar la CLI o
un timeout no cancela SFAN de manera fiable; un syscall bloqueado tampoco
autoriza forzar descarga. No hay una transacción ni rollback implementados.

## Transporte: reutilización y diseño de activación mínimo

Se conserva el transporte kernel staged, la política C compartida y el
serializador Rust, sin duplicar lectores ni escribir nada nuevo al firmware.
La constante `legion_control_execution_authorized=false` permanece intacta;
no se crea un comando aparente de aplicación que la omita.

Diseño para el diff de activación FUTURO: endpoint experimental separado,
exclusivo y raíz, no integrado en legion-cli normal; feature de compilación
off por defecto y confirmación literal
`I-AUTHORIZE-ONE-DEFAULT-FAN-TABLE-83EX-NUCN15WW` por ejecución. Solicitud
versionada con longitud exacta, solo payload byte-a-byte igual al preset;
rechazar cualquier selector, relleno, índice o longitud distintos. Verificar
precondiciones observadas y recientes, no solo parámetros gz44/full_speed
aportados por el usuario. Consumir la guardia atómica antes del primer intento
de firmware, incluso si hay error/copia fallida. Un único propietario/proceso;
ningún segundo open, señal o fallo permite reintentar durante esa carga.

La función staged ya valida DMI/preset/modo/FullSpeed, usa mutex con remove y
libera output ACPI. Sus argumentos de modo y FullSpeed todavía no se adquieren
de forma independiente por un llamador real: **no basta cambiar false por true**.
El diff pendiente debe conectar esos controles y el endpoint bajo aprobación
expresa. No ejecutar firmware desde init/probe/remove; no autoload/instalación,
setters genéricos, payload de usuario arbitrario ni bucle de control.

No hay actualmente un comando ejecutable de escritura autorizado. El comando
exacto y los hashes del artefacto activado deben entregarse para aprobación
después de resolver la fuente GPU y revisar ese diff; no se presenta un comando
inexistente como si ya permitiera realizar la prueba.

## Limpieza y evidencia posterior

Tras retornar éxito o error, descargar el módulo experimental si no está ocupado;
comprobar eliminación de sus nodos y conservar registros. Un futuro wrapper
independiente debe usar trap, detectar módulo cargado aun si insmod falla y
mostrar errores de descarga. No rmmod -f, reintentos ni limpieza que borre
capturas. Si el método sigue bloqueado, no forzar su retirada.
Descargar el módulo **no** revierte cambios del EC.

Integer(0) solo acredita forma de respuesta; WMAB lo devuelve también cuando
omite SFAN. El preset puede no causar cambio visible. RPM/temperaturas dentro
del protocolo indican funcionamiento observado, no validación de los 107
destinos. GFAN no es una comprobación universal inocua (F10A=0 llama INIF);
no se añade automáticamente al ensayo. No pedir otra FACT como readback EC.

Windows/LLT 2.34.0: solo cuando sea térmicamente seguro y se haya autorizado,
arrancar Windows, abrir LLT, Personalizado/GodMode V2, FanFullSpeed desactivado,
Predefinido -> Guardar; comprobar RPM/temperaturas y ajustes de potencia
revisados. Dos aplicaciones manuales ya observadas respaldan ese flujo normal,
no recuperación tras escritura parcial. Fn+Q, reinicio y Windows no garantizan
rollback; persisten fallo parcial, firmware/EC bloqueado y equipo sin arranque.

## Comprobación offline disponible ahora

```bash
/home/x4vier19/.cargo/bin/cargo run --locked --offline -p legion-cli \
  --features experimental-fan-control-preflight \
  --bin legion-cli-fan-control-preflight -- --simulate --format json
```

El preflight existente publica `thermal_trial_protocol` y
`fact_capture_reference` con procedencias separadas. El evaluador puro reutiliza
TelemetryValue/FanTelemetry, distingue cancelar antes de anomalía después,
rechaza ausencia/errores/unidades incorrectas/límites térmicos como sensores,
datos viejos y RPM sin validar. Ni una fixture perfecta construye autorización.
