<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  progress: number // 0-100
  status: 'backlog' | 'todo' | 'in_progress' | 'review' | 'done' | 'cancelled'
  size?: 'sm' | 'md' | 'lg'
  showLabel?: boolean
  striped?: boolean
  animated?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  size: 'md',
  showLabel: true,
  striped: true,
  animated: false,
})

const statusColors = {
  backlog: 'bg-gray-400',
  todo: 'bg-blue-500',
  in_progress: 'bg-yellow-500',
  review: 'bg-purple-500',
  done: 'bg-green-500',
  cancelled: 'bg-red-500',
}

const heightClasses = {
  sm: 'h-1.5',
  md: 'h-2.5',
  lg: 'h-4',
}

const labelClasses = {
  sm: 'text-xs',
  md: 'text-sm',
  lg: 'text-base',
}

const progressValue = computed(() => Math.max(0, Math.min(100, props.progress)))
const barColor = computed(() => statusColors[props.status])
</script>

<template>
  <div class="w-full">
    <div v-if="showLabel" class="flex justify-between mb-1">
      <span :class="['font-medium text-gray-700 dark:text-gray-300', labelClasses[size]]">
        {{ props.status.replace('_', ' ') }}
      </span>
      <span :class="['font-mono tabular-nums', labelClasses[size]]">{{ progressValue }}%</span>
    </div>
    <div
      :class="[
        'w-full rounded-full overflow-hidden bg-gray-200 dark:bg-gray-700',
        heightClasses[size],
      ]"
      role="progressbar"
      :aria-valuenow="progressValue"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-label="`任務進度 ${props.status}: ${progressValue}%`"
    >
      <div
        :class="[
          'h-full transition-all duration-500 ease-out',
          barColor,
          props.striped && 'bg-stripes',
          props.animated && props.striped && 'animate-stripes',
        ]"
        :style="{ width: `${progressValue}%` }"
      />
    </div>
  </div>
</template>

<style scoped>
.bg-stripes {
  background-image: linear-gradient(
    45deg,
    rgba(255, 255, 255, 0.15) 25%,
    transparent 25%,
    transparent 50%,
    rgba(255, 255, 255, 0.15) 50%,
    rgba(255, 255, 255, 0.15) 75%,
    transparent 75%,
    transparent
  );
  background-size: 1rem 1rem;
}

.animate-stripes {
  animation: stripes 1s linear infinite;
}

@keyframes stripes {
  from {
    background-position: 0 0;
  }
  to {
    background-position: 1rem 0;
  }
}
</style>
