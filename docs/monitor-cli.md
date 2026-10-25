# Monitor periódico, solo lectura — Nobara 83EX

```bash
# Intervalo por defecto: 2 segundos; Ctrl+C termina.
cargo run --locked --offline -p legion-cli --bin legion-cli -- monitor

# Diez intentos de adquisición, cada tres segundos, sin sudo.
cargo run --locked --offline -p legion-cli --bin legion-cli -- monitor --interval 3 --samples 10
```

Reutiliza StatusReader: comprueba DMI una vez al abrir la sesión y adquiere
Tctl, NVIDIA, AC, perfil y entradas hwmon nuevas en cada muestra. No duplica
lectores ni usa valores anteriores para suplir errores. `status` y su JSON
conservan la consulta puntual. Intervalo admitido 1..3600 segundos (decimales
permitidos); N debe ser positivo. Una adquisición lenta retrasa el siguiente
muestreo, sin ráfagas para recuperar tiempo ni reintento de una muestra fallida.

La salida es una línea por intento, con CPU/GPU, AC/perfil y cada entrada RPM
con fuente y validación. Los errores se muestran, cuentan dentro de N y no
se sustituyen por cero. Sin archivos de registro, modo crudo, servicios ni
procesos residentes. Ctrl+C usa el SIGINT normal del grupo de terminal
(salida habitual 130); cierra el monitor y alcanza los procesos de consulta
de ese grupo. No requiere dependencias de señales ni manejadores unsafe.
Esto no garantiza cancelar una llamada de kernel no interrumpible; tampoco
hay operaciones de control o módulos que restaurar. No ejecutar como root.

`modinfo -k 7.2.11-200.nobara.fc44.x86_64 fan` identifica `ACPI Fan Driver`,
`drivers/acpi/fan`, integrado/builtin. El Makefile de las cabeceras instaladas
asocia CONFIG_ACPI_FAN a fan.o y CONFIG_HWMON a fan_hwmon.o. Por ello SOLO la
pareja hwmon acpi_fan + módulo fan se etiqueta StandardAcpiFan, no Unexpected.
No promociona GenericUnvalidated a RPM validadas ni demuestra dos ventiladores.
fan bajo un hwmon yogafan u otro módulo inesperado conserva la advertencia.

El yogafan original continúa no fiable; 0 RPM no significa detención física.
No se asignan los canales a CPU/GPU ni se duplica acpi_fan para inventar un
segundo ventilador. Sin carga/descarga de módulos, perfiles ni EC/WMI de control.
Las tres barreras de escritura siguen desactivadas. Desarrollo y pruebas se
realizaron con datos simulados; no se ejecutó monitor sobre los sensores reales.
