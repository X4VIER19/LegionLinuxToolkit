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

Bloqueo actual: no existe recuperación garantizada ni semántica térmica
suficiente. Ninguna validación offline autoriza una primera escritura.
Fan_Set_Table, transporte de escritura y controles CLI permanecen ausentes.
