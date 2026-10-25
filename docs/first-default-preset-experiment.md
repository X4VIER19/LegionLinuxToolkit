# Primera aplicación experimental del preset [1..10]

83EX/NUCN15WW, Nobara 7.2.11; artefactos del commit
`099421f2ce138e354f711f92fe23f17219ff57a0`. Análisis exclusivamente offline.
Registro privado original: `trial-logs/supervision-1792916982824677489-61530.jsonl`.
El propietario facilitó una copia privada para lectura, sin cambiar el original.
SHA-256 de la copia analizada:
`24da93ff15cb28f8dec2c3740683147271b4fa5750df02d317b98ee4c3e2c421`.
Hashes de artefactos activados de referencia (ahora retirados/recompilados):
monitor `56ab69f2994032840a9cd6e986fa6b477a869d46a2f94cc8d2d6863682262a12`;
helper `1febdb209599ab6d6b1559977d7e6153daf91ef29e8a475c0b740ab9709d2697`.
Los JSONL completos permanecen excluidos de Git; no publicar sus metadatos crudos.

## Resultado observado

368 eventos válidos, 181 muestras (61 baseline y 120 POST), una intención de
despacho, un marcador y un resultado del endpoint. Hora local America/Mexico_City,
25 de octubre de 2026, UTC-06:00:

| Evento/muestra | Hora | CPU °C | GPU °C | RPM1/RPM2 reportadas |
|---|---|---:|---:|---:|
| Última muestra previa, 60 | 02:30:43.104 | 37.75 | 36 | 1500/1500 |
| Intención de despacho | 02:30:43.122 | — | — | — |
| Marcador de tentativa | 02:30:43.123 | — | — | — |
| Primera muestra POST, 61 | 02:30:44.250 | 39.25 | 36 | 0/0 |
| Resultado registrado, muestra 61 | 02:30:44.280 | — | — | `Ok(())` |
| Muestra 90 | 02:31:13.260 | 36.125 | 35 | 0/0 |
| Muestra 120 | 02:31:43.260 | 37.5 | 34 | 0/0 |
| Muestra 180 | 02:32:43.260 | 41.375 | 36 | 0/0 |

La transición observada queda entre las muestras 60 y 61, separadas por
1.146 s. No conocemos el instante físico de transición ni el instante exacto
de retorno del firmware: el resultado asíncrono se recogió en la muestra 61.
Ambos canales conservaron la lectura cero en las 120 muestras POST. CPU POST:
35.5–43.875 °C; GPU POST: 34–36 °C. AC fue true y el perfil custom en todas
las muestras; en frontera FanFullSpeed devolvió Integer(0), disabled, sin error.

El retorno del endpoint fue `{"Ok":null}`: la ruta kernel validó el retorno
Integer(0), no el estado final del EC. El registro termina a las 02:32:43.274
con `POST window finished with anomalies; no rollback or acceptance established`.
Cada muestra POST incumplió RPM<1000 y caída>50% en ambos canales (240 motivos
de cada clase). No se alcanzaron los cortes térmicos POST. Se completó la
observación temporal, pero NO fue un ensayo aceptado por la política vigente;
esa política no se cambia para acomodar el resultado.

## Observación posterior y límites

El propietario reportó limpieza sin módulos experimentales restantes. También
reportó posteriormente acpi_fan=1500 RPM, CPU=45.5 °C, GPU=40 °C y ventilación
audible. Estos datos NO están en este JSONL y no se volvieron a consultar.

El resultado es compatible con Fan Stop, pero las lecturas 0/0 no demuestran
detención física. Tampoco descartan una limitación de telemetría. El registro
acredita el recorrido del llamador y la respuesta observada, no la aplicación
íntegra de los 107 destinos EC, la selección efectiva de FNT0 durante SFAN,
atomicidad, restauración completa, rollback ni seguridad de otra curva.
El retorno posterior de RPM no equivale a recuperación garantizada.

## Cierre y siguiente funcionalidad offline

Se restituyen exactamente shell=0, Rust=false y kernel=false. Se limpian las
salidas Cargo de legion-cli/legion-linux y las salidas Kbuild de este módulo,
reconstruyéndolas bloqueadas en las rutas del wrapper. Se conservan el registro,
su copia privada, los hashes activados documentados y el commit fuente; no se
mantienen los ejecutables activados como artefactos operativos conocidos.
El modo sin escritura y la política térmica no se alteran.

`OfflineCurveProposal` ya valida diez índices 1..10, mínimos LLT V2
[1,1,1,1,1,1,1,1,3,5] y monotonía no decreciente, y serializa 64 bytes con
ceros reservados. Son reglas del editor LLT, no validaciones garantizadas del EC.
Se añade `preview_fnt0(&catalogues)` sobre catálogos guardados: paquete 9,
coordenadas históricas (1,4), offset índice+2; reutiliza el mapeador existente.
Con la referencia nominal [0,1700,1900,2100,2400,2900,3200,3500,3800,5000],
la propuesta offline [2,2,3,4,5,6,7,8,9,10] previsualiza
[1700,1700,1900,2100,2400,2900,3200,3500,3800,5000]. No es una recomendación
de aplicación ni un objetivo térmico validado. Se preservan los literales,
la escala matemática y la interpretación física desconocida del modelo AML;
no son RPM medidas. No se añaden transportes ni endpoints para curvas arbitrarias.
