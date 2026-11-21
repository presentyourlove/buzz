<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/utils'

interface Props {
  modelValue: string
  type?: string
  placeholder?: string
  disabled?: boolean
  readonly?: boolean
  error?: string
  label?: string
  helperText?: string
  autocomplete?: string
}

const props = withDefaults(defineProps<Props>(), {
  type: 'text',
  disabled: false,
  readonly: false,
  autocomplete: 'off',
})

const emit = defineEmits<{ 'update:modelValue': [value: string]; blur: [event: FocusEvent] }>()

const inputClasses = computed(() =>
  cn(
    'input w-full px-3 py-2 rounded-md border transition-shadow',
    'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
    'disabled:opacity-50 disabled:cursor-not-allowed',
    props.error && 'border-red-500 focus:ring-red-500',
    (props.disabled || props.readonly) && 'bg-gray-50'
  )
)
</script>

<template>
  <div class="w-full">
    <label v-if="label" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
      {{ label }}
    </label>
    <input
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readonly"
      :autocomplete="autocomplete"
      :class="inputClasses"
      @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      @blur="emit('blur', $event)"
      aria-invalid="false"
      aria-describedby="error helper"
    />
    <p v-if="error" id="error" class="mt-1 text-sm text-red-600 dark:text-red-400" role="alert">{{ error }}</p>
    <p v-else-if="helperText" id="helper" class="mt-1 text-sm text-gray-500 dark:text-gray-400">{{ helperText }}</p>
  </div>
</template>
