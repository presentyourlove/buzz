<script setup lang="ts">
import { computed } from 'vue'
import Button from '../common/Button.vue'
import { ChevronLeft } from 'lucide-vue-next'

interface Props {
  title: string
  description?: string
  action?: {
    label: string
    onClick: () => void
    icon?: string
  }
  breadcrumbs?: Array<{ label: string; href?: string }>
}

const props = defineProps<Props>()

const breadcrumbItems = computed(() => {
  if (!props.breadcrumbs) return []
  return props.breadcrumbs.map((item, index) => ({
    ...item,
    isLast: index === props.breadcrumbs!.length - 1,
  }))
})
</script>

<template>
  <div class="mb-6">
    <nav v-if="breadcrumbs && breadcrumbs.length > 0" class="mb-4" aria-label="麵包屑導航">
      <ol class="flex items-center gap-2 text-sm">
        <li v-for="(item, index) in breadcrumbItems" :key="index" class="flex items-center gap-2">
          <ChevronLeft v-if="index > 0" class="w-4 h-4 text-gray-400" aria-hidden="true" />
          <a
            v-if="item.href && !item.isLast"
            :href="item.href"
            class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
          >
            {{ item.label }}
          </a>
          <span
            v-else
            :class="[
              'font-medium',
              item.isLast ? 'text-gray-900 dark:text-gray-100' : 'text-gray-500 dark:text-gray-400',
            ]"
            :aria-current="item.isLast ? 'page' : undefined"
          >
            {{ item.label }}
          </span>
        </li>
      </ol>
    </nav>

    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-gray-100">{{ title }}</h1>
        <p v-if="description" class="mt-1 text-gray-600 dark:text-gray-400">{{ description }}</p>
      </div>
      <div v-if="action" class="flex-shrink-0">
        <Button :icon="action.icon" @click="action.onClick" variant="primary">
          {{ action.label }}
        </Button>
      </div>
    </div>
  </div>
</template>
