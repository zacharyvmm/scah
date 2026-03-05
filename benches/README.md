# Performance Benchmark Report
![Benchmark Image](./images/criterion_bench.png)

## Simple All Selection Comparison

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.022270** 
| tl | **0.046285** 
| lol_html | **0.050179** 
| lexbor | **0.110079** 
| scraper | **0.290444** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.212090** 
| tl | **0.475236** 
| lol_html | **0.488771** 
| lexbor | **0.993170** 
| scraper | **2.915074** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **2.121494** 
| tl | **4.623117** 
| lol_html | **4.849870** 
| lexbor | **10.397207** 
| scraper | **29.610598** 

---

## Simple First Selection Comparison

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000509** 
| lol_html | **0.001141** 
| tl | **0.020902** 
| lexbor | **0.086477** 
| scraper | **0.256770** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000530** 
| lol_html | **0.001141** 
| tl | **0.225558** 
| lexbor | **0.711178** 
| scraper | **2.556799** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000554** 
| lol_html | **0.001136** 
| tl | **2.164622** 
| lexbor | **7.494570** 
| scraper | **25.462792** 

---

