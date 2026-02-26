import { bench, group, run } from 'mitata'

import { parse, Query } from '../index.js'
import * as cheerio from 'cheerio'
import { parse as nhpParse } from 'node-html-parser'
import { JSDOM } from 'jsdom'
import { parseHTML as linkedomParse } from 'linkedom'
import { Window as HappyWindow } from 'happy-dom'

function generateHtml(count: number): string {
  let html = "<html><body><div id='content'>"

  for (let i = 0; i < count; i++) {
    // Added some entities (&lt;) and bold tags (<b>) to make text extraction work harder
    html += `<div class="article"><a href="/post/${i}"><b>Post</b> &lt;${i}&gt;</a></div>`
  }

  html += '</div></body></html>'
  return html
}

const QUERY = 'a'
const HTML = generateHtml(5_000)

group('parse + query', () => {
  bench('scah', () => {
    const query = Query.all(QUERY, { innerHtml: true, textContent: true }).build()
    const store = parse(HTML, [query])

    for (let i = 0; i < store.length; i++) {
      const e = store.get(i)
      const _inner = e?.innerHtml
      const _text = e?.textContent
    }
  })

  bench('cheerio', () => {
    const $ = cheerio.load(HTML)
    $(QUERY).each((_, el) => {
      const _inner = $(el).html()
      const _text = $(el).text()
    })
  })

  bench('jsdom', () => {
    const dom = new JSDOM(HTML)
    const els = dom.window.document.querySelectorAll(QUERY)

    for (const el of els) {
      const _inner = el.innerHTML
      const _text = el.textContent
    }
  })

  bench('node-html-parser', () => {
    const root = nhpParse(HTML)
    const els = root.querySelectorAll(QUERY)

    for (const el of els) {
      const _inner = el.innerHTML
      const _text = el.textContent
    }
  })

  bench('linkedom', () => {
    const { document } = linkedomParse(HTML)
    const els = document.querySelectorAll(QUERY)

    for (const el of els) {
      const _inner = el.innerHTML
      const _text = el.textContent
    }
  })

  // happy-dom's querySelectorAll crashes under Bun, so use getElementsByTagName
  bench('happy-dom', () => {
    const window = new HappyWindow()
    window.document.write(HTML)
    const els = window.document.getElementsByTagName(QUERY)

    for (const el of els) {
      const _inner = el.innerHTML
      const _text = el.textContent
    }
    window.close()
  })
})

await run()
