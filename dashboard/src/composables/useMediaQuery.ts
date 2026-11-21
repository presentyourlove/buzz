import { ref, onMounted, onUnmounted } from 'vue'

export function useMediaQuery(query: string): boolean {
  const matches = ref(false)

  let mediaQuery: MediaQueryList | null = null

  const handler = (event: MediaQueryListEvent) => {
    matches.value = event.matches
  }

  onMounted(() => {
    if (typeof window !== 'undefined') {
      mediaQuery = window.matchMedia(query)
      matches.value = mediaQuery.matches
      mediaQuery.addEventListener('change', handler)
    }
  })

  onUnmounted(() => {
    if (mediaQuery) {
      mediaQuery.removeEventListener('change', handler)
    }
  })

  return matches.value
}

// 常用斷點
export const useIsMobile = () => useMediaQuery('(max-width: 639px)')
export const useIsTablet = () => useMediaQuery('(min-width: 640px) and (max-width: 1023px)')
export const useIsDesktop = () => useMediaQuery('(min-width: 1024px)')
export const useIsDark = () => useMediaQuery('(prefers-color-scheme: dark)')
export const useReducedMotion = () => useMediaQuery('(prefers-reduced-motion: reduce)')
