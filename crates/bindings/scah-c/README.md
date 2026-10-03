# scah C bindings

A C ABI for [scah](../../../README.md): CSS selectors meet streaming HTML
parsing. The header is [`include/scah.h`](include/scah.h); it documents every
function along with the ownership, error, and threading rules.

## Build and link

```bash
cargo build --release -p scah-c
```

This produces `target/release/libscah_c.a` and a shared library
(`libscah_c.so`, `libscah_c.dylib`, or `scah_c.dll`). Compile against
`crates/bindings/scah-c/include` and link with `-lscah_c`. On Linux, static
linking also needs `-lpthread -ldl -lm`.

## Example

```c
#include <stdio.h>
#include <string.h>
#include "scah.h"

static ScahStringView sv(const char *s) {
  ScahStringView view = {(const uint8_t *)s, strlen(s)};
  return view;
}

int main(void) {
  ScahSave save = {.text = true, .attributes = true};
  ScahQueryBuilder *builder = NULL;
  ScahQuery *query = NULL;
  ScahStore *store = NULL;
  ScahElementList *links = NULL;
  ScahError *err = NULL;

  scah_query_builder_new_all(sv("a[href]"), save, &builder, NULL);
  if (scah_query_build(builder, &query, &err) != SCAH_STATUS_OK) {
    ScahStringView message = scah_error_message(err);
    fprintf(stderr, "%.*s\n", (int)message.len, (const char *)message.data);
    return 1;
  }
  scah_query_builder_free(builder);

  const ScahQuery *queries[] = {query};
  scah_parse(sv("<a href='/one'>One</a><a href='/two'>Two</a>"), queries, 1,
             &store, NULL);
  scah_query_free(query);

  scah_store_get(store, sv("a[href]"), &links, NULL);
  const ScahElementId *ids = scah_element_list_ids(links);
  for (size_t i = 0; i < scah_element_list_len(links); i++) {
    ScahOptionalStringView link_text, href;
    scah_element_text(store, ids[i], &link_text, NULL);
    scah_element_attribute(store, ids[i], sv("href"), &href, NULL);
    printf("%.*s %.*s\n", (int)link_text.value.len,
           (const char *)link_text.value.data, (int)href.value.len,
           (const char *)href.value.data); /* One /one */
  }

  scah_element_list_free(links);
  scah_store_free(store);
  return 0;
}
```

Every handle is released with its `*_free` function, and every `*_free`
function accepts NULL. A store copies the HTML and shares its queries' selector strings,
so both may be freed right after `scah_parse`. String views borrow from the
store and stay valid until `scah_store_free`.

Run the C and C++ smoke tests with `just test-c`. After changing the ABI,
regenerate the header with `just header`; CI rejects a stale header.
