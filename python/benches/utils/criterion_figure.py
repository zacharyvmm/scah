import json
import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt
import argparse
import os

def parse_criterion_json(filename):
    results = []
    with open(filename, 'r') as f:
        for line in f:
            try:
                data = json.loads(line)
                if data.get('reason') == 'benchmark-complete':
                    parts = data['id'].split('/')
                    group_name = parts[0]
                    library = parts[1]
                    size = int(parts[2])

                    mean_ns = data['mean']['estimate']
                    
                    results.append({
                        "Group": group_name,
                        "Library": library,
                        "Size": size,
                        "Mean (ms)": mean_ns / 1_000_000,
                        "StdDev (ms)": (data['mean']['upper_bound'] - data['mean']['lower_bound']) / 2_000_000
                    })
            except (json.JSONDecodeError, KeyError, IndexError):
                continue
    return pd.DataFrame(results)

def generate_markdown_table(df):
    md_content = "# Performance Benchmark Report\n\n"
    
    groups = df['Group'].unique()
    
    for group_name in groups:
        md_content += f"## {group_name.replace('_', ' ').title()}\n\n"
        group_df = df[df['Group'] == group_name]
        
        sizes = sorted(group_df['Size'].unique())
        
        for size in sizes:
            size_df = group_df[group_df['Size'] == size].sort_values("Mean (ms)")
            
            md_content += f"### Input Size: {size} Elements\n\n"
            md_content += ("| Library | Min (ms) | StdDev (ms) |\n"
                           "| :--- | :--- | :--- |\n")
            
            for _, row in size_df.iterrows():
                md_content += (f"| {row['Library']} "
                               f"| **{row['Mean (ms)']:,.6f}** "
                               f"| {row['StdDev (ms)']:,.6f} |\n")
            md_content += "\n"
            
        md_content += "---\n\n"
        
    return md_content

def create_plots(df, output_image):
    sns.set_theme(style="whitegrid")
    groups = df['Group'].unique()

    fig, axes = plt.subplots(len(groups), 1, figsize=(12, 7 * len(groups)))
    if len(groups) == 1: axes = [axes]

    for ax, group_name in zip(axes, groups):
        group_df = df[df['Group'] == group_name]

        group_df.sort_values(by='Mean (ms)', inplace=True)

        sns.barplot(
            data=group_df,
            x="Size",
            y="Mean (ms)",
            hue="Library",
            ax=ax,
            palette="viridis"
        )
        for container in ax.containers:
            ax.bar_label(container, fmt='%.3f', padding=3)

        ax.set_title(f"Comparison: {group_name.replace('_', ' ').capitalize()}", fontsize=16, pad=15)
        ax.set_ylabel("Mean Execution Time (ms)")
        ax.set_xlabel("Input Size")
        ax.legend(title="Library", bbox_to_anchor=(1.05, 1), loc='upper left')

    plt.tight_layout()
    plt.savefig(output_image, dpi=300)
    print(f"Graph saved as {output_image}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument('filename', type=str,)
    parser.add_argument('-o', '--output', type=str, default="criterion_benches.png")
    parser.add_argument('-m', '--markdown', type=str, default="criterion_report.md")

    args = parser.parse_args()

    benchmark_df = parse_criterion_json(args.filename)

    if not benchmark_df.empty:
        create_plots(benchmark_df, args.output)

        md_table = generate_markdown_table(benchmark_df)
        with open(args.markdown, 'w') as f:
            f.write(md_table)

        print(f"Markdown table saved as {args.markdown}")
        print("\nTable:\n")
        print(md_table)
    else:
        print("Error: No valid Criterion data found. Ensure the JSON contains 'benchmark-complete' entries.")

# cargo criterion --message-format=json >> criterion.json
# python3 ./python/benches/utils/criterion_figure.py ./criterion.json
# rm criterion.json