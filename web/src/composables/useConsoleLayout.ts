import { ref, watch } from 'vue'

export type ConsoleLayout = 'current' | 'floating' | 'sidebar'

const STORAGE_KEY = 'consoleLayout'
const VALID_LAYOUTS: ConsoleLayout[] = ['current', 'floating', 'sidebar']

function readStoredLayout(): ConsoleLayout {
  const stored = localStorage.getItem(STORAGE_KEY)
  return VALID_LAYOUTS.includes(stored as ConsoleLayout)
    ? stored as ConsoleLayout
    : 'current'
}

const consoleLayout = ref<ConsoleLayout>(readStoredLayout())

watch(consoleLayout, layout => {
  localStorage.setItem(STORAGE_KEY, layout)
})

export function useConsoleLayout() {
  function setConsoleLayout(layout: ConsoleLayout) {
    if (VALID_LAYOUTS.includes(layout)) {
      consoleLayout.value = layout
    }
  }

  return {
    consoleLayout,
    setConsoleLayout,
  }
}
