using BenchmarkDotNet.Running;
using Scah.Benchmarks;

// Runs every benchmark; BenchmarkDotNet's options, such as --filter '*SimpleAll*', narrow it.
var summary = BenchmarkRunner.Run<ScrapeBenchmark>(args: args);
// Fail when a benchmark throws, such as when a library finds the wrong elements.
return summary.HasCriticalValidationErrors || summary.Reports.Any(report => !report.Success) ? 1 : 0;
