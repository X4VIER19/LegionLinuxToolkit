# Una captura FACT: preparación, no autorización de ejecución

Objetivo exclusivo: 83EX/NUCN15WW, Nobara 7.2.11-200.nobara.fc44.x86_64.
No se ha ejecutado esta captura. No se habilita Fan_Set_Table.

## Contrato confirmado en el DSDT local

- Las nueve variantes WDG contienen el bloque A7: GUID
  `87FB2A6D-D802-48E7-9208-4576C5F5C8D8`, 15 instancias, flags=1
  (data/expensive, no método). Ejemplo: líneas 37409–37411.
- WQA7 (38346–38348) llama `SFTW(Arg0)`: Arg0 es el índice de paquete,
  no MethodId ni un búfer de entrada. Se consultan exactamente 6,9,10,11.
- SFTW (34877–34948) selecciona FNT por PMID/PGID/DCID con las mismas
  ramas que SFAN; copia los campos a FACT y retorna Buffer(0x48).
- FACT (29877–29915), little-endian, conserva estos 72 bytes:

| Offset | Tipo/contenido |
|---|---|
| 0 / 2 / 4 | UInt16 FNIM / UInt16 FNID / UInt32 FNLE |
| 8–27 | Diez UInt16 FNS0–9 |
| 28 / 32 | UInt32 SEID / UInt32 STLE |
| 36–55 | Diez UInt16 SST0–9 |
| 56 / 57 / 58 | Byte SOU1 / Byte SOU2 / UInt16 CFMS |
| 60 / 61 / 62 | Byte SOU3 / Byte SOU4 / UInt16 CFIS |
| 64 / 66 / 68 / 70 | UInt16 FSSP / MST1 / MST2 / MSTP |

SFTW modifica el búfer AML compartido FACT, no los registros EC en su cuerpo
visible; no llama SFAN/INIF ni cambia GZ44. Es NotSerialized: otras consultas
pueden competir por ese búfer. No se afirma ausencia universal de efectos
secundarios fuera del código recuperado.

## Acceso y protecciones

Se reutiliza el módulo experimental, sin registrar ni apropiarse de ningún
GUID en la configuración FACT. `wmi_query_block()` está declarada en los
headers Nobara y exportada GPL en Module.symvers. No se utiliza
`wmi_evaluate_method()` ni se invoca MethodId 5/6/17.

La API GUID de consulta está deprecated. El código público
[WMI core upstream](https://github.com/torvalds/linux/blob/v6.19/drivers/platform/wmi/core.c)
se inspeccionó como referencia, no como fuente exacta certificada de Nobara:
puede activar/desactivar WC para bloques expensive. No hay WCA7 en el DSDT
local; el endpoint además busca `\\_SB.GZFD.WCA7` sin evaluarlo y aborta si
existe o si la búsqueda falla de otro modo que AE_NOT_FOUND. Antes de ejecutar
se exige el mismo firmware y una única instancia de dispositivo GUID A7,
con object_id=A7 y 15 instancias de datos. No hay seguimiento a otro GUID,
consulta ACPI alternativa ni fallback. La API mantiene referencias de dispositivo;
el mutex propio no excluye al controlador nativo ni otros consumidores.

Solo `CONFIG_LEGION_LINUX_EXPERIMENTAL_FACT_READ=y` compila el endpoint real;
el Makefile rechaza otras opciones de firmware simultáneas y valores distintos
de unset/y. Confirmación literal en módulo y herramienta; DMI/BIOS exactos en
kernel; guardia atómica consumida antes de adquisición, una tentativa por carga.
No hay invocaciones durante init/probe/remove. El endpoint raíz 0400 solo tiene
read y no admite argumentos, escritura, ioctl ni seek.

Un read devuelve 316 bytes: cabecera LE de 12 bytes (versión=1, cabecera=12,
estado, tamaño completado), más cuatro registros de 76 bytes (índice UInt16,
reservado UInt16=0, FACT original de 72 bytes). Éxito exige estado=0 y tamaño=304.
Se valida tipo ACPI Buffer, longitud=72 y ambos conteos=10 antes de copiar.
Error de transporte=2, tipo ACPI=3, longitud/conteos=4; detenerse al primer error.
Los registros completados quedan en la captura, pero nunca forman éxito parcial.
Se libera el objeto ACPI incluso ante error. La herramienta hace un único read
syscall de 316 bytes, sin completar lecturas cortas ni repetir EINTR.

## Procedimiento futuro exacto (requiere aprobación independiente)

No ejecutarlo todavía. Sin cambios de perfil ni otras herramientas de consulta
concurrentes, desde una terminal como usuario normal:

```bash
bash /home/x4vier19/Documentos/GitHub/LegionLinuxToolkit/scripts/one-shot-fact-capture.sh \
  --confirm I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE
```

El script verifica kernel/cabeceras/DMI/metadatos A7, compila temporalmente
solo FACT, construye la herramienta y pide sudo interactivo. Carga una vez,
captura las cuatro instancias y descarga inmediatamente por trap, incluso ante
fallo o insmod fallido que haya dejado el módulo cargado. No toca yogafan ni
lenovo_wmi_other. Conserva el archivo `/tmp/legion-fact-capture-XXXXXX.bin`,
también ante error. No hay reintentos. Borra solo su directorio privado de
compilación después de retirar los recursos; ante fallo de descarga conserva
el artefacto y muestra error, sin forzar rmmod. SIGKILL, apagado o bloqueo del
kernel impiden garantizar la limpieza; nunca instala persistentemente.

Analizar después, sin sudo ni firmware, sustituyendo el nombre del archivo:

```bash
/home/x4vier19/.cargo/bin/cargo run --locked --offline -p legion-linux \
  --bin legion-linux-fact -- analyze /tmp/legion-fact-capture-XXXXXX.bin \
  /home/x4vier19/Documentos/GitHub/LegionLinuxToolkit/docs/firmware-evidence/gmzn-16arp9/DSDT.dsl \
  --format json
```

`--format text` también conserva los bytes originales. El análisis rechaza
enlaces/archivos especiales/rutas de interfaces reales y limita capture a 316
bytes y DSDT a 8 MiB. No necesita la feature real. Un fallo de captura puede
analizarse como error; no se transforma en catálogo válido ni se recaptura.

## Qué podremos concluir

Comparar todos los 72 bytes de cada paquete contra los cinco FNT del DSDT,
no solo RPM: informar coincidencia única, ninguna o ambigua. Comparación
independiente con la observación histórica Windows: paquetes 9/10/11 Mode=255,
coordenadas (1,4)/(2,5)/(1,1), niveles nominales
[0,1700,1900,2100,2400,2900,3200,3500,3800,5000]. No inferir nuevos datos
Windows ni temperaturas/RPM actuales desde esta publicación.

Si los cuatro paquetes coinciden únicamente con FNT0, será evidencia actual
de que SFTW publicó esa rama durante las consultas, consistente con la ruta
de selección SFAN. Bajo el modelo del DSDT implica la condición PMID!=1 y
DCID!=0, pero no revela sus valores exactos ni PGID. Las cuatro consultas
no son atómicas y FACT es compartido; tampoco acreditan la configuración EC
ya aplicada, FI02 leído directamente o la selección en una futura escritura.
No otorga RealReadAuthorization ni retira la barrera constante false.

Abortar antes de consultar por kernel/identidad/metadatos distintos, módulo
previo, endpoint inesperado, confirmación ausente, WCA7 visible o error de
apertura. Tras error WMI/tipo/tamaño/copia, conservar evidencia y descargar;
no reintentar. Si los datos no concuerdan, no avanzar a Fan_Set_Table.
