/* Smoke test for the scah C API. Built by scripts/smoke-test.sh. */

#include "scah.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(cond)                                                            \
  do {                                                                         \
    if (!(cond)) {                                                             \
      fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #cond); \
      exit(1);                                                                 \
    }                                                                          \
  } while (0)

/* Run `call`, which may refer to `&err`, and fail unless it succeeds. */
#define OK(call)                                                               \
  do {                                                                         \
    ScahError *err = NULL;                                                     \
    ScahStatus status_ = (call);                                               \
    expect_ok(status_, err, __LINE__);                                         \
  } while (0)

static void expect_ok(ScahStatus status, ScahError *err, int line) {
  if (status != SCAH_STATUS_OK) {
    ScahStringView message = scah_error_message(err);
    fprintf(stderr, "%s:%d: status %d: %.*s\n", __FILE__, line, (int)status,
            (int)message.len, (const char *)message.data);
    exit(1);
  }
  CHECK(err == NULL);
}

static ScahStringView sv(const char *s) {
  ScahStringView view;
  view.data = (const uint8_t *)s;
  view.len = strlen(s);
  return view;
}

static int view_eq(ScahStringView view, const char *expected) {
  size_t len = strlen(expected);
  return view.len == len && (len == 0 || memcmp(view.data, expected, len) == 0);
}

static int some_eq(ScahOptionalStringView view, const char *expected) {
  return view.is_some && view_eq(view.value, expected);
}

static int view_starts_with(ScahStringView view, const char *prefix) {
  size_t len = strlen(prefix);
  return view.len >= len && memcmp(view.data, prefix, len) == 0;
}

static const ScahSave SAVE_ALL = {true, true, true, true};
static const ScahSave SAVE_TEXT = {false, false, true, true};

static const char HTML[] =
    "<main>"
    "<section id='intro' class='lead'>"
    "<h2>Intro</h2>"
    "<a href='/one' data-empty=''>One &amp; only</a>"
    "<a href='/two' disabled>  Two  </a>"
    "</section>"
    "<section><a href='/three'>Three</a></section>"
    "<p class='note'>  Hello <b>world</b>  </p>"
    "</main>";

/* section -> [first("> h2"), all("a")], plus first("p.note"). */
static void build_queries(ScahQuery **sections, ScahQuery **note) {
  ScahQueryBuilder *section = NULL;
  ScahQueryBuilder *heading = NULL;
  ScahQueryBuilder *links = NULL;
  ScahQueryBuilder *paragraph = NULL;

  OK(scah_query_builder_new_all(sv("section"), SAVE_ALL, &section, &err));
  OK(scah_query_builder_new_first(sv("> h2"), SAVE_ALL, &heading, &err));
  OK(scah_query_builder_new_all(sv("a"), SAVE_ALL, &links, &err));
  {
    const ScahQueryBuilder *children[2];
    children[0] = heading;
    children[1] = links;
    OK(scah_query_builder_then(section, children, 2, &err));
  }
  /* Children are copied, so they can be freed before building. */
  scah_query_builder_free(heading);
  scah_query_builder_free(links);
  OK(scah_query_build(section, sections, &err));
  scah_query_builder_free(section);

  OK(scah_query_builder_new_first(sv("p.note"), SAVE_TEXT, &paragraph, &err));
  OK(scah_query_build(paragraph, note, &err));
  scah_query_builder_free(paragraph);
}

static void check_invalid_selector(void) {
  ScahQueryBuilder *builder = NULL;
  ScahQuery *query = NULL;
  ScahError *err = NULL;

  OK(scah_query_builder_new_all(sv("main"), SAVE_ALL, &builder, &err));
  /* Selectors are only compiled by scah_query_build. */
  OK(scah_query_builder_all(builder, sv("a["), SAVE_ALL, &err));
  CHECK(scah_query_build(builder, &query, &err) ==
        SCAH_STATUS_INVALID_SELECTOR);
  CHECK(query == NULL);
  CHECK(err != NULL);
  CHECK(view_starts_with(scah_error_message(err), "invalid selector: "));
  scah_error_free(err);

  /* out_error may be NULL. */
  CHECK(scah_query_build(builder, &query, NULL) ==
        SCAH_STATUS_INVALID_SELECTOR);
  scah_query_builder_free(builder);
}

