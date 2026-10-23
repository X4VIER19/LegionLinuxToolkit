# Validación pasiva de independencia de canales hwmon

## Hechos observados

En el Lenovo Legion Slim 5 16ARP9 (`83EX`), BIOS `NUCN15WW`, se observaron
dos canales del quirk temporal `yogafan_83ex`: `1900`/`1900` RPM en una
consulta y `2100`/`2100` RPM con el perfil Personalizado ya seleccionado por
el usuario. `acpi_fan` también produjo lecturas coherentes. Son observaciones
puntuales, no una demostración de que los ventiladores sean independientes ni
una asignación de canal a CPU o GPU.

El `yogafan` original sigue siendo incompatible con el layout 83EX conocido:
su nombre hwmon no valida sus valores. La lectura Rust clasifica el quirk solo
si concurren DMI exacto y el enlace sysfs `device/driver/module` termina en
`yogafan_83ex`; esa comprobación identifica el módulo cargado, no certifica
criptográficamente su contenido.

## Comprobación futura propuesta

Requiere autorización separada para cada captura de solo lectura. No debe
cambiar perfiles, curvas, PWM, cargas de trabajo ni módulos durante la
comprobación.

1. Registrar una captura única de ambos `fanN_input`, su procedencia de módulo
   y las temperaturas estándar disponibles en ese instante.
2. Repetir únicamente en sesiones normales con cambios naturales de carga o
   temperatura, sin estrés inducido ni sondeo automático.
3. Conservar hora, perfil ya activo, fuentes y valores originales. No asignar
   CPU/GPU por orden de canal.
4. Considerar evidencia de independencia solo si, bajo condiciones registradas,
   un canal cambia de forma reproducible sin que el otro cambie en la misma
   dirección o magnitud. Valores iguales —incluidos 1900/1900 y 2100/2100— no
   prueban acoplamiento ni independencia.
5. Detener la interpretación si falta procedencia del módulo, DMI no coincide,
   aparece un valor inválido/cero no validado, o las fuentes cambian entre
   capturas.

La comprobación no autoriza operaciones de escritura ni invocaciones WMI,
ACPI o EC.
