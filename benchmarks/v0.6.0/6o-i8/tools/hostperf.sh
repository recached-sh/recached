#!/bin/bash
# Run the Flatpak SDK's perf natively on the host through the runtime's own loader.
R=${FLATPAK_SDK_FILES:-/var/lib/flatpak/runtime/org.freedesktop.Sdk/x86_64/26.08/active/files}
exec $R/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 --library-path $R/lib/x86_64-linux-gnu $R/bin/perf "$@"
