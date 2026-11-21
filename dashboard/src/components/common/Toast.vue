<script setup lang="ts">
import { ref, computed, onMounted, watch, h } from 'vue'

interface Props {
  modelValue: boolean
  type: 'success' | 'error' | 'warning' | 'info'
  title: string
  message?: string
  duration?: number
  action?: { label: string; onClick: () => void }
}

const props = withDefaults(defineProps<Props>(), {
  type: 'info',
  duration: 5000,
})

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const isVisible = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null

const bgClasses = computed(() => ({
  success: 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800',
  error: 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800',
  warning: 'bg-yellow-50 dark:bg-yellow-900/20 border-yellow-200 dark:border-yellow-800',
  info: 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800',
})[props.type])

const iconClasses = computed(() => ({
  success: 'text-green-500',
  error: 'text-red-500',
  warning: 'text-yellow-500',
  info: 'text-blue-500',
})[props.type])

const icons = {
  success: () => h('svg', { class: 'w-5 h-5', fill: 'currentColor', viewBox: '0 0 20 20' }, [
    h('path', { fillRule: 'evenodd', d: 'M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z', clipRule: 'evenodd' }),
  ]),
  error: () => h('svg', { class: 'w-5 h-5', fill: 'currentColor', viewBox: '0 0 20 20' }, [
    h('path', { fillRule: 'evenodd', d: 'M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z', clipRule: 'evenodd' }),
  ]),
  warning: () => h('svg', { class: 'w-5 h-5', fill: 'currentColor', viewBox: '0 0 20 20' }, [
    h('path', { fillRule: 'evenodd', d: 'M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z', clipRule: 'evenodd' }),
  ]),
  info: () => h('svg', { class: 'w-5 h-5', fill: 'currentColor', viewBox: '0 0 20 20' }, [
    h('path', { fillRule: 'evenodd', d: 'M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z', clipRule: 'evenodd' }),
  ]),
}

const showToast = () => {
  isVisible.value = true
  if (props.duration && props.duration > 0) {
    timer = setTimeout(() => {
      hideToast()
    }, props.duration)
  }
}

const hideToast = () => {
  isVisible.value = false
  if (timer) {
    clearTimeout(timer)
    timer = null
  }
  emit('update:modelValue', false)
}

onMounted(() => {
  if (props.modelValue) showToast()
})

watch(() => props.modelValue, (val) => {
  if (val) showToast()
  else hideToast()
})
</script>

<template>
  <Transition name="toast">
    <div
      v-if="isVisible"
      :class="[
        'flex items-start gap-3 p-4 rounded-lg border shadow-lg animate-slide-in',
        bgClasses,
      ]"
      role="alert"
      :aria-live="props.type === 'error' ? 'assertive' : 'polite'"
    >
      <div class="flex-shrink-0" :class="iconClasses">
        <component :is="icons[props.type]" />
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium text-gray-900 dark:text-gray-100">
          {{ props.title }}
        </p>
        <p v-if="message" class="mt-1 text-sm text-gray-500 dark:text-gray-400">
          {{ message }}
        </p>
        <button
          v-if="action"
          @click="action.onClick"
          class="mt-2 text-sm font-medium underline hover:no-underline focus:outline-none focus:ring-2 focus:ring-offset-2"
          :class="iconClasses.replace('text-', 'focus:ring-')"
        >
          {{ action.label }}
        </button>
      </div>
      <button
        @click="hideToast"
        class="flex-shrink-0 text-gray-400 hover:text-gray-500 dark:hover:text-gray-300 transition-colors p-1"
        aria-label="關閉通知"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  </Transition>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.3s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateX(100%);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(100%);
}
.animate-slide-in {
  animation: slideIn 0.3s ease-out;
}
@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateX(100%);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
</style>
