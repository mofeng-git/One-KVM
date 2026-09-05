import { inject, provide, type InjectionKey, type Ref } from 'vue'
import type { ConsoleLayout } from '@/composables/useConsoleLayout'

// Injection follows component ownership through portals, without styling settings/login.
const consoleAppearanceKey: InjectionKey<Readonly<Ref<ConsoleLayout>>> = Symbol('consoleAppearance')

export function provideConsoleAppearance(layout: Readonly<Ref<ConsoleLayout>>) {
  provide(consoleAppearanceKey, layout)
}

export function useConsoleAppearance() {
  return inject(consoleAppearanceKey, undefined)
}

// Focus the panel itself so the first help tooltip does not obscure its controls on open.
// Keyboard users can then Tab into the panel's controls in their normal order.
export function focusConsolePanel(event: Event) {
  if (event.target instanceof HTMLElement) {
    event.preventDefault()
    event.target.focus({ preventScroll: true })
  }
}