static void check_parse_errors(ScahQuery *query) {
  const ScahQuery *queries[1];
  ScahStore *store = NULL;
  ScahError *err = NULL;
  ScahStringView bad;
  static const uint8_t bad_bytes[] = {'<', 'a', '>', 0xff};

  queries[0] = query;
  CHECK(scah_parse(sv("<a></a>"), NULL, 0, &store, &err) ==
        SCAH_STATUS_EMPTY_QUERIES);
  CHECK(store == NULL);
  scah_error_free(err);

  bad.data = bad_bytes;
  bad.len = sizeof bad_bytes;
  CHECK(scah_parse(bad, queries, 1, &store, &err) == SCAH_STATUS_INVALID_UTF8);
  CHECK(store == NULL);
  scah_error_free(err);

  CHECK(scah_parse(sv("<a></a>"), queries, 1, NULL, NULL) ==
        SCAH_STATUS_NULL_POINTER);
}

static ScahElementList *get(const ScahStore *store, const char *selector) {
  ScahElementList *list = NULL;
  OK(scah_store_get(store, sv(selector), &list, &err));
  return list;
}

static ScahElementList *get_nested(const ScahStore *store, ScahElementId id,
                                   const char *selector) {
  ScahElementList *list = NULL;
  OK(scah_element_get(store, id, sv(selector), &list, &err));
  return list;
}

static ScahOptionalStringView attribute(const ScahStore *store,
                                        ScahElementId id, const char *key) {
  ScahOptionalStringView value;
  OK(scah_element_attribute(store, id, sv(key), &value, &err));
  return value;
}

static void check_sections(const ScahStore *store) {
  ScahElementList *sections = get(store, "section");
  const ScahElementId *ids = scah_element_list_ids(sections);
  ScahElementList *headings;
  ScahStringView name;
  ScahOptionalStringView value;

  CHECK(scah_element_list_len(sections) == 2);

  OK(scah_element_name(store, ids[0], &name, &err));
  CHECK(view_eq(name, "section"));
  OK(scah_element_id(store, ids[0], &value, &err));
  CHECK(some_eq(value, "intro"));
  OK(scah_element_class_name(store, ids[0], &value, &err));
  CHECK(some_eq(value, "lead"));
  OK(scah_element_id(store, ids[1], &value, &err));
  CHECK(!value.is_some);

  headings = get_nested(store, ids[0], "> h2");
  CHECK(scah_element_list_len(headings) == 1);
  OK(scah_element_text(store, scah_element_list_ids(headings)[0], &value,
                       &err));
  CHECK(some_eq(value, "Intro"));
  scah_element_list_free(headings);

  /* The second section has no heading: success with a NULL list. */
  CHECK(get_nested(store, ids[1], "> h2") == NULL);
  CHECK(scah_element_list_len(NULL) == 0);
  CHECK(scah_element_list_ids(NULL) == NULL);

  scah_element_list_free(sections);
}

