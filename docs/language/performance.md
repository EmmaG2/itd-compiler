# Medición de la VM

Ejecutar desde la raíz:

```sh
make benchmark
```

El benchmark usa la biblioteca estándar: tres calentamientos y la mediana de 21 repeticiones. Separa análisis, compilación y ejecución; ambos motores reciben el mismo programa analizado, con límites predeterminados y sin salida en el ciclo medido. Comprueba que ambos finalizan sin diagnósticos y con la misma salida. La VM incluye su validación de bytecode en el tiempo de ejecución.

Medición local del 5 de septiembre de 2026, Apple M1, Darwin 25.5.0, perfil release. Tiempos en microsegundos:

| Caso       | Análisis | Compilación | Intérprete |   VM | Mejora |
| ---------- | -------: | ----------: | ---------: | ---: | -----: |
| Aritmética |       15 |           3 |       4625 | 2604 |  1.78× |
| Llamadas   |        5 |           1 |       1861 |  929 |  2.00× |
| Recursión  |        6 |           1 |       1760 |  754 |  2.33× |
| Decimales  |        4 |           1 |       1943 |  985 |  1.97× |
| Métodos    |        9 |           3 |       2477 | 1315 |  1.88× |

Estos resultados describen los cinco programas del benchmark, no una garantía para cualquier programa o equipo. No se usan umbrales temporales en CI. No se añadieron cachés, JIT ni transformaciones que reordenen aritmética o efectos.
