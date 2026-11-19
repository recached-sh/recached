Traceback (most recent call last):
  File "/home/<user>/Documents/OpenSource/recached-6n1/benchmarks/v0.6.0/6m/summarize.py", line 86, in <module>
    main()
    ~~~~^^
  File "/home/<user>/Documents/OpenSource/recached-6n1/benchmarks/v0.6.0/6m/summarize.py", line 40, in main
    rows = list(csv.DictReader(open(d / "results.csv")))
                               ~~~~^^^^^^^^^^^^^^^^^^^
NotADirectoryError: [Errno 20] Not a directory: '/home/<user>/recached-6n1-bench/results-20261118T165242Z/AB/A3-w4-set-c50/results.csv/results.csv'
