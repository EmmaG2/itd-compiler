import { afterEach, expect, it, vi } from 'vitest'
import {
  analyzeSource,
  analyzeProject,
  cancelRun,
  commandErrorMessage,
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
