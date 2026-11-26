| scenario | redis | base |
|---|---|---|
| fixed-window | ['3', 'live', None, -2] | ['3', 'live', '3', -1] ✗ |
| sliding-window | ['3', 'live', None] | ['3', 'live', None] ✓ |
| incr-then-extend | ['2', 'live'] | ['2', 'live'] ✓ |
| incr-then-persist | ['2', -1] | ['2', -1] ✓ |
| incr-then-rename | [None, 0] | ['2', 0] ✗ |
| recreated-after-expiry | ['1', -1] | ['1', -1] ✓ |
| counter-in-multi | [None] | ['3'] ✗ |
| multi-after-expiry | ['1', 'live'] | ['1', 'live'] ✓ |
| append-live | [None] | ['+'] ✗ |
| append-after-expiry | ['new', -1] | ['new', -1] ✓ |
| hincrby-live | [[]] | [['3', 'f']] ✗ |
| hincrby-after-expiry | [['2', 'f'], -1] | [['2', 'f'], -1] ✓ |
| rpush-after-expiry | [['z'], -1] | [['z'], -1] ✓ |
| lpush-live | [[]] | [['z']] ✗ |
| sadd-after-expiry | [['z'], -1] | [['z'], -1] ✓ |
| zadd-after-expiry | [['q'], -1] | [['q'], -1] ✓ |
| lset-volatile | [[]] | ['RESTART REFUSED'] ✗ |
| rename-volatile-source | [None] | ['RESTART REFUSED'] ✗ |
| snapshot-then-aof | [None, None, '1', -1] | ['3', '+', '4', -1] ✗ |
| multiple-restarts | ['c', -1, 'c'] | ['bc', -1, 'bc'] ✗ |

10 cell(s) differ from redis
