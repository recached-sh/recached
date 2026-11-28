| run | bounce at s | PONG s | full sync installed after s | disconnects until the next bounce / the end | new link's receive buffer: at start → 30 s later |
|---|---|---|---|---|---|
| raof-1840-bounce h | before the first bounce | | | 1 [105] | |
| raof-1840-bounce h | 135 | 4.26 | 4.8 | 0  | 2.11 MB → 5.02 MB |
| raof-1840-bounce h | 255 | 17.08 | 17.8 | 0  | 2.08 MB → 2.16 MB |
| raof-1840-bounce h | 375 | 15.33 | 15.9 | 0  | 0.13 MB → 33.55 MB |
| raof-1840-bounce h | 495 | 15.52 | 16.2 | 0  | 1.22 MB → 1.22 MB |
| raof-1840-bounce h | after the load | restart 15.27 s | | checks {'primary': 'true', 'replica': 'true', 'restart': 'true'} | |
| raof-1840-bounce i | before the first bounce | | | 0  | |
| raof-1840-bounce i | 135 | 20.77 | 21.6 | 0  | 1.07 MB → 9.12 MB |
| raof-1840-bounce i | 255 | 15.28 | 16.0 | 0  | 1.66 MB → 33.55 MB |
| raof-1840-bounce i | 375 | 16.27 | 16.9 | 0  | 1.19 MB → 26.39 MB |
| raof-1840-bounce i | 495 | 16.03 | 16.8 | 0  | 0.94 MB → 1.43 MB |
| raof-1840-bounce i | after the load | restart 15.77 s | | checks {'primary': 'true', 'replica': 'true', 'restart': 'true'} | |
| raof-1840-bounce p | before the first bounce | | | 0  | |
| raof-1840-bounce p | 135 | 20.52 | 21.5 | 0  | 0.13 MB → 4.58 MB |
| raof-1840-bounce p | 255 | 15.27 | 16.2 | 1 [275] | 1.07 MB → (closed) |
| raof-1840-bounce p | 375 | 15.02 | 15.8 | 1 [399] | 1.48 MB → (closed) |
| raof-1840-bounce p | 495 | 14.65 | 15.3 | 0  | 2.07 MB → 2.21 MB |
| raof-1840-bounce p | after the load | restart 15.77 s | | checks {'primary': 'true', 'replica': 'true', 'restart': 'true'} | |
| raof-1380-bounce h | before the first bounce | | | 0  | |
| raof-1380-bounce h | 135 | 15.02 | 15.5 | 1 [155] | 2.37 MB → (closed) |
| raof-1380-bounce h | 255 | 11.09 | 11.6 | 0  | 1.34 MB → 1.36 MB |
| raof-1380-bounce h | 375 | 12.38 | 12.8 | 0  | 1.53 MB → 9.34 MB |
| raof-1380-bounce h | 495 | 12.02 | 12.7 | 1 [539] | 0.97 MB → 0.97 MB |
| raof-1380-bounce h | after the load | restart 8.26 s | | checks {'primary': 'true', 'replica': 'true', 'restart': 'true'} | |
| raof-1380-bounce i | before the first bounce | | | 0  | |
| raof-1380-bounce i | 135 | 15.52 | 16.2 | 0  | 1.45 MB → 1.48 MB |
| raof-1380-bounce i | 255 | 12.26 | 12.8 | 0  | 1.50 MB → 4.50 MB |
| raof-1380-bounce i | 375 | 12.52 | 13.2 | 1 [398] | 1.23 MB → (closed) |
| raof-1380-bounce i | 495 | 11.03 | 11.6 | 0  | 2.43 MB → 2.43 MB |
| raof-1380-bounce i | after the load | restart 12.51 s | | checks {'primary': 'true', 'replica': 'true', 'restart': 'true'} | |
| raof-1380-bounce p | before the first bounce | | | 0  | |
| raof-1380-bounce p | 135 | 15.52 | 16.2 | 0  | 2.18 MB → 15.60 MB |
| raof-1380-bounce p | 255 | 12.06 | 12.6 | 0  | 1.36 MB → 24.78 MB |
| raof-1380-bounce p | 375 | 12.71 | 13.2 | 0  | 1.21 MB → 1.21 MB |
| raof-1380-bounce p | 495 | 12.77 | 13.4 | 0  | 0.13 MB → 2.21 MB |
| raof-1380-bounce p | after the load | restart 12.26 s | | checks {'primary': 'true', 'replica': 'true', 'restart': 'true'} | |
