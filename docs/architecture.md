# Arquitectura modular propuesta

## Meta de diseño

La aplicación debe poder servir a Nobara, CachyOS y Linux Mint sin asumir que
una interfaz concreta exista o sea segura en todos los modelos. La unidad de
compatibilidad es el conjunto **modelo + identificador DMI + BIOS + interfaz
WMI + operación + versión del kernel cuando aplique + evidencia validada**. No
es la familia Legion ni un GUID WMI aislado.

## Capas

1. **Identidad y política.** Recoge DMI, versión de BIOS, kernel y distribución;
   carga una política de compatibilidad local. Decide qué adaptadores pueden
   observarse. No ejecuta WMI, ACPI ni EC, y no escribe nada.
2. **Descubrimiento pasivo.** Inspecciona rutas sysfs/hwmon y presencia de
   dispositivos WMI, devolviendo `available`, `unavailable` o `unknown` con
   evidencia. La detección no llama métodos WMI.
3. **Adaptadores de lectura.** Implementaciones pequeñas y aisladas para
   `platform_profile`, firmware attributes, hwmon y, más adelante, una consulta
   WMI explícitamente iniciada por el usuario y autorizada para el equipo real.
   Cada adaptador valida formato, límites y unidades antes de publicar datos.
4. **Modelo de dominio.** Expone instantáneas inmutables: `DeviceIdentity`,
   `EnergyProfiles`, `TelemetrySnapshot`, `TelemetryValue`, `FanTelemetry` y
   `Capability`. Cada valor conserva origen, unidad, clasificación
   (instantáneo, límite configurado o nominal), estado y confianza; evita
   presentar límites térmicos como temperaturas en tiempo real o límites de
   potencia como consumo.
5. **Presentación.** Un CLI inicialmente; una GUI futura consume la misma API.
   Debe mostrar claramente datos no disponibles, no validados y potencialmente
   obsoletos.

## Control de capacidades

Las capacidades se expresan de forma granular: por ejemplo,
`profile.read`, `power_limits.read`, `fan.rpm.read` y `fan.table.read`.
`fan.table.write` y las demás capacidades de mutación no se implementan en el
primer prototipo. Una capacidad solo pasa de `unknown` a `validated` con una
evidencia documentada para la combinación completa de compatibilidad. Un GUID
compartido no habilita ninguna operación.

## Seguridad y extensibilidad

Los adaptadores de mutación futuros deben vivir en un paquete separado, exigir
una confirmación interactiva específica y registrar una transacción auditable.
No se habilitan simplemente porque exista el método de firmware. Esta división
evita que una herramienta de inventario alcance accidentalmente una ruta de
escritura.

Antes de considerar una escritura, el diseño debe exigir: validación completa
de modelo/BIOS/capacidad; límites de valor y tamaño de buffer; confirmación
específica; registro detallado de solicitud y resultado; supervisión de
temperaturas y funcionamiento; y un procedimiento documentado de interrupción
y recuperación. Estos requisitos no autorizan ni implementan una operación de
escritura.

## Pruebas aisladas

La primera aplicación usa exclusivamente una raíz de datos simulados como
entrada de pruebas. Los adaptadores de producción se inyectan explícitamente y
están ausentes en la configuración de test; los tests deben rechazar las rutas
reales `/sys`, `/proc` y `/dev`, sin recurrir al hardware como mecanismo de
respaldo. CI solo usa fixtures versionadas, no ejecuta pruebas con `sudo` y debe
fallar si una prueba intenta crear un transporte WMI/ACPI/EC real. Las respuestas
WMI conocidas se representan como buffers de fixture, no como llamadas al
firmware.

## Decisiones iniciales

- Preferir sysfs y hwmon del kernel sobre duplicar protocolos Lenovo.
- No desarrollar un controlador propio hasta demostrar, para una capacidad
  concreta, que las interfaces y controladores existentes del kernel no bastan.
- Mantener WMI como adaptador opcional, con invocación manual y separado de la
  detección.
- Tratar el valor 0 RPM como una observación, no como una conclusión.
- No depender de módulos externos para el primer prototipo.
