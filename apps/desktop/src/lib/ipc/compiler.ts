import { invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import strings from '../../locales/es.json'
import type { AnalysisDto, ProjectDto, RunDto } from '$lib/types/compiler'

import { ErrorWithCode } from '$lib/errors'

function requireDesktop() {
  if (!isTauri())
    throw new ErrorWithCode('DESKTOP_REQUIRED', strings.desktopRequired)
}

async function invoke<T>(
  command: string,
  args: Record<string, unknown>,
): Promise<T> {
  requireDesktop()
  return tauriInvoke<T>(command, args)
}

export async function selectProjectDirectory() {
  requireDesktop()
  return open({ directory: true, multiple: false })
}

export function commandErrorMessage(error: unknown): string {
  if (typeof error === 'string' && error) return error
  if (typeof error !== 'object' || error === null) return strings.failed
  if (!('message' in error) || typeof error.message !== 'string')
    return strings.failed
  return 'code' in error && typeof error.code === 'string'
    ? `${error.code}: ${error.message}`
    : error.message || strings.failed
}

export const analyzeSource = (source: string, virtualPath: string) =>
  invoke<AnalysisDto>('analyze_source', { source, virtualPath })
export const runSource = (
  source: string,
  virtualPath: string,
  executionId: string,
) => invoke<RunDto>('run_source', { source, virtualPath, executionId })
export const cancelRun = (executionId: string) =>
  invoke('cancel_run', { executionId })
export const openProject = (root: string) =>
  invoke<ProjectDto>('open_project', { root })
export const analyzeProject = (entryModule: string) =>
  invoke<ProjectDto>('analyze_project', { entryModule })
export const runProject = (entryModule: string, executionId: string) =>
  invoke<RunDto>('run_project', { entryModule, executionId })
