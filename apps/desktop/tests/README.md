# Verificación de la interfaz

`make test-ui` valida adaptadores IPC, ubicaciones, layout y el catálogo compartido. `make compile` comprueba los tipos de la aplicación y genera el frontend.

`browser-smoke.mjs` exporta una función que recibe una página Playwright. Con el frontend servido en `http://127.0.0.1:1420`, se puede invocar desde cualquier runner Playwright existente:

```js
import smoke from './browser-smoke.mjs'
await smoke(page)
```

La prueba usa IPC simulado para aislar la interfaz: verifica la carga del editor, conservación de cambios, respuesta obsoleta, navegación desde bytecode, controles oscuros, modal de pantalla completa, Escape, restauración del foco y viewport móvil. Las pruebas Rust verifican por separado la ejecución real y los DTO de Tauri. No se presenta la simulación como prueba de ejecución nativa.

`csstype` está fijado en 3.1.3 para los tipos generados de `bits-ui` 2.19. `skipLibCheck` evita evaluar las uniones excesivas de los `.d.ts` de esa dependencia; `strict` y la comprobación de los archivos de la app siguen activos. Retirar esta excepción cuando las declaraciones de la dependencia sean compatibles.
