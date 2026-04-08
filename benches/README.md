# Performance Benchmark Report

## Nested All Selection Comparison

![Criterion Simple](https://raw.githubusercontent.com/zacharyvmm/scah/main/benches/images/criterion_nested.png)

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.072955** 
| tl | **0.155305** 
| lol_html | **0.168812** 
| lexbor | **0.215914** 
| scraper | **0.431706** 
| lxml | **0.578461** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.717624** 
| tl | **1.608891** 
| lol_html | **1.650700** 
| lexbor | **2.016662** 
| scraper | **4.281605** 
| lxml | **5.561729** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **7.577791** 
| tl | **15.956983** 
| lol_html | **16.366029** 
| lexbor | **23.950227** 
| scraper | **42.663811** 
| lxml | **70.249608** 

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
| scah | **0.032746** 
| tl | **0.043941** 
| lol_html | **0.048846** 
| lexbor | **0.116722** 
| lxml | **0.232907** 
| scraper | **0.284249** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.322802** 
| tl | **0.452792** 
| lol_html | **0.477883** 
| lexbor | **1.035157** 
| lxml | **2.212452** 
| scraper | **2.851043** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **3.636213** 
| tl | **4.444398** 
| lol_html | **4.743502** 
| lexbor | **10.939556** 
| lxml | **23.978326** 
| scraper | **28.734239** 

---

## Simple First Selection Comparison

![Criterion Simple](https://raw.githubusercontent.com/zacharyvmm/scah/main/benches/images/criterion_simple.png)

### Input Size: 100 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000607** 
| lol_html | **0.001129** 
| tl | **0.020985** 
| lexbor | **0.092363** 
| lxml | **0.176442** 
| scraper | **0.254047** 

### Input Size: 1000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000583** 
| lol_html | **0.001125** 
| tl | **0.229419** 
| lexbor | **0.784825** 
| lxml | **1.687698** 
| scraper | **2.527543** 

### Input Size: 10000 Elements

| Library | Mean (ms) |
| :--- | :--- |
| scah | **0.000604** 
| lol_html | **0.001123** 
| tl | **2.142480** 
| lexbor | **8.239055** 
| lxml | **18.304213** 
| scraper | **25.146224** 

---

## WHATWG All Links

| Library | Mean (ms) |
| :--- | :--- |
| scah | **46.233550** 
| lol_html | **48.753165** 
| tl | **62.236425** 
| lexbor | **121.425865** 
| scraper | **275.784674** 
| lxml | **323.821592** 

---

