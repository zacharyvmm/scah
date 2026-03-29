import json
import pandas as pd
import matplotlib.pyplot as plt
from matplotlib.collections import PolyCollection
from matplotlib.patches import Patch
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
                    
                    if len(parts) >= 3:
                        group_name = parts[0]
                        library = parts[1]
                        size = int(parts[2])

                        mean_ns = data['mean']['estimate']
                        
                        results.append({
                            "Group": group_name,
                            "Library": library,
                            "Size": size,
                            "Mean_ms": mean_ns / 1_000_000
                        })
            except (json.JSONDecodeError, KeyError, IndexError, ValueError):
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
            size_df = group_df[group_df['Size'] == size].sort_values("Mean_ms")
            
            md_content += f"### Input Size: {size} Elements\n\n"
            md_content += ("| Library | Mean (ms) |\n"
                           "| :--- | :--- |\n")
            
            for _, row in size_df.iterrows():
                md_content += (f"| {row['Library']} "
                               f"| **{row['Mean_ms']:,.6f}** \n")
            md_content += "\n"
            
        md_content += "---\n\n"
        
    return md_content

def create_timeline_plots(df, output_base_name):
    pivot_df = df.pivot(index=['Library', 'Size'], columns='Group', values='Mean_ms').reset_index()
    
    pivot_df.rename(columns={'simple_all_selection_comparison': 'All', 'simple_first_selection_comparison': 'First'}, inplace=True)
    if 'All' not in pivot_df.columns or 'First' not in pivot_df.columns:
        print("Error: The benchmark JSON must contain both 'All' and 'First' groups.")
        return

    pivot_df['Selection_Time'] = pivot_df['All'] - pivot_df['First']
    pivot_df['DOM_Time'] = pivot_df['First']
    
    # pivot_df.to_csv('library_performance.csv', index=False)

    sizes = sorted(pivot_df['Size'].unique())
    colormapping = {"Selection Time": "C0", "DOM Time": "C1"}

    legend_elements = [
        Patch(facecolor=colormapping['DOM Time'], label='DOM Time (First)'),
        Patch(facecolor=colormapping['Selection Time'], label='Selection Time (All - First)'),
    ]

    # Split input filename to create dynamic names (e.g., timeline -> timeline_100.png)
    base_name, ext = os.path.splitext(output_base_name)
    if not ext: ext = ".png"

    for size in sizes:
        fig, ax = plt.subplots(figsize=(10, 5)) 
        
        subset = pivot_df[pivot_df['Size'] == size].copy()

        subset = subset.sort_values(by='All', ascending=False).reset_index(drop=True)
        cats = {row['Library']: i+1 for i, row in subset.iterrows()}
        
        verts = []
        colors = []
        
        for _, row in subset.iterrows():
            lib = row['Library']
            dom = row['DOM_Time']
            selection = row['Selection_Time']
            
            if pd.isna(selection) or selection < 0: selection = 0
            if pd.isna(dom) or dom < 0: dom = 0
                
            y = cats[lib]

            # Block 1: DOM Time
            v1 = [(0, y-0.4), (0, y+0.4), (dom, y+0.4), (dom, y-0.4), (0, y-0.4)]
            verts.append(v1)
            colors.append(colormapping["DOM Time"])

            # Block 2: Selection Time
            v2 = [(dom, y-0.4), (dom, y+0.4), (dom + selection, y+0.4), (dom + selection, y-0.4), (dom, y-0.4)]
            verts.append(v2)
            colors.append(colormapping["Selection Time"])
            
        bars = PolyCollection(verts, facecolors=colors)
        ax.add_collection(bars)
        ax.autoscale()
        
        ax.set_yticks(list(cats.values()))
        ax.set_yticklabels(list(cats.keys()))
        ax.set_title(f"Input Size: {size} Elements", fontsize=14, pad=10)
        ax.set_xlabel("Time (ms)", fontsize=12)
        ax.tick_params(axis='both', which='major', labelsize=11)
        
        fig.legend(handles=legend_elements, loc='upper right', bbox_to_anchor=(0.98, 0.96), frameon=True)

        specific_filename = f"{base_name}_{size}{ext}"
        plt.tight_layout()
        plt.savefig(specific_filename, dpi=300)
        print(f"Generated image: {specific_filename}")
        plt.close(fig)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Parse Criterion JSON into separate Timeline Plots.")
    parser.add_argument('filename', type=str, help="Path to criterion.json")
    parser.add_argument('-o', '--output', type=str, default="timeline.png", help="Base output filename (e.g., 'plot.png' becomes 'plot_100.png')")

    args = parser.parse_args()

    benchmark_df = parse_criterion_json(args.filename)

    if not benchmark_df.empty:
        create_timeline_plots(benchmark_df, args.output)

        md_table = generate_markdown_table(benchmark_df)
        with open('criterion.md', 'w') as f:
            f.write(md_table)

        print(f"Markdown table saved as 'criterion.md'")
        print("\nTable:\n")
        print(md_table)
    else:
        print("Error: No valid Criterion data found. Make sure you fed it '--message-format=json'.")