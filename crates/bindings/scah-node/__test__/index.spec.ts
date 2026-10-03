import { test, expect } from 'bun:test'

import { parse, Query } from '../index'

test('Basic selection', () => {
  const html = `
  <div>
    Hello World
    <a href="https://example.com">Example Website</a>
  </div>
  `
  const query = Query.all('div', { innerHtml: true, text: true })
    .all('a', { innerHtml: true, text: true })
    .build()
  const store = parse(html, [query])

  expect(store.length).toBe(2)

  expect(store.get('div')?.length).toBe(1)

  let div = store.get('div')?.at(0)
  expect(div?.toJson()).toEqual({
    name: 'div',
    class: undefined,
    id: undefined,
    attributes: {},
    innerHtml: `
    Hello World
    <a href="https://example.com">Example Website</a>
  `,
    rawText: undefined,
    text: 'Hello World Example Website',
  })

  let a = div?.get('a').at(0)

  expect(a?.toJson()).toEqual({
    name: 'a',
    class: undefined,
    id: undefined,
    attributes: { href: 'https://example.com' },
    innerHtml: `Example Website`,
    rawText: undefined,
    text: 'Example Website',
  })
})

test('Tree selection', () => {
  const html = `
  <section id="products">
    <div class="product">
      <h1>Product #1</h1>
      <img src="https://example.com/p1.png"/>
      <p>
        Hello World for Product #1
      </p>
    </div>
    <div class="product">
      <h1>Product #2</h1>
      <img src="https://example.com/p2.png"/>
      <p>
        Hello World for Product #2
      </p>
    </div>
  </section>
  `
  const query = Query.all('#products', { innerHtml: true, text: true })
    .all('.product', { innerHtml: true, text: true })
    .then((p) => [
      p.all('h1', { innerHtml: true, text: true }),
      p.all('img', { innerHtml: false, text: false }),
      p.all('p', { innerHtml: true, text: true }),
    ])
    .build()
  const store = parse(html, [query])

  expect(store.length).toBe(9)

  const products_section = store.get('#products')
  expect(products_section?.length).toBe(1)

  expect(products_section![0]?.name).toBe('section')
  expect(products_section![0]?.id).toBe('products')

  const products = products_section![0].get('.product')!

  expect(products[0].name).toBe('div')
  expect(products[0].className).toBe('product')

  const product1 = {
    h1: products[0].get('h1')[0],
    img: products[0].get('img')[0],
    p: products[0].get('p')[0],
  }
  expect(product1.h1.name).toBe('h1')
  expect(product1.h1.innerHtml).toBe('Product #1')
  expect(product1.h1.text).toBe('Product #1')

  expect(product1.img.name).toBe('img')
  expect(product1.img.attributes).toEqual({ src: 'https://example.com/p1.png' })

  expect(product1.p.name).toBe('p')
  expect(product1.p.text).toBe('Hello World for Product #1')

  expect(products[1].name).toBe('div')
  expect(products[1].className).toBe('product')

  const product2 = {
    h1: products[1].get('h1')[0],
    img: products[1].get('img')[0],
    p: products[1].get('p')[0],
  }

  expect(product2.h1.name).toBe('h1')
  expect(product2.h1.innerHtml).toBe('Product #2')
  expect(product2.h1.text).toBe('Product #2')

  expect(product2.img.name).toBe('img')
  expect(product2.img.attributes).toEqual({ src: 'https://example.com/p2.png' })

  expect(product2.p.name).toBe('p')
  expect(product2.p.text).toBe('Hello World for Product #2')
})

function generateHtml(count: number): string {
  let html = "<html><body><div id='content'>"

  for (let i = 0; i < count; i++) {
    // Added some entities (&lt;) and bold tags (<b>) to make text extraction work harder
    html += `<div class="article"><a href="/post/${i}"><b>Post</b> &lt;${i}&gt;</a></div>`
  }

  html += '</div></body></html>'
  return html
}
test('find 5_000 anchor tags', () => {
  const html = generateHtml(5000)
  const query = Query.all('a', {
    innerHtml: true,
    text: true,
  }).build()
  const store = parse(html, [query])

  const links = store.get('a')?.map((e) => e.toJson())

  const generated_links = Array.from({ length: 5000 }, (_, i) => ({
    name: 'a',
    id: undefined,
    class: undefined,
    attributes: { href: `/post/${i}` },
    innerHtml: `<b>Post</b> &lt;${i}&gt;`,
    rawText: undefined,
    text: `Post <${i}>`,
  }))

  expect(links).toEqual(generated_links)
})

