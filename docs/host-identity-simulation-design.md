# Detector de compatibilidad de identidad: simulación (Fase 5C.6A)

## Alcance

Esta fase incorpora modelos puros de Rust para comparar una identidad inyectada
con el registro declarativo ya existente. No descubre el equipo local, no abre
archivos, no invoca comandos y no modifica ni autoriza el uso de hardware.

`HostIdentity` contiene fabricante, identificador de producto, modelo, BIOS,
kernel y distribución. Su único constructor público en esta fase es
`declared_simulation`; por tanto, cada instancia tiene procedencia
`static-simulation`. La variante `HostObservationUnavailable` deja constancia
de que la observación real es un concepto futuro, no una capacidad disponible.

## Evaluación

`HostCompatibilityEvaluator` reutiliza `CompatibilityRegistry` y su coincidencia
estricta de fabricante, modelo, producto y BIOS. La evidencia de kernel y
distribución se asocia, como datos estáticos, al identificador de un perfil del
registro: no es un segundo catálogo de modelos.

Los resultados distinguen:

- `compatible`: todos los valores inyectados coinciden con el perfil y su
  evidencia ambiental;
- `incompatible`: el fabricante no coincide con ningún perfil estático;
- `unknown`: no hay perfil estático aplicable, la coincidencia es ambigua o el
  kernel/distribución inyectado no está cubierto por la evidencia;
- `insufficient-evidence`: falta un campo obligatorio;
- `contradictory`: algunos campos apuntan al perfil conocido y otros lo
  contradicen.

Una coincidencia `compatible` lleva el marcador único
`unverified-static-simulation`; no existe un booleano público que un consumidor
pueda cambiar a `true`. No sustituye ninguna futura lectura DMI ni desbloquea
el proveedor sysfs.
Que falte kernel o distribución es `insufficient-evidence`; que difiera no
demuestra incompatibilidad del equipo, por lo que permanece `unknown`.

## Barreras de seguridad

Los campos de `HostIdentity` son privados y se validan al construir una
simulación: se rechazan cadenas vacías y caracteres de control. `None` conserva
la ausencia original; no se convierte en una cadena vacía ni en un valor por
defecto. `RealReadAuthorization` no expone constructor, contiene un campo
privado y el evaluador nunca lo devuelve. En Rust seguro, código externo no
puede fabricarlo.

La fase no cambia `StaticCompatibilityGate`,
`ApprovedRealReadActivation`, la feature experimental ni los binarios. La CLI
tampoco consume todavía este evaluador. Una fase posterior deberá añadir una
fuente de identidad observada, validarla por separado y revisar de nuevo toda
conexión con una operación real.

## Limitaciones

Los valores de kernel y distribución son observaciones documentadas para Nobara,
no detección actual ni evidencia de compatibilidad en otras distribuciones,
versiones de BIOS o kernels. Tampoco hay identificador DMI documentado. La
compatibilidad declarativa permanece descriptiva y no habilita controles.
