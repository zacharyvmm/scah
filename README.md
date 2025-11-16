# 1Go

The export languages I would like to support are rust (obviously), python, javascript/typescript, ruby, php, lua, java, c#. 

## Export Format
The Export format should be a mixture of hashmaps, list, and classes.

### Nested First Selection
```html
<span class="hello" id="world" hello="world">Hello <a href="https://www.example.com">World</a></span>
```
```js
document.querySelector('#world')
    .querySelector('a')
```
```ts
type Element {
    name: string;
    class?: string;
    id?: string;
    attributes?: {[key: string]: string},
    innerHtml?: string;
    textContent?: string;
    children?: Element | Element[];
}
```
```py
class Element:
    _name: str
    _class: Optional[str]
    _id: Optional[str]
    _attributes: list[dict[str, str]]
    innerHtml: Optional[str]
    textContent: Optional[str]
    children: dict[str, Element | list[Element]]

    def __getitem__(self, key):
        return self.children[key]
```
```rust
struct Element<'query, 'html> {
    name: &'html str,
    class: Option<&'html str>,
    id: Option<&'html str>,
    attributes: Vec<(&'html str, &'html str)>,
    inner_html: Option<&'html str>,
    text_content: Option<&'html str>,
    children: Vec<(&'query, Self)>,
}
impl Index<&str> for Element<'query, 'html> {
    type Output = Self;

    fn index(&self, key: &str) -> Option<&Self::Output> {
        for query, element in self.children {
            if key == query {
                return Some(element);
            }
        }
        return None;
    }
}
```
```json
{
    "#world": {
        "name": "span",
        "class": "hello",
        "id": "world",
        "attributes": {
            "hello": "world",
        },
        "<innerHtml>": "Hello <a href=\"https://www.example.com\">World</a>",
        "<textContent>": "Hello World",
        "<children>": {
            "a": {
                "name": "a",
                "class": null,
                "id": null,
                "attributes": {
                    "href": "https://www.example.com",
                },
                "<innerHtml>": "World",
                "<textContent>": "World",
                "<children>": null
            }
        }
    }
}
```
```py
doc.select["#world"]["a"]
```

### Nested All Selection
```html
<span class="hello" id="world" hello="world">Hello <a href="https://www.example.com">World</a></span>
<p class="example_class" id="example_id" hello="example">My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a></p>
```
```js
document.querySelectorAll('#world')
    .querySelectorAll('a')
```
```json
{
    "#world": [
        {
            "name": "span",
            "class": "hello",
            "id": "world",
            "attributes": {
                "hello": "world",
            },
            "<innerHtml>": "Hello <a href=\"https://www.example.com\">World</a>",
            "<textContent>": "Hello World",
            "<children>": {
                "a": [
                    {
                        "name": "a",
                        "class": null,
                        "id": null,
                        "attributes": {
                            "href": "https://www.example.com",
                        },
                        "<innerHtml>": "World",
                        "<textContent>": "World",
                        "<children>": null
                    }
                ]
            }
        },
        {
            "name": "p",
            "class": "example_class",
            "id": "example_id",
            "attributes": {
                "hello": "example",
            },
            "<innerHtml>": "My <a href=\"https://www.example.com\">Example</a> or <a href=\"https://www.notexample.com\">Not Example</a>",
            "<textContent>": "My Example or Not Example",
            "<children>": {
                "a": [
                    {
                        "name": "a",
                        "class": null,
                        "id": null,
                        "attributes": {
                            "href": "https://www.example.com",
                        },
                        "<innerHtml>": "World",
                        "<textContent>": "World",
                        "<children>": null
                    },
                    {
                        "name": "a",
                        "class": null,
                        "id": null,
                        "attributes": {
                            "href": "https://www.notexample.com",
                        },
                        "<innerHtml>": "Not Example",
                        "<textContent>": "Not Example",
                        "<children>": null
                    }
                ]
            }
        }
    ]
}
```
```py
for world in doc.select["#world"]:
    for anchor in world["a"]:
        print("href:", anchor.href)
```