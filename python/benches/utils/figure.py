import json
import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt

def create_benchmark_graph(json_file:str, bench_name:str, output_image="benchmark_results.png"):
    with open(json_file, 'r') as f:
        data = json.load(f)

    bench_results = []
    for bench in data['benchmarks']:
        bench_results.append({
            "Library": bench['name'].split('[')[0],  # Extracts name before any params
            "Test": bench['fullname'],
            "Mean (ms)": bench['stats']['mean'] * 1000, # Convert seconds to ms
            "StdDev (ms)": bench['stats']['stddev'] * 1000
        })

    df = pd.DataFrame(bench_results)
    df.sort_values(by='Mean (ms)', inplace=True)

    sns.set_theme(style="whitegrid", palette="viridis")
    plt.figure(figsize=(10, 6))

    ax = sns.barplot(
        data=df, 
        x="Library", 
        y="Mean (ms)", 
        hue="Library", 
        legend=False
    )

    plt.title(f"Performance Comparison: Execution Time for {bench_name} bench", fontsize=16, pad=20)
    plt.ylabel("Mean Time (milliseconds)", fontsize=12)
    plt.xlabel("Library / Implementation", fontsize=12)
    
    for container in ax.containers:
        ax.bar_label(container, fmt='%.3f', padding=3)

    plt.tight_layout()
    plt.savefig(output_image, dpi=300)
    print(f"Graph saved as {output_image}")

import argparse
if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument('filename', type=str)
    parser.add_argument('-o', '--output', type=str, required=True)

    args = parser.parse_args()
    bench_name = args.filename.split('/')[-1].split('.json')[0]

    create_benchmark_graph(args.filename, bench_name.capitalize(), args.output)