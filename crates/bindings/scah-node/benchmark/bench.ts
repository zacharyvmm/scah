import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

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
const BENCHMARK_NAME = 'node-parse-query'
const DEFAULT_JSON_OUTPUT = './benchmark/results/synthethic.json'
const DEFAULT_IMAGE_OUTPUT = './benchmark/images/synthethic.png'

type CliOptions = {
  jsonOutput?: string
  imageOutput?: string
}

type MitataRun = {
  name: string
  stats?: {
    avg: number
    samples?: number[]
  }
}

type MitataBenchmark = {
  alias: string
  runs: MitataRun[]
}

function parseCliArgs(argv: string[]): CliOptions {
  const options: CliOptions = {}

  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index]

    if (arg === '--json') {
      const next = argv[index + 1]
      if (next && !next.startsWith('--')) {
        options.jsonOutput = next
        index++
      } else {
        options.jsonOutput = DEFAULT_JSON_OUTPUT
      }
    }

    if (arg === '--image') {
      const next = argv[index + 1]
      if (next && !next.startsWith('--')) {
        options.imageOutput = next
        index++
      } else {
        options.imageOutput = DEFAULT_IMAGE_OUTPUT
      }
    }
  }

  return options
}

function mean(values: number[]): number {
  return values.reduce((total, value) => total + value, 0) / values.length
}

function standardDeviation(values: number[]): number {
  if (values.length < 2) {
    return 0
  }

  const average = mean(values)
  const variance = values.reduce((total, value) => total + (value - average) ** 2, 0) / values.length
  return Math.sqrt(variance)
}

function toPytestBenchmarkJson(benchmarks: MitataBenchmark[]) {
  return {
    machine_info: {
      runtime: 'bun',
    },
    bench_name: BENCHMARK_NAME,
    benchmarks: benchmarks.flatMap((benchmark) =>
      benchmark.runs.map((run) => {
        const samples = run.stats?.samples ?? []
        const meanNanoseconds = run.stats?.avg ?? 0
        const stddevNanoseconds = standardDeviation(samples)

        return {
          group: 'Node parse + query',
          name: run.name,
          fullname: `${benchmark.alias}::${run.name}`,
          stats: {
            mean: meanNanoseconds / 1_000_000_000,
            stddev: stddevNanoseconds / 1_000_000_000,
            rounds: samples.length,
          },
        }
      }),
    ),
  }
}

function writeJsonOutput(outputPath: string, benchmarks: MitataBenchmark[]) {
  const absolutePath = resolve(outputPath)
  mkdirSync(dirname(absolutePath), { recursive: true })
  writeFileSync(absolutePath, JSON.stringify(toPytestBenchmarkJson(benchmarks), null, 2))
  console.log(`Benchmark JSON saved to ${absolutePath}`)
  return absolutePath
}

function renderImageFromJson(jsonPath: string, imagePath: string) {
  const absoluteImagePath = resolve(imagePath)
  mkdirSync(dirname(absoluteImagePath), { recursive: true })

  const figureScript = resolve('../scah-python/benches/utils/figure.py')
  const env = {
    ...process.env,
    MPLCONFIGDIR: process.env.MPLCONFIGDIR ?? '/tmp/matplotlib',
    UV_CACHE_DIR: process.env.UV_CACHE_DIR ?? '/tmp/uv-cache',
  }
  const pythonBindingRoot = resolve('../scah-python')

  const commands: Array<[string, string[]]> = [
    [
      'uv',
      [
        'run',
        '--directory',
        pythonBindingRoot,
        '--all-extras',
        'python3',
        './benches/utils/figure.py',
        jsonPath,
        '-o',
        absoluteImagePath,
      ],
    ],
    ['python3', [figureScript, jsonPath, '-o', absoluteImagePath]],
  ]

  for (const [command, args] of commands) {
    const result = spawnSync(command, args, { env, stdio: 'inherit' })
    if (result.status === 0) {
      console.log(`Benchmark image saved to ${absoluteImagePath}`)
      return
    }

    if (result.error && 'code' in result.error && result.error.code === 'ENOENT') {
      continue
    }
  }

  throw new Error(`Failed to render benchmark image via ${figureScript}`)
}

group('parse + query', () => {
  bench('scah', () => {
    const query = Query.all(QUERY, { innerHtml: true, textContent: true }).build()
    const store = parse(HTML, [query])

    for (const element of store.get(QUERY)!) {
      const _inner = element?.innerHtml
      const _text = element?.textContent
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

const options = parseCliArgs(process.argv.slice(2))

if (!options.jsonOutput && !options.imageOutput) {
  await run()
} else {
  const result = await run({
    format: 'quiet',
  })

  const benchmarks = result.benchmarks as MitataBenchmark[]
  const jsonPath = writeJsonOutput(options.jsonOutput ?? DEFAULT_JSON_OUTPUT, benchmarks)

  if (options.imageOutput) {
    renderImageFromJson(jsonPath, options.imageOutput)
  }
}
