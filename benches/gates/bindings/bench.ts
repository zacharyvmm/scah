// Time scah's Node binding on fixed workloads and print the results as JSON.
//
// check-binding-performance.sh runs this script with Bun against a base and a
// candidate build of the binding. Every workload uses only API that both builds
// share. Usage: bun bench.ts <path to crates/bindings/scah-node/index.js>

const SAMPLES = 20

const { Query, parse } = await import(process.argv[2])

function linksHtml(count: number): string {
  let items = ''
  for (let i = 0; i < count; i++) {
    items += `<div class="article"><a id="post-${i}" class="link" href="/post/${i}"><b>Post</b> &lt;${i}&gt;</a></div>`
  }
  return `<html><body><div id='content'>${items}</div></body></html>`
}

function catalogHtml(count: number): string {
  let items = ''
  for (let i = 0; i < count; i++) {
    items += `<div class="product"><h1>Product #${i}</h1><span class="rating">${(i % 5) + 1}/5</span><p class="description">Description</p></div>`
  }
  return `<html><body><section id="products">${items}</section></body></html>`
}

function productQuery() {
  return Query.all('div.product')
    .then((product: any) => [
      product.first('h1', { text: true }),
      product.first('span.rating', { text: true }),
      product.first('p.description', { innerHtml: true }),
    ])
    .build()
}

function workloads(): Record<string, () => void> {
  const links = linksHtml(10_000)
  const catalog = catalogHtml(2_000)
  const linkQuery = Query.all('a', { innerHtml: true, text: true }).build()
  const firstQuery = Query.first('a', { innerHtml: true, text: true }).build()
  const nestedQuery = productQuery()

  const store = parse(links, [linkQuery])
  const anchors = store.get('a')
  const products = parse(catalog, [nestedQuery]).get('div.product')

  return {
    parse_all: () => {
      parse(links, [linkQuery]).get('a')
    },
    parse_first: () => {
      for (let i = 0; i < 100; i++) {
        parse(links, [firstQuery]).get('a')
      }
    },
    parse_nested: () => {
      for (const product of parse(catalog, [nestedQuery]).get('div.product')) {
        product.get('h1')[0].text
        product.get('span.rating')[0].text
      }
    },
    store_get: () => {
      for (let i = 0; i < 10; i++) {
        store.get('a')
      }
    },
    element_fields: () => {
      for (const anchor of anchors) {
        anchor.name
        anchor.id
        anchor.className
        anchor.getAttribute('href')
        anchor.innerHtml
        anchor.text
      }
    },
    element_attributes: () => {
      for (const anchor of anchors) {
        anchor.attributes
      }
    },
    element_get: () => {
      for (const product of products) {
        product.get('h1')
        product.get('span.rating')
      }
    },
    element_to_json: () => {
      for (const anchor of anchors) {
        anchor.toJson()
      }
    },
    build_query: () => {
      for (let i = 0; i < 1_000; i++) {
        productQuery()
      }
    },
  }
}

// Return the fastest of several timed runs, in nanoseconds.
function measure(workload: () => void): number {
  for (let i = 0; i < 3; i++) {
    workload()
  }
  let best = Infinity
  for (let i = 0; i < SAMPLES; i++) {
    Bun.gc(true)
    const start = Bun.nanoseconds()
    workload()
    best = Math.min(best, Bun.nanoseconds() - start)
  }
  return best
}

const results: Record<string, number> = {}
for (const [name, workload] of Object.entries(workloads())) {
  results[name] = measure(workload)
}
process.stdout.write(JSON.stringify(results))
