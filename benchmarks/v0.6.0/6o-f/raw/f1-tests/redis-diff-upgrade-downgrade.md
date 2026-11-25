| check | redis | upgrade(base→ttl) | downgrade(ttl→base) |
|---|---|---|---|
| live replies | [6, 10, -2, 1, 1, 2] | [6, 10, -2, 1, 1, 2] | [6, 10, -2, 1, 1, 2] |
| short GET/PTTL | (None, -2) | ('10', -1) | (None, -2) |
| long GET/exact deadline | ('-2', True) | ('-2', True) | ('-2', True) |
| plain GET/PTTL | ('1', -1) | ('1', -1) | ('1', -1) |
| window GET/exact deadline | ('2', True) | ('2', True) | ('2', True) |
redis: MATCHES redis
upgrade(base→ttl): DIFFERS FROM redis
downgrade(ttl→base): MATCHES redis
