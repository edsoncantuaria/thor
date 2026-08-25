#!/usr/bin/env node
/**
 * Dispara o workflow "Release" (build dos 4 instaladores + GitHub Release) via gh CLI.
 *
 * NÃO altera versão: publica a partir do HEAD atual do branch no remoto, usando a
 * versão que estiver em src-tauri/tauri.conf.json. Rode `npm run release` antes,
 * para que o bump já esteja no remoto.
 *
 * Uso:
 *   npm run release:publish            # dispara no branch atual
 *   npm run release:publish -- main    # dispara num ref específico
 *
 * Requer gh CLI autenticado (https://cli.github.com/ · gh auth login).
 */
import { execSync } from 'node:child_process'

function sh(cmd) {
  return execSync(cmd, { stdio: 'pipe', encoding: 'utf8' }).trim()
}
function fail(msg) {
  console.error(`\n✗ ${msg}\n`)
  process.exit(1)
}

try {
  sh('gh --version')
} catch {
  fail('gh CLI não encontrado. Instale em https://cli.github.com/ e rode: gh auth login')
}
try {
  sh('gh auth status')
} catch {
  fail('gh não autenticado. Rode: gh auth login')
}

const refArg = process.argv.slice(2).find((a) => !a.startsWith('--'))
const ref = refArg ?? sh('git rev-parse --abbrev-ref HEAD')

console.log(`\n  Disparando workflow "Release" no ref "${ref}"...\n`)
execSync(`gh workflow run release.yml --ref ${ref}`, { stdio: 'inherit' })
console.log('\n✓ Disparado. Acompanhe em: Actions → Release')
console.log('  ou pelo terminal:  gh run watch\n')
