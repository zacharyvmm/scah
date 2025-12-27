# 1Go

## Build
### Python Build
```bash
maturin build --features python --release
```
### Rust Build
```bash
cargo build --release
```


## What I envision (for the python API)?
### Nested First Selection
```html
<span class="hello" id="world" hello="world">Hello <a href="https://www.example.com">World</a></span>
```
```js
document.querySelector('#world')
    .querySelector('a')
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
```py
for world in doc.select["#world"]:
    for anchor in world["a"]:
        print("href:", anchor.href)
```