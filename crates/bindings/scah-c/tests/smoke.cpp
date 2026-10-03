// Smoke test for the scah C API from C++. Built by scripts/smoke-test.sh.

#include "scah.h"

#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <initializer_list>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#define CHECK(cond)                                                          \
  do {                                                                       \
    if (!(cond)) {                                                           \
      std::fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, \
                   #cond);                                                   \
      std::exit(1);                                                          \
    }                                                                        \
  } while (0)

namespace {

struct Deleter {
  void operator()(ScahQueryBuilder *p) const { scah_query_builder_free(p); }
  void operator()(ScahQuery *p) const { scah_query_free(p); }
  void operator()(ScahStore *p) const { scah_store_free(p); }
  void operator()(ScahElementList *p) const { scah_element_list_free(p); }
  void operator()(ScahError *p) const { scah_error_free(p); }
};

template <typename T>
using Owned = std::unique_ptr<T, Deleter>;

ScahStringView view(std::string_view s) {
  return ScahStringView{reinterpret_cast<const uint8_t *>(s.data()), s.size()};
}

std::string_view str(ScahStringView v) {
  return v.len == 0 ? std::string_view{}
                    : std::string_view{reinterpret_cast<const char *>(v.data),
                                       v.len};
}

std::optional<std::string_view> opt(ScahOptionalStringView v) {
  if (!v.is_some) {
    return std::nullopt;
  }
  return str(v.value);
}

struct Error : std::runtime_error {
  ScahStatus status;
  Error(ScahStatus s, std::string message)
      : std::runtime_error(std::move(message)), status(s) {}
};

// Call `f(&err)` and throw on failure.
template <typename F>
void call(F &&f) {
  ScahError *raw = nullptr;
  ScahStatus status = f(&raw);
  Owned<ScahError> err(raw);
  if (status != SCAH_STATUS_OK) {
    throw Error(status, std::string(str(scah_error_message(err.get()))));
  }
  CHECK(err == nullptr);
}

constexpr ScahSave kAll{true, true, true, true};
constexpr ScahSave kText{false, false, true, true};

Owned<ScahQueryBuilder> all(std::string_view selector, ScahSave save) {
  ScahQueryBuilder *out = nullptr;
  call([&](ScahError **e) {
    return scah_query_builder_new_all(view(selector), save, &out, e);
  });
  return Owned<ScahQueryBuilder>(out);
}

Owned<ScahQueryBuilder> first(std::string_view selector, ScahSave save) {
  ScahQueryBuilder *out = nullptr;
  call([&](ScahError **e) {
    return scah_query_builder_new_first(view(selector), save, &out, e);
  });
  return Owned<ScahQueryBuilder>(out);
}

Owned<ScahQueryBuilder> then(Owned<ScahQueryBuilder> parent,
                             std::initializer_list<const ScahQueryBuilder *> children) {
  call([&](ScahError **e) {
    return scah_query_builder_then(parent.get(), children.begin(),
                                   children.size(), e);
  });
  return parent;
}

Owned<ScahQuery> build(const Owned<ScahQueryBuilder> &builder) {
  ScahQuery *out = nullptr;
  call([&](ScahError **e) { return scah_query_build(builder.get(), &out, e); });
  return Owned<ScahQuery>(out);
}

// Returns an empty vector when nothing matched.
std::vector<ScahElementId> ids(Owned<ScahElementList> list) {
  const ScahElementId *data = scah_element_list_ids(list.get());
  return std::vector<ScahElementId>(data,
                                    data + scah_element_list_len(list.get()));
}

std::optional<std::vector<ScahElementId>> get(const ScahStore *store,
                                              std::string_view selector) {
  ScahElementList *out = nullptr;
  call([&](ScahError **e) {
    return scah_store_get(store, view(selector), &out, e);
  });
  if (out == nullptr) {
    return std::nullopt;
  }
  return ids(Owned<ScahElementList>(out));
}

std::optional<std::vector<ScahElementId>> get(const ScahStore *store,
                                              ScahElementId id,
                                              std::string_view selector) {
  ScahElementList *out = nullptr;
  call([&](ScahError **e) {
    return scah_element_get(store, id, view(selector), &out, e);
  });
  if (out == nullptr) {
    return std::nullopt;
  }
  return ids(Owned<ScahElementList>(out));
}

using Getter = ScahStatus (*)(const ScahStore *, ScahElementId,
                              ScahOptionalStringView *, ScahError **);

std::optional<std::string_view> field(const ScahStore *store, ScahElementId id,
                                      Getter getter) {
  ScahOptionalStringView out{};
  call([&](ScahError **e) { return getter(store, id, &out, e); });
  return opt(out);
}

std::optional<std::string_view> attribute(const ScahStore *store,
                                          ScahElementId id,
                                          std::string_view key) {
  ScahOptionalStringView out{};
  call([&](ScahError **e) {
    return scah_element_attribute(store, id, view(key), &out, e);
  });
  return opt(out);
}

std::vector<std::pair<std::string_view, std::optional<std::string_view>>>
attributes(const ScahStore *store, ScahElementId id) {
  size_t count = 0;
  call([&](ScahError **e) {
    return scah_element_attribute_count(store, id, &count, e);
  });
  std::vector<std::pair<std::string_view, std::optional<std::string_view>>>
      result;
  for (size_t i = 0; i < count; ++i) {
    ScahStringView key{};
    ScahOptionalStringView value{};
    call([&](ScahError **e) {
      return scah_element_attribute_at(store, id, i, &key, &value, e);
    });
    result.emplace_back(str(key), opt(value));
  }
  return result;
}

}  // namespace

