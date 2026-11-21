<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/utils'

interface Props {
  variant?: 'default' | 'outlined' | 'elevated'
  padding?: 'none' | 'sm' | 'md' | 'lg'
  hover?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
  padding: 'md',
  hover: false,
})

const classes = computed(() =>
  cn(
    'card rounded-lg transition-shadow',
    {
      'border border-gray-200 dark:border-gray-700': props.variant === 'outlined',
      'shadow-md': props.variant === 'elevated',
      'shadow-sm': props.variant === 'default',
      'hover:shadow-lg cursor-pointer': props.hover,
      'p-0': props.padding === 'none',
      'p-3': props.padding === 'sm',
      'p-4': props.padding === 'md',
      'p-6': props.padding === 'lg',
    }
  )
)
</script>

<template>
  <div :class="classes">
    <slot />
  </div>
</template>
