import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const store = vi.hoisted(() => ({
  setPreferences: vi.fn(),
  state: {
    preferences: {
      appIconTheme: 'dark',
      gitControlPlacement: 'left',
      language: 'en',
      motionPreference: 'animated',
      terminalTheme: null,
      topbarStyle: 'classic',
      uiTheme: 'dark',
      uiZoom: 1,
      visualStyle: 'normal',
      windowOpacity: 1,
    },
    setPreferences: vi.fn(),
    setTerminalTheme: vi.fn(),
    setUiTheme: vi.fn(),
    setUiZoom: vi.fn(),
  },
}))

const platform = vi.hoisted(() => ({
  isWindows: vi.fn(() => false),
}))

vi.mock('../../../stores/projectsStore', () => ({
  UI_ZOOM_LIMITS: { min: 0.75, max: 1.5, step: 0.05 },
  useProjectsStore: (selector: (state: typeof store.state) => unknown) => selector(store.state),
}))

vi.mock('../../../lib/platform', () => ({
  isWindows: () => platform.isWindows(),
}))

import { AppearancePage } from './AppearancePage'

describe('AppearancePage motion preference', () => {
  beforeEach(() => {
    store.state.setPreferences.mockReset()
    platform.isWindows.mockReturnValue(false)
  })

  it('shows visual Animated and Reduced choices and stores the selected mode', () => {
    render(<AppearancePage />)

    const animated = screen.getByRole('button', { name: /Animated/ })
    const reduced = screen.getByRole('button', { name: /Reduced/ })
    expect(animated.getAttribute('aria-pressed')).toBe('true')
    expect(reduced.getAttribute('aria-pressed')).toBe('false')

    fireEvent.click(reduced)
    expect(store.state.setPreferences).toHaveBeenCalledWith({ motionPreference: 'reduced' })
  })
})

describe('AppearancePage window opacity', () => {
  beforeEach(() => {
    store.state.setPreferences.mockReset()
  })

  it('hides the opacity control outside Windows', () => {
    platform.isWindows.mockReturnValue(false)
    render(<AppearancePage />)
    expect(screen.queryByLabelText(/Window opacity/i)).toBeNull()
  })

  it('shows the opacity control on Windows', () => {
    platform.isWindows.mockReturnValue(true)
    render(<AppearancePage />)
    expect(screen.getByLabelText(/Window opacity/i)).toBeTruthy()
  })
})
