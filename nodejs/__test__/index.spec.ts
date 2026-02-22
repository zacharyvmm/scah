import { test, expect } from 'bun:test'

import { parse, Query, Save } from '../index'

test('Basic selection', () => {
  const html = `
  <div>
    Hello World
    <a href="https://example.com">Example Website</a>
  </div>
  `;
  const query = Query.all('div', {innerHtml: true, textContent: true})
    .all('a', {innerHtml: true, textContent: true}).build();
  const store = parse(Buffer.from(html), [query]);

  expect(store.length).toBe(3);

  expect(store.get(0)?.children).toEqual([
    [Buffer.from('div'), [1]],
  ])

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
