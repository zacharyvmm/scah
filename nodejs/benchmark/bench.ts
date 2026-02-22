import { Bench } from 'tinybench'

import { parse, Query, Save } from '../index.js'

function generateHtml(count: number): string {
  let html = "<html><body><div id='content'>";
  
  for (let i = 0; i < count; i++) {
      // Added some entities (&lt;) and bold tags (<b>) to make text extraction work harder
      html += `<div class="article"><a href="/post/${i}"><b>Post</b> &lt;${i}&gt;</a></div>`;
  }
  
  html += "</div></body></html>";
  return html;
}

const QUERY = "a";
const HTML = generateHtml(5_000);

const b = new Bench({})

b.add('scah', () => {
  const query = Query.all(QUERY, {innerHtml: true, textContent: true}).build();

  const store = parse(Buffer.from(HTML), [query]);

  for (let i = 0; i < store.length; i++) {
    const _ = store.get(i);
  }
})

// b.add('JavaScript a + 100', () => {
//   add(10)
// })

await b.run()

console.table(b.table())