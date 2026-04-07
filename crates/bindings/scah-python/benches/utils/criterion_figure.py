import json
import os
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd

from figure import (
    TIMING_COLORS,
    apply_graph_style,
    create_horizontal_benchmark_chart,
    finalize_horizontal_bar_chart,
)

GROUP_LABELS = {
    "simple_all_selection_comparison": "Simple All",
    "simple_first_selection_comparison": "Simple First",
    "nested_all_selection_comparison": "Nested All",
    "nested_first_selection_comparison": "Nested First",
    "whatwg_html_spec_all_links": "WHATWG All Links",
}

REPO_ROOT = Path(__file__).resolve().parents[5]
RUST_BENCH_IMAGE_DIR = REPO_ROOT / "benches" / "images"
MARKDOWN_OUTPUT = REPO_ROOT / "criterion.md"


def parse_criterion_json(filename):
    results = []
    with open(filename, "r") as f:
        for line in f:
            try:
                data = json.loads(line)
                if data.get("reason") != "benchmark-complete":
                    continue

                parts = data["id"].split("/")
                if len(parts) < 2:
                    continue

                size = None
                if len(parts) >= 3:
                    try:
                        size = int(parts[2])
                    except ValueError:
                        size = None

                results.append(
                    {
                        "Group": parts[0],
                        "Library": parts[1],
                        "Size": size,
                        "Mean_ms": data["mean"]["estimate"] / 1_000_000,
                    }
                )
            except (json.JSONDecodeError, KeyError, IndexError, ValueError):
                continue

    return pd.DataFrame(results)


def label_for_group(group_name):
    return GROUP_LABELS.get(group_name, group_name.replace("_", " ").title())


def generate_markdown_table(df):
    md_content = "# Performance Benchmark Report\n\n"

    for group_name in df["Group"].unique():
        md_content += f"## {label_for_group(group_name)}\n\n"
        group_df = df[df["Group"] == group_name]

        sizes = [size for size in group_df["Size"].unique() if pd.notna(size)]
        if not sizes:
            size_df = group_df.sort_values("Mean_ms", ascending=True).reset_index(drop=True)
            md_content += "| Library | Mean (ms) |\n| :--- | :--- |\n"
            for _, row in size_df.iterrows():
                md_content += f"| {row['Library']} | **{row['Mean_ms']:,.6f}** \n"
            md_content += "\n"
            md_content += "---\n\n"
            continue

        for size in sorted(sizes):
            size_df = (
                group_df[group_df["Size"] == size]
                .sort_values("Mean_ms", ascending=True)
                .reset_index(drop=True)
            )

            md_content += f"### Input Size: {int(size)} Elements\n\n"
            md_content += "| Library | Mean (ms) |\n| :--- | :--- |\n"

            for _, row in size_df.iterrows():
                md_content += f"| {row['Library']} | **{row['Mean_ms']:,.6f}** \n"
            md_content += "\n"

        md_content += "---\n\n"

    return md_content


def create_simple_timeline_plots(df):
    pivot_df = (
        df.pivot(index=["Library", "Size"], columns="Group", values="Mean_ms")
        .reset_index()
        .rename(
            columns={
                "simple_all_selection_comparison": "All",
                "simple_first_selection_comparison": "First",
            }
        )
    )

    if "All" not in pivot_df.columns or "First" not in pivot_df.columns:
        print("Skipping simple timeline plots: missing simple 'All' or 'First' groups.")
        return

    pivot_df["Selection Time"] = (pivot_df["All"] - pivot_df["First"]).clip(lower=0)
    pivot_df["DOM Time"] = pivot_df["First"].clip(lower=0)

    for size in sorted(pivot_df["Size"].unique()):
        subset = (
            pivot_df[pivot_df["Size"] == size]
            .sort_values(by="All", ascending=True)
            .reset_index(drop=True)
        )

        apply_graph_style()
        fig, ax = plt.subplots(figsize=(10, 6))
        ax.barh(
            subset["Library"],
            subset["DOM Time"],
            color=TIMING_COLORS["DOM Time"],
            label="DOM Time (First)",
        )
        ax.barh(
            subset["Library"],
            subset["Selection Time"],
            left=subset["DOM Time"],
            color=TIMING_COLORS["Selection Time"],
            label="Selection Time (All - First)",
        )

        finalize_horizontal_bar_chart(ax, f"Simple Selection Breakdown: {size} Elements", "Time (ms)")
        ax.legend(loc="lower right", frameon=True)

        specific_filename = RUST_BENCH_IMAGE_DIR / f"criterion_simple_breakdown_{size}.png"
        plt.tight_layout()
        plt.savefig(specific_filename, dpi=300)
        print(f"Generated image: {specific_filename}")
        plt.close(fig)


def create_group_benchmark_plots(df):
    for group_name in df["Group"].unique():
        group_df = df[df["Group"] == group_name]
        group_label = label_for_group(group_name)
        group_slug = group_name.replace("_comparison", "")

        sizes = [size for size in group_df["Size"].unique() if pd.notna(size)]
        if not sizes:
            subset = group_df[["Library", "Mean_ms"]].rename(columns={"Mean_ms": "Mean (ms)"})
            create_horizontal_benchmark_chart(
                subset,
                group_label,
                "Mean (ms)",
                RUST_BENCH_IMAGE_DIR / f"criterion_{group_slug}.png",
            )
            continue

        for size in sorted(sizes):
            subset = group_df[group_df["Size"] == size][["Library", "Mean_ms"]].rename(
                columns={"Mean_ms": "Mean (ms)"}
            )
            create_horizontal_benchmark_chart(
                subset,
                f"{group_label}: {int(size)} Elements",
                "Mean (ms)",
                RUST_BENCH_IMAGE_DIR / f"criterion_{group_slug}_{int(size)}.png",
            )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Render Rust benchmark images from Criterion JSON.")
    parser.add_argument("filename", type=str, help="Path to criterion.json")
    args = parser.parse_args()

    RUST_BENCH_IMAGE_DIR.mkdir(parents=True, exist_ok=True)
    benchmark_df = parse_criterion_json(args.filename)

    if benchmark_df.empty:
        print("Error: No valid Criterion data found. Make sure you fed it '--message-format=json'.")
    else:
        create_simple_timeline_plots(benchmark_df)
        create_group_benchmark_plots(benchmark_df)

        md_table = generate_markdown_table(benchmark_df)
        with MARKDOWN_OUTPUT.open("w") as f:
            f.write(md_table)

        print(f"Markdown table saved as '{MARKDOWN_OUTPUT}'")
        print("\nTable:\n")
        print(md_table)
