# Ensayo del preset predeterminado: preparado, NO AUTORIZADO

Solo 83EX/NUCN15WW; no cambia el perfil. Se conservan FACT=FNT0 y la
referencia Windows como evidencia histórica, no como estado actual del EC.

El mismo helper combina `FAN_FULL_SPEED_READ=y` y `FAN_WMI_CONTROL_STAGING=y`.
Registra pasivamente el GUID de ventiladores; conserva el propietario nativo
de OTHER_METHOD. Ningún callback ejecuta firmware. El endpoint root-only
`/dev/legion-fan-default-preset-experimental` admite exclusivamente 64 bytes:
selector 1, ceros reservados y UInt16 LE [1..10]. Sin ioctl, otros presets,
fallback ni autoload. Una guardia atómica global se consume antes del intento,
también ante errores. Las tres constantes se activaron por autorización de
preparación; los artefactos NO se han ejecutado y la operación física sigue
requiriendo autorización independiente.

El monitor reutiliza los 60 segundos previos y 120 posteriores, los cortes
absolutos CPU<=55/GPU<=50/RPM>=1200 y el criterio de calentamiento >3 desde
un mínimo previo. Valida AC, custom, DMI, módulo/canales, temperaturas y
FullSpeed=0 recientes en la misma ejecución. La única consulta FullSpeed se
conserva además en el helper por un máximo de 3 s; otro proceso no puede usar
una lectura de una carga anterior. La comprobación de perfil no lee GZ44 ni
garantiza que firmware/otros procesos no cambien el estado inmediatamente.

Después de persistir intención, un worker realiza UN write(64), sin write_all,
reintentos ni repetición ante EINTR/short write. El monitor no espera a WMI:
continúa los sensores y registra su resultado o falta de respuesta. Un error
WMI o anomalía térmica POST no cancela SFAN: se sigue observando la ventana,
se informa fallo final y nunca se reenvía el preset. Un fallo de journal,
planificación, procedencia o bloqueo del kernel puede impedir supervisión;
no se garantiza independencia física de los sensores frente al EC/AML.

Compuerta de despacho: el informe preflight no autoriza nada. Inmediatamente
antes del callback se exige coincidencia exacta del registro, FullSpeed disabled,
procedencias coherentes (nunca fixtures en el camino real) y una reevaluación
BeforeWrite del frame con su edad tras el journal: AC, custom, temperaturas,
RPM, unidades/estados válidos y límites existentes. Un rechazo registra motivos
y nunca consume un despacho. Las comprobaciones kernel permanecen adicionales.

Comportamiento POST comprobado: se sigue observando ante error/resultado pendiente
del escritor, incumplimiento térmico/RPM/AC/custom expresado en un frame, o error
de adquisición reportado por el worker (se registra sin reutilizar valores).
No se repite esa muestra; solo se intenta la siguiente muestra programada.
Terminan el monitor los fallos de journal, retraso de planificación, adquisición
fuera de cadencia/edad, identidad incompatible o cambio de huella de fuentes.
Un timeout del worker se puede registrar, pero un bloqueo de kernel o pérdida
continuada de sensores no permite prometer supervisión efectiva ni recuperación.

**Preparación activada, operación NO AUTORIZADA:** constantes shell 1,
Rust true y kernel true, exactamente las tres líneas revisadas. El comando
ya NO está bloqueado por esas constantes: NO ejecutarlo sin nueva autorización.

```bash
sudo bash /home/x4vier19/Documentos/GitHub/LegionLinuxToolkit/scripts/run-no-write-supervision-with-modules.sh \
  I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET
```

El wrapper reutiliza la sustitución/restauración de yogafan, mantiene ambos
módulos durante todo el ensayo y conserva logs privados en trial-logs/.
Descarga el helper y el quirk antes de restaurar yogafan; no fuerza descargas.
Un worker bloqueado puede mantener el módulo ocupado. Si falla la limpieza,
no repetir la escritura ni forzar rmmod; informar estado y conservar logs.

Un retorno Integer(0) solo verifica el retorno WMI/AML: incluso una ruta que
omite SFAN puede devolverlo. RPM/temperaturas permiten observar consecuencias,
no demostrar los 107 destinos EC, la selección efectiva de catálogo ni una
curva aplicada. GFAN tampoco acredita por sí solo esos estados. CMRD, Fn+Q,
reiniciar o restaurar desde Windows/LLT no garantizan rollback parcial. Antes
de ejecutar: autorización independiente del comando concreto, con los hashes
preparados, la contingencia térmica y la restauración manual Windows ensayada.
Ni un log previo ni una respuesta correcta constituyen autorización.

