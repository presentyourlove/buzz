import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import type { App } from 'vue'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'Dashboard',
    component: () => import('@/views/DashboardView.vue'),
    meta: {
      title: '監控面板',
      requiresAuth: false,
    },
  },
  {
    path: '/org-chart',
    name: 'OrgChart',
    component: () => import('@/views/OrgChartView.vue'),
    meta: {
      title: '組織圖',
      requiresAuth: false,
    },
  },
  {
    path: '/kanban',
    name: 'Kanban',
    component: () => import('@/views/KanbanView.vue'),
    meta: {
      title: '任務看板',
      requiresAuth: false,
    },
  },
  {
    path: '/settings',
    name: 'Settings',
    component: () => import('@/views/SettingsView.vue'),
    meta: {
      title: '設定',
      requiresAuth: false,
    },
  },
  {
    path: '/:pathMatch(.*)*',
    name: 'NotFound',
    component: () => import('@/views/NotFoundView.vue'),
    meta: {
      title: '頁面不存在',
      requiresAuth: false,
    },
  },
]

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
  scrollBehavior(_to, _from, savedPosition) {
    if (savedPosition) {
      return savedPosition
    }
    return { top: 0 }
  },
})

// 全域前置守衛 - 頁面標題更新
router.beforeEach((to, _from, next) => {
  const baseTitle = 'Buzz 監控面板'
  document.title = to.meta.title ? `${to.meta.title} - ${baseTitle}` : baseTitle
  next()
})

export function setupRouter(app: App) {
  app.use(router)
}

export default router
