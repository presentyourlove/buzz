<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  status: 'idle' | 'working' | 'blocked' | 'offline' | 'error'
  size?: number
  strokeWidth?: number
  showLabel?: boolean
  animate?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  size: 48,
  strokeWidth: 4,
  showLabel: true,
  animate: true,
})

const radius = computed(() => (props.size - props.strokeWidth) / 2)
const circumference = computed(() => 2 * Math.PI * radius.value)

const statusColors = {
  idle: { bg: 'bg-gray-100 dark:bg-gray-800', ring: 'stroke-gray-400', label: 'text-gray-600 dark:text-gray-400', text: '待機' },
  working: { bg: 'bg-blue-100 dark:bg-blue-900/30', ring: 'stroke-blue-500', label: 'text-blue-600 dark:text-blue-400', text: '執行中' },
  blocked: { bg: 'bg-yellow-100 dark:bg-yellow-900/30', ring: 'stroke-yellow-500', label: 'text-yellow-600 dark:text-yellow-400', text: '阻塞' },
  offline: { bg: 'bg-gray-100 dark:bg-gray-800', ring: 'stroke-gray-300', label: 'text-gray-500 dark:text-gray-500', text: '離線' },
  error: { bg: 'bg-red-100 dark:bg-red-900/30', ring: 'stroke-red-500', label: 'text-red-600 dark:text-red-400', text: '錯誤' },
}

const currentStatus = computed(() => statusColors[props.status])

const ringStyle = computed(() => ({
  strokeDasharray: circumference.value,
  strokeDashoffset: props.animate && props.status === 'working' ? 0 : circumference.value,
  transition: props.animate ? 'stroke-dashoffset 1s linear' : 'none',
}))
</script>

<template>
  <div class="flex flex-col items-center gap-2">
    <div class="relative" :style="{ width: `${size}px`, height: `${size}px` }">
      <svg :width="size" :height="size" viewBox="0 0 48 48" class="transform -rotate-90">
        <!-- Background circle -->
        <circle
          cx="24"
          cy="24"
          :r="radius"
          :stroke-width="strokeWidth"
          :class="currentStatus.bg.replace('bg-', 'fill-')"
          fill="currentColor"
        />
        <!-- Progress ring -->
        <circle
          v-if="props.status === 'working' && animate"
          cx="24"
          cy="24"
          :r="radius"
          :stroke-width="strokeWidth"
          :class="currentStatus.ring"
          fill="none"
          :style="ringStyle"
          stroke-linecap="round"
        />
        <!-- Static ring for non-working states -->
        <circle
          v-else
          cx="24"
          cy="24"
          :r="radius"
          :stroke-width="strokeWidth"
          :class="currentStatus.ring"
          fill="none"
          stroke-dasharray="100%"
        />
      </svg>
      <!-- Center status indicator -->
      <div
        class="absolute inset-0 flex items-center justify-center"
        :style="{ width: `${size - 16}px`, height: `${size - 16}px`, top: '8px', left: '8px' }"
      >
        <div
          :class="[
            'w-3 h-3 rounded-full',
            props.status === 'working' ? 'bg-blue-500 animate-pulse' :
            props.status === 'blocked' ? 'bg-yellow-500' :
            props.status === 'error' ? 'bg-red-500' :
            props.status === 'offline' ? 'bg-gray-400' : 'bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.6)]',
          ]"
        />
      </div>
    </div>
    <span v-if="showLabel" :class="['text-xs font-medium', currentStatus.label]">{{ currentStatus.text }}</span>
  </div>
</template>
