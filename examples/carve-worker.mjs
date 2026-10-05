import init, { ParserSession, toHtmlWithOptions } from '@markup-carve/carve-wasm/web'

const ready = init()
let session
self.onmessage = async ({ data: { id, action, source, changes } }) => {
  try {
    await ready
    let value
    switch (action) {
      case 'open': {
        const next = new ParserSession(source)
        session?.free()
        session = next
        value = JSON.parse(session.snapshot())
        break
      }
      case 'edit':
        if (!session) throw new Error('Open a document before editing')
        value = JSON.parse(session.edit(JSON.stringify(changes)))
        break
      case 'render':
        value = toHtmlWithOptions(source, { rawHtml: false, profile: 'comment' })
        break
      case 'close':
        session?.free()
        session = undefined
        value = null
        break
      default: throw new Error('Unknown worker action')
    }
    self.postMessage({ id, value })
  } catch (error) {
    self.postMessage({ id, error: { name: error.name, message: error.message } })
  }
}
