// Deterministic original wolf icon, built using only Node's standard library.
import { Buffer } from 'node:buffer'
import { mkdirSync, writeFileSync } from 'node:fs'
import { deflateSync } from 'node:zlib'

const size = 64
const pixels = Buffer.alloc(size * size * 4)
function polygon(points, color) {
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let inside = false
      for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
        const [xi, yi] = points[i]
        const [xj, yj] = points[j]
        if ((yi > y) !== (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi)
          inside = !inside
      }
      if (inside)
        pixels.set(color, (y * size + x) * 4)
    }
  }
}
polygon([[7, 6], [25, 17], [39, 17], [57, 6], [55, 40], [45, 54], [32, 60], [19, 54], [9, 40]], [183, 195, 222, 255])
polygon([[13, 14], [22, 22], [13, 29]], [151, 121, 177, 255])
polygon([[51, 14], [42, 22], [51, 29]], [151, 121, 177, 255])
polygon([[12, 38], [25, 39], [32, 47], [39, 39], [52, 38], [44, 51], [32, 57], [20, 51]], [241, 244, 251, 255])
polygon([[20, 31], [25, 32], [25, 38], [20, 37]], [95, 79, 151, 255])
polygon([[39, 32], [44, 31], [44, 37], [39, 38]], [95, 79, 151, 255])
polygon([[28, 45], [36, 45], [32, 49]], [55, 61, 83, 255])

function crc32(data) {
  let crc = 0xFFFFFFFF
  for (const byte of data) {
    crc ^= byte
    for (let i = 0; i < 8; i++)
      crc = (crc >>> 1) ^ ((crc & 1) ? 0xEDB88320 : 0)
  }
  return (crc ^ 0xFFFFFFFF) >>> 0
}
function chunk(name, data) {
  const type = Buffer.from(name)
  const length = Buffer.alloc(4)
  length.writeUInt32BE(data.length)
  const checksum = Buffer.alloc(4)
  checksum.writeUInt32BE(crc32(Buffer.concat([type, data])))
  return Buffer.concat([length, type, data, checksum])
}
const header = Buffer.alloc(13)
header.writeUInt32BE(size, 0)
header.writeUInt32BE(size, 4)
header[8] = 8
header[9] = 6
const rows = Buffer.alloc(size * (size * 4 + 1))
for (let y = 0; y < size; y++)
  pixels.copy(rows, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4)
const png = Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', deflateSync(rows)), chunk('IEND', Buffer.alloc(0))])
const icoHeader = Buffer.alloc(22)
icoHeader.writeUInt16LE(1, 2)
icoHeader.writeUInt16LE(1, 4)
icoHeader[6] = size
icoHeader[7] = size
icoHeader.writeUInt16LE(1, 10)
icoHeader.writeUInt16LE(32, 12)
icoHeader.writeUInt32LE(png.length, 14)
icoHeader.writeUInt32LE(22, 18)
const directory = new URL('../apps/desktop/src-tauri/icons/', import.meta.url)
mkdirSync(directory, { recursive: true })
writeFileSync(new URL('icon.png', directory), png)
writeFileSync(new URL('icon.ico', directory), Buffer.concat([icoHeader, png]))
