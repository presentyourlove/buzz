<script setup lang="ts">
import { computed, h } from 'vue'
import Card from '../common/Card.vue'

interface Props {
  title: string
  value: string | number
  change?: number
  changeLabel?: string
  trend?: 'up' | 'down' | 'neutral'
  icon?: string
  iconColor?: 'primary' | 'success' | 'warning' | 'danger'
  loading?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  trend: 'neutral',
  iconColor: 'primary',
  loading: false,
})

const iconColors = computed(() => ({
  primary: 'bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400',
  success: 'bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400',
  warning: 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-600 dark:text-yellow-400',
  danger: 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400',
})[props.iconColor])

const trendColors = computed(() => ({
  up: 'text-green-600 dark:text-green-400',
  down: 'text-red-600 dark:text-red-400',
  neutral: 'text-gray-500 dark:text-gray-400',
}))

const trendIcons = {
  up: () => h('svg', { class: 'w-4 h-4', fill: 'none', stroke: 'currentColor', viewBox: '0 0 24 24' }, [
    h('path', { strokeLinecap: 'round', strokeLinejoin: 'round', strokeWidth: '2', d: 'M5 10l7-7m0 0l7 7m-7-7v18' }),
  ]),
  down: () => h('svg', { class: 'w-4 h-4', fill: 'none', stroke: 'currentColor', viewBox: '0 0 24 24' }, [
    h('path', { strokeLinecap: 'round', strokeLinejoin: 'round', strokeWidth: '2', d: 'M19 14l-7 7m0 0l-7-7m7 7V5' }),
  ]),
  neutral: () => h('svg', { class: 'w-4 h-4', fill: 'none', stroke: 'currentColor', viewBox: '0 0 24 24' }, [
    h('path', { strokeLinecap: 'round', strokeLinejoin: 'round', strokeWidth: '2', d: 'M5 12h14' }),
  ]),
}
</script>

<template>
  <Card :padding="'md'" class="relative overflow-hidden">
    <div class="flex items-start justify-between">
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium text-gray-600 dark:text-gray-400 truncate">{{ title }}</p>
        <div class="mt-2 flex items-baseline gap-2">
          <span v-if="loading" class="text-2xl font-bold text-gray-900 dark:text-gray-100 animate-pulse">
            <span class="bg-gray-200 dark:bg-gray-700 h-8 w-24 inline-block rounded" />
          </span>
          <span v-else class="text-2xl font-bold text-gray-900 dark:text-gray-100 tabular-nums">{{ value }}</span>
          <div v-if="change !== undefined" class="flex items-center gap-1">
            <component :is="trendIcons[trend]" :class="trendColors[trend]" />
            <span :class="['text-sm font-medium', trendColors[trend]]">
              {{ change >= 0 ? '+' : '' }}{{ change }}%
            </span>
            <span v-if="changeLabel" class="text-sm text-gray-500 dark:text-gray-400">{{ changeLabel }}</span>
          </div>
        </div>
      </div>
      <div v-if="icon" :class="['p-3 rounded-xl', iconColors]" aria-hidden="true">
        <slot name="icon">
          <component :is="icon" class="w-6 h-6" />
        </slot>
      </div>
    </div>
  </Card>
</template>
