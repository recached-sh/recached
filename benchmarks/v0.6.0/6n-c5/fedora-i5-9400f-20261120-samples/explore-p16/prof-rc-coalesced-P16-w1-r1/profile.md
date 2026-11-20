samples: 46640, user cycles/op 9579

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| frame build (broadcast_for_at) | allocator |   9.1% |     871 |
| frame build (broadcast_for_at) | code |   8.6% |     828 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.3% |     127 |
| **frame build (broadcast_for_at)** | **all** | ** 19.1%** | **   1827** |
| store execute | code |  13.1% |    1255 |
| store execute | libc (memcpy, syscall wrappers) |   5.5% |     529 |
| store execute | allocator |   0.1% |       7 |
| **store execute** | **all** | ** 18.7%** | **   1790** |
| connection (decode, reply, socket) | code |  12.8% |    1226 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.4% |     229 |
| connection (decode, reply, socket) | allocator |   0.4% |      39 |
| connection (decode, reply, socket) | tokio mpsc |   0.3% |      31 |
| connection (decode, reply, socket) | tokio io/split |   0.1% |      12 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.1% |       7 |
| **connection (decode, reply, socket)** | **all** | ** 16.1%** | **   1545** |
| tokio scheduler | tokio wake/schedule |   5.8% |     560 |
| tokio scheduler | code |   2.2% |     213 |
| tokio scheduler | tokio io/split |   0.6% |      57 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.1% |      14 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | **  8.8%** | **    844** |
| ordered write path (rest) | code |   6.3% |     605 |
| ordered write path (rest) | tokio mpsc |   0.8% |      74 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.5% |      50 |
| ordered write path (rest) | allocator |   0.1% |       8 |
| **ordered write path (rest)** | **all** | **  7.7%** | **    738** |
| repl sender: task (recv, flush, ack read) | code |   2.4% |     230 |
| repl sender: task (recv, flush, ack read) | tokio mpsc |   1.7% |     164 |
| repl sender: task (recv, flush, ack read) | tokio io/split |   1.3% |     121 |
| repl sender: task (recv, flush, ack read) | tokio wake/schedule |   0.7% |      63 |
| repl sender: task (recv, flush, ack read) | libc (memcpy, syscall wrappers) |   0.5% |      48 |
| **repl sender: task (recv, flush, ack read)** | **all** | **  6.5%** | **    626** |
| on_write_logged (frame copy, dispatch) | code |   4.0% |     384 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.9% |      82 |
| on_write_logged (frame copy, dispatch) | allocator |   0.8% |      80 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  5.7%** | **    547** |
| stripe locks | code |   3.6% |     340 |
| stripe locks | tokio mpsc |   1.4% |     131 |
| **stripe locks** | **all** | **  4.9%** | **    471** |
| AOF append/lead/gate | tokio wake/schedule |   2.0% |     188 |
| AOF append/lead/gate | code |   1.3% |     127 |
| AOF append/lead/gate | tokio io/split |   0.6% |      54 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.4% |      41 |
| AOF append/lead/gate | allocator |   0.1% |      11 |
| AOF append/lead/gate | atomics/Arc drop |   0.0% |       3 |
| **AOF append/lead/gate** | **all** | **  4.4%** | **    425** |
| repl fan_out: backlog, offset, lock | code |   3.6% |     341 |
| repl fan_out: backlog, offset, lock | allocator |   0.3% |      31 |
| repl fan_out: backlog, offset, lock | atomics/Arc drop |   0.3% |      25 |
| repl fan_out: backlog, offset, lock | libc (memcpy, syscall wrappers) |   0.0% |       0 |
| **repl fan_out: backlog, offset, lock** | **all** | **  4.1%** | **    397** |
| repl fan_out: per-replica enqueue | code |   2.4% |     227 |
| repl fan_out: per-replica enqueue | tokio mpsc |   0.2% |      15 |
| **repl fan_out: per-replica enqueue** | **all** | **  2.5%** | **    242** |
| repl sender: write_repl_frame | tokio io/split |   0.8% |      80 |
| repl sender: write_repl_frame | libc (memcpy, syscall wrappers) |   0.4% |      34 |
| **repl sender: write_repl_frame** | **all** | **  1.2%** | **    114** |
| tokio io driver | code |   0.1% |      10 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       4 |
| **tokio io driver** | **all** | **  0.1%** | **     14** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   8.0%    762.4 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   6.9%    657.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.3%    414.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   4.0%    381.2 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   3.6%    340.5 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   3.0%    288.8 | `do_rallocx` |
|   2.4%    226.9 | `<alloc::vec::Vec<recached_server::replication::ReplicaHandle>>::retain::<<recached_server::replication::ReplH…` |
|   2.4%    225.7 | `alloc::fmt::format::format_inner` |
|   2.1%    202.7 | `_rjem_je_arena_ralloc` |
|   1.8%    177.0 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.8%    173.3 | `[vdso]:+a8c` |
|   1.7%    164.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|   1.6%    157.9 | `_rjem_je_arena_ralloc_no_move` |
|   1.6%    157.7 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|   1.6%    157.3 | `recached_server::clients::record_metrics` |
|   1.6%    155.7 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   1.5%    140.3 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.4%    137.4 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.4%    132.5 | `core::fmt::write` |
|   1.3%    123.0 | `<tokio::sync::notify::NotifiedProject>::poll_notified` |
|   1.2%    115.6 | `recached_server::propagation::resp_push` |
|   1.2%    114.0 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.2%    112.5 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   1.2%    111.7 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%    104.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.1%    102.7 | `_rjem_malloc` |
|   1.1%    101.5 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   1.0%     99.2 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.0%     91.4 | `_rjem_sdallocx` |
|   0.9%     88.1 | `core_engine::request::build_ref` |
|   0.9%     86.3 | `tokio::runtime::task::waker::wake_by_val` |
|   0.8%     81.3 | `core::str::converts::from_utf8` |
|   0.8%     81.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.8%     78.5 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   0.8%     77.4 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   0.8%     77.2 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.8%     76.2 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   0.8%     75.4 | `recached_server::propagation::try_execute_borrowed` |
|   0.7%     70.4 | `core_engine::store::lock_stripe` |
|   0.7%     70.2 | `<core::fmt::Formatter>::pad_integral` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   9578.7 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  97.0%   9295.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  87.7%   8397.2 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  86.5%   8283.0 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  84.7%   8109.3 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  70.5%   6749.5 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  61.2%   5861.8 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  37.4%   3586.3 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  22.0%   2108.6 | `recached_server::clients::execute_and_record_ref` |
|  19.1%   1826.8 | `recached_server::propagation::broadcast_for_at` |
|  18.7%   1792.7 | `recached_server::propagation::resp_push` |
|  17.7%   1693.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  16.5%   1582.0 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|  15.8%   1516.9 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  15.2%   1458.6 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  15.0%   1441.1 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  12.5%   1200.8 | `core_engine::store::write_slot::<&str>` |
|  11.7%   1116.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   9.3%    894.2 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|   8.8%    845.9 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   8.3%    793.8 | `tokio::runtime::task::raw::poll::<recached_server::replication::run_repl_server::{closure#0}::{closure#0}, al…` |
|   8.0%    771.0 | `alloc::fmt::format::format_inner` |
|   8.0%    762.4 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   7.8%    749.8 | `do_rallocx` |
|   7.8%    747.0 | `recached_server::replication::run_repl_server::{closure#0}::{closure#0}` |
|   7.5%    718.6 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   7.2%    689.2 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   5.1%    484.9 | `core::fmt::write` |
|   4.7%    452.0 | `_rjem_je_arena_ralloc` |
|   4.7%    450.4 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   3.9%    377.9 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.8%    361.1 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.4%    323.5 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   3.1%    300.1 | `recached_server::clients::record_metrics` |
|   3.0%    291.8 | `<alloc::vec::Vec<recached_server::replication::ReplicaHandle>>::retain::<<recached_server::replication::ReplH…` |
|   2.7%    256.1 | `<usize as core::fmt::Display>::fmt` |
|   2.3%    217.5 | `recached_server::persistence::gate_wait::{closure#0}` |
|   2.1%    204.8 | `<recached_server::persistence::AofDone>::wait::{closure#0}` |
|   1.9%    186.5 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   1.9%    184.0 | `core_engine::request::build_ref` |
