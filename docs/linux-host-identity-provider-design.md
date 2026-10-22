# Proveedor Linux de identidad: simulación (Fase 5C.6B)

`legion-linux` incorpora una capa de adquisición inyectable para los seis
campos de identidad: fabricante, identificador de producto, modelo, BIOS,
kernel y distribución. Esta etapa no implementa rutas Linux ni inspecciona el
host.

## Separación de responsabilidades

- `HostIdentitySource` adquiere un `HostIdentityInput` inyectado y puede
  devolver un error de origen explícito.
- `LinuxHostIdentityProvider::read_once()` valida ese contenido mediante
  `HostIdentity::declared_simulation`.
- `HostCompatibilityEvaluator` de `legion-core` conserva la evaluación contra
  el único `CompatibilityRegistry` existente.

El único origen público es `SimulatedLinuxHostIdentitySource`. El trait está
sellado, no acepta rutas, configuración ni variables de entorno, y no puede
ser implementado desde otro crate. Todo resultado exitoso mantiene
`StaticSimulation`; no existe API para crear una identidad observada,
verificada o una `RealReadAuthorization`.

## Evolución futura

Una fuente Linux real necesitaría aprobación independiente y una revisión de
su acceso a DMI, kernel y distribución. Deberá permanecer dentro de
`legion-linux`, conservar errores de adquisición separados de validación y
exponer una procedencia nueva solo después de validar la lectura. Esta fase no
cambia ni desbloquea el proveedor experimental de `platform_profile`.
