import assert from 'node:assert/strict'

export default async function smoke(page) {
  await page.addInitScript(() => {
    window.isTauri = true
    window.__calls = []
    const location = { file: 'main.itd', start: 0, end: 3, line: 1, column: 1 }
    const ast = {
      truncated: false,
      nodes: [
        {
          id: 'ast-0',
          parentId: null,
          relation: null,
          kind: 'Program',
          label: 'Program',
          location: null,
        },
        {
          id: 'ast-1',
          parentId: 'ast-0',
          relation: 'item',
          kind: 'Variable',
          label: 'amount',
          location,
        },
        {
          id: 'ast-2',
          parentId: 'ast-1',
          relation: 'initializer',
          kind: 'Binary',
          label: 'Add',
          location,
        },
        {
          id: 'ast-3',
          parentId: 'ast-2',
          relation: 'left',
          kind: 'Literal',
          label: '0.1dec',
          location,
        },
        {
          id: 'ast-4',
          parentId: 'ast-2',
          relation: 'right',
          kind: 'Binary',
          label: 'Multiply',
          location,
        },
        {
          id: 'ast-5',
          parentId: 'ast-4',
          relation: 'left',
          kind: 'Literal',
          label: '0.2dec',
          location,
        },
        {
          id: 'ast-6',
          parentId: 'ast-4',
          relation: 'right',
          kind: 'Literal',
          label: '0.3dec',
          location,
        },
      ],
    }
    const bytecode = {
      truncated: false,
      rows: [
        {
          function: '<main>',
          offset: 0,
          instruction: 'Binary(Multiply)',
          location,
        },
      ],
    }
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        window.__calls.push({ command, args })
        if (command === 'analyze_source')
          return { diagnostics: [], tokens: [], symbols: [], ast, bytecode }
        if (command === 'run_source') {
          if (window.__delayRun)
            await new Promise((resolve) => {
              window.__finishRun = resolve
            })
          return { output: '0.16\n', diagnostics: [], bytecode }
        }
        if (command === 'run_project')
          return { output: '25.50\n', diagnostics: [], bytecode }
        if (command === 'cancel_run') {
          window.__finishRun?.()
          return
        }
        throw new Error(`Unexpected command ${command}`)
      },
    }
  })
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.goto('http://127.0.0.1:1420')
  await page
    .getByRole('combobox', { name: 'Ejemplos', exact: true })
    .selectOption('bank-account')
  await page
    .getByRole('button', { name: 'Cargar ejemplo', exact: true })
    .click()
  await page.waitForFunction(() =>
    document
      .querySelector('.cm-content')
      ?.textContent.includes('class BankAccount'),
  )
  await page.getByRole('button', { name: 'Analizar', exact: true }).click()
  assert(
    (await page.evaluate(() => window.__calls.at(-1).args.source)).includes(
      'class BankAccount',
    ),
  )
  await page.getByRole('tab', { name: 'AST', exact: true }).click()
  await page.getByRole('button', { name: 'Ampliar AST', exact: true }).click()
  const dialog = page.getByRole('dialog', {
    name: 'Grafo AST · pantalla completa',
  })
  await dialog.waitFor({ state: 'visible' })
  await dialog.getByRole('button', { name: 'Acercar', exact: true }).waitFor()
  const bounds = await dialog.boundingBox()
  assert.equal(bounds.width, 1440)
  assert.equal(bounds.height, 1000)
  const colors = await dialog
    .locator('.svelte-flow__controls-button')
    .first()
    .evaluate((button) => ({
      color: getComputedStyle(button).color,
      background: getComputedStyle(button).backgroundColor,
    }))
  assert.notEqual(colors.color, colors.background)
  assert.notEqual(colors.background, 'rgb(255, 255, 255)')
  await dialog.getByRole('button', { name: 'Acercar', exact: true }).click()
  await dialog
    .getByRole('button', { name: 'Ajustar grafo', exact: true })
    .click()
  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'hidden' })
  assert.equal(
    await page
      .getByRole('button', { name: 'Ampliar AST', exact: true })
      .evaluate((button) => document.activeElement === button),
    true,
  )
  await page.getByRole('tab', { name: 'Bytecode', exact: true }).click()
  await page.getByRole('button', { name: /Binary\(Multiply\)/ }).click()
  assert.equal(
    await page
      .locator('.cm-content')
      .evaluate((editor) => editor.contains(document.activeElement)),
    true,
  )

  const editor = page.locator('.cm-content')
  await editor.fill('let changed := 1;')
  await page
    .getByRole('combobox', { name: 'Ejemplos', exact: true })
    .selectOption('expressions')
  await page
    .getByRole('button', { name: 'Cargar ejemplo', exact: true })
    .click()
  await page
    .getByRole('button', { name: 'Cancelar y conservar cambios', exact: true })
    .click()
  assert.equal(await editor.textContent(), 'let changed := 1;')
  await page
    .getByRole('button', { name: 'Cargar ejemplo', exact: true })
    .click()
  await page
    .getByRole('button', { name: 'Reemplazar código', exact: true })
    .click()
  await page.waitForFunction(() =>
    document
      .querySelector('.cm-content')
      ?.textContent.includes('let a := 0.1dec'),
  )

  await page.evaluate(() => {
    window.__delayRun = true
  })
  await page.getByRole('button', { name: 'Ejecutar', exact: true }).click()
  await page.waitForFunction(() => typeof window.__finishRun === 'function')
  await editor.fill('let newer := 2;')
  await page.evaluate(() => window.__finishRun())
  await page
    .getByRole('button', { name: 'Ejecutar', exact: true })
    .waitFor({ state: 'visible' })
  await page.waitForFunction(
    () =>
      !Array.from(document.querySelectorAll('button')).find(
        (button) => button.textContent.trim() === 'Ejecutar',
      )?.disabled,
  )
  assert.equal(
    await page.getByRole('button', { name: /Binary\(Multiply\)/ }).count(),
    0,
  )
  assert.equal(await page.locator('main pre').textContent(), 'Sin salida')

  await page.getByRole('button', { name: 'Analizar', exact: true }).click()
  await page.getByRole('tab', { name: 'AST', exact: true }).click()
  await page.setViewportSize({ width: 390, height: 844 })
  await page.getByRole('button', { name: 'Ampliar AST', exact: true }).click()
  await dialog.waitFor({ state: 'visible' })
  assert.equal((await dialog.boundingBox()).width, 390)
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    true,
  )
  await page
    .getByRole('button', { name: 'Cerrar vista ampliada', exact: true })
    .click()
  return {
    passed: [
      'catalog',
      'source sync',
      'fullscreen',
      'dark controls',
      'Escape and focus',
      'bytecode navigation',
      'unsaved changes',
      'stale result',
      'mobile viewport',
    ],
  }
}
