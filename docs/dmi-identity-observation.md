# Observación DMI real: 83EX / NUCN15WW

Una única lectura real autorizada mediante el proveedor experimental produjo:

- `sys_vendor`: `LENOVO`;
- `product_name`: `83EX`;
- `product_version`: `Legion Slim 5 16ARP9`;
- `bios_version`: `NUCN15WW`.

Los bytes originales conservaron un salto de línea final en cada atributo. La
procedencia de esa lectura fue `LinuxDmiObservedUnverified`: observar DMI no
verifica por sí solo la identidad física ni habilita `RealReadAuthorization`.

Por ello el proveedor asigna `product_name` al identificador de producto y
`product_version` al modelo para este equipo documentado. Es una observación
del 83EX con `NUCN15WW`, no una regla extrapolable a otros modelos o BIOS.
