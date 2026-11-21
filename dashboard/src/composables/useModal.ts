import { useUIStore } from '@/stores'

export function useModal(key: string) {
  const uiStore = useUIStore()

  const open = (modal: Omit<Parameters<typeof uiStore.openModal>[1], 'isOpen'>) => {
    uiStore.openModal(key, { ...modal, isOpen: true })
  }

  const close = () => {
    uiStore.closeModal(key)
  }

  const isOpen = () => {
    return uiStore.modals[key]?.isOpen ?? false
  }

  return {
    open,
    close,
    isOpen,
    state: uiStore.modals[key] as any,
  }
}
