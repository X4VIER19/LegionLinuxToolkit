# SFAN: contrato estático y bloqueos de seguridad

Fuente: DSDT.dsl recuperado del 83EX/NUCN15WW, disponible localmente en
`docs/firmware-evidence/gmzn-16arp9/`. No se añade el DSDT completo al commit
para evitar publicar identificadores no revisados. Análisis exclusivamente
estático; ninguna escritura se ha probado ni autorizado.

## Entrada reconstruida

`WMAB` (líneas 39450–39469), método 6, llama `SFAN(Arg2)` solo si
`EC.GZ44 != 7`. Retorna cero incluso cuando omite la operación: cero no
demuestra aplicación. No verifica Arg0/instancia ni tamaño/tipo de Arg2.
`SFAN` (35091–35447) crea estos campos, con enteros AML little-endian:

| Offset decimal | Campo AML | Tipo | Uso demostrado |
|---|---|---|---|
| 0 | F000 | byte | Selector de catálogo/perfil |
| 1 | F001 | byte | Declarado, no consumido |
| 2–5 | F002 | dword | Declarado, no consumido; significado no probado |
| 6–25 | F003–F00C | diez words | Índices CFST y selección FNT |
| 26 | F00D | byte | Declarado, no consumido |
| 27–30 | F00E | dword | Declarado, no consumido; significado no probado |
| 31–50 | F00F–F018 | diez words | Declarados, no consumidos |
| 51 | F019 | byte | Declarado, no consumido |

El span mínimo es 52 bytes. El AML no exige longitud exacta ni comprueba
conteos. Los nombres `declared_count` del modelo offline son etiquetas de
inspección de los dwords, no semántica confirmada por SFAN. Tampoco se ha
demostrado el framing adicional que un proveedor Windows pudiera insertar.
El modelo solo inspecciona evidencia en memoria y no ofrece codificador.

## Selección y efectos

Si PMID=1: PGID=2 o 3 selecciona FNT2; PGID=5 FNT4; cualquier otro FNT3.
Si PMID!=1: DCID=0 selecciona FNT1; cualquier otro FNT0.
En cada caso F000=1/2/3/FF/E0 selecciona el paquete FNT
6/3/0/9/12, respectivamente. El selector no significa FanID demostrado.

| FNT | FI para selectores 1 / 2 / 3 / FF / E0 |
|---|---|
| FNT0 | FI02 / FI01 / FI00 / FI03 / FI04 |
| FNT1 | FI07 / FI06 / FI05 / FI08 / FI09 |
| FNT2 | FI0C / FI0B / FI0A / FI0D / FI0E |
| FNT3 | FI11 / FI10 / FI0F / FI12 / FI13 |
| FNT4 | FI16 / FI15 / FI14 / FI17 / FI18 |

No hay rama para un selector desconocido: Local0/Local2 podrían quedar sin
inicializar. El parser de catálogos anterior solo contempla FI00–FI04; eso
no cubre todas las ramas de SFAN.

SFAN copia los diez words a CFST y a EC.F101–F10A (campos de 8 bits).
Después escribe en CRP0–9, GRP0–9 y ERP0–9 el mismo cálculo por punto:
`FNT_seleccionado[indice + 2] / 100`, con división entera. No se demuestran
unidades físicas ni canales independientes por estos nombres. Hay riesgo de
truncamiento y acceso fuera del paquete; ni siquiera la cota conservadora
1..10 está impuesta por AML. Esa cota evita índices ajenos al dominio conocido,
pero no demuestra seguridad térmica ni monotonía de la curva.

FI aporta tres paquetes de 22 elementos: para los grupos C/G/E se escriben
10 campos L, 10 campos T y dos campos adicionales RA8/TA9. Por tanto, la
operación también reemplaza parámetros distintos de los diez índices.
Termina con `EC.CMRD=1` (35446). Su semántica EC completa sigue desconocida.
SFAN es NotSerialized; las escrituras son secuenciales, sin transacción,
rollback ni protección visible frente a otra operación de perfiles.

## Recuperación: evidencia y límites

`SFA2` (35449–35730) selecciona catálogos, llama `INIF(Arg0)` y reescribe
grupos EC y CMRD. `INIF` (34972–35018) utiliza SFTW y genera CFST a partir
de SOU3: índices crecientes saturados al conteo; escribe F101–F10A.
No es una API de rollback al estado anterior.

