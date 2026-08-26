import { quickLogin } from '../support/onboardingFlow'
import { suppressWindowFocusTax } from '../support/perf'
import { snapshot } from '../support/uiKit'

/**
 * Ad-hoc exploration sandbox — NOT a regression test, it doesn't assert
 * anything about the app being right or wrong. It exists so I (Claude) can
 * quickly navigate a new screen via real click/typing, without
 * needing to write a dedicated helper in `projectUi.ts` every time I
 * just want to LOOK at what a screen does. Freely edit the body of `it()`
 * for each exploration — it isn't versioned as "the" test for anything
 * specific, it's rewritten as needed at the moment.
 *
 * Run: npx wdio run e2e/wdio.conf.ts --spec e2e/specs/_sandbox.spec.ts
 */
describe('sandbox: ad-hoc exploration', () => {
  before(async () => {
    await suppressWindowFocusTax()
    await quickLogin(`E2E Sandbox ${Date.now()}`)
  })

  it('checks whether the Home ascii background canvas actually renders anything', async () => {
    await new Promise((resolve) => setTimeout(resolve, 1500))
    await snapshot('home-after-login')

    const skipButton = await $('button*=Skip')
    if (await skipButton.isExisting()) {
      await skipButton.click()
    }

    await new Promise((resolve) => setTimeout(resolve, 1500))
    await snapshot('home-after-dismissing-welcome')

    const canvasStats = await browser.execute(() => {
      const canvases = Array.from(document.querySelectorAll('canvas'))
      return canvases.map((canvas) => {
        const ctx = canvas.getContext('2d')
        if (!ctx || canvas.width === 0 || canvas.height === 0) {
          return { width: canvas.width, height: canvas.height, nonEmptyPixels: null }
        }
        const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data
        let nonEmptyPixels = 0
        for (let i = 0; i < data.length; i += 4) {
          if (data[i + 3] > 0) nonEmptyPixels += 1
        }
        return { width: canvas.width, height: canvas.height, nonEmptyPixels }
      })
    })
    console.log('CANVAS_STATS:', JSON.stringify(canvasStats))

    const logs = (await browser.getLogs('browser').catch(() => [])) as Array<{
      level: string
      message: string
    }>
    const relevant = logs.filter((l) => /ascii-effect|Thor/i.test(l.message))
    console.log('CONSOLE_LOGS:', JSON.stringify(relevant))
  })
})
