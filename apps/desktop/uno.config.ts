import { defineConfig, presetIcons, presetWind3 } from 'unocss'

export default defineConfig({
  presets: [presetWind3(), presetIcons()],
  shortcuts: {
    'row': 'flex items-center',
    'row-between': 'flex items-center justify-between',
    'quiet': 'text-[var(--muted)]',
    'ui-button-icon': 'block h-[18px] w-[18px] shrink-0',
  },
})
