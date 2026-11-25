#!/bin/bash
# tx (without x2048: the screen showed 1,024 not promising), then barrier, then cmds.
B=$HOME/recached-6of-bench
echo "$(date -u +%FT%TZ) restarted: tx without x2048, then barrier, cmds" >> $B/f-driver.log
STAGE=tx LABELS_TX="base x256 x512 x1024 x1024s" bash $B/f-driver.sh
STAGE=barrier bash $B/f-driver.sh
STAGE=cmds bash $B/f-driver.sh
