ClockQ (or something?)
===

A tool for tracking tasks that need to be done regularly.

Usage
---

```
$ clockq add "water plants" --frequency 7

Task                 Last completed
===                  ===
water plants         Never
```

```
$ clockq add "tidy house" --frequency 7 --done

Task                 Last completed
===                  ===
water plants         Never
tidy house           2017-10-22           Today        (Due in 7 days)
```

```
$ clockq did plants

Mark task 'water plants' as done on 2017-10-22? (y/N)
y
Task                 Last completed
===                  ===
water plants         2017-10-22           Today        (Due in 7 days)
tidy house           2017-10-22           Today        (Due in 7 days)
```

```
$ clockq did house --on 2017-01-01 -y

Task                 Last completed
===                  ===
tidy house           2017-01-02       293 days ago     (286 days overdue!)
water plants         2017-10-22           Today        (Due in 7 days)
```
