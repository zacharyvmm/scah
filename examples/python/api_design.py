# This is what I'm thinking the python API should look like.
# Anything in this document is subject to change.


from scrooge import HtmlParser, Selectors

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
    "key 3": s.all("main > p").then(
        lambda section: section.first("a[href]"), concat="SubsequentSibling" # this should also work `concat="~"`
    ),
}

doc = HtmlParser(html="...", selectors=s)


doc.select("key")
# Returns
"""
{
    "section": {
        attributes: [...],
        textContent: "...",

        children: {
            "p" : {
                attributes: [...],
                innerHtml: "...",
                textContent: "..."
            },
            "a[href]" : {
                attributes: [
                    "href": "...",
                    ...
                ],
            }
        }
    }
}
"""

# Returns the whole dict
doc.selections()

# I should be able to unpack the selections for `and`
p, a = doc.select("key 2")
