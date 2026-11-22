samples: 22474, user cycles/op 13978

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   9.1%   1277.5 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|  37.0%   5176.7 | `borrowed execute (try_execute_borrowed)` |
|   0.0%      0.0 | `ordered write entry (execute_ordered_write)` |
|  29.1%   4073.9 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   3.2%    444.1 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   6.0%    835.9 | `clock reads` |
|   5.5%    770.6 | `metrics record` |
|   1.1%    149.3 | `reply serialize (Value)` |
|   0.9%    129.4 | `Value drop` |
|   0.0%      0.0 | `Command drop` |
|   2.1%    299.2 | `allocator (all)` |
|   3.4%    473.3 | `select overflow arm (mpsc recv)` |
|   8.4%   1175.5 | `BufWriter flush (all)` |
|   9.6%   1339.1 | `AofGatedWriter (all)` |
|   2.0%    281.8 | `DeferredAof pop` |
|   3.3%    462.7 | `socket read (poll_read_priv)` |
|   1.6%    221.4 | `socket write (poll_write_priv)` |
|   9.6%   1339.1 | `split lock (ReadHalf/WriteHalf)` |
|  14.2%   1987.8 | `tokio park/driver` |
|  76.0%  10618.3 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  22.2% |    3099 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   4.4% |     610 |
| connection (decode, reply, socket) | tokio mpsc |   1.8% |     253 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.9% |     126 |
| **connection (decode, reply, socket)** | **all** | ** 29.2%** | **   4088** |
| store execute | code |  21.6% |    3025 |
| store execute | libc (memcpy, syscall wrappers) |   5.4% |     749 |
| store execute | allocator |   2.1% |     299 |
| **store execute** | **all** | ** 29.1%** | **   4074** |
| tokio scheduler | tokio wake/schedule |  12.8% |    1795 |
| tokio scheduler | code |   9.0% |    1258 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   4.1% |     567 |
| tokio scheduler | tokio io/split |   1.6% |     220 |
| **tokio scheduler** | **all** | ** 27.5%** | **   3840** |
| AOF append/lead/gate | tokio io/split |   5.9% |     822 |
| AOF append/lead/gate | code |   3.5% |     490 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.8% |     118 |
| **AOF append/lead/gate** | **all** | ** 10.2%** | **   1429** |
| tokio io driver | libc (memcpy, syscall wrappers) |   2.2% |     301 |
| tokio io driver | code |   1.8% |     246 |
| **tokio io driver** | **all** | **  3.9%** | **    547** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   5.9%    831.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   5.8%    808.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   4.3%    597.7 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   3.6%    506.9 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   3.5%    483.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|   2.4%    332.8 | `recached_server::clients::record_metrics` |
|   2.4%    329.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.3%    317.2 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.1%    300.4 | `[vdso]:+a8c` |
|   2.1%    297.3 | `libc.so.6:+1c7919` |
|   2.0%    281.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   2.0%    276.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.8%    256.9 | `libc.so.6:+1c79c7` |
|   1.8%    256.9 | `recached_server::propagation::try_execute_borrowed` |
|   1.7%    239.5 | `core_engine::request::build_ref` |
|   1.7%    232.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.6%    222.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   1.6%    220.2 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.6%    219.6 | `libc.so.6:+1c7d91` |
|   1.6%    218.9 | `<tokio::runtime::io::driver::Driver>::turn` |
|   1.6%    217.1 | `<core_engine::store::KeyIndex>::sync` |
|   1.5%    214.6 | `core::str::converts::from_utf8` |
|   1.4%    190.9 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.4%    189.7 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.3%    188.5 | `<core_engine::resp::Value>::parse_inner` |
|   1.2%    172.9 | `_rjem_malloc` |
|   1.2%    162.3 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.1%    158.0 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.1%    155.5 | `<tokio::runtime::time::Driver>::park_internal` |
|   1.1%    154.2 | `core_engine::store::lock_stripe` |
|   1.1%    148.7 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.1%    147.4 | `<core_engine::resp::Value>::serialize_into` |
|   1.0%    134.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.0%    133.7 | `<tokio::process::imp::GlobalOrphanQueue>::reap_orphans` |
|   1.0%    133.1 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.9%    132.5 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.9%    130.6 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   0.9%    129.4 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.9%    128.1 | `<core_engine::request::Args>::key_ref` |
|   0.9%    126.3 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.9%    122.5 | `tokio::runtime::task::waker::clone_waker` |
|   0.9%    121.3 | `core_engine::store::entry_size` |
|   0.9%    120.7 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.8%    115.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.8%    108.8 | `<tokio::runtime::time::wheel::Wheel>::next_expiration` |
|   0.8%    105.7 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.7%    103.9 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.7%    100.8 | `core_engine::store::write_slot::<&str>` |
|   0.7%     98.3 | `_rjem_sdallocx` |
|   0.7%     95.8 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.6%     90.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.6%     88.9 | `libc.so.6:+17362f` |
|   0.6%     82.7 | `<tokio::runtime::time::handle::Handle>::process_at_time` |
|   0.6%     79.6 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.6%     78.4 | `tokio::runtime::task::waker::wake_by_val` |
|   0.6%     77.1 | `[unknown]` |
|   0.5%     76.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.5%     68.4 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.5%     66.6 | `<tokio::runtime::signal::Driver>::process` |
|   0.5%     65.9 | `<std::sys::pal::unix::time::Timespec>::now` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  13978.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  76.0%  10618.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  75.0%  10484.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  73.6%  10294.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  70.6%   9872.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  37.0%   5176.7 | `recached_server::propagation::try_execute_borrowed` |
|  29.1%   4073.9 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  17.4%   2430.1 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  14.2%   1987.8 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|  12.0%   1683.7 | `core_engine::store::write_slot::<&str>` |
|  11.5%   1610.3 | `<tokio::runtime::time::Driver>::park_internal` |
|  10.2%   1426.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   9.1%   1277.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|   8.4%   1175.5 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   6.5%    901.9 | `<tokio::runtime::io::driver::Driver>::turn` |
|   6.0%    838.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   6.0%    835.9 | `<std::sys::pal::unix::time::Timespec>::now` |
|   5.5%    770.6 | `recached_server::clients::record_metrics` |
|   4.9%    680.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   4.7%    654.9 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   4.5%    631.9 | `libc.so.6:+139b5b` |
|   4.3%    597.7 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   4.3%    597.7 | `libc.so.6:+edc42` |
|   4.3%    597.7 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   4.3%    597.7 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   4.3%    597.7 | `libc.so.6:+17319c` |
|   4.2%    582.2 | `core_engine::request::build_ref` |
|   4.2%    580.3 | `<core_engine::store::KeyIndex>::sync` |
|   3.4%    473.3 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.3%    462.7 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.7%    372.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.5%    354.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.5%    342.7 | `<core_engine::request::Args>::key_ref` |
|   2.4%    335.9 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   2.4%    329.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.1%    300.4 | `[vdso]:+a8c` |
|   2.1%    297.3 | `libc.so.6:+1c7919` |
|   2.1%    292.3 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.0%    281.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.9%    263.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.8%    256.9 | `libc.so.6:+1c79c7` |
|   1.8%    248.2 | `tokio::runtime::task::waker::wake_by_val` |
|   1.8%    248.2 | `libm.so.6:+c8227` |
|   1.6%    221.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.6%    220.2 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.6%    219.6 | `libc.so.6:+1c7d91` |
|   1.5%    214.6 | `core::str::converts::from_utf8` |
|   1.3%    188.5 | `<core_engine::resp::Value>::parse_inner` |
|   1.2%    172.9 | `_rjem_malloc` |
|   1.2%    163.0 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.1%    154.2 | `core_engine::store::lock_stripe` |
|   1.1%    152.4 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.1%    149.9 | `<tokio::runtime::time::handle::Handle>::process_at_time` |
|   1.1%    149.3 | `<core_engine::resp::Value>::serialize_into` |
|   1.0%    133.7 | `<tokio::process::imp::GlobalOrphanQueue>::reap_orphans` |
|   0.9%    132.5 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.9%    129.4 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.9%    122.5 | `tokio::runtime::task::waker::clone_waker` |
|   0.9%    121.3 | `core_engine::store::entry_size` |
|   0.8%    108.8 | `<tokio::runtime::time::wheel::Wheel>::next_expiration` |
