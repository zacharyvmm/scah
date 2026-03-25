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
    store = parse(HTML, [q])

    worlds = store.get("#world")
    assert worlds

    assert len(worlds) == 1
    world = dict(worlds[0])
    
    assert world['id'] == 'world'
    assert world['class'] == 'hello'
    
    anchors = worlds[0].get('a')
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
    store = parse(HTML, [q])
    
    worlds = store.get("#world")
    assert worlds
    world = worlds[0]
    
    anchors = world.get("a")
    
    assert len(anchors) == 1
    assert anchors[0].text_content == "World"

def test_intro():
    html_intro = """
    <div id="project-intro">
        <header>
            <h1 class="title">scah: Streamlined CSS-Selector HTML Extraction</h1>
            <p class="subtitle">A high-performance parsing library built as a bachelor's thesis project.</p>
        </header>
        <article class="overview">
            <p><strong>scah</strong> (<em>scan HTML</em>) bridges the gap between SAX/StAX streaming efficiency and DOM convenience.</p>
            <p>Instead of manually tracking parser state or loading massive documents into memory, you declare your extraction targets using standard CSS selectors.</p>
        </article>

        <aside class="ecosystem">
            <h3>Language Bindings</h3>
            <ul>
                <li class="existing">Python</li>
                <li class="existing">Node.js</li>
                <li class="planned">Unified C API</li>
            </ul>
        </aside>
    </div>
    """

    # Extract the core description and the existing language bindings
    query_intro = Query.all("div#project-intro", Save.all()) \
        .then(lambda intro: [
            intro.all("article.overview p", Save.all()),
            intro.all("aside.ecosystem li.existing", Save.all())
        ]) \
        .build()

    store_intro = parse(html_intro, [query_intro])

    intro = store_intro.get("div#project-intro")[0]
    assert intro
    
    p_tags = intro.get("article.overview p")
    assert len(p_tags) == 2
    assert p_tags[0].text_content == "scah (scan HTML) bridges the gap between SAX/StAX streaming efficiency and DOM convenience."
    assert p_tags[1].text_content == "Instead of manually tracking parser state or loading massive documents into memory, you declare your extraction targets using standard CSS selectors."

    li_tags = intro.get("aside.ecosystem li.existing")
    assert len(li_tags) == 2
    assert li_tags[0].text_content == "Python"
    assert li_tags[1].text_content == "Node.js"
