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

/* Element ids the Arrow export of the section links should hold. */
typedef struct {
  ScahElementId sections[2];
  ScahElementId links[3];
} LinkIds;

static LinkIds link_ids(const ScahStore *store) {
  ScahElementList *sections = get(store, "section");
  LinkIds ids;
  size_t row = 0;
  size_t i, j;

  CHECK(scah_element_list_len(sections) == 2);
  for (i = 0; i < 2; i++) {
    ScahElementList *links;
    ids.sections[i] = scah_element_list_ids(sections)[i];
    links = get_nested(store, ids.sections[i], "a");
    for (j = 0; j < scah_element_list_len(links); j++) {
      CHECK(row < 3);
      ids.links[row++] = scah_element_list_ids(links)[j];
    }
    scah_element_list_free(links);
  }
  CHECK(row == 3);
  scah_element_list_free(sections);
  return ids;
}

/* Export the links of every section with a few attribute columns. */
static void export_links(const ScahStore *store, struct ArrowSchema *schema,
                         struct ArrowArray *array) {
  ScahStringView attributes[3];
  ScahStringView no_parent;
  ScahError *err = NULL;

  attributes[0] = sv("href");
  attributes[1] = sv("disabled");
  attributes[2] = sv("data-empty");
  OK(scah_store_export_arrow(store, sv("a"), sv("section"), attributes, 3,
                             schema, array, &err));

  /* Requesting a fixed column name fails and leaves both outputs released. */
  {
    struct ArrowSchema bad_schema;
    struct ArrowArray bad_array;
    attributes[0] = sv("text");
    no_parent.data = NULL;
    no_parent.len = 0;
    CHECK(scah_store_export_arrow(store, sv("a"), no_parent, attributes, 1,
                                  &bad_schema, &bad_array,
                                  &err) == SCAH_STATUS_INVALID_ARGUMENT);
    CHECK(bad_schema.release == NULL && bad_array.release == NULL);
    CHECK(view_eq(scah_error_message(err), "duplicate column name `text`"));
    scah_error_free(err);
  }
}

static const struct ArrowArray *column(const struct ArrowArray *array,
                                       int64_t index) {
  return array->children[index];
}

static int is_valid(const struct ArrowArray *array, int64_t row) {
  const uint8_t *validity = (const uint8_t *)array->buffers[0];
  return validity == NULL || ((validity[row / 8] >> (row % 8)) & 1);
}

/* Read one Utf8View value; returns 0 for null. Records whether the value was
 * inlined in its view and, if not, which data buffer holds it. */
static int view_at(const struct ArrowArray *array, int64_t row,
                   ScahStringView *out, int *inlined, int32_t *buffer) {
  const uint8_t *view = (const uint8_t *)array->buffers[1] + 16 * row;
  int32_t length;

  if (!is_valid(array, row)) {
    return 0;
  }
  memcpy(&length, view, 4);
  out->len = (size_t)length;
  *inlined = length <= 12;
  *buffer = -1;
  if (*inlined) {
    out->data = view + 4;
  } else {
    int32_t offset;
    const uint8_t *data;
    memcpy(buffer, view + 8, 4);
    memcpy(&offset, view + 12, 4);
    data = (const uint8_t *)array->buffers[2 + *buffer];
    CHECK(memcmp(view + 4, data + offset, 4) == 0); /* the prefix */
    out->data = data + offset;
  }
  return 1;
}

