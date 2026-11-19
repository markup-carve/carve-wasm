import init, { parse, render } from '../pkg/web/carve_wasm.js'

const ready = init()
self.onmessage = async ({ data: { id, action, source, options } }) => {
  try {
    await ready
    let value
    switch (action) {
      case 'parse':
        value = parse(source, options)
        break
      case 'render':
        value = render(source, options)
        break
      default:
        throw new Error('Unknown worker action')
    }
    self.postMessage({ id, value })
  } catch (error) {
    self.postMessage({ id, error: { name: error.name, message: error.message } })
  }
}
