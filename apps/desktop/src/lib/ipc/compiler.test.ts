import { afterEach, expect, it, vi } from 'vitest'
import {
  analyzeSource,
  analyzeProject,
  cancelRun,
  commandErrorMessage,
  inspectNative,
  openProject,
  runProject,
  runSource,
  selectProjectDirectory,
} from './compiler'

afterEach(() => vi.unstubAllGlobals())

it('explains the missing desktop runtime for every native action', async () => {
  vi.stubGlobal('isTauri', false)
  const actions = [
    () => analyzeSource('', 'main.itd'),
    () => runSource('', 'main.itd', 'test'),
    () => cancelRun('test'),
    () => inspectNative('', 'main.itd'),
    () => openProject('/project'),
    () => runProject('main', 'test'),
    () => analyzeProject('main'),
    () => selectProjectDirectory(),
  ]
  for (const action of actions) {
    await expect(action()).rejects.toMatchObject({
      code: 'DESKTOP_REQUIRED',
      message: expect.stringContaining('pnpm tauri dev'),
    })
  }
})

it('preserves native and JavaScript error details', () => {
  expect(commandErrorMessage(new Error('Connection lost'))).toBe(
    'Connection lost',
  )
  expect(commandErrorMessage('Command not found')).toBe('Command not found')
  expect(
    commandErrorMessage({ code: 'NO_PROJECT', message: 'Select a project' }),
  ).toBe('NO_PROJECT: Select a project')
  expect(commandErrorMessage(null)).toBe('Falló')
})

it('passes native inspection DTOs through IPC', async () => {
  vi.stubGlobal('isTauri', true)
  const inspection = {
    quadruples: [
      {
        function: 'main',
        operation: 'ADD',
        arg1: '2',
        arg2: '3',
        result: 't0:i32',
        valueType: 'i32',
        location: {
          file: 'lesson.itd',
          start: 0,
          end: 5,
          line: 1,
          column: 1,
        },
      },
    ],
    truncated: false,
    assembly: '.globl _main',
    diagnostics: [],
  }
  const invoke = vi.fn().mockResolvedValue(inspection)
  vi.stubGlobal('window', { __TAURI_INTERNALS__: { invoke } })
  await expect(inspectNative('2 + 3;', 'lesson.itd')).resolves.toEqual(
    inspection,
  )
  expect(invoke).toHaveBeenCalledWith(
    'inspect_native',
    { source: '2 + 3;', virtualPath: 'lesson.itd' },
    undefined,
  )
})

it('preserves semantic trace deltas and absence across native IPC', async () => {
  vi.stubGlobal('isTauri', true)
  const location = { file: 'lesson.itd', start: 0, end: 1, line: 1, column: 1 }
  const semanticStack = {
    events: [
      {
        kind: 'push',
        expression: 0,
        location,
        rule: 'literal',
        depthBefore: 0,
        depthAfter: 1,
        popCount: 0,
        pushed: { expression: 0, typeLabel: 'int', location },
        conversionType: null,
      },
    ],
    truncated: true,
  }
  const invoke = vi.fn().mockResolvedValue({ semanticStack })
  vi.stubGlobal('window', { __TAURI_INTERNALS__: { invoke } })
  expect((await analyzeSource('2;', 'lesson.itd')).semanticStack).toEqual(
    semanticStack,
  )
  expect(invoke).toHaveBeenCalledWith(
    'analyze_source',
    { source: '2;', virtualPath: 'lesson.itd' },
    undefined,
  )
  invoke.mockResolvedValue({ semanticStack: null })
  expect(
    (await analyzeSource('let := ;', 'lesson.itd')).semanticStack,
  ).toBeNull()
})
