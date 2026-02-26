import pytest
import os
import io

@pytest.fixture(scope="session")
def spec_html_content():
    path = "/home/zmm/Music/html.spec.whatwg.index.html"
    if os.path.exists(path):
        # with open(path, 'r') as f:
        #     return f.read()
        with open(path, 'rb') as f:
            return io.BytesIO(f.read())
    pytest.skip(f"Spec file not found at {path}")

@pytest.fixture(scope="session")
def html_content():
    element_number = 5000
    content = f"""<html><body>{"".join(f"<div class='container'><a href='#'>Link {i}</a></div>" for i in range(element_number))}</body></html>"""
    return io.BytesIO(content.encode("utf-8"))

element_number = 5000
content = f"""<html><body>{"".join(f"<div class='container'><a href='#'>Link {i}</a></div>" for i in range(element_number))}</body></html>"""
content = io.BytesIO(content.encode("utf-8"))
