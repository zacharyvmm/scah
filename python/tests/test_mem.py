from scah import Query, Save, parse
HTML = """
<span class="hello" id="world" hello="world">
    Hello <a href="https://www.example.com">World</a>
</span>
<p class="example_class" id="example_id" hello="example">
    My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
"""

#q = Query.all("#world", Save.all()).all("a", Save.all()).build()

# for i in range(500000):
#     print(f"Start Iter: {i}")
#     query = Query.all("#world", Save.all()).all("a", Save.all()).build()
#     print(f"End Iter: {i}")

for i in range(5000):
    first = Query.all("#world", Save.all())
    second = first.all("a", Save.all()) 
    q = second.build()
    store = parse(HTML, [q])
    result = store.get("#world")