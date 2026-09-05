import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import strings from './locales/es.json'
import './styles.css'

type Status = 'ready' | 'analyzing' | 'running' | 'cancelled' | 'failed'

interface LocationDto {
  file: string
  start: number
  end: number
  line: number
  column: number
}

interface DiagnosticDto {
  code: string
  severity: 'error' | 'warning'
  message: string
  location: LocationDto
}

interface SymbolDto {
  name: string
  kind: string
  ty: string
}

interface AnalysisDto {
  diagnostics: DiagnosticDto[]
  tokens: Array<{ kind: string; lexeme: string; location: LocationDto }>
  symbols: SymbolDto[]
}

interface RunDto {
  output: string
  diagnostics: DiagnosticDto[]
}

interface ProjectDto {
  modules: string[]
  diagnostics: DiagnosticDto[]
}

interface CommandError {
  code: string
  message: string
}

const app = document.querySelector<HTMLElement>('#app')
if (!app) throw new Error('Missing application root')

app.innerHTML = `
  <header class="masthead">
    <h1>${strings.title}</h1>
    <div id="status" class="status" data-status="ready">${strings.ready}</div>
  </header>
  <section class="toolbar" aria-label="Acciones">
    <label>${strings.virtualPath}<input id="path" value="main.itd" /></label>
    <button id="analyze">${strings.analyze}</button>
    <button id="run">${strings.run}</button>
    <button id="stop" disabled>${strings.stop}</button>
    <button id="open-project">${strings.openProject}</button>
    <label>${strings.entryModule}<input id="entry" value="main" /></label>
    <button id="run-project">${strings.runProject}</button>
  </section>
  <section class="workspace">
    <article class="editor-panel">
      <h2>${strings.source}</h2>
      <textarea id="source" spellcheck="false" aria-label="${strings.source}">let amount := 0.1dec + 0.2dec;
print(to_string(amount));</textarea>
    </article>
    <aside class="diagnostics-panel">
      <h2>${strings.diagnostics} <span id="token-count"></span></h2>
      <ol id="diagnostics"><li class="empty">${strings.noDiagnostics}</li></ol>
    </aside>
    <aside class="symbols-panel">
      <h2>${strings.symbols}</h2>
      <div id="symbols" class="empty">${strings.noSymbols}</div>
    </aside>
    <article class="output-panel">
      <h2>${strings.output}</h2>
      <pre id="output">${strings.noOutput}</pre>
    </article>
  </section>`

const source = element<HTMLTextAreaElement>('source')
const path = element<HTMLInputElement>('path')
const entry = element<HTMLInputElement>('entry')
const output = element<HTMLElement>('output')
const diagnostics = element<HTMLOListElement>('diagnostics')
const symbols = element<HTMLElement>('symbols')
const tokenCount = element<HTMLElement>('token-count')
const stopButton = element<HTMLButtonElement>('stop')
let executionId: string | null = null

element('analyze').addEventListener('click', () => void analyze())
element('run').addEventListener('click', () => void runSource())
stopButton.addEventListener('click', () => void stop())
element('open-project').addEventListener('click', () => void openProject())
element('run-project').addEventListener('click', () => void runProject())

async function analyze(): Promise<void> {
  setStatus('analyzing')
  try {
    const result = await invoke<AnalysisDto>('analyze_source', {
      source: source.value,
      virtualPath: path.value,
    })
    renderDiagnostics(result.diagnostics)
    renderSymbols(result.symbols)
    tokenCount.textContent = `${result.tokens.length} ${strings.tokens}`
    setStatus(
      result.diagnostics.some((item) => item.severity === 'error')
        ? 'failed'
        : 'ready',
    )
  } catch (error: unknown) {
    fail(error)
  }
}

async function runSource(): Promise<void> {
  executionId = crypto.randomUUID()
  stopButton.disabled = false
  setStatus('running')
  try {
    const result = await invoke<RunDto>('run_source', {
      source: source.value,
      virtualPath: path.value,
      executionId,
    })
    output.textContent = result.output || strings.noOutput
    renderDiagnostics(result.diagnostics)
    const cancelled = result.diagnostics.some((item) => item.code === 'E5004')
    setStatus(
      cancelled ? 'cancelled' : result.diagnostics.length ? 'failed' : 'ready',
    )
  } catch (error: unknown) {
    fail(error)
  } finally {
    executionId = null
    stopButton.disabled = true
  }
}

async function stop(): Promise<void> {
  if (!executionId) return
  await invoke('cancel_run', { executionId })
  setStatus('cancelled')
}

async function openProject(): Promise<void> {
  const selected = await open({ directory: true, multiple: false })
  if (typeof selected !== 'string') return
  try {
    await invoke<ProjectDto>('open_project', { root: selected })
    setStatus('ready', strings.projectReady)
  } catch (error: unknown) {
    fail(error)
  }
}

async function runProject(): Promise<void> {
  setStatus('analyzing')
  try {
    const result = await invoke<ProjectDto>('run_project', {
      entryModule: entry.value,
    })
    renderDiagnostics(result.diagnostics)
    output.textContent = result.modules.join('\n') || strings.noOutput
    setStatus(result.diagnostics.length ? 'failed' : 'ready')
  } catch (error: unknown) {
    fail(error)
  }
}

function renderDiagnostics(items: DiagnosticDto[]): void {
  diagnostics.replaceChildren()
  if (!items.length) {
    diagnostics.append(emptyItem(strings.noDiagnostics))
    return
  }
  for (const item of items) {
    const row = document.createElement('li')
    const button = document.createElement('button')
    button.className = `diagnostic ${item.severity}`
    button.textContent = `${item.location.file}:${item.location.line}:${item.location.column} ${item.code} ${item.message}`
    button.addEventListener('click', () => {
      source.focus()
      source.setSelectionRange(item.location.start, item.location.end)
    })
    row.append(button)
    diagnostics.append(row)
  }
}

function renderSymbols(items: SymbolDto[]): void {
  symbols.replaceChildren()
  if (!items.length) {
    symbols.textContent = strings.noSymbols
    return
  }
  const table = document.createElement('table')
  for (const item of items) {
    const row = table.insertRow()
    for (const value of [item.name, item.kind, item.ty])
      row.insertCell().textContent = value
  }
  symbols.append(table)
}

function setStatus(status: Status, text = strings[status]): void {
  const node = element<HTMLElement>('status')
  node.dataset.status = status
  node.textContent = text
}

function fail(error: unknown): void {
  const value = isCommandError(error)
    ? `${error.code}: ${error.message}`
    : strings.failed
  output.textContent = value
  setStatus('failed')
}

function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    'message' in value
  )
}

function emptyItem(text: string): HTMLLIElement {
  const item = document.createElement('li')
  item.className = 'empty'
  item.textContent = text
  return item
}

function element<T extends HTMLElement = HTMLElement>(id: string): T {
  const value = document.getElementById(id)
  if (!value) throw new Error(`Missing element: ${id}`)
  return value as T
}
