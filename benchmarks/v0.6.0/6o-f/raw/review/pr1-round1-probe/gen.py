import time
M=9223372036854775807
now=int(time.time()*1000)
# a relative ms that fits with ~1 day margin
okpx = M - now - 86400000
badpx = M - now + 86400000
cmds = []
def k(*a): cmds.append(' '.join(str(x) for x in a))
k('FLUSHALL')
for setup, c in [
  ('SET k v', f'SET k v EX {M//1000}'),
  ('SET k v', f'SET k v EX {M//1000+1}'),
  ('SET k v', f'SET k v PX {okpx}'),
  ('SET k v', f'SET k v PX {badpx}'),
  ('SET k v', f'SET k v EXAT {M//1000}'),
  ('SET k v', f'SET k v EXAT {M//1000+1}'),
  ('SET k v', f'SET k v PXAT {M}'),
  ('SET k v', f'SET k v PXAT {M+1}'),
  ('SET k v', f'SET k v EX 0'),
  ('SET k v', f'SET k v EX -1'),
  ('SET k v', f'SET k v PXAT 1'),
  ('SET k v', f'SET k v EXAT 1'),
  ('SET k v', f'SET k v NX EX {M//1000}'),
  ('DEL k', f'SET k v XX EX {M//1000}'),
  ('DEL k', f'SET k v XX PXAT {M}'),
  ('SET k v', f'SET k w GET EX {M//1000}'),
  ('RPUSH k v', f'SET k w GET EX {M//1000}'),
  ('RPUSH k v', f'SET k w GET PXAT {M}'),
  ('SET k v', f'SET k w NX GET PX {badpx}'),
  ('SET k v', f'SETEX k {M//1000} v'),
  ('SET k v', f'SETEX k {okpx//1000} v'),
  ('SET k v', f'PSETEX k {okpx} v'),
  ('SET k v', f'PSETEX k {badpx} v'),
  ('SET k v', f'PSETEX k {M} v'),
  ('SET k v', f'SETEX k 0 v'),
  ('SET k v', f'EXPIRE k {M}'),
  ('SET k v', f'EXPIRE k {M//1000}'),
  ('SET k v', f'EXPIRE k {okpx//1000}'),
  ('SET k v', f'PEXPIRE k {okpx}'),
  ('SET k v', f'PEXPIRE k {badpx}'),
  ('SET k v', f'PEXPIRE k {M}'),
  ('SET k v', f'EXPIREAT k {M//1000}'),
  ('SET k v', f'EXPIREAT k {M//1000+1}'),
  ('SET k v', f'EXPIREAT k {M}'),
  ('SET k v', f'PEXPIREAT k {M}'),
  ('SET k v', f'PEXPIREAT k {M+1}'),
  ('SET k v', f'EXPIRE k 0'),
  ('SET k v', f'EXPIRE k -1'),
  ('SET k v', f'EXPIREAT k 0'),
  ('SET k v', f'PEXPIREAT k 1'),
  ('DEL k', f'EXPIRE k {M}'),
  ('DEL k', f'PEXPIRE k {badpx}'),
  ('DEL k', f'EXPIREAT k {M}'),
  ('RPUSH k v', f'EXPIRE k {M//1000}'),
  ('SET k v', f'GETEX k EX {M//1000}'),
  ('SET k v', f'GETEX k EX {M//1000+1}'),
  ('SET k v', f'GETEX k PX {badpx}'),
  ('SET k v', f'GETEX k PX {okpx}'),
  ('SET k v', f'GETEX k EXAT {M//1000}'),
  ('SET k v', f'GETEX k EXAT {M//1000+1}'),
  ('SET k v', f'GETEX k PXAT {M}'),
  ('SET k v', f'GETEX k EX 0'),
  ('SET k v', f'GETEX k EXAT 1'),
  ('DEL k', f'GETEX k EX {M//1000}'),
  ('DEL k', f'GETEX k PX {badpx}'),
  ('DEL k', f'GETEX k EX {M//1000+1}'),
  ('DEL k', f'GETEX k EX 0'),
  ('DEL k', f'GETEX k PX -5'),
  ('RPUSH k v', f'GETEX k PX {badpx}'),
  ('RPUSH k v', f'GETEX k EX {M//1000}'),
  ('RPUSH k v', f'GETEX k EX {M//1000+1}'),
  ('RPUSH k v', f'GETEX k EX 0'),
  ('RPUSH k v', f'GETEX k PXAT {M}'),
  ('SET k v PX 1', f'GETEX k PX {badpx}'),
]:
  k('DEL k'); k(setup)
  if setup=='SET k v PX 1': k('DEBUG SLEEP 0.01')
  k(c); k('PEXPIRETIME k')
# MULTI
k('DEL k'); k('SET k v'); k('MULTI'); k(f'SET k v EX {M//1000}'); k('SET k2 x'); k(f'EXPIRE k {M//1000}'); k(f'GETEX k PX {badpx}'); k('EXEC'); k('GET k2')
k('MULTI'); k('SET k3 x'); k(f'GETEX k EX {M//1000+1}'); k('EXEC'); k('GET k3')
k('MULTI'); k('SET k4 x'); k('SET k v EX 0'); k('EXEC'); k('GET k4')
open('cmds.txt','w').write('\n'.join(cmds)+'\n')