int main() {
  CHECK(scah_abi_version() == SCAH_ABI_VERSION);

  // Invalid selectors surface at build time with a message.
  try {
    build(all("a[", kAll));
    CHECK(false);
  } catch (const Error &err) {
    CHECK(err.status == SCAH_STATUS_INVALID_SELECTOR);
    CHECK(std::string_view(err.what()).rfind("invalid selector: ", 0) == 0);
  }

  // Query.all("ul", ...).then(ul => [ul.all("> li", ...).then(li => [li.first("a", ...)])])
  auto link = first("a", kAll);
  auto item = then(all("> li", kAll), {link.get()});
  auto list = then(all("ul", kAll), {item.get()});
  Owned<ScahQuery> lists = build(list);
  Owned<ScahQuery> title = build(first("h1", kText));
  link.reset();
  item.reset();
  list.reset();

  Owned<ScahStore> store;
  {
    std::string html =
        "<h1> The   title </h1>"
        "<ul class='menu'>"
        "<li><a href='/a' title=''>A &lt; B</a></li>"
        "<li>no link</li>"
        "<li><a href='/c' hidden>C</a></li>"
        "</ul>";
    const ScahQuery *queries[] = {lists.get(), title.get()};
    ScahStore *out = nullptr;
    call([&](ScahError **e) {
      return scah_parse(view(html), queries, 2, &out, e);
    });
    store.reset(out);
  }
  // The HTML string and queries are gone; the store still works.
  lists.reset();
  title.reset();
  const ScahStore *s = store.get();

  auto h1 = get(s, "h1").value();
  CHECK(h1.size() == 1);
  CHECK(field(s, h1[0], scah_element_text) == "The title");
  CHECK(field(s, h1[0], scah_element_raw_text) == std::nullopt);

  auto uls = get(s, "ul").value();
  CHECK(uls.size() == 1);
  CHECK(field(s, uls[0], scah_element_class_name) == "menu");

  auto items = get(s, uls[0], "> li").value();
  CHECK(items.size() == 3);
  CHECK(get(s, items[1], "a") == std::nullopt);

  auto a = get(s, items[0], "a").value();
  CHECK(a.size() == 1);
  CHECK(field(s, a[0], scah_element_text) == "A < B");
  CHECK(field(s, a[0], scah_element_raw_text) == "A &lt; B");
  CHECK(field(s, a[0], scah_element_inner_html) == "A &lt; B");
  CHECK(attribute(s, a[0], "href") == "/a");
  CHECK(attribute(s, a[0], "title") == "");
  CHECK(attribute(s, a[0], "missing") == std::nullopt);

  auto c = get(s, items[2], "a").value();
  using Attr = std::pair<std::string_view, std::optional<std::string_view>>;
  CHECK((attributes(s, c[0]) ==
         std::vector<Attr>{{"href", "/c"}, {"hidden", std::nullopt}}));

  CHECK(get(s, "li") == std::nullopt);  // nested, not top-level
  try {
    field(s, scah_store_len(s), scah_element_text);
    CHECK(false);
  } catch (const Error &err) {
    CHECK(err.status == SCAH_STATUS_INDEX_OUT_OF_BOUNDS);
  }

  // Arrow export: the `> li` rows under `ul`, each with its parent's index.
  {
    ArrowSchema schema;
    ArrowArray array;
    const ScahStringView names[] = {view("class")};
    call([&](ScahError **e) {
      return scah_store_export_arrow(s, view("> li"), view("ul"), names, 1,
                                     &schema, &array, e);
    });
    const char *columns[] = {"index",    "parent", "tag",  "inner_html",
                             "raw_text", "text",   "class"};
    CHECK(std::string_view(schema.format) == "+s");
    CHECK(schema.n_children == 7 && array.n_children == 7);
    CHECK(array.length == 3);
    for (int i = 0; i < 7; i++) {
      CHECK(std::string_view(schema.children[i]->name) == columns[i]);
    }
    auto parents = static_cast<const uint32_t *>(array.children[1]->buffers[1]);
    auto tags = static_cast<const uint8_t *>(array.children[2]->buffers[1]);
    for (int row = 0; row < 3; row++) {
      CHECK(parents[row] == uls[0]);
      int32_t length;
      std::memcpy(&length, tags + 16 * row, 4);
      CHECK(length == 2 && std::memcmp(tags + 16 * row + 4, "li", 2) == 0);
    }
    CHECK(array.children[6]->null_count == 3);  // no `li` has a class
    schema.release(&schema);
    array.release(&array);
    CHECK(schema.release == nullptr && array.release == nullptr);
  }

  std::puts("C++ smoke test passed");
  return 0;
}
