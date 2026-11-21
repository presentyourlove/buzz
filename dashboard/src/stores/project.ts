import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Project, Sprint } from '@/types'

export const useProjectStore = defineStore('project', () => {
  // State
  const projects = ref<Project[]>([])
  const currentProject = ref<Project | null>(null)
  const sprints = ref<Sprint[]>([])
  const currentSprint = ref<Sprint | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  // Computed
  const activeProjects = computed(() => projects.value.filter(p => p.isActive))
  const projectOptions = computed(() =>
    activeProjects.value.map(p => ({ label: p.name, value: p.id }))
  )
  const activeSprints = computed(() =>
    sprints.value.filter(s => s.status === 'active')
  )

  // Actions
  async function fetchProjects() {
    isLoading.value = true
    error.value = null
    try {
      // TODO: Replace with actual API call
      // const response = await api.getProjects()
      // projects.value = response.data
      projects.value = [
        {
          id: 'proj-1',
          name: 'Buzz 核心專案',
          slug: 'buzz-core',
          description: 'Buzz 協作平台核心開發',
          ownerId: 'user-1',
          createdAt: '2026-01-01T00:00:00Z',
          updatedAt: '2026-11-01T00:00:00Z',
          isActive: true,
        },
        {
          id: 'proj-2',
          name: '行動應用',
          slug: 'buzz-mobile',
          description: 'Buzz Flutter 行動應用',
          ownerId: 'user-1',
          createdAt: '2026-03-01T00:00:00Z',
          updatedAt: '2026-11-01T00:00:00Z',
          isActive: true,
        },
      ]
    } catch (e) {
      error.value = e instanceof Error ? e.message : '載入專案失敗'
    } finally {
      isLoading.value = false
    }
  }

  async function fetchSprints(projectId: string) {
    isLoading.value = true
    error.value = null
    try {
      // TODO: Replace with actual API call
      // const response = await api.getSprints(projectId)
      // sprints.value = response.data
      sprints.value = [
        {
          id: 'sprint-1',
          projectId,
          name: 'Sprint 1',
          goal: '專案初始化與架構',
          startDate: '2026-11-01T00:00:00Z',
          endDate: '2026-11-14T00:00:00Z',
          status: 'completed',
          tasks: [],
        },
        {
          id: 'sprint-2',
          projectId,
          name: 'Sprint 2',
          goal: '儀表板前端實作',
          startDate: '2026-11-15T00:00:00Z',
          endDate: '2026-11-28T00:00:00Z',
          status: 'active',
          tasks: ['DASH-001', 'DASH-002', 'DASH-003'],
        },
      ]
    } catch (e) {
      error.value = e instanceof Error ? e.message : '載入 Sprint 失敗'
    } finally {
      isLoading.value = false
    }
  }

  function setCurrentProject(project: Project | null) {
    currentProject.value = project
    if (project) {
      fetchSprints(project.id)
    }
  }

  function setCurrentSprint(sprint: Sprint | null) {
    currentSprint.value = sprint
  }

  function addProject(project: Project) {
    projects.value.push(project)
  }

  function updateProject(id: string, updates: Partial<Project>) {
    const idx = projects.value.findIndex(p => p.id === id)
    if (idx !== -1) {
      projects.value[idx] = { ...projects.value[idx], ...updates }
      if (currentProject.value?.id === id) {
        currentProject.value = projects.value[idx]
      }
    }
  }

  function removeProject(id: string) {
    projects.value = projects.value.filter(p => p.id !== id)
    if (currentProject.value?.id === id) {
      currentProject.value = null
    }
  }

  function clearError() {
    error.value = null
  }

  return {
    // State
    projects,
    currentProject,
    sprints,
    currentSprint,
    isLoading,
    error,
    // Computed
    activeProjects,
    projectOptions,
    activeSprints,
    // Actions
    fetchProjects,
    fetchSprints,
    setCurrentProject,
    setCurrentSprint,
    addProject,
    updateProject,
    removeProject,
    clearError,
  }
})
