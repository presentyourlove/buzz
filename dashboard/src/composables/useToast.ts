import { useUIStore } from '@/stores'

export function useToast() {
  const uiStore = useUIStore()

  return {
    success: uiStore.success,
    error: uiStore.error,
    warning: uiStore.warning,
    info: uiStore.info,
    show: uiStore.showToast,
    dismiss: uiStore.dismissToast,
    clear: uiStore.clearToasts,
  }
}
