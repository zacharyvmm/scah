from onego import Query

"""
<span class="hello" id="world" hello="world">
    Hello <a href="https://www.example.com">World</a>
</span>
<p class="example_class" id="example_id" hello="example">
    My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
</p>
"""

doc = Query()
q1 = doc.all('#world').all('a')

for world in doc.select["#world"]:
    for anchor in world["a"]:
        print("href:", anchor.href)


doc = Query()
doc.all('#world').then(lambda x: [x.all('a'), x.all('p')])

for world in doc.select["#world"]:
    for anchor in world["a"]:
        print("href:", anchor.href)