static void check_arrow(struct ArrowSchema *schema, struct ArrowArray *array,
                        const LinkIds *ids) {
  static const char *const names[] = {
      "index", "parent", "tag",      "inner_html", "raw_text",
      "text",  "href",   "disabled", "data-empty"};
  const struct ArrowArray *text, *raw_text, *inner_html, *disabled, *empty;
  const uint32_t *index_values, *parent_values;
  const int64_t *variadic_sizes;
  ScahStringView value;
  int inlined;
  int32_t buffer;
  int64_t i;

  CHECK(schema->release != NULL && array->release != NULL);
  CHECK(strcmp(schema->format, "+s") == 0);
  CHECK(schema->n_children == 9 && array->n_children == 9);
  CHECK(array->length == 3 && array->null_count == 0);
  for (i = 0; i < 9; i++) {
    const struct ArrowSchema *field = schema->children[i];
    CHECK(strcmp(field->name, names[i]) == 0);
    CHECK(strcmp(field->format, i < 2 ? "I" : "vu") == 0);
    CHECK((field->flags & ARROW_FLAG_NULLABLE) == (i < 3 ? 0 : 2));
    CHECK(column(array, i)->length == 3);
    /* validity and values, or validity, views, the data buffers the column
     * uses (at most HTML, raw text, and text), and their sizes */
    if (i < 2) {
      CHECK(column(array, i)->n_buffers == 2);
    } else {
      CHECK(column(array, i)->n_buffers >= 3);
      CHECK(column(array, i)->n_buffers <= 6);
    }
  }

  index_values = (const uint32_t *)column(array, 0)->buffers[1];
  parent_values = (const uint32_t *)column(array, 1)->buffers[1];
  for (i = 0; i < 3; i++) {
    CHECK(index_values[i] == ids->links[i]);
    CHECK(parent_values[i] == ids->sections[i < 2 ? 0 : 1]);
  }

  /* `text` is inlined; `raw_text` and `inner_html` point into the raw text
   * buffer and the HTML, each the only data buffer of its column. */
  inner_html = column(array, 3);
  raw_text = column(array, 4);
  text = column(array, 5);
  CHECK(view_at(text, 0, &value, &inlined, &buffer));
  CHECK(inlined && view_eq(value, "One & only"));
  CHECK(view_at(raw_text, 0, &value, &inlined, &buffer));
  CHECK(!inlined && buffer == 0 && view_eq(value, "One &amp; only"));
  CHECK(view_at(inner_html, 0, &value, &inlined, &buffer));
  CHECK(!inlined && buffer == 0 && view_eq(value, "One &amp; only"));
  CHECK(view_at(raw_text, 1, &value, &inlined, &buffer));
  CHECK(inlined && view_eq(value, "  Two  "));
  CHECK(text->null_count == 0 && text->buffers[0] == NULL);
  CHECK(text->n_buffers == 3); /* every value is inlined */
  CHECK(inner_html->n_buffers == 4);
  variadic_sizes = (const int64_t *)inner_html->buffers[3];
  CHECK(variadic_sizes[0] == (int64_t)strlen(HTML));

  /* Valueless and missing attributes are null; empty values are not. */
  disabled = column(array, 7);
  CHECK(disabled->null_count == 3);
  for (i = 0; i < 3; i++) {
    CHECK(!view_at(disabled, i, &value, &inlined, &buffer));
  }
  empty = column(array, 8);
  CHECK(empty->null_count == 2);
  CHECK(view_at(empty, 0, &value, &inlined, &buffer) && value.len == 0);
  CHECK(!view_at(empty, 1, &value, &inlined, &buffer));
  CHECK(view_at(column(array, 6), 2, &value, &inlined, &buffer));
  CHECK(view_eq(value, "/three"));

  schema->release(schema);
  array->release(array);
  CHECK(schema->release == NULL && array->release == NULL);
}

int main(void) {
  ScahQuery *sections = NULL;
  ScahQuery *note = NULL;
  const ScahQuery *queries[2];
  ScahStore *store = NULL;
  struct ArrowSchema schema;
  struct ArrowArray array;
  LinkIds ids;
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

  /* Arrow exports keep the store's data alive after scah_store_free. */
  ids = link_ids(store);
  export_links(store, &schema, &array);
  scah_store_free(store);
  check_arrow(&schema, &array, &ids);

  scah_store_free(NULL);
  scah_query_free(NULL);
  scah_query_builder_free(NULL);
  scah_element_list_free(NULL);
  scah_error_free(NULL);

  puts("C smoke test passed");
  return 0;
}