La llamada SFA2 visible en 39989 depende de un cambio de categoría Local2
respecto de TPC1 y de la inicialización TPC0; no se ejecuta siempre. PWSW
(10325–10372) modifica GZ44 mediante RXMS/WXMS, llama SSFM y notifica GZFD.
Estos caminos muestran cambios de perfil, pero no una cadena garantizada
Fn+Q → restauración íntegra de todos los registros modificados por SFAN.
No se ha demostrado persistencia, recuperación tras fallo parcial, reinicio
o apagado. Descargar nuestro módulo no revierte registros EC.

## Requisitos antes de considerar una escritura

- Identificar y registrar PMID, PGID, DCID y GZ44 de forma autorizada; cubrir
  todos los catálogos FI seleccionables y su semántica física.
- Demostrar framing completo, campos ignorados y diferencias Windows/AML.
- Verificar límites por paquete, ancho de campo EC y conversiones; definir
  límites térmicos, mínimos de ventilación y política de monotonía respaldados
  por evidencia. No fabricar límites físicos desde los índices.
- Contar con supervisión independiente validada de RPM y temperaturas,
  condiciones de cancelación y exclusión de cambios concurrentes de perfil.
- Obtener y validar un estado seguro de referencia y un mecanismo de
  recuperación completo ante aplicación parcial, previamente revisado.
- Confirmación específica, registro de operación y aprobación independiente
  para firmware/modelo/BIOS y prueba concreta.

La evaluación inicial no acreditó recuperación ante fallos parciales.
El transporte de preparación añadido posteriormente permanece bloqueado;
la decisión actual y la evidencia pendiente se precisan a continuación.

## Resolución concentrada: selector, catálogos y recuperación

Esta sección actualiza las conclusiones iniciales anteriores; el transporte de
preparación ya existe, pero conserva su barrera constante `false`. Fuente:
análisis offline del DSDT local, sin ejecutar AML ni leer el host.

### Selector 1 no significa FNT1 ni Personalizado

`SFAN` (35119–35296) selecciona primero el catálogo, después el paquete:

| Condición | Catálogo | Paquete con F000=1 | Parámetros auxiliares |
|---|---|---|---|
| PMID=1, PGID=2 o 3 | FNT2 | 6 | FI0C |
| PMID=1, PGID=5 | FNT4 | 6 | FI16 |
| PMID=1, otro PGID | FNT3 | 6 | FI11 |
| PMID!=1, DCID=0 | FNT1 | 6 | FI07 |
| PMID!=1, DCID!=0 | FNT0 | 6 | FI02 |

El fragmento común es `If (F000 == One) { Local0 = DerefOf
(Local1 [0x06]); CopyObject (FIxx, Local2) }`. GZ44 no participa en esta
selección: WMAB solo excluye GZ44=7. Personalizado/GZ44=3 no transforma el
selector 1 en FF. PMID/PGID/DCID están declarados en OGNS/SystemMemory
(536–550); no son los tres registros EC que hay que leer directamente.
No se encontró asignación explícita a estos tres campos en este DSDT;
su inicialización externa y valores actuales no quedan demostrados.

Con índices 1..10, CRP/GRP/ERP reciben `Local0[indice+2]/100`:

| Catálogo, paquete 6 | Diez valores escritos a cada grupo |
|---|---|
| FNT0 | 0,17,19,21,24,29,32,35,38,50 |
| FNT1 | 0,18,21,22,26,30,33,36,42,48 |
| FNT2 | 0,14,17,19,22,25,29,35,43,58 |
| FNT3 / FNT4 | 0,14,17,19,22,25,31,35,48,48 |

Son resultados matemáticos, no nuevas mediciones RPM. FI aporta además
66 valores (tres paquetes de 22), que difieren entre FI02, FI07 y FI0C;
FI0C/FI11/FI16 coinciden en la evidencia local. No basta comparar índices.

Los paquetes 9/10/11 de FNT0 tienen Mode=255 y coordenadas (1,4), (2,5),
(1,1), con los diez niveles nominales comunicados desde Windows. Entre los
cinco catálogos examinados, solo FNT0 reproduce esa secuencia. Es evidencia
histórica fuerte de la rama FNT0 (PMID!=1 y DCID!=0), no una captura actual
de esos campos en Linux. Paquete 6 y paquetes 9..11 de FNT0 comparten niveles,
pero no todos los parámetros: SFAN con selector 1 usa FI02, no FI03.

