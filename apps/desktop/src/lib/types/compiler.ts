export interface LocationDto {
  file: string
  start: number
  end: number
  line: number
  column: number
}
export interface DiagnosticDto {
  code: string
  severity: 'error' | 'warning'
  message: string
  location: LocationDto
}
export interface TokenDto {
  kind: string
  lexeme: string
  location: LocationDto
}
export interface SymbolDto {
  name: string
  kind: string
  ty: string
}
export interface AstNodeDto {
  id: string
  parentId: string | null
  relation: string | null
  kind: string
  label: string
  location: LocationDto | null
  semantic?: {
    inferredType: string | null
    sourceType: string | null
    symbol: { id: number; name: string } | null
  }
}
export interface AstGraphDto {
  nodes: AstNodeDto[]
  truncated: boolean
  partial?: boolean
}
export interface AnalysisDto {
  diagnostics: DiagnosticDto[]
  tokens: TokenDto[]
  symbols: SymbolDto[]
  ast: AstGraphDto
  semanticAst: AstGraphDto | null
  bytecode: BytecodeDto
}
export interface RunDto {
  bytecode: BytecodeDto
  output: string
  diagnostics: DiagnosticDto[]
}
export interface ProjectDto {
  modules: string[]
  diagnostics: DiagnosticDto[]
}
export interface CommandError {
  code: string
  message: string
}

export interface BytecodeDto {
  rows: {
    function: string
    offset: number
    instruction: string
    location: LocationDto
  }[]
  truncated: boolean
}
