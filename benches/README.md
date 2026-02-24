# Performance Benchmark Report
![Benchmark Image](./images/criterion_bench.png)

## Simple All Selection Comparison

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.024775** 
| tl | **0.042899** 
| lol_html | **0.049933** 
| lexbor | **0.109116** 
| scraper | **0.282720** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.233145** 
| tl | **0.435816** 
| lol_html | **0.484030** 
| lexbor | **0.938643** 
| scraper | **2.843023** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **2.438346** 
| tl | **4.310641** 
| lol_html | **4.830281** 
| lexbor | **10.078457** 
| scraper | **28.679969** 

---

## Simple First Selection Comparison

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000576** 
| lol_html | **0.001190** 
| tl | **0.021150** 
| lexbor | **0.084939** 
| scraper | **0.253248** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000571** 
| lol_html | **0.001199** 
| tl | **0.229589** 
| lexbor | **0.710905** 
| scraper | **2.555849** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000568** 
| lol_html | **0.001186** 
| tl | **2.180991** 
| lexbor | **7.432926** 
| scraper | **25.549476** 

---

