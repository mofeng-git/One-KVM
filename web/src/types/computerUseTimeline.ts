import type { ComputerUseAction, ComputerUseScreenshot } from '@/api'

export type ComputerUseTimelineItem =
  | { id: string; type: 'user'; text: string }
  | { id: string; type: 'assistant'; text: string }
  | { id: string; type: 'screenshot'; screenshot: ComputerUseScreenshot }
  | { id: string; type: 'actions_executed'; actions: ComputerUseAction[] }
  | { id: string; type: 'error'; text: string }
  | { id: string; type: 'status'; text: string }

export type NewComputerUseTimelineItem = ComputerUseTimelineItem extends infer Item
  ? Item extends { id: string }
    ? Omit<Item, 'id'>
    : never
  : never
