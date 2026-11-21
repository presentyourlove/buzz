import { onMounted, onUnmounted, type Ref } from 'vue'

export function useClickOutside(
  element: Ref<HTMLElement | null>,
  callback: (event: MouseEvent) => void
) {
  const handler = (event: MouseEvent) => {
    if (element.value && !element.value.contains(event.target as Node)) {
      callback(event)
    }
  }

  onMounted(() => {
    document.addEventListener('mousedown', handler)
  })

  onUnmounted(() => {
    document.removeEventListener('mousedown', handler)
  })
}