test('Save defaults missing keys to false', () => {
  const html = `<div><span>Hello</span></div>`
  const query = Query.all('div', { text: true }).all('span', { innerHtml: true }).build()
  const store = parse(html, [query])

  const div = store.get('div')?.at(0)
  expect(div?.innerHtml).toBeNull()
  expect(div?.text).toBe('Hello')

  const span = div?.get('span').at(0)
  expect(span?.innerHtml).toBe('Hello')
  expect(span?.text).toBeNull()
})

test('Save defaults omitted object to false', () => {
  const html = `<div><span>Hello</span></div>`
  const query = Query.all('div').all('span').build()
  const store = parse(html, [query])

  const div = store.get('div')?.at(0)
  expect(div?.innerHtml).toBeNull()
  expect(div?.text).toBeNull()

  const span = div?.get('span').at(0)
  expect(span?.innerHtml).toBeNull()
  expect(span?.text).toBeNull()
})

test('Save can match attributes without retaining them', () => {
  const html = `<a id="hero" class="promoted" href="/post">Post</a>`
  const query = Query.all('a.promoted[href]', { attributes: false }).build()
  const store = parse(html, [query])

  const anchor = store.get('a.promoted[href]')?.at(0)
  expect(anchor?.name).toBe('a')
  expect(anchor?.attributes).toEqual({})
  expect(anchor?.id).toBeNull()
  expect(anchor?.className).toBeNull()
})

test("store remains valid after query object goes out of scope", () => {
  // Query tapes (selector strings) are owned by the query objects.
  // This test verifies that dropping the query does not invalidate
  // the store, because the store keeps its queries alive.
  const store = (() => {
    const q = Query.all("a[href]", { innerHtml: true, text: true }).build()
    return parse("<a href='x'>x</a>", [q])
  })()

  const hits = store.get("a[href]")
  expect(hits).toHaveLength(1)
  expect(hits![0].name).toBe("a")
  expect(hits![0].attributes).toEqual({ href: "x" })
})

test('rawText preserves entities while text decodes them', () => {
  const html = '<p>A&nbsp;&amp;&#x20;B</p>'
  const query = Query.all('p', { rawText: true, text: true }).build()
  const store = parse(html, [query])
  const p = store.get('p')?.at(0)
  expect(p?.rawText).toBe('A&nbsp;&amp;&#x20;B')
  expect(p?.text).toBe('A\u00a0& B')
})

test('requested empty text is empty string not null', () => {
  const html = '<div></div>'
  const query = Query.all('div', { rawText: true, text: true }).build()
  const store = parse(html, [query])
  const div = store.get('div')?.at(0)
  expect(div?.rawText).toBe('')
  expect(div?.text).toBe('')
})

test('save options map rawText and text independently', () => {
  const html = '<p>A&amp;B</p>'

  const rawOnly = parse(html, [Query.all('p', { rawText: true }).build()])
  expect(rawOnly.get('p')?.at(0)?.rawText).toBe('A&amp;B')
  expect(rawOnly.get('p')?.at(0)?.text).toBeNull()

  const textOnly = parse(html, [Query.all('p', { text: true }).build()])
  expect(textOnly.get('p')?.at(0)?.rawText).toBeNull()
  expect(textOnly.get('p')?.at(0)?.text).toBe('A&B')

  const both = parse(html, [Query.all('p', { rawText: true, text: true }).build()])
  expect(both.get('p')?.at(0)?.rawText).toBe('A&amp;B')
  expect(both.get('p')?.at(0)?.text).toBe('A&B')

  const omitted = parse(html, [Query.all('p', {}).build()])
  expect(omitted.get('p')?.at(0)?.rawText).toBeNull()
  expect(omitted.get('p')?.at(0)?.text).toBeNull()
})

test('legacy textContent still requests normalized text', () => {
  const html = '<p>A&nbsp;&amp; B</p>'
  const store = parse(html, [Query.all('p', { textContent: true }).build()])
  const p = store.get('p')?.at(0)

  expect(p?.rawText).toBeNull()
  expect(p?.text).toBe('A\u00a0& B')
  expect(p?.textContent).toBe('A\u00a0& B')
})

test('Save is an options interface, not a runtime helper export', () => {
  const scah = require('../index')
  expect(scah.Save).toBeUndefined()
})

test('multi-child then appends all branches', () => {
  const html = `<main><a href="1">one</a><span>s</span><p>p</p></main>`
  const query = Query.all('main', { text: true })
    .then((q) => [q.all('a', { text: true }), q.all('span', { text: true }), q.all('p', { text: true })])
    .build()
  const store = parse(html, [query])
  const main = store.get('main')!.at(0)!
  expect(main.get('a')[0].text).toBe('one')
  expect(main.get('span')[0].text).toBe('s')
  expect(main.get('p')[0].text).toBe('p')
})

