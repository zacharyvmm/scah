# This is what I'm thinking the python API should look like.
# Anything in this document is subject to change.


from scrooge import HtmlParser, Selectors

# NOTE: Because of the nesting in the created by conditionals I would either need to
# 1) Conditional FSM
# 1.1) Using a tree
# 1.2) Using a custom linear tree (foward polish notation style)
# 1.3) Using Conditional Vec of Vec
# 1.4) A other list with (1st item start index, 2nd item start index)s
# 2) Nested FSM (Fsm within the FSM)
s = Selectors()
s.template = {
    "key": s.all("main > section", textContent=True).then(
        # To handle the `or` clause I would need to run write the `__or__` dunder to genereate a OR_CLAUSE object
        # Then I would run the lambda (returns OR_CLAUSE)
        lambda section: section.first("p", innerHtml=True)
            or section.all("a[href]", textContent=False)
            # The `textContent=False` would overide the `textContent=True` from before
    ),
    "key 2": s.all("main").then(
        lambda section: section.first("p") and section.all("a[href]"), concat="child"
    ),
}

doc = HtmlParser(html="...", selectors=s)


doc.select("key")
# Returns
"""
[
    {
        "p" : {
            attributes: [],
            innerHtml: "...",
            textContent: "..."
        }
    },
    {
        "a[href]" : {
            attributes: [
                "href": "..."
            ],
        }
    }
]
"""

# Returns the whole dict
doc.selections()

# I should be able to unpack the selections for `and`
p, a = doc.select("key 2")
