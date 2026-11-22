samples: 17815, user cycles/op 5735

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|   6.9%    394.0 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|  38.8%   2226.7 | `borrowed execute (try_execute_borrowed)` |
|   0.0%      0.0 | `ordered write entry (execute_ordered_write)` |
|  31.2%   1787.3 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   1.7%     98.5 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   4.0%    231.8 | `clock reads` |
|   4.7%    271.7 | `metrics record` |
|   0.4%     25.1 | `reply serialize (Value)` |
|   0.2%     12.9 | `Value drop` |
|   0.0%      0.0 | `Command drop` |
|   0.4%     25.1 | `allocator (all)` |
|   0.0%      0.0 | `select overflow arm (mpsc recv)` |
|  11.3%    646.7 | `BufWriter flush (all)` |
|  12.1%    695.7 | `AofGatedWriter (all)` |
|   1.3%     72.8 | `DeferredAof pop` |
|   4.3%    247.9 | `socket read (poll_read_priv)` |
|   3.9%    221.2 | `socket write (poll_write_priv)` |
|  12.1%    695.7 | `split lock (ReadHalf/WriteHalf)` |
|   5.9%    339.3 | `tokio park/driver` |
|  88.2%   5060.1 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| store execute | code |  22.0% |    1264 |
| store execute | libc (memcpy, syscall wrappers) |   8.7% |     498 |
| store execute | allocator |   0.4% |      25 |
| **store execute** | **all** | ** 31.2%** | **   1787** |
| connection (decode, reply, socket) | code |  25.5% |    1461 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   4.5% |     256 |
| **connection (decode, reply, socket)** | **all** | ** 29.9%** | **   1716** |
| tokio scheduler | tokio wake/schedule |  18.4% |    1054 |
| tokio scheduler | code |   4.7% |     272 |
| tokio scheduler | tokio io/split |   2.3% |     133 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.6% |      36 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | ** 26.1%** | **   1496** |
| AOF append/lead/gate | tokio io/split |   6.8% |     389 |
| AOF append/lead/gate | code |   3.3% |     190 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.9% |     108 |
| **AOF append/lead/gate** | **all** | ** 12.0%** | **    687** |
| tokio io driver | code |   0.7% |      39 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.2% |      10 |
| **tokio io driver** | **all** | **  0.8%** | **     49** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  11.6%    666.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  11.2%    642.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   5.8%    332.5 | `libc.so.6:+1c7919` |
|   4.4%    254.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   4.2%    242.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   4.1%    232.4 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   3.5%    198.6 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.2%    185.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.3%    132.9 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.1%    119.1 | `[vdso]:+a8c` |
|   2.1%    118.8 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|   2.0%    113.3 | `recached_server::clients::record_metrics` |
|   1.9%    109.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.9%    106.6 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.8%    100.8 | `recached_server::propagation::try_execute_borrowed` |
|   1.7%     98.5 | `tokio::runtime::task::waker::wake_by_val` |
|   1.6%     93.0 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.6%     91.4 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.6%     90.5 | `core_engine::store::lock_stripe` |
|   1.3%     75.6 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.3%     72.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.1%     64.7 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%     63.7 | `core::str::converts::from_utf8` |
|   1.1%     60.8 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.0%     57.6 | `core_engine::store::entry_size` |
|   1.0%     55.7 | `<core_engine::store::KeyIndex>::sync` |
|   1.0%     55.4 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.9%     53.4 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.9%     53.1 | `tokio::runtime::task::waker::clone_waker` |
|   0.9%     51.8 | `libc.so.6:+1c79c7` |
|   0.9%     49.9 | `[unknown]` |
|   0.9%     48.9 | `core_engine::request::build_ref` |
|   0.8%     47.6 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.8%     45.4 | `libc.so.6:+175818` |
|   0.8%     45.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.8%     44.1 | `<core_engine::request::Args>::key_ref` |
|   0.7%     37.7 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   0.6%     36.4 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.6%     36.1 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.6%     34.1 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.6%     33.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.6%     33.8 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.5%     31.2 | `libc.so.6:+1c7d91` |
|   0.4%     20.9 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.4%     20.6 | `<core_engine::resp::Value>::serialize_into` |
|   0.3%     18.0 | `libc.so.6:+f6332` |
|   0.3%     17.7 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.3%     17.7 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.3%     17.1 | `libc.so.6:+1c791d` |
|   0.3%     16.1 | `<core_engine::resp::Value>::parse_inner` |
|   0.3%     15.5 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   0.3%     14.8 | `core_engine::store::write_slot::<&str>` |
|   0.2%     13.8 | `libc.so.6:+17581c` |
|   0.2%     13.2 | `[vdso]:+ac2` |
|   0.2%     12.9 | `<tokio::runtime::scheduler::defer::Defer>::defer` |
|   0.2%     12.9 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   0.2%     12.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   0.2%     11.3 | `__floattidf` |
|   0.2%     10.9 | `libc.so.6:+175826` |
|   0.2%     10.6 | `_rjem_sdallocx` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   5734.9 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  88.2%   5060.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  84.0%   4817.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  79.9%   4584.4 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  76.4%   4380.3 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  38.8%   2226.7 | `recached_server::propagation::try_execute_borrowed` |
|  31.2%   1787.3 | `<core_engine::store::KeyValueStore>::execute_ref_into` |
|  24.2%   1388.4 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  19.6%   1125.7 | `core_engine::store::write_slot::<&str>` |
|  18.3%   1050.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  11.3%    646.7 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   8.5%    487.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   6.9%    394.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   6.6%    380.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   5.9%    339.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   5.8%    332.5 | `libc.so.6:+1c7919` |
|   4.7%    271.7 | `recached_server::clients::record_metrics` |
|   4.4%    254.3 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   4.4%    254.3 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   4.4%    254.3 | `libc.so.6:+edc42` |
|   4.4%    254.3 | `libc.so.6:+17319c` |
|   4.4%    254.3 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   4.3%    247.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   4.0%    231.8 | `<tokio::runtime::time::Driver>::park_internal` |
|   4.0%    231.8 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.9%    221.2 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   3.4%    196.4 | `<tokio::runtime::io::driver::Driver>::turn` |
|   3.3%    190.3 | `libc.so.6:+139b5b` |
|   3.0%    173.8 | `tokio::runtime::task::waker::wake_by_val` |
|   2.7%    156.8 | `core_engine::request::build_ref` |
|   2.7%    155.5 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.6%    147.8 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   2.3%    131.7 | `<core_engine::store::KeyIndex>::sync` |
|   2.1%    119.1 | `[vdso]:+a8c` |
|   2.1%    118.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.9%    107.8 | `<core_engine::request::Args>::key_ref` |
|   1.6%     90.5 | `core_engine::store::lock_stripe` |
|   1.6%     89.5 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   1.3%     72.8 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.2%     70.8 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.2%     68.6 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   1.2%     68.6 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   1.1%     64.7 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%     63.7 | `core::str::converts::from_utf8` |
|   1.1%     61.8 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   1.0%     60.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.0%     57.6 | `core_engine::store::entry_size` |
|   0.9%     53.1 | `tokio::runtime::task::waker::clone_waker` |
|   0.9%     51.8 | `libc.so.6:+1c79c7` |
|   0.9%     49.9 | `[unknown]` |
|   0.9%     48.9 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.8%     47.6 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.8%     45.4 | `libc.so.6:+175818` |
|   0.8%     43.5 | `<tokio::runtime::scheduler::defer::Defer>::defer` |
|   0.7%     38.3 | `libc.so.6:+175813` |
|   0.6%     34.1 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.5%     31.2 | `libc.so.6:+1c7d91` |
|   0.5%     30.9 | `libc.so.6:+1754b3` |
|   0.4%     25.1 | `<core_engine::resp::Value>::serialize_into` |
|   0.4%     21.2 | `libm.so.6:+c8227` |
