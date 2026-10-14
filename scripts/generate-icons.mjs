// Resize the imagegen ribbon artwork with the installed Tauri CLI, then pack a Windows ICO.
import { Buffer } from 'node:buffer'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const source = join(root, 'docs/assets/silverwolf-ribbon.png')
const cli = join(root, 'apps/desktop/node_modules/@tauri-apps/cli/tauri.js')
const directory = join(root, 'apps/desktop/src-tauri/icons')
const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
const temporaryRoot = resolve(tmpdir())
const temporary = mkdtempSync(join(temporaryRoot, 'silverwolf-icons-'))

try {
  const result = spawnSync(process.execPath, [
    cli,
    'icon',
    source,
    '--output',
    temporary,
    ...sizes.flatMap(size => ['--png', String(size)]),
  ], { cwd: root, stdio: 'inherit' })
  if (result.error)
    throw result.error
  if (result.status !== 0)
    throw new Error(`Tauri icon conversion failed (${result.status})`)

  const images = sizes.map(size => readFileSync(join(temporary, `${size}x${size}.png`)))
  const header = Buffer.alloc(6 + sizes.length * 16)
  header.writeUInt16LE(1, 2)
  header.writeUInt16LE(sizes.length, 4)
  let offset = header.length
  sizes.forEach((size, index) => {
    const entry = 6 + index * 16
    header[entry] = size === 256 ? 0 : size
    header[entry + 1] = size === 256 ? 0 : size
    header.writeUInt16LE(1, entry + 4)
    header.writeUInt16LE(32, entry + 6)
    header.writeUInt32LE(images[index].length, entry + 8)
    header.writeUInt32LE(offset, entry + 12)
    offset += images[index].length
  })

  mkdirSync(directory, { recursive: true })
  copyFileSync(join(temporary, '256x256.png'), join(directory, 'icon.png'))
  writeFileSync(join(directory, 'icon.ico'), Buffer.concat([header, ...images]))
}
finally {
  if (dirname(resolve(temporary)) === temporaryRoot)
    rmSync(temporary, { recursive: true, force: true })
}
