import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Toast, ModalState } from '@/types'

export const useUIStore = defineStore('ui', () => {
  // State
  const toasts = ref<Toast[]>([])
  const modals = ref<Record<string, ModalState>>({})
  const loadingStates = ref<Record<string, boolean>>({})
  const drawerStates = ref<Record<string, boolean>>({})

  // Computed
  const activeToasts = computed(() => toasts.value.filter(t => !t._dismissed))
  const hasActiveModal = computed(() =>
    Object.values(modals.value).some(m => m.isOpen)
  )

  // Toast Actions
  function showToast(toast: Omit<Toast, 'id'>) {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
    const newToast: Toast = {
      id,
      duration: 4000,
      ...toast,
    }
    toasts.value.push(newToast)

    if (newToast.duration && newToast.duration > 0) {
      setTimeout(() => dismissToast(id), newToast.duration)
    }

    return id
  }

  function dismissToast(id: string) {
    const idx = toasts.value.findIndex(t => t.id === id)
    if (idx !== -1) {
      toasts.value[idx] = { ...toasts.value[idx], _dismissed: true } as Toast & { _dismissed: boolean }
      setTimeout(() => {
        toasts.value = toasts.value.filter(t => t.id !== id)
      }, 300)
    }
  }

  function clearToasts() {
    toasts.value = []
  }

  // 便利方法
  function success(title: string, message?: string) {
    return showToast({ type: 'success', title, message })
  }

  function error(title: string, message?: string) {
    return showToast({ type: 'error', title, message, duration: 6000 })
  }

  function warning(title: string, message?: string) {
    return showToast({ type: 'warning', title, message })
  }

  function info(title: string, message?: string) {
    return showToast({ type: 'info', title, message })
  }

  // Modal Actions
  function openModal(key: string, modal: ModalState) {
    modals.value[key] = { ...modal, isOpen: true }
  }

  function closeModal(key: string) {
    if (modals.value[key]) {
      modals.value[key] = { ...modals.value[key], isOpen: false }
    }
  }

  function closeAllModals() {
    Object.keys(modals.value).forEach(key => {
      modals.value[key] = { ...modals.value[key], isOpen: false }
    })
  }

  // Loading Actions
  function setLoading(key: string, loading: boolean) {
    loadingStates.value[key] = loading
  }

  function isLoading(key: string): boolean {
    return loadingStates.value[key] ?? false
  }

  // Drawer Actions
  function openDrawer(key: string) {
    drawerStates.value[key] = true
  }

  function closeDrawer(key: string) {
    drawerStates.value[key] = false
  }

  function toggleDrawer(key: string) {
    drawerStates.value[key] = !drawerStates.value[key]
  }

  function isDrawerOpen(key: string): boolean {
    return drawerStates.value[key] ?? false
  }

  return {
    // State
    toasts,
    modals,
    loadingStates,
    drawerStates,
    // Computed
    activeToasts,
    hasActiveModal,
    // Actions
    showToast,
    dismissToast,
    clearToasts,
    success,
    error,
    warning,
    info,
    openModal,
    closeModal,
    closeAllModals,
    setLoading,
    isLoading,
    openDrawer,
    closeDrawer,
    toggleDrawer,
    isDrawerOpen,
  }
})
