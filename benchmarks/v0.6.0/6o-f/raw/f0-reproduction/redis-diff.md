| check | redis | base | ttl |
|---|---|---|---|
| live replies | [6, 10, -2, 1, 1, 2] | [6, 10, -2, 1, 1, 2] | [6, 10, -2, 1, 1, 2] |
| short GET/PTTL | (None, -2) | ('10', -1) | (None, -2) |
| long GET/exact deadline | ('-2', True) | ('-2', True) | ('-2', True) |
| plain GET/PTTL | ('1', -1) | ('1', -1) | ('1', -1) |
| window GET/exact deadline | ('2', True) | ('2', True) | ('2', True) |
redis: MATCHES redis
base: DIFFERS FROM redis
ttl: MATCHES redis
