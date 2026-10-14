import { defineConfig, presetWind3 } from 'unocss'

export default defineConfig({
  presets: [presetWind3()],
  shortcuts: {
    'row': 'flex items-center',
    'row-between': 'flex items-center justify-between',
    'quiet': 'text-[var(--muted)]',
  },
})
