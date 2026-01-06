import pytest
from utils.parsers import get_available_parsers

# Get parsers at module level for parametrization
PARSERS = get_available_parsers()

@pytest.mark.parametrize("name", PARSERS.keys())
def test_parser_benchmark(benchmark, html_content, name):
    lib = PARSERS[name]
    benchmark.group = "Synthetic Data"
    benchmark.name = name
    # We benchmark the parser function
    # The parser function takes (html, query)
    result = benchmark.pedantic(lib, args=(html_content, "a"), rounds=25, iterations=10)
    
    # Optional: Basic validation that it found something
    # This might add overhead if done inside benchmark, but we are checking return value
    # result is the return value of the function called
    assert result is not None
