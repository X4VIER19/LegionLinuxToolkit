# Editor CLI offline — 83EX / GodMode V2

Sin sudo ni features experimentales. No carga módulos, consulta sensores ni
envía WMI/EC. Conserva las tres barreras de escritura. Las curvas no autorizan
aplicación real ni sustituyen el protocolo térmico.

```bash
# Crear y previsualizar; --save crea un archivo NUEVO, no sobrescribe.
cargo run --locked --offline -p legion-cli --bin legion-cli -- curve \
  --indices 2,2,3,4,5,6,7,8,9,10 --save mi-curva.json --payload

# Abrir con validación completa y previsualizar el ejemplo distribuido.
cargo run --locked --offline -p legion-cli --bin legion-cli -- curve \
  --open examples/curves/83ex-default-v1.json --payload

# Reporte JSON de la previsualización (distinto del documento guardado).
cargo run --locked --offline -p legion-cli --bin legion-cli -- curve \
  --open mi-curva.json --format json
```

Documento versionado: schema_version=1, hardware=83EX/NUCN15WW,
rules=LLT-GodMode-V2 e indices con diez UInt16. Se rechazan campos extra,
versiones/equipos/reglas desconocidos, valores fuera de 1..10, descensos y
valores inferiores a [1,1,1,1,1,1,1,1,3,5]. Se reutiliza OfflineCurveProposal;
las reglas son del editor LLT, no una garantía de validación o seguridad EC.

`--payload` muestra únicamente los 64 bytes SIMULADOS del serializador.
El reporte JSON lleva source=offline-curve-simulation y execution_enabled=false.
Importación máxima 4096 bytes; solo archivos regulares, sin enlaces simbólicos,
dispositivos ni /sys, /proc o /dev. Guardado exclusivo, sin sobrescritura ni
seguimiento de enlaces; sin fallback si openat2 no está disponible.

Datos incorporados: transcripción decimal de FNT0 paquetes 9/10/11 y FI02
de la DSDT estática 83EX/NUCN15WW (SHA-256
`f7bf55d2af4d54292d797603b023e66bbe4a16a0652c540fc91e001d9a279c1b`).
Se comprobó la igualdad de todos los valores incorporados con esa evidencia.
La DSDT completa no se incluye. FACT y Windows corroboran los niveles nominales;
ninguno identifica automáticamente el estado actualmente aplicado al EC.

La tabla relaciona cada índice elegido con RPM NOMINALES FNT0, no mediciones.
S4/S5 son los campos de eje sensor de los paquetes (1,4)/(2,5). CPU CL/CT y
GPU GL/GT son los valores crudos FI02 escritos por AML en esos campos; no se
les atribuye calibración térmica ni mapeo físico de canales. Los umbrales son
por posición del punto y NO se desplazan al cambiar su índice de RPM.
