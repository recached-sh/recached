| check | redis | base | comb |
|---|---|---|---|
| replica reads | [None, '2', None, '2', [], -1] | [None, '2', None, '2', [], -1] ✓ | [None, '2', None, '2', [], -1] ✓ |
| promoted | [None, '2', None, '2', 'live'] | [None, '2', None, '2', 'live'] ✓ | [None, '2', None, '2', 'live'] ✓ |
| restarted | ['1', '2', None, '2', -1] | ['1', '2', '+', None, -1] ✗ | ['1', '2', None, '2', -1] ✓ |
