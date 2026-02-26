import pytest

from io import BytesIO
from scah import Query, Save, parse
HTML = BytesIO(b"""
<span class="hello" id="world" hello="world">
    Hello <a href="https://www.example.com">World</a>
</span>
<p class="example_class" id="example_id" hello="example">
    My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
""")

q = Query.all("#world", Save.all()).all("a", Save.all()).build()
store = parse(HTML.getbuffer(), q)

store.elements[1].name.tobytes()
store.elements[1].text_content.tobytes()

def test_nested_selection():
    q = Query.all("#world", Save.all()).all("a", Save.all()).build()
    store = parse(HTML, q)
    print(store)
    
    assert "#world" in store
    worlds = store["#world"]
    assert len(worlds) == 1
    world = worlds[0]
    
    assert world['id'] == 'world'
    assert world['class'] == 'hello'
    
    assert 'children' in world
    assert 'a' in world['children']
    anchors = world['children']['a']
    assert len(anchors) == 1
    anchor = anchors[0]
    
    assert anchor['name'] == 'a'
    assert 'attributes' in anchor
    assert anchor['attributes']['href'] == "https://www.example.com"
    assert anchor['textContent'] == "World"

def test_branching_selection():
    q = Query.all("#world", Save.all())\
        .then(lambda world: [
            world.all('a', Save.all()), world.all('p', Save.all())
        ]).build()
    store = parse(HTML, q)
    
    assert "#world" in store
    world = store["#world"][0]
    
    children = world.get('children', {})
    
    assert 'a' in children
    assert len(children['a']) == 1
    assert children['a'][0]['textContent'] == "World"
    
    if 'p' in children:
        assert len(children['p']) == 0
