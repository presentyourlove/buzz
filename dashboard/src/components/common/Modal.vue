<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useClickOutside } from '@/composables'

interface Props {
  modelValue: boolean
  title?: string
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
  closeOnOverlayClick?: boolean
  showCloseButton?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  size: 'md',
  closeOnOverlayClick: true,
  showCloseButton: true,
})

const emit = defineEmits<{ 'update:modelValue': [value: boolean]; close: [] }>()

const modalRef = ref<HTMLDivElement | null>(null)

useClickOutside(modalRef, () => {
  if (props.closeOnOverlayClick) {
    emit('update:modelValue', false)
  }
})

const sizeClasses = computed(() => ({
  sm: 'max-w-sm',
  md: 'max-w-md',
  lg: 'max-w-lg',
  xl: 'max-w-xl',
  full: 'max-w-4xl',
})[props.size])

watch(() => props.modelValue, (open) => {
  if (open) {
    document.body.style.overflow = 'hidden'
  } else {
    document.body.style.overflow = ''
  }
})
</script>

<template>
  <Transition name="modal">
    <div v-if="modelValue" class="fixed inset-0 z-50 overflow-y-auto">
      <div class="flex min-h-full items-center justify-center p-4">
        <!-- Overlay -->
        <div
          class="fixed inset-0 bg-black/50 transition-opacity"
          aria-hidden="true"
          @click="closeOnOverlayClick && $emit('update:modelValue', false)"
        />

        <!-- Modal Panel -->
        <div
          ref="modalRef"
          :class="['relative w-full bg-white dark:bg-gray-800 rounded-xl shadow-xl', sizeClasses]"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="title ? 'modal-title' : undefined"
        >
          <div class="flex items-start justify-between p-4 border-b border-gray-200 dark:border-gray-700">
            <h2 v-if="title" id="modal-title" class="text-lg font-semibold text-gray-900 dark:text-gray-100">
              {{ title }}
            </h2>
            <button
              v-if="showCloseButton"
              @click="$emit('update:modelValue', false)"
              class="text-gray-400 hover:text-gray-500 dark:hover:text-gray-300 transition-colors p-1"
              aria-label="關閉對話框"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
          <div class="p-4">
            <slot />
          </div>
          <div v-if="$slots.actions" class="flex items-center justify-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700">
            <slot name="actions" />
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
</style>
