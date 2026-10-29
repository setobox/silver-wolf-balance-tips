<script setup lang="ts">
import DesktopWidget from './components/DesktopWidget.vue'
import SettingsWindow from './components/settings/SettingsWindow.vue'

const settings = new URLSearchParams(location.search).get('view') === 'settings'
</script>

<template>
  <SettingsWindow v-if="settings" />
  <DesktopWidget v-else />
</template>
