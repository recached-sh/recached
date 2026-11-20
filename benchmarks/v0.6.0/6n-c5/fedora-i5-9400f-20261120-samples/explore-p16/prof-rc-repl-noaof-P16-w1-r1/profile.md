samples: 22186, user cycles/op 6412

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| frame build (broadcast_for_at) | allocator |  12.9% |     826 |
| frame build (broadcast_for_at) | code |  11.4% |     729 |
| frame build (broadcast_for_at) | libc (memcpy, syscall wrappers) |   1.9% |     119 |
| **frame build (broadcast_for_at)** | **all** | ** 26.1%** | **   1674** |
| store execute | code |  17.2% |    1100 |
| store execute | libc (memcpy, syscall wrappers) |   7.6% |     487 |
| store execute | allocator |   0.1% |       7 |
| **store execute** | **all** | ** 24.9%** | **   1594** |
| connection (decode, reply, socket) | code |  13.1% |     839 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.4% |     218 |
| connection (decode, reply, socket) | allocator |   0.6% |      36 |
| connection (decode, reply, socket) | tokio mpsc |   0.3% |      17 |
| connection (decode, reply, socket) | tokio io/split |   0.2% |      12 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.1% |       6 |
| **connection (decode, reply, socket)** | **all** | ** 17.6%** | **   1127** |
| ordered write path (rest) | code |   6.4% |     409 |
| ordered write path (rest) | libc (memcpy, syscall wrappers) |   0.9% |      57 |
| ordered write path (rest) | tokio mpsc |   0.9% |      56 |
| ordered write path (rest) | allocator |   0.1% |       7 |
| **ordered write path (rest)** | **all** | **  8.2%** | **    529** |
| repl fan_out: backlog, offset, lock | code |   4.3% |     273 |
| repl fan_out: backlog, offset, lock | atomics/Arc drop |   0.4% |      26 |
| repl fan_out: backlog, offset, lock | allocator |   0.3% |      21 |
| **repl fan_out: backlog, offset, lock** | **all** | **  5.0%** | **    321** |
| stripe locks | code |   3.9% |     250 |
| stripe locks | tokio mpsc |   1.0% |      65 |
| **stripe locks** | **all** | **  4.9%** | **    315** |
| repl sender: task (recv, flush, ack read) | tokio mpsc |   1.8% |     115 |
| repl sender: task (recv, flush, ack read) | code |   1.6% |     102 |
| repl sender: task (recv, flush, ack read) | tokio io/split |   0.2% |      14 |
| repl sender: task (recv, flush, ack read) | tokio wake/schedule |   0.1% |       9 |
| repl sender: task (recv, flush, ack read) | libc (memcpy, syscall wrappers) |   0.1% |       5 |
| **repl sender: task (recv, flush, ack read)** | **all** | **  3.8%** | **    244** |
| repl fan_out: per-replica enqueue | code |   3.4% |     215 |
| repl fan_out: per-replica enqueue | tokio mpsc |   0.3% |      18 |
| repl fan_out: per-replica enqueue | allocator |   0.0% |       2 |
| repl fan_out: per-replica enqueue | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **repl fan_out: per-replica enqueue** | **all** | **  3.7%** | **    236** |
| repl sender: write_repl_frame | tokio io/split |   1.3% |      82 |
| repl sender: write_repl_frame | libc (memcpy, syscall wrappers) |   0.4% |      27 |
| **repl sender: write_repl_frame** | **all** | **  1.7%** | **    109** |
| tokio scheduler | tokio wake/schedule |   1.0% |      64 |
| tokio scheduler | code |   0.5% |      29 |
| tokio scheduler | tokio io/split |   0.2% |      12 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.0% |       2 |
| tokio scheduler | allocator |   0.0% |       0 |
| **tokio scheduler** | **all** | **  1.7%** | **    109** |
| AOF append/lead/gate | code |   0.5% |      35 |
| AOF append/lead/gate | tokio io/split |   0.5% |      33 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.2% |      13 |
| **AOF append/lead/gate** | **all** | **  1.3%** | **     81** |
| on_write_logged (frame copy, dispatch) | code |   0.6% |      41 |
| on_write_logged (frame copy, dispatch) | libc (memcpy, syscall wrappers) |   0.3% |      20 |
| on_write_logged (frame copy, dispatch) | allocator |   0.1% |       8 |
| **on_write_logged (frame copy, dispatch)** | **all** | **  1.1%** | **     69** |
| tokio io driver | code |   0.0% |       3 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       0 |
| **tokio io driver** | **all** | **  0.1%** | **      3** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   9.9%    637.0 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   8.5%    545.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.3%    272.5 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   3.6%    233.5 | `do_rallocx` |
|   3.4%    215.3 | `<alloc::vec::Vec<recached_server::replication::ReplicaHandle>>::retain::<<recached_server::replication::ReplH…` |
|   3.2%    204.6 | `alloc::fmt::format::format_inner` |
|   3.1%    198.3 | `_rjem_je_arena_ralloc` |
|   3.0%    192.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   2.5%    162.4 | `[vdso]:+a8c` |
|   2.4%    153.7 | `<core_engine::request::RequestDecoder>::decode_any` |
|   2.0%    126.0 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.9%    123.4 | `_rjem_je_arena_ralloc_no_move` |
|   1.8%    118.5 | `core::fmt::write` |
|   1.7%    112.1 | `recached_server::clients::record_metrics` |
|   1.7%    108.7 | `<tokio::sync::batch_semaphore::Semaphore>::add_permits_locked` |
|   1.6%    105.8 | `_rjem_malloc` |
|   1.6%    102.9 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   1.5%     95.7 | `<alloc::string::String as core::fmt::Write>::write_str` |
|   1.4%     92.8 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|   1.4%     91.0 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   1.4%     89.9 | `recached_server::propagation::resp_push` |
|   1.3%     85.8 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|   1.3%     82.4 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.2%     78.6 | `_rjem_sdallocx` |
|   1.2%     74.6 | `core::str::converts::from_utf8` |
|   1.0%     65.3 | `<core::fmt::Formatter>::pad_integral` |
|   1.0%     64.2 | `recached_server::propagation::try_execute_borrowed` |
|   1.0%     61.0 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<tokio::io::split::WriteHalf<toki…` |
|   0.9%     60.7 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   0.9%     59.8 | `<usize>::_fmt_inner` |
|   0.9%     58.4 | `<tokio::sync::mutex::Mutex<std::collections::hash::map::HashMap<alloc::string::String, alloc::vec::Vec<recach…` |
|   0.9%     57.8 | `core_engine::store::lock_stripe` |
|   0.9%     57.8 | `core_engine::request::build_ref` |
|   0.9%     57.5 | `<tokio::sync::batch_semaphore::Acquire as core::future::future::Future>::poll` |
|   0.9%     56.4 | `core_engine::store::entry_size` |
|   0.8%     49.7 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|   0.8%     49.1 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   0.7%     45.7 | `core::ptr::drop_glue::<smallvec::SmallVec<[tokio::sync::mutex::MutexGuard<()>; 2]>>` |
|   0.7%     43.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   0.7%     43.6 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   6411.8 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  99.5%   6378.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  93.7%   6005.5 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  93.5%   5995.1 | `recached_server::run::{closure#0}::{closure#29}::{closure#0}` |
|  93.0%   5959.8 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  78.0%   5003.5 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  69.3%   4444.3 | `recached_server::propagation::execute_locked_write_identified::{closure#0}` |
|  38.8%   2490.6 | `recached_server::propagation::apply_write_effects_identified::{closure#0}` |
|  29.0%   1856.6 | `recached_server::clients::execute_and_record_ref` |
|  26.1%   1673.9 | `recached_server::propagation::broadcast_for_at` |
|  25.8%   1653.1 | `recached_server::propagation::resp_push` |
|  23.4%   1503.4 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  21.3%   1363.8 | `<core_engine::store::KeyValueStore>::run_probed_write_ordered::<&core_engine::cmd::Command, <core_engine::sto…` |
|  20.3%   1302.0 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|  20.0%   1283.5 | `<core_engine::store::KeyValueStore>::set::<&str, &[u8]>` |
|  16.6%   1066.4 | `core_engine::store::write_slot::<&str>` |
|  15.4%    985.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  11.8%    757.8 | `<alloc::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc::alloc::Global>` |
|  11.3%    721.9 | `alloc::fmt::format::format_inner` |
|  11.2%    716.2 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   9.9%    637.0 | `libc.so.6:_dl_mcount_wrapper@GLIBC_2.2.5` |
|   9.9%    634.9 | `do_rallocx` |
|   9.8%    629.4 | `<recached_server::server_state::ServerState>::on_write_logged::{closure#0}` |
|   8.8%    563.6 | `<recached_server::replication::ReplHub>::fan_out::{closure#0}` |
|   7.0%    448.2 | `core::fmt::write` |
|   6.2%    400.0 | `_rjem_je_arena_ralloc` |
|   5.6%    359.2 | `tokio::runtime::task::raw::poll::<recached_server::replication::run_repl_server::{closure#0}::{closure#0}, al…` |
|   5.5%    352.6 | `recached_server::replication::run_repl_server::{closure#0}::{closure#0}` |
|   5.4%    346.8 | `recached_server::replication::handle_replica::<tokio::net::tcp::stream::TcpStream>::{closure#0}` |
|   5.1%    328.6 | `<std::sys::pal::unix::time::Timespec>::now` |
|   4.9%    316.7 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.8%    309.2 | `<recached_server::replication::ReplHub>::lock_stripes::<core::iter::sources::empty::Empty<&str>>::{closure#0}` |
|   4.6%    292.5 | `libc.so.6:clock_gettime@GLIBC_2.2.5` |
|   4.0%    256.3 | `recached_server::clients::record_metrics` |
|   3.8%    242.8 | `<alloc::vec::Vec<recached_server::replication::ReplicaHandle>>::retain::<<recached_server::replication::ReplH…` |
|   3.8%    241.3 | `<usize as core::fmt::Display>::fmt` |
|   2.5%    162.4 | `[vdso]:+a8c` |
|   2.5%    161.6 | `recached_server::propagation::try_execute_borrowed` |
|   2.2%    141.6 | `core_engine::request::build_ref` |
|   2.1%    135.8 | `<alloc::string::String as core::fmt::Write>::write_str` |