## Paquete final activado, sin ejecutar, y contingencia del operador

El diff separado `default-preset-activation.NOT-APPLIED.patch` muestra SOLO
las tres constantes revisadas y YA APLICADAS. Se conserva su nombre histórico;
no volver a aplicarlo. Se comprobó con `git apply --reverse --check`, sin
revertirlo. La autorización recibida cubre solo preparación, no operación.
No se cambia AGENTS.md ni se amplía a otros métodos, presets o cargas.

Artefactos actuales, activados pero SIN EJECUTAR/SIN CARGAR:

- Monitor: SHA-256 `56ab69f2994032840a9cd6e986fa6b477a869d46a2f94cc8d2d6863682262a12`.
- Helper combinado: SHA-256 `1febdb209599ab6d6b1559977d7e6153daf91ef29e8a475c0b740ab9709d2697`,
  `target/fan-default-preset-staged/legion_lenovo_fan_wmi.ko`, nombre interno
  `legion_lenovo_fan_wmi`, dependencia wmi, vermagic Nobara 7.2.11 completo.
  Las copias de C, headers y Makefile coinciden con el árbol actual y el comando
  guardado de Kbuild activa SOLO FullSpeed y control staging.

El modo SIN escritura permanece disponible con el helper anterior exclusivo
de FullSpeed, el mismo quirk y el monitor recién recompilado:

```bash
# PROPUESTO, NO EJECUTADO: también requiere autorización de lecturas/carga.
sudo bash /home/x4vier19/Documentos/GitHub/LegionLinuxToolkit/scripts/run-no-write-supervision-with-modules.sh \
  I-AUTHORIZE-TEMPORARY-MODULES-AND-NO-WRITE-SUPERVISION
```

Monitor y helper ya reconstruidos; hashes nuevos fijados y verificados en el
wrapper. No suprimir comprobaciones ni usar los hashes bloqueados anteriores.

No ejecutar pruebas que invoquen el modo staged después de activarlo. En
particular, NO ejecutar la antigua prueba de barrera false ni tests de la
feature sin filtro. El filtro offline revisado es `trial_monitor::tests::`.
La llamada final propuesta es la del wrapper con el literal del preset arriba;
permanece NO AUTORIZADA. El modo normal del wrapper no se convierte en escritura.
No leer a mano el endpoint FullSpeed antes: consumiría su única observación.
AC conectada, custom seleccionado manualmente, sin cargas intensivas ni Fn+Q
durante la ventana, Windows/LLT disponible para recuperación manual posterior.

- **Antes del intento:** ante cualquier rechazo, cancelar y dejar actuar el
  trap. No reintentar el preset ni corregir condiciones automáticamente.
- **Después:** CPU>=70 °C, GPU>=65 °C, incremento>=10 °C, RPM<1000 o caída>50%
  son alarmas operativas, no prueba aislada de avería. Detener cualquier carga
  de trabajo, no tapar ventilación y vigilar los sensores; no esperar a que
  terminen los 120 s si existe una situación térmica peligrosa o sin telemetría
  fiable. Solicitar apagado normal del sistema si no se estabiliza o no puede
  mantenerse una observación fiable. El monitor NO apaga ni rescata el equipo.
- **Error WMI:** no volver a aplicar ni mandar otra curva. El resultado puede
  ser parcial aunque sea error; conservar logs, observar temperaturas/RPM si es
  posible y permitir limpieza normal sin forzar. Un Integer(0) tampoco prueba
  éxito físico ni autoriza una segunda tentativa.
- **Kernel bloqueado:** no se puede cancelar SFAN con garantías ni confiar en
  traps/watchdog. Intentar apagado normal si responde; si no responde y existe
  peligro térmico, el apagado físico es último recurso del operador, con riesgo
  de pérdida de datos. No usar rmmod -f, unbind forzado ni escrituras EC.
- **Después de un apagado seguro:** Windows/LLT permite intentar restaurar
  manualmente Personalizado, FanFullSpeed desactivado y Predefinido → Guardar,
  como se ensayó. Fn+Q, LLT, reinicio y apagado NO garantizan restaurar todos
  los registros de una ejecución parcial; no constituyen rollback demostrado.

La retirada de módulos solo retira los controladores, no deshace cambios EC.
Antes de disparar se puede cancelar sin escritura; después de disparar, parar
el monitor o descargar el módulo no deshace ni interrumpe con seguridad SFAN.
