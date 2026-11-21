import { onMounted, onUnmounted } from 'vue'

export interface KeyboardShortcut {
  key: string
  ctrl?: boolean
  shift?: boolean
  alt?: boolean
  meta?: boolean
  handler: (event: KeyboardEvent) => void
  preventDefault?: boolean
  stopPropagation?: boolean
}

export function useKeyboard(shortcuts: KeyboardShortcut[]) {
  const handler = (event: KeyboardEvent) => {
    for (const shortcut of shortcuts) {
      const keyMatch = event.key.toLowerCase() === shortcut.key.toLowerCase()
      const ctrlMatch = (shortcut.ctrl ?? false) === event.ctrlKey
      const shiftMatch = (shortcut.shift ?? false) === event.shiftKey
      const altMatch = (shortcut.alt ?? false) === event.altKey
      const metaMatch = (shortcut.meta ?? false) === event.metaKey

      if (keyMatch && ctrlMatch && shiftMatch && altMatch && metaMatch) {
        if (shortcut.preventDefault) event.preventDefault()
        if (shortcut.stopPropagation) event.stopPropagation()
        shortcut.handler(event)
        break
      }
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handler)
  })

  onUnmounted(() => {
    window.removeEventListener('keydown', handler)
  })
}

export function useShortcut(
  key: string,
  handler: (event: KeyboardEvent) => void,
  options: Partial<Omit<KeyboardShortcut, 'key' | 'handler'>> = {}
) {
  useKeyboard([{ key, handler, ...options }])
}
