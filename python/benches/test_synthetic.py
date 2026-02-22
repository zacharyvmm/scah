import pytest
from utils.parsers import get_available_parsers

PARSERS = get_available_parsers()

@pytest.mark.parametrize("name", PARSERS.keys())
def test_parser_benchmark(benchmark, html_content, name):
    lib = PARSERS[name]
    benchmark.group = "Synthetic Data"
    benchmark.name = name

    result = benchmark.pedantic(lib, args=(html_content, "a"), rounds=25, iterations=10)
    
    assert result is not None