test('build can be reused without consuming the builder', () => {
  const builder = Query.all('a', { text: true })
  const q1 = builder.build()
  const q2 = builder.build()
  const store1 = parse('<a>1</a>', [q1])
  const store2 = parse('<a>2</a>', [q2])
  expect(store1.get('a')![0].text).toBe('1')
  expect(store2.get('a')![0].text).toBe('2')
})

test('one query can be shared by several parses', () => {
  const q = Query.all('a', { text: true }).build()
  const stores = ['<a>1</a>', '<a>2</a>', '<a>3</a>'].map((html) => parse(html, [q]))
  expect(stores.map((s) => s.get('a')![0].text)).toEqual(['1', '2', '3'])
})

test('element remains valid after store is dropped', () => {
  const element = (() => {
    const q = Query.all('a', { text: true }).build()
    const store = parse('<a href="x">hi</a>', [q])
    return store.get('a')![0]
  })()
  Bun.gc(true)
  expect(element.name).toBe('a')
  expect(element.text).toBe('hi')
  expect(element.getAttribute('href')).toBe('x')
})

test('nested element remains valid after parent list is dropped', () => {
  const child = (() => {
    const q = Query.all('div', { text: true }).all('a', { text: true }).build()
    const store = parse('<div><a href="nested">n</a></div>', [q])
    const parents = store.get('div')!
    return parents[0]!.get('a')[0]!
  })()
  Bun.gc(true)
  expect(child.name).toBe('a')
  expect(child.getAttribute('href')).toBe('nested')
  expect(child.text).toBe('n')
})

test('JsonElement typing is exported', () => {
  const q = Query.all('a', { text: true }).build()
  const store = parse('<a id="x" class="c">hi</a>', [q])
  const json = store.get('a')![0]!.toJson()
  const typed: import('../index').JsonElement = json
  expect(typed.name).toBe('a')
  expect(typed.id).toBe('x')
  expect(typed.class).toBe('c')
  expect(typed.text).toBe('hi')
  expect(typed.attributes).toEqual({})
})

test('selective lookup cardinality', () => {
  const html =
    '<s1>x</s1>' +
    Array.from({ length: 10 }, () => '<s10>x</s10>').join('') +
    Array.from({ length: 100 }, () => '<a>x</a>').join('')
  const store = parse(html, [
    Query.all('s1', { text: true }).build(),
    Query.all('s10', { text: true }).build(),
    Query.all('a', { text: true }).build(),
  ])
  expect(store.get('s1')).toHaveLength(1)
  expect(store.get('s10')).toHaveLength(10)
  expect(store.get('a')).toHaveLength(100)
  expect(store.get('missing')).toBeNull()
})

test('parse with empty queries throws', () => {
  expect(() => parse('<a></a>', [])).toThrow(/parse requires at least one query/)
})

test('invalid selector fails at build', () => {
  expect(() => Query.all('').build()).toThrow()
  expect(() => Query.all('a[').build()).toThrow()
})

test('attributes preserve missing versus empty values', () => {
  const q = Query.all('input', { innerHtml: true, text: true }).build()
  const store = parse('<input disabled value="">', [q])
  const element = store.get('input')![0]
  const attributes = element.attributes as Record<string, string | null>
  // napi maps Option::None to null and Some("") to "".
  expect(attributes.disabled).toBeNull()
  expect(attributes.value).toBe('')
  expect(element.getAttribute('value')).toBe('')
  expect(element.getAttribute('missing')).toBeNull()
})

test('attributes materialize zero to many', () => {
  const q = Query.all('el', { innerHtml: true, text: true }).build()
  const attrs = (n: number) => Array.from({ length: n }, (_, i) => `k${i}="v${i}"`).join(' ')
  const expected = (n: number) => Object.fromEntries(Array.from({ length: n }, (_, i) => [`k${i}`, `v${i}`]))

  for (const n of [0, 1, 8, 9, 24]) {
    expect(parse(`<el ${attrs(n)}></el>`, [q]).get('el')![0].attributes).toEqual(expected(n))
  }
})

test('large result collection correctness', () => {
  const count = 10_000
  let html = ''
  for (let i = 0; i < count; i++) {
    html += `<a href="/${i}">x</a>`
  }
  const q = Query.all('a', { text: true }).build()
  const store = parse(html, [q])
  const hits = store.get('a')
  expect(hits).toHaveLength(count)
  expect(hits![0].getAttribute('href')).toBe('/0')
  expect(hits![count - 1].getAttribute('href')).toBe(`/${count - 1}`)
})
