# Verificación de la interfaz

`make test-ui` valida adaptadores IPC, ubicaciones, layout, etiquetas semánticas, reproducción de la pila de atributos y el catálogo compartido. `make compile` comprueba los tipos de la aplicación y genera el frontend.

`browser-smoke.mjs` exporta una función que recibe una página Playwright. Inicie el frontend con `nix-shell --run 'make web'` y utilice la URL que Vite imprima en la terminal: el puerto puede cambiar si está ocupado. No hay un objetivo Make para ejecutar este smoke ni un runner autónomo incluido. La función está preparada para un runner Playwright existente:

```js
import smoke from './browser-smoke.mjs'
await smoke(page)
```

El smoke incluye controles de la pila semántica, navegación al código, reinicio, limpieza al editar y alineación de pestañas en escritorio/móvil. Los estados sin traza y truncados usan fixtures, no el compilador real.

La prueba usa IPC simulado para aislar la interfaz: verifica las dos vistas del árbol, metadatos semánticos, resultados parciales/no disponibles, limpieza de ambos árboles al editar, la carga del editor, conservación de cambios, respuesta obsoleta, navegación desde bytecode, controles oscuros, modal de pantalla completa, Escape, restauración del foco y viewport móvil. Las pruebas Rust verifican por separado la ejecución real y los DTO de Tauri. No se presenta la simulación como prueba de ejecución nativa.

`csstype` está fijado en 3.1.3 para los tipos generados de `bits-ui` 2.19. `skipLibCheck` evita evaluar las uniones excesivas de los `.d.ts` de esa dependencia; `strict` y la comprobación de los archivos de la app siguen activos. Retirar esta excepción cuando las declaraciones de la dependencia sean compatibles.

## Demostración nativa pendiente

Las pruebas automatizadas no sustituyen abrir Tauri con `nix-shell --run 'make dev'`,
analizar los ejemplos de la [guía de pila semántica](../../../docs/language/semantic-stack.md)
y recorrer los eventos recibidos del compilador real. Se debe verificar la
navegación a la fuente, los estados de error/truncamiento y la limpieza al cambiar
el código. La ejecución del smoke con mocks y las pruebas Rust de DTO constituyen
evidencias distintas; no se presentan como una inspección visual nativa completada.
