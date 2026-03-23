from scah import Query, Save, parse
HTML = """
<span class="hello" id="world" hello="world">
    Hello <a href="https://www.example.com">World</a>
</span>
<p class="example_class" id="example_id" hello="example">
    My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
"""

for _ in range(5000):
    q = Query.all("#world", Save.all()).all("a", Save.all()).build()
    store = parse(HTML, q)