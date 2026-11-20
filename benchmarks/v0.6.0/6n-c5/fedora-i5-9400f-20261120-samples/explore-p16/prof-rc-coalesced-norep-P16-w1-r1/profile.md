samples: 27110, user cycles/op 7562

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| frame build (broadcast_for_at) | allocator |  11.0% |     832 |
| frame build (broadcast_for_at) | code |   9.8% |     739 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.5% |     113 |
| **frame build (broadcast_for_at)** | **all** | ** 22.3%** | **   1684** |
| store execute | code |  15.0% |    1136 |
| store execute | libc (memcpy, syscall wrappers) |   7.0% |     529 |
| store execute | allocator |   0.1% |       6 |
| **store execute** | **all** | ** 22.1%** | **   1670** |
| connection (decode, reply, socket) | code |  13.6% |    1027 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.8% |     210 |
| connection (decode, reply, socket) | allocator |   0.5% |      34 |
| connection (decode, reply, socket) | tokio mpsc |   0.4% |      30 |
| connection (decode, reply, socket) | tokio io/split |   0.2% |      13 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.1% |       7 |
| **connection (decode, reply, socket)** | **all** | ** 17.5%** | **   1322** |
| ordered write path (rest) | code |   6.7% |     503 |
| ordered write path (rest) | tokio mpsc |   1.0% |      75 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.7% |      50 |
| ordered write path (rest) | allocator |   0.1% |       9 |
| **ordered write path (rest)** | **all** | **  8.4%** | **    638** |
| tokio scheduler | tokio wake/schedule |   5.7% |     431 |
| tokio scheduler | code |   2.2% |     168 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      13 |
| tokio scheduler | tokio io/split |   0.1% |      10 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | **  8.2%** | **    622** |
| on_write_logged (frame copy, dispatch) | code |   5.0% |     379 |
| on_write_logged (frame copy, dispatch) | allocator |   0.9% |      72 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.7% |      52 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  6.7%** | **    503** |
| AOF append/lead/gate | tokio wake/schedule |   2.6% |     196 |
| AOF append/lead/gate | code |   1.4% |     110 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.6% |      43 |
| AOF append/lead/gate | tokio io/split |   0.6% |      42 |
| AOF append/lead/gate | allocator |   0.1% |       9 |
| AOF append/lead/gate | atomics/Arc drop |   0.0% |       3 |
| **AOF append/lead/gate** | **all** | **  5.3%** | **    401** |
| stripe locks | code |   3.4% |     261 |
| stripe locks | tokio mpsc |   1.6% |     121 |
| **stripe locks** | **all** | **  5.1%** | **    382** |
| repl fan_out: backlog, offset, lock | code |   3.7% |     277 |
| repl fan_out: backlog, offset, lock | atomics/Arc drop |   0.3% |      26 |
| repl fan_out: backlog, offset, lock | allocator |   0.3% |      22 |
| repl fan_out: backlog, offset, lock | libc (memcpy, syscall wrappers) |   0.0% |       0 |
| **repl fan_out: backlog, offset, lock** | **all** | **  4.3%** | **    326** |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       8 |
| tokio io driver | code |   0.1% |       6 |
| **tokio io driver** | **all** | **  0.2%** | **     14** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.9%    670.0 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   7.6%    578.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.9%    373.8 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   4.5%    339.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   3.7%    277.3 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   3.5%    264.4 | `do_rallocx` |
|   2.6%    199.7 | `alloc::fmt::format::format_inner` |
|   2.6%    196.7 | `_rjem_je_arena_ralloc` |
|   2.3%    176.0 | `[vdso]:+a8c` |
|   2.2%    164.0 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   2.2%    162.9 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.9%    146.7 | `_rjem_je_arena_ralloc_no_move` |
|   1.8%    139.8 | `recached_server::clients::record_metrics` |
|   1.7%    127.8 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.6%    124.4 | `core::fmt::write` |
|   1.6%    121.6 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   1.4%    109.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.4%    103.5 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.3%     99.6 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   1.3%     98.7 | `_rjem_malloc` |
|   1.3%     97.4 | `recached_server::propagation::resp_push` |
|   1.3%     97.4 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.2%     90.9 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.2%     89.8 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%     83.1 | `_rjem_sdallocx` |
|   1.1%     81.7 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   1.1%     80.3 | `tokio::runtime::task::waker::wake_by_val` |
|   1.1%     79.8 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.0%     78.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   1.0%     78.7 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   1.0%     77.8 | `core_engine::request::build_ref` |
|   1.0%     75.3 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   0.9%     66.1 | `<core::fmt::Formatter>::pad_integral` |
|   0.9%     65.8 | `core::str::converts::from_utf8` |
|   0.9%     65.8 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   0.9%     65.3 | `<tokio::sync::notify::NotifiedProject>::drop_notified` |
|   0.8%     62.5 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   0.8%     60.3 | `core_engine::store::entry_size` |
|   0.8%     60.0 | `<usize>::_fmt_inner` |
|   0.8%     58.0 | `core_engine::store::lock_stripe` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   7562.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  96.5%   7297.8 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  95.7%   7233.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  94.2%   7124.0 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  92.4%   6990.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  77.2%   5841.1 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  67.5%   5102.2 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  39.9%   3020.7 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  25.8%   1951.2 | `recached_server::clients::execute_and_record_ref` |
|  22.3%   1684.0 | `recached_server::propagation::broadcast_for_at` |
|  22.0%   1665.9 | `recached_server::propagation::resp_push` |
|  20.7%   1562.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  18.5%   1397.5 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  17.8%   1346.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  17.6%   1329.7 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  15.4%   1165.7 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|  14.7%   1110.2 | `core_engine::store::write_slot::<&str>` |
|  13.7%   1035.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  11.0%    831.0 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|  10.3%    777.7 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   9.6%    724.1 | `alloc::fmt::format::format_inner` |
|   9.1%    691.5 | `do_rallocx` |
|   8.9%    670.0 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   6.1%    462.8 | `core::fmt::write` |
|   5.6%    421.5 | `_rjem_je_arena_ralloc` |
|   4.9%    374.1 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.5%    340.9 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.5%    338.1 | `<std::sys::pal::unix::time::Timespec>::now` |
|   4.3%    325.8 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   4.1%    310.5 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   3.6%    274.2 | `recached_server::clients::record_metrics` |
|   3.1%    236.3 | `<usize as core::fmt::Display>::fmt` |
|   3.0%    224.6 | `recached_server::persistence::gate_wait::{closure#0}` |
|   2.8%    212.3 | `<recached_server::persistence::AofDone>::wait::{closure#0}` |
|   2.4%    182.4 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   2.3%    176.0 | `[vdso]:+a8c` |
|   2.2%    169.9 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   2.1%    155.4 | `core_engine::request::build_ref` |
|   2.0%    149.2 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   1.9%    146.7 | `_rjem_je_arena_ralloc_no_move` |