static void check_links(const ScahStore *store) {
  ScahElementList *sections = get(store, "section");
  ScahElementList *links =
      get_nested(store, scah_element_list_ids(sections)[0], "a");
  const ScahElementId *ids = scah_element_list_ids(links);
  ScahOptionalStringView value;
  ScahStringView key;
  size_t count = 0;

  CHECK(scah_element_list_len(links) == 2);

  CHECK(some_eq(attribute(store, ids[0], "href"), "/one"));
  CHECK(some_eq(attribute(store, ids[0], "HREF"), "/one"));
  /* Empty and missing are different. */
  value = attribute(store, ids[0], "data-empty");
  CHECK(value.is_some && value.value.len == 0);
  CHECK(!attribute(store, ids[0], "missing").is_some);

  OK(scah_element_text(store, ids[0], &value, &err));
  CHECK(some_eq(value, "One & only"));
  OK(scah_element_raw_text(store, ids[0], &value, &err));
  CHECK(some_eq(value, "One &amp; only"));
  OK(scah_element_inner_html(store, ids[0], &value, &err));
  CHECK(some_eq(value, "One &amp; only"));
  OK(scah_element_text(store, ids[1], &value, &err));
  CHECK(some_eq(value, "Two"));
  OK(scah_element_raw_text(store, ids[1], &value, &err));
  CHECK(some_eq(value, "  Two  "));

  /* Iteration tells a valueless attribute from a missing one. */
  OK(scah_element_attribute_count(store, ids[1], &count, &err));
  CHECK(count == 2);
  OK(scah_element_attribute_at(store, ids[1], 0, &key, &value, &err));
  CHECK(view_eq(key, "href") && some_eq(value, "/two"));
  OK(scah_element_attribute_at(store, ids[1], 1, &key, &value, &err));
  CHECK(view_eq(key, "disabled") && !value.is_some);
  CHECK(!attribute(store, ids[1], "disabled").is_some);

  scah_element_list_free(links);
  scah_element_list_free(sections);
}

static void check_note(const ScahStore *store) {
  ScahElementList *notes = get(store, "p.note");
  ScahElementId id;
  ScahOptionalStringView value;

  CHECK(scah_element_list_len(notes) == 1);
  id = scah_element_list_ids(notes)[0];
  OK(scah_element_text(store, id, &value, &err));
  CHECK(some_eq(value, "Hello world"));
  /* Not requested by the query's save options. */
  OK(scah_element_raw_text(store, id, &value, &err));
  CHECK(!value.is_some);
  OK(scah_element_inner_html(store, id, &value, &err));
  CHECK(!value.is_some);
  scah_element_list_free(notes);
}

static void check_lookup_errors(const ScahStore *store) {
  ScahOptionalStringView value;
  ScahElementList *list = NULL;
  ScahError *err = NULL;
  size_t count = 0;

  /* Unknown selectors are not errors. */
  CHECK(get(store, "nope") == NULL);
  CHECK(get(store, "h2") == NULL);

  CHECK(scah_element_text(store, scah_store_len(store), &value, &err) ==
        SCAH_STATUS_INDEX_OUT_OF_BOUNDS);
  CHECK(view_starts_with(scah_error_message(err), "element id "));
  scah_error_free(err);

  OK(scah_element_attribute_count(store, 0, &count, &err));
  CHECK(scah_element_attribute_at(store, 0, count, NULL, &value, &err) ==
        SCAH_STATUS_NULL_POINTER);
  scah_error_free(err);

  CHECK(scah_store_get(NULL, sv("section"), &list, &err) ==
        SCAH_STATUS_NULL_POINTER);
  CHECK(list == NULL);
  CHECK(view_eq(scah_error_message(err), "`store` must not be NULL"));
  scah_error_free(err);
}

int main(void) {
  ScahQuery *sections = NULL;
  ScahQuery *note = NULL;
  const ScahQuery *queries[2];
  ScahStore *store = NULL;
  char *html;

  CHECK(scah_abi_version() == SCAH_ABI_VERSION);

  check_invalid_selector();
  build_queries(&sections, &note);
  check_parse_errors(note);

  /* The store copies the HTML and keeps the queries alive. */
  html = malloc(sizeof HTML);
  CHECK(html != NULL);
  memcpy(html, HTML, sizeof HTML);
  queries[0] = sections;
  queries[1] = note;
  OK(scah_parse(sv(html), queries, 2, &store, &err));
  memset(html, 0, sizeof HTML);
  free(html);
  scah_query_free(sections);
  scah_query_free(note);

  CHECK(scah_store_len(store) > 0);
  check_sections(store);
  check_links(store);
  check_note(store);
  check_lookup_errors(store);
  scah_store_free(store);

  scah_store_free(NULL);
  scah_query_free(NULL);
  scah_query_builder_free(NULL);
  scah_element_list_free(NULL);
  scah_error_free(NULL);

  puts("C smoke test passed");
  return 0;
}
