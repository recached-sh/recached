samples: 17423, user cycles/op 5202

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   9.9%    513.2 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  25.3%   1316.0 | `ordered write entry (execute_ordered_write)` |
|  22.4%   1166.4 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   1.7%     90.5 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   3.9%    204.8 | `clock reads` |
|   5.9%    307.2 | `metrics record` |
|   0.5%     26.6 | `reply serialize (Value)` |
|   0.2%     10.2 | `Value drop` |
|   0.4%     20.0 | `Command drop` |
|   3.0%    157.3 | `allocator (all)` |
|   0.0%      0.0 | `select overflow arm (mpsc recv)` |
|  14.4%    748.2 | `BufWriter flush (all)` |
|  15.0%    781.3 | `AofGatedWriter (all)` |
|   1.2%     60.3 | `DeferredAof pop` |
|   5.2%    270.5 | `socket read (poll_read_priv)` |
|   5.0%    259.4 | `socket write (poll_write_priv)` |
|  15.0%    781.3 | `split lock (ReadHalf/WriteHalf)` |
|   6.0%    310.2 | `tokio park/driver` |
|  88.5%   4606.0 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  31.4% |    1632 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   4.1% |     213 |
| connection (decode, reply, socket) | allocator |   2.3% |     119 |
| **connection (decode, reply, socket)** | **all** | ** 37.8%** | **   1964** |
| tokio scheduler | tokio wake/schedule |  19.5% |    1017 |
| tokio scheduler | code |   4.2% |     219 |
| tokio scheduler | tokio io/split |   2.7% |     142 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.6% |      33 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 27.1%** | **   1412** |
| store execute | code |  12.3% |     638 |
| store execute | libc (memcpy, syscall wrappers) |   3.4% |     177 |
| store execute | allocator |   0.7% |      38 |
| **store execute** | **all** | ** 16.4%** | **    854** |
| AOF append/lead/gate | tokio io/split |   8.7% |     451 |
| AOF append/lead/gate | code |   3.9% |     201 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   2.3% |     121 |
| **AOF append/lead/gate** | **all** | ** 14.9%** | **    773** |
| ordered write path (rest) | code |   2.9% |     150 |
| **ordered write path (rest)** | **all** | **  2.9%** | **    150** |
| tokio io driver | code |   0.7% |      39 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.2% |      11 |
| **tokio io driver** | **all** | **  1.0%** | **     50** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  14.4%    747.3 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.1%    266.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   4.5%    232.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   4.4%    229.6 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.3%    222.1 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   2.8%    145.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.8%    143.9 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.7%    142.4 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.7%    140.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.6%    134.9 | `recached_server::clients::record_metrics` |
|   2.4%    123.9 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   2.2%    114.9 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.9%     98.5 | `[vdso]:+a8c` |
|   1.8%     95.5 | `tokio::runtime::task::waker::wake_by_val` |
|   1.8%     95.5 | `<core_engine::store::KeyValueStore>::push::<&str, &[u8], core::iter::adapters::map::Map<core::slice::iter::It…` |
|   1.8%     94.6 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.8%     93.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.6%     83.6 | `_rjem_malloc` |
|   1.5%     77.6 | `[unknown]` |
|   1.5%     76.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.3%     68.4 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|   1.2%     63.0 | `core_engine::request::build` |
|   1.2%     63.0 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   1.2%     60.6 | `libc.so.6:+175818` |
|   1.2%     60.3 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.1%     57.3 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.1%     55.2 | `tokio::runtime::task::waker::clone_waker` |
|   1.0%     52.2 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.0%     51.9 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.0%     50.2 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.9%     49.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.9%     47.8 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.8%     39.4 | `core_engine::store::lock_stripe` |
|   0.7%     38.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.7%     37.6 | `_rjem_sdallocx` |
|   0.7%     37.6 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.7%     37.3 | `core::str::converts::from_utf8` |
|   0.6%     29.9 | `libc.so.6:+1c7919` |
|   0.6%     28.7 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.5%     26.6 | `<core_engine::resp::Value>::serialize_into` |
|   0.5%     25.7 | `libc.so.6:+1c7d91` |
|   0.5%     25.4 | `core_engine::resp::format_i64` |
|   0.5%     25.4 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.5%     24.5 | `core_engine::store::write_slot::<&str>` |
|   0.5%     23.9 | `recached_server::clients::execute_and_record_ref` |
|   0.4%     23.3 | `core_engine::request::values` |
|   0.4%     23.3 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.4%     20.9 | `core_engine::request::build_ref` |
|   0.4%     19.4 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.3%     17.3 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.3%     17.0 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.3%     15.8 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.3%     14.3 | `libc.so.6:+f6332` |
|   0.3%     14.3 | `[vdso]:+ac2` |
|   0.3%     14.3 | `core_engine::resp::write_header` |
|   0.3%     14.0 | `libc.so.6:+1754b8` |
|   0.3%     13.7 | `core_engine::store::mutation_scope` |
|   0.3%     13.7 | `libc.so.6:+17581c` |
|   0.3%     13.4 | `<core_engine::resp::Value>::parse_inner` |
|   0.3%     13.4 | `recached_server::propagation::is_write_command` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   5201.6 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  88.5%   4606.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  83.4%   4337.9 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  78.9%   4105.9 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  75.7%   3939.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  25.2%   1310.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  22.4%   1166.4 | `recached_server::clients::execute_and_record_ref` |
|  14.7%    765.2 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  14.4%    748.2 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|  11.7%    607.5 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  11.1%    576.5 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|  10.4%    539.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  10.0%    518.9 | `<core_engine::store::KeyValueStore>::push::<&str, &[u8], core::iter::adapters::map::Map<core::slice::iter::It…` |
|   9.9%    513.2 | `<core_engine::request::RequestDecoder>::decode_any` |
|   8.7%    452.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   6.0%    310.2 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   5.9%    307.2 | `recached_server::clients::record_metrics` |
|   5.2%    270.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   5.0%    259.4 | `core_engine::request::build` |
|   5.0%    259.4 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   4.3%    222.1 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   4.3%    222.1 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   4.3%    222.1 | `libc.so.6:+17319c` |
|   4.3%    222.1 | `libc.so.6:+edc42` |
|   4.3%    222.1 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   4.1%    214.4 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.9%    204.8 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.7%    193.8 | `core_engine::store::write_slot::<&str>` |
|   3.5%    181.2 | `<tokio::runtime::io::driver::Driver>::turn` |
|   3.4%    175.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   3.3%    174.1 | `libc.so.6:+139b5b` |
|   3.1%    162.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   2.8%    146.9 | `tokio::runtime::task::waker::wake_by_val` |
|   2.5%    131.7 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.2%    115.8 | `<core_engine::request::Args>::key` |
|   2.1%    109.3 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.0%    106.0 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   1.9%     98.5 | `[vdso]:+a8c` |
|   1.6%     83.9 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.6%     83.6 | `_rjem_malloc` |
|   1.5%     77.6 | `[unknown]` |
|   1.4%     70.5 | `core_engine::request::values` |
|   1.2%     60.6 | `libc.so.6:+175818` |
|   1.2%     60.3 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.1%     58.2 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.1%     56.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.1%     55.5 | `core_engine::resp::write_header` |
|   1.1%     55.2 | `tokio::runtime::task::waker::clone_waker` |
|   1.0%     52.2 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.0%     51.3 | `_rjem_je_malloc_default` |
|   1.0%     50.5 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   0.9%     47.8 | `<core_engine::request::Args>::key_ref` |
|   0.9%     45.7 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.9%     45.1 | `_rjem_je_tcache_alloc_small_hard` |
|   0.8%     43.6 | `_rjem_je_arena_cache_bin_fill_small` |
|   0.8%     40.3 | `libc.so.6:+175813` |
|   0.8%     39.4 | `core_engine::store::lock_stripe` |
|   0.7%     37.6 | `_rjem_sdallocx` |
|   0.7%     37.3 | `core::str::converts::from_utf8` |
|   0.7%     36.7 | `libc.so.6:+1754b3` |
