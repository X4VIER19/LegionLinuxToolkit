# Ensayo del preset predeterminado: preparado, NO AUTORIZADO

Solo 83EX/NUCN15WW; no cambia el perfil. Se conservan FACT=FNT0 y la
referencia Windows como evidencia histórica, no como estado actual del EC.

El mismo helper combina `FAN_FULL_SPEED_READ=y` y `FAN_WMI_CONTROL_STAGING=y`.
Registra pasivamente el GUID de ventiladores; conserva el propietario nativo
de OTHER_METHOD. Ningún callback ejecuta firmware. El endpoint root-only
`/dev/legion-fan-default-preset-experimental` admite exclusivamente 64 bytes:
selector 1, ceros reservados y UInt16 LE [1..10]. Sin ioctl, otros presets,
fallback ni autoload. Una guardia atómica global se consume antes del intento,
también si la barrera lo bloquea. La constante kernel sigue siendo `false`.

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

**Tres barreras pendientes de revisión/autorización:** constante shell 0,
constante Rust false (antes de todo acceso al host) y constante kernel false.
Compilar la variante combinada no las elimina. La ejecución propuesta, hoy
bloqueada antes de cargar módulos, es **NO AUTORIZADA**:

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
de activar: revisar diff y hashes nuevos, aprobar las tres barreras, el comando
concreto, la contingencia térmica y la restauración manual Windows ensayada.
Ni un log previo ni una respuesta correcta constituyen autorización.
