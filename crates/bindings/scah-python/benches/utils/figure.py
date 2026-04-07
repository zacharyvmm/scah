import argparse
import json

import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns

LIBRARY_COLORS = {
    "Scah": "#1b5e20",
    "scah": "#1b5e20",
    "TL": "#1565c0",
    "tl": "#1565c0",
    "lol_html": "#6a1b9a",
    "Lol_html": "#6a1b9a",
    "Lexbor": "#ef6c00",
    "lexbor": "#ef6c00",
    "Lxml": "#c62828",
    "lxml": "#c62828",
    "Scraper": "#00838f",
    "scraper": "#00838f",
    "BS4 (lxml)": "#c62828",
    "BS4 (html.parser)": "#ad1457",
    "Selectolax": "#ef6c00",
    "Parsel": "#6a1b9a",
    "Gazpacho": "#00838f",
}

TIMING_COLORS = {
    "DOM Time": "#90caf9",
    "Selection Time": "#1565c0",
}


def library_palette(libraries):
    return [LIBRARY_COLORS.get(library, "#546e7a") for library in libraries]


def apply_graph_style():
    sns.set_theme(style="whitegrid")


def finalize_horizontal_bar_chart(ax, title, xlabel):
    ax.set_title(title, fontsize=16, pad=16)
    ax.set_xlabel(xlabel, fontsize=12)
    ax.set_ylabel("")
    ax.tick_params(axis="both", which="major", labelsize=11)
    ax.invert_yaxis()


def create_horizontal_benchmark_chart(df, title, value_column, output_image):
    ordered_df = df.sort_values(by=value_column, ascending=True).reset_index(drop=True)

    apply_graph_style()
    fig, ax = plt.subplots(figsize=(10, 6))
    bars = ax.barh(
        ordered_df["Library"],
        ordered_df[value_column],
        color=library_palette(ordered_df["Library"]),
    )

    finalize_horizontal_bar_chart(ax, title, value_column)
    ax.bar_label(bars, fmt="%.3f", padding=4)

    plt.tight_layout()
    plt.savefig(output_image, dpi=300)
    print(f"Graph saved as {output_image}")
    plt.close(fig)


def create_benchmark_graph(json_file: str, bench_name: str, output_image="benchmark_results.png"):
    with open(json_file, "r") as f:
        data = json.load(f)

    bench_results = []
    for bench in data["benchmarks"]:
        bench_results.append(
            {
                "Library": bench["name"].split("[")[0],
                "Test": bench["fullname"],
                "Mean (ms)": bench["stats"]["mean"] * 1000,
                "StdDev (ms)": bench["stats"]["stddev"] * 1000,
            }
        )

    df = pd.DataFrame(bench_results)
    create_horizontal_benchmark_chart(
        df,
        f"Performance Comparison: Execution Time for {bench_name} bench",
        "Mean (ms)",
        output_image,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("filename", type=str)
    parser.add_argument("-o", "--output", type=str, required=True)

    args = parser.parse_args()
    bench_name = args.filename.split("/")[-1].split(".json")[0]

    create_benchmark_graph(args.filename, bench_name.capitalize(), args.output)