Los diez índices oficiales caben en los paquetes y campos EC de 8 bits;
el cero resultante en el primer escalón es intencional en esa tabla y no
significa ausencia de datos. AML no impone los límites LLT, monotonía,
selector válido, tamaño completo ni ausencia de concurrencia.

### Reaplicar no es rollback

SFAN escribe 107 destinos EC distintos: 10 índices, 30 CRP/GRP/ERP,
66 parámetros FI y CMRD. Reaplicar el mismo payload completa esos mismos
destinos si termina y la selección no cambia; no restaura un estado previo
desconocido. Entre selección y finalización tampoco hay exclusión visible
del firmware: SFAN es NotSerialized.

Una comparación offline de asignaciones demuestra que SFA2+INIF cubre
esos mismos 107 destinos, no solo una parte. Sin embargo, genera otros valores:
INIF(1) llama SFTW(6), usa SOU3 y satura los índices. En FNT0[6], SOU3=5:
produce [1,2,3,4,5,5,5,5,5,5], no [1..10]. Además, en la rama Arg0=4 de
FNT0/FNT1, SFA2 selecciona paquete 12 mientras INIF selecciona 9. Cobertura
de destinos no equivale a equivalencia de configuración ni a rollback.

CMRD es un campo de un bit (13268); sus únicas referencias nominales en
este DSDT son la declaración y las dos asignaciones finales a 1. No hay
limpieza previa, espera, lectura de aceptación ni manejo de error. Si SFAN
falla antes de ese punto, quedan potencialmente escritos los destinos previos;
CMRD incluso podría estar ya a 1. No puede inferirse que el EC descarte o
aplique atómicamente una escritura parcial. Falta la implementación interna EC.

SFA2 aparece en WMAE/GetFeatureValue (39978–39989), condicionado por cambio
de categoría TPC1 tras inicializar TPC0; no es una restauración garantizada
por Fn+Q. GFAN también llama INIF(2) si F10A=0 (35049–35065): un readback
no debe utilizarse como comprobación universal sin efectos secundarios.

### Decisión y evidencia mínima pendiente

El protocolo y el preset permiten diseñar una prueba acotada; no existe aún
evidencia actual suficiente para ejecutarla. La vía más corta para resolver
el selector efectivo es capturar, bajo autorización aparte, el FACT completo
del catálogo activo mediante WQA7/SFTW, índices 6 y 9..11, y contrastarlo con
FNT0. WQA7 (38346–38348) solo llama SFTW, que construye FACT sin escrituras
EC visibles; sigue siendo una invocación activa que requiere revisión y permiso.
No hace falta acceso directo a OGNS/EC si ese testigo identifica la rama;
no demuestra que el firmware no la cambie concurrentemente.

Justo antes de una eventual prueba: identidad/BIOS exactos, AC estable,
Personalizado/GZ44=3, FanFullSpeed=0 reciente, catálogo acreditado, ausencia
de cambios concurrentes, dos RPM fiables y supervisión térmica independiente
con límites y actuación de aborto aprobados. Ninguna observación histórica
sustituye esos estados. Falta completar esa comprobación y el protocolo térmico;
no se exige demostrar un rollback imposible desde este AML.

La eventual prueba sería una sola aplicación del preset, sin reintentos,
cargas forzadas ni cambio automático de modo; abortar antes de invocar ante
datos desconocidos o supervisión fallida. Tras un fallo ambiguo no repetir.
Integer(0), GFAN coincidente y RPM normales no prueban todos los destinos.
Ante anomalía térmica, detener la actividad y aplicar el protocolo de apagado
aprobado, no intentar arreglos EC. Windows/LLT disponible, Personalizado,
FanFullSpeed desactivado y Predefinido → Guardar es una vía operacional ya
probada dos veces según el propietario, pero no garantiza recuperación de
una ejecución parcial, fallo EC o equipo que ya no arranque. Fn+Q, reinicio
y descarga del módulo tampoco la garantizan. Una primera escritura requerirá
aceptación explícita de ese riesgo residual y autorización específica.
