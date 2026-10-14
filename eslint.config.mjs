import antfu from '@antfu/eslint-config'

export default antfu({
  vue: true,
  typescript: true,
  ignores: ['**/gen/**', 'target/**'],
}, {
  files: ['**/*.test.ts'],
  rules: {
    'test/no-import-node-test': 'off',
  },
}, {
  files: ['scripts/*.mjs'],
  rules: {
    'unused-imports/no-unused-vars': 'off',
    'no-unused-vars': 'error',
  },
}, {
  rules: {
    'vue/one-component-per-file': 'off',
  },
})
