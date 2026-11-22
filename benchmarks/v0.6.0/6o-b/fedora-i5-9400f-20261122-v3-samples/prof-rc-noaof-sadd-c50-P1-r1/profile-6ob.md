samples: 18072, user cycles/op 6333

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  10.4%    660.9 | `decode (all)` |
|   0.0%      0.4 | `decode: general Value path (Args::general)` |
|   0.0%      0.4 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  40.0%   2532.0 | `ordered write entry (execute_ordered_write)` |
|  37.5%   2375.0 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   2.2%    138.8 | `store: key hash (sip)` |
|   0.0%      0.0 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   3.7%    235.5 | `clock reads` |
|   5.0%    314.3 | `metrics record` |
|   0.3%     22.1 | `reply serialize (Value)` |
|   0.1%      8.4 | `Value drop` |
|   0.7%     45.2 | `Command drop` |
|   2.0%    127.2 | `allocator (all)` |
|   0.0%      0.0 | `select overflow arm (mpsc recv)` |
|   8.9%    562.1 | `BufWriter flush (all)` |
|   9.4%    592.9 | `AofGatedWriter (all)` |
|   0.7%     42.4 | `DeferredAof pop` |
|   3.8%    238.6 | `socket read (poll_read_priv)` |
|   3.3%    209.9 | `socket write (poll_write_priv)` |
|   9.4%    592.9 | `split lock (ReadHalf/WriteHalf)` |
|   5.2%    329.7 | `tokio park/driver` |
|  90.0%   5696.9 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  28.6% |    1811 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.8% |     175 |
| connection (decode, reply, socket) | allocator |   1.9% |     122 |
| **connection (decode, reply, socket)** | **all** | ** 33.3%** | **   2108** |
| store execute | code |  24.7% |    1565 |
| store execute | libc (memcpy, syscall wrappers) |   7.6% |     479 |
| store execute | allocator |   0.1% |       5 |
| **store execute** | **all** | ** 32.3%** | **   2048** |
| tokio scheduler | tokio wake/schedule |  15.2% |     964 |
| tokio scheduler | code |   4.0% |     253 |
| tokio scheduler | tokio io/split |   2.0% |     124 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.6% |      35 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 21.7%** | **   1377** |
| AOF append/lead/gate | tokio io/split |   5.2% |     331 |
| AOF append/lead/gate | code |   2.4% |     154 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.7% |     107 |
| **AOF append/lead/gate** | **all** | **  9.4%** | **    593** |
| ordered write path (rest) | code |   2.5% |     157 |
| **ordered write path (rest)** | **all** | **  2.5%** | **    157** |
| tokio io driver | code |   0.6% |      40 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       9 |
| **tokio io driver** | **all** | **  0.8%** | **     49** |
| other | code |   0.0% |       0 |
| **other** | **all** | **  0.0%** | **      0** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  13.5%    857.4 | `<hashbrown::raw::RawTable<usize>>::find::<indexmap::inner::equivalent<alloc::string::String, (), str>::{closu…` |
|  12.1%    766.0 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.2%    331.1 | `libc.so.6:+1c7919` |
|   4.3%    274.0 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.9%    248.1 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   3.6%    227.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   3.1%    194.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   2.7%    171.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.3%    143.3 | `recached_server::clients::record_metrics` |
|   2.3%    142.6 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.0%    124.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.8%    111.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.6%    104.4 | `[vdso]:+a8c` |
|   1.6%    100.9 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.5%     95.3 | `tokio::runtime::task::waker::wake_by_val` |
|   1.4%     91.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.4%     90.4 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.4%     90.1 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   1.4%     89.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.4%     85.5 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.2%     78.5 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.2%     76.7 | `_rjem_malloc` |
|   1.0%     65.9 | `core_engine::request::build` |
|   1.0%     62.7 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.9%     59.9 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.9%     58.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.9%     58.2 | `core_engine::request::build_ref` |
|   0.8%     52.2 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|   0.8%     51.2 | `[unknown]` |
|   0.8%     49.4 | `core::str::converts::from_utf8` |
|   0.8%     48.4 | `tokio::runtime::task::waker::clone_waker` |
|   0.7%     47.0 | `libc.so.6:+175818` |
|   0.7%     45.2 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.7%     43.1 | `_rjem_sdallocx` |
|   0.7%     42.4 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.6%     40.6 | `recached_server::clients::execute_and_record_ref` |
|   0.6%     38.2 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.6%     37.8 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.6%     37.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.6%     35.7 | `core_engine::store::lock_stripe` |
|   0.6%     35.4 | `<core_engine::request::Args>::name_is` |
|   0.5%     34.0 | `<core_engine::store::CompactSet>::contains` |
|   0.5%     33.3 | `recached_server::propagation::is_write_command` |
|   0.5%     30.8 | `core_engine::request::texts` |
|   0.5%     30.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.5%     30.1 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   0.4%     27.7 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.4%     27.3 | `core_engine::resp::write_header` |
|   0.4%     25.9 | `libc.so.6:+1c7d91` |
|   0.4%     24.5 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.4%     24.5 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.3%     22.1 | `<core_engine::resp::Value>::serialize_into` |
|   0.3%     21.7 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.3%     21.0 | `core::ptr::drop_glue::<core_engine::cmd::Command>` |
|   0.3%     17.9 | `core_engine::store::write_slot::<&str>` |
|   0.3%     16.5 | `libc.so.6:+1c791d` |
|   0.2%     15.8 | `libc.so.6:+f6332` |
|   0.2%     15.8 | `[vdso]:+ac2` |
|   0.2%     15.8 | `<core_engine::request::Args>::key_ref` |
|   0.2%     15.4 | `libc.so.6:+175826` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   6332.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  90.0%   5696.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  86.3%   5468.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  83.3%   5274.0 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  80.2%   5075.6 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  39.8%   2517.7 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  37.5%   2375.0 | `recached_server::clients::execute_and_record_ref` |
|  30.3%   1921.6 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  28.1%   1776.9 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  27.2%   1724.7 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  20.3%   1288.1 | `<core_engine::store::CompactSet>::contains` |
|  18.7%   1181.2 | `<hashbrown::raw::RawTable<usize>>::find::<indexmap::inner::equivalent<alloc::string::String, (), str>::{closu…` |
|  10.4%    660.9 | `<core_engine::request::RequestDecoder>::decode_any` |
|   8.9%    562.1 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   7.0%    441.2 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   5.6%    351.8 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   5.4%    339.2 | `core_engine::request::build` |
|   5.2%    331.1 | `libc.so.6:+1c7919` |
|   5.2%    329.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   5.0%    314.3 | `recached_server::clients::record_metrics` |
|   3.9%    248.1 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   3.9%    248.1 | `libc.so.6:+17319c` |
|   3.9%    248.1 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   3.9%    248.1 | `libc.so.6:+edc42` |
|   3.9%    248.1 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   3.8%    238.6 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   3.7%    235.5 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.5%    220.1 | `<tokio::runtime::time::Driver>::park_internal` |
|   3.3%    210.2 | `core_engine::store::write_slot::<&str>` |
|   3.3%    209.9 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.9%    183.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   2.8%    179.4 | `libc.so.6:+139b5b` |
|   2.8%    178.4 | `<tokio::runtime::io::driver::Driver>::turn` |
|   2.6%    163.3 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   2.4%    149.3 | `tokio::runtime::task::waker::wake_by_val` |
|   2.3%    144.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   2.0%    129.3 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.8%    111.1 | `<core_engine::request::Args>::key` |
|   1.6%    104.4 | `[vdso]:+a8c` |
|   1.6%    102.7 | `core_engine::request::texts` |
|   1.6%    100.9 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.3%     80.2 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   1.2%     76.7 | `_rjem_malloc` |
|   1.2%     73.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.0%     63.4 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.9%     58.2 | `core_engine::request::build_ref` |
|   0.8%     51.2 | `[unknown]` |
|   0.8%     49.4 | `core::str::converts::from_utf8` |
|   0.8%     49.4 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task` |
|   0.8%     49.4 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.8%     49.1 | `<tokio::runtime::scheduler::multi_thread::stats::Stats>::end_processing_scheduled_tasks` |
|   0.8%     48.4 | `tokio::runtime::task::waker::clone_waker` |
|   0.7%     47.0 | `libc.so.6:+175818` |
|   0.7%     45.2 | `core::ptr::drop_glue::<core_engine::cmd::Command>` |
|   0.7%     43.5 | `<core_engine::request::Args>::key_ref` |
|   0.7%     43.1 | `_rjem_sdallocx` |
|   0.7%     42.4 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.7%     42.4 | `<tokio::runtime::scheduler::defer::Defer>::defer` |
|   0.6%     37.8 | `<tokio::runtime::scheduler::multi_thread::handle::Handle>::schedule_task::{closure#0}` |
|   0.6%     36.4 | `core_engine::resp::write_header` |
