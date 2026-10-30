// Train scah's Node binding for profile-guided optimization.
//
// build.sh runs this with Node against an instrumented build of the binding.
// It parses page.html, and a large document made of its repeated content,
// with each query in queries.json alone and with all of them at once. Then it
// reads every field of every result.
//
// Bun is not used here: it exits without running the exit handlers that write
// the profile. Usage: node train.mjs <path to crates/bindings/scah-node/index.js>

import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { join } from 'node:path'

const SMALL_ROUNDS = 150
const LARGE_ROUNDS = 4
const BUILD_ROUNDS = 100

const { Query, parse } = createRequire(import.meta.url)(process.argv[2])

function save(spec) {
  return {
    innerHtml: spec.inner_html ?? false,
    text: spec.text ?? false,
    attributes: spec.attributes ?? true,
    rawText: spec.raw_text ?? false,
  }
}

function selector(spec) {
  return spec.all ?? spec.first
}

function builder(spec, factory = Query) {
  const start = spec.all !== undefined ? factory.all : factory.first
  const query = start.call(factory, selector(spec), save(spec.save))
  const children = spec.then
  return children ? query.then((child) => children.map((c) => builder(c, child))) : query
}

function read(elements, spec) {
  for (const element of elements ?? []) {
    void [element.name, element.id, element.className, element.attributes, element.getAttribute('href')]
    void [element.innerHtml, element.rawText, element.text, element.toJson()]
    for (const child of spec.then ?? []) {
      try {
        read(element.get(selector(child)), child)
      } catch {
        // No results under this element.
      }
    }
  }
}

function run(html, queries, specs) {
  queries.forEach((query, index) => read(parse(html, [query]).query(0), specs[index]))
  const store = parse(html, queries)
  specs.forEach((spec, index) => {
    read(store.query(index), spec)
    read(store.get(selector(spec)), spec)
  })
}

function largeDocument(page) {
  const [head, rest] = page.split('<main id="content">')
  const [content, tail] = rest.split('</main>')
  return `${head}<main id="content">${content.repeat(50)}</main>${tail}`
}

const page = readFileSync(join(import.meta.dirname, 'page.html'), 'utf8')
const specs = JSON.parse(readFileSync(join(import.meta.dirname, 'queries.json'), 'utf8'))
const queries = specs.map((spec) => builder(spec).build())

for (let i = 0; i < BUILD_ROUNDS; i++) specs.forEach((spec) => builder(spec).build())
for (let i = 0; i < SMALL_ROUNDS; i++) run(page, queries, specs)
const large = largeDocument(page)
for (let i = 0; i < LARGE_ROUNDS; i++) run(large, queries, specs)
