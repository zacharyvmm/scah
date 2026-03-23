import pytest

from scah import Query, Save, parse
HTML = """
<span class="hello" id="world" hello="world">
    Hello <a href="https://www.example.com">World</a>
</span>
<p class="example_class" id="example_id" hello="example">
    My <a href="https://www.example.com">Example</a> or <a href="https://www.notexample.com">Not Example</a>
"""

def test_nested_selection():
    q = Query.all("#world", Save.all()).all("a", Save.all()).build()
    store = parse(HTML, q)

    worlds = store.get("#world")
    assert worlds

    assert len(worlds) == 1
    world = dict(worlds[0])
    
    assert world['id'] == 'world'
    assert world['class'] == 'hello'
    
    anchors = worlds[0].children('a')
    assert len(anchors) == 1
    anchor = dict(anchors[0])
    
    assert anchor['name'] == 'a'
    assert 'attributes' in anchor
    assert anchor['attributes']['href'] == "https://www.example.com"
    assert anchor['text_content'] == "World"

def test_branching_selection():
    q = Query.all("#world", Save.all())\
        .then(lambda world: [
            world.all('a', Save.all()), world.all('p', Save.all())
        ]).build()
    store = parse(HTML, q)
    
    worlds = store.get("#world")
    assert worlds
    world = worlds[0]
    
    anchors = world.children("a")
    
    assert len(anchors) == 1
    assert anchors[0].text_content == "World"