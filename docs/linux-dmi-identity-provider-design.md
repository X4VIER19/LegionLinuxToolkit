# Proveedor experimental DMI de identidad (Fase 5C.6C)

Esta fase prepara, sin activar, un proveedor privado de identidad DMI dentro de
`legion-linux`. Se compila únicamente con la feature no predeterminada
`experimental-dmi-identity-read` o para pruebas aisladas. No tiene constructor
para el host, binario, ruta configurable, variables de entorno ni función de
activación.

## Diseño de adquisición

El proveedor trabaja con un descriptor de directorio ya abierto y consulta solo
cuatro nombres constantes mediante `openat2`:

- `sys_vendor` para fabricante;
- `product_version` como identificador de producto candidato;
- `product_name` como modelo candidato;
- `bios_version` como BIOS.

La asignación de `product_version` y `product_name` es una hipótesis de diseño,
no una afirmación validada para el 83EX. Kernel y distribución no se adquieren
en esta fase y permanecen ausentes para la evaluación posterior.

Cada apertura usa solo lectura (`O_RDONLY`, `O_CLOEXEC`, `O_NOFOLLOW`,
`O_NONBLOCK`) y las restricciones `RESOLVE_BENEATH`,
`RESOLVE_NO_MAGICLINKS` y `RESOLVE_NO_SYMLINKS`. Se exige un archivo regular,
se limita cada atributo a 1024 bytes y se falla de forma cerrada ante enlaces,
EOF anómalo, UTF-8 inválido, interrupciones o errores de `openat2`.

Los árboles temporales de prueba se etiquetan exclusivamente como
`StaticSimulation`. La fuente pasa por `LinuxHostIdentityProvider` y
`HostIdentity::declared_simulation`; ninguna lectura —incluida una futura—
produce automáticamente una identidad verificada ni una `RealReadAuthorization`.

## Barrera de activación

`ApprovedDmiIdentityReadActivation` está intencionadamente inhabitable. Habilitar
la feature no permite abrir DMI real: no existe constructor de raíz real ni
punto de entrada. Cualquier activación futura requerirá un cambio explícito,
una revisión de seguridad y autorización independiente. Esta fase no modifica
la barrera ni el binario de `platform_profile`.
