# Performance Benchmark Report

## Nested All Selection Comparison

![Criterion Simple](https://raw.githubusercontent.com/zacharyvmm/scah/main/benches/images/criterion_nested.png)

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.076186** 
| tl | **0.165992** 
| lol_html | **0.183135** 
| lexbor | **0.228684** 
| scraper | **0.449133** 
| lxml | **0.616218** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.749340** 
| tl | **1.668134** 
| lol_html | **1.731521** 
| lexbor | **2.180895** 
| scraper | **4.529098** 
| lxml | **5.996209** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **8.172846** 
| lol_html | **16.402781** 
| tl | **17.492240** 
| lexbor | **26.168626** 
| scraper | **45.485753** 
| lxml | **70.917758** 

---

## Nested First Selection Comparison

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| lol_html | **0.004932** 
| tl | **0.036061** 
| scah | **0.073914** 
| lexbor | **0.142988** 
| scraper | **0.258404** 
| lxml | **0.415266** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| lol_html | **0.004937** 
| tl | **0.315313** 
| scah | **0.722124** 
| lexbor | **1.300084** 
| scraper | **2.526277** 
| lxml | **3.963909** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| lol_html | **0.005199** 
| tl | **3.603669** 
| scah | **7.245054** 
| lexbor | **16.735194** 
| scraper | **26.918348** 
| lxml | **58.037319** 

---

## Simple All Selection Comparison

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.032909** 
| tl | **0.044921** 
| lol_html | **0.049019** 
| lexbor | **0.116987** 
| lxml | **0.230234** 
| scraper | **0.287764** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.324803** 
| tl | **0.451569** 
| lol_html | **0.479748** 
| lexbor | **1.032057** 
| lxml | **2.224932** 
| scraper | **2.874352** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **3.661858** 
| tl | **4.463700** 
| lol_html | **4.753026** 
| lexbor | **11.056077** 
| lxml | **24.097459** 
| scraper | **28.903708** 

---

## Simple First Selection Comparison

![Criterion Simple](https://raw.githubusercontent.com/zacharyvmm/scah/main/benches/images/criterion_simple.png)

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000605** 
| lol_html | **0.001152** 
| tl | **0.021215** 
| lexbor | **0.092930** 
| lxml | **0.178734** 
| scraper | **0.252844** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000601** 
| lol_html | **0.001138** 
| tl | **0.230779** 
| lexbor | **0.783790** 
| lxml | **1.683188** 
| scraper | **2.528285** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000597** 
| lol_html | **0.001128** 
| tl | **2.201056** 
| lexbor | **8.277647** 
| lxml | **18.342267** 
| scraper | **25.189047** 

---