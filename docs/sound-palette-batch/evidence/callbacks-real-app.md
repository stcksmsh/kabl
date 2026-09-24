# Real-app callback telemetry (cloud container, see README)

| run | frames | callbacks | execution worst µs | over half | late | arrival worst µs | late arrivals | xruns |
|---|---|---|---|---|---|---|---|---|
| example bass | 256 | 6867 | 2588 | 0 | 0 | 11812 | 4 | 0 |
| example breath-reduced | 256 | 7841 | 4073 | 3 | 0 | 24501 | 14 | 2 |
| example breath | 256 | 7670 | 3772 | 5 | 0 | 10651 | 8 | 0 |
| example lead-reduced | 256 | 7814 | 7537 | 5 | 2 | 19939 | 13 | 0 |
| example lead | 256 | 7428 | 5497 | 1 | 1 | 34686 | 13 | 2 |
| example pad-reduced | 256 | 8821 | 5215 | 7 | 0 | 17169 | 5 | 0 |
| example pad | 256 | 8592 | 3090 | 1 | 0 | 11164 | 6 | 0 |
| example perc-reduced | 256 | 6518 | 7015 | 4 | 1 | 10313 | 3 | 0 |
| example perc | 256 | 6142 | 2379 | 0 | 0 | 9680 | 3 | 0 |
| example strings-reduced | 256 | 9012 | 4220 | 6 | 0 | 36039 | 9 | 2 |
| example strings | 256 | 8822 | 5008 | 3 | 0 | 32087 | 14 | 2 |
| piece take | 512 | 54394 | 23781 | 1926 | 65 | 108101 | 17 | 3 |
| walkthrough | 512 | 13956 | 35839 | 580 | 8 | 59058 | 3 | 1 |
