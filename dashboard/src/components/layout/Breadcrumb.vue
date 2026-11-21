<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { ChevronRight, Home } from 'lucide-vue-next'

const route = useRoute()

const breadcrumbs = computed(() => {
  const matched = route.matched.filter(m => m.meta.title)
  return matched.map((match, idx) => ({
    title: match.meta.title as string,
    path: match.path,
    isLast: idx === matched.length - 1,
  }))
})
</script>

<template>
  <nav class="flex items-center gap-2 text-sm" aria-label="麵包屑導航" v-if="breadcrumbs.length > 0">
    <ol class="flex items-center gap-2">
      <li>
        <router-link
          to="/"
          class="flex items-center gap-1 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
        >
          <Home class="w-4 h-4" aria-hidden="true" />
          <span class="hidden sm:inline">首頁</span>
        </router-link>
      </li>
      <li v-for="(crumb, _) in breadcrumbs" :key="crumb.path" class="flex items-center gap-2">
        <ChevronRight class="w-4 h-4 text-gray-400 flex-shrink-0" aria-hidden="true" />
        <router-link
          v-if="!crumb.isLast"
          :to="crumb.path"
          class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
        >
          {{ crumb.title }}
        </router-link>
        <span v-else class="font-medium text-gray-900 dark:text-gray-100" aria-current="page">
          {{ crumb.title }}
        </span>
      </li>
    </ol>
  </nav>
</template>
