# Integración experimental de lectura real en CLI (Fase 5C.5)

## Alcance

La Fase 5C.5 prepara el binario independiente
`legion-cli-platform-profile-real-read`. No modifica el comando normal
`legion-cli platform-profile`, que continúa usando exclusivamente escenarios
embebidos y `StaticSimulation`.

El binario nuevo se compila solo con la feature no predeterminada
`legion-cli/approved-real-platform-profile-read`, que reenvía de forma
explícita la feature del proveedor existente en `legion-linux`. No incorpora
un lector, rutas, apertura, fallback ni lógica de coherencia nuevos: llama a
`approved_one_shot_platform_profile_read()` y, por tanto, reutiliza el
proveedor privado, `openat2`, el límite de 4096 bytes y la secuencia
`choices → active → choices` ya revisados.

## Condiciones de activación

Una futura ejecución requeriría simultáneamente:

1. compilar el binario con la feature experimental no predeterminada;
2. invocarlo explícitamente, nunca desde `legion-cli` normal ni al inicio;
3. proporcionar el literal `I-UNDERSTAND-READ-ONLY-SYSFS` mediante
   `--confirm`;
4. proporcionar el único selector cerrado
   `--compatibility 83ex-nucn15ww-static`;
5. obtener éxito en la puerta de compatibilidad estática antes de llegar a la
   barrera de identidad; en esta fase esa barrera siempre bloquea antes de la
   única llamada de lectura compartida.

El selector representa datos embebidos del registro para 83EX/`NUCN15WW`; no
consulta DMI, sysfs, BIOS ni otra identidad del equipo y el resultado incluye
`host_identity_verified: false`. Sirve para fallar cerrado ante datos ausentes,
desconocidos, ambiguos o malformados en la integración, no como prueba de que
el binario se ejecuta sobre el host correcto.

No acepta raíces, rutas, variables de entorno, configuración, monitorización,
reintentos, fallback ni opciones de escritura. `StaticCompatibilityGate` no
tiene constructor público y su único origen actual fija
`host_identity_verified: false`; `require_verified_host_identity()` detiene la
ejecución antes de invocar el proveedor. No existe una opción CLI que pueda
omitir o cambiar esa barrera. Una futura fase, aprobada por separado, deberá
incorporar un verificador de identidad real y revisado antes de que pueda
producirse una lectura. Si se llegara a producir una lectura en tal fase, la
salida solo se aceptará como real cuando el resultado contenga
`LinuxReadOnly`; una simulación se rechaza en lugar de relabelarse.

## Estado de autorización

Esta fase implementa y prueba únicamente puertas estáticas y datos simulados.
No autoriza ejecutar el binario, leer `/sys`, repetir el hito anterior ni
considerar permanente la feature. Una ejecución futura requerirá revisión
separada del diff, del comando literal y de las rutas autorizadas conforme a
`AGENTS.md`.
