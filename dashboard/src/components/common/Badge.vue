<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/utils'

interface Props {
  variant?: 'success' | 'warning' | 'danger' | 'info' | 'gray'
  size?: 'sm' | 'md' | 'lg'
  dot?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'gray',
  size: 'md',
  dot: false,
})

const classes = computed(() =>
  cn(
    'badge inline-flex items-center gap-1 font-medium',
    {
      'badge-success': props.variant === 'success',
      'badge-warning': props.variant === 'warning',
      'badge-danger': props.variant === 'danger',
      'badge-info': props.variant === 'info',
      'badge-gray': props.variant === 'gray',
      'px-1.5 py-0.5 text-xs': props.size === 'sm',
      'px-2 py-0.5 text-xs': props.size === 'md',
      'px-2.5 py-1 text-sm': props.size === 'lg',
    }
  )
)
</script>

<template>
  <span :class="classes">
    <span v-if="dot" class="w-1.5 h-1.5 rounded-full bg-current opacity-70" />
    <slot />
  </span>
</template>
