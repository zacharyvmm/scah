import { test, expect } from 'bun:test'

import { parse, Query } from '../index'

test('Basic selection', () => {
  const html = `
  <div>
    Hello World
    <a href="https://example.com">Example Website</a>
  </div>
  `
  const query = Query.all('div', { innerHtml: true, textContent: true })
    .all('a', { innerHtml: true, textContent: true })
    .build()
  const store = parse(Buffer.from(html), [query])

  expect(store.length).toBe(3)

  expect(store.get(0)?.children).toEqual([[Buffer.from('div'), [1]]])

  expect(store.get(1)).toEqual({
    name: Buffer.from('div'),
    class: undefined,
    id: undefined,
    attributes: [],
    innerHtml: Buffer.from(`
    Hello World
    <a href="https://example.com">Example Website</a>
  `),
    textContent: Buffer.from('Hello World Example Website'),
    children: [[Buffer.from('a'), [2]]],
  })

  expect(store.get(2)).toEqual({
    name: Buffer.from('a'),
    class: undefined,
    id: undefined,
    attributes: [[Buffer.from('href'), Buffer.from('https://example.com')]],
    innerHtml: Buffer.from(`Example Website`),
    textContent: Buffer.from('Example Website'),
    children: [],
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
  </section>
  `
  const query = Query.all('#products', { innerHtml: true, textContent: true })
    .all('.product', { innerHtml: true, textContent: true })
    .then((p) => [
      p.all('h1', { innerHtml: true, textContent: true }),
      p.all('img', { innerHtml: false, textContent: false }),
      p.all('p', { innerHtml: true, textContent: true }),
    ])
    .build()
  const store = parse(Buffer.from(html), [query])

  // for (let i = 0; i < store.length; i++) {
  //   const element = store.get(i);
  //   console.log(`#${i}`);
  //   console.log("Name: ", element?.name.toString());
  //   console.log("id: ", element?.id? element.id.toString() : undefined);
  //   console.log("class: ", element?.class? element.class.toString() : undefined);
  //   console.log("inner_html: ", element?.innerHtml? element.innerHtml.toString() : undefined);
  //   console.log("text_content: ", element?.textContent? element.textContent.toString() : undefined);
  //   console.log();
  // }

  expect(store.length).toBe(6)
})